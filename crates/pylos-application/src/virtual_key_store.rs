use std::path::Path;

use sha2::{Digest, Sha256};
use sqlx::Row;
use tracing::info;

use crate::db_pool::DbPool;
use pylos_core::domain::config::{EnvVar, VirtualKeyConfig, VkProviderConfig};
use pylos_core::error::PylosError;
use pylos_core::key_decrypt;
use pylos_core::key_encrypt;

#[derive(Clone)]
pub struct VirtualKeyStore {
    pool: DbPool,
}

impl VirtualKeyStore {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "virtual_key_store", 4).await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "Virtual key store opened (SQLite)");
        Ok(store)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "virtual_key_store").await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!("Virtual key store opened (PostgreSQL)");
        Ok(store)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;

        if let Some(p) = pool.as_sqlite() {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS virtual_keys (
                    id                TEXT PRIMARY KEY,
                    name              TEXT NOT NULL,
                    description       TEXT,
                    value             TEXT NOT NULL,
                    value_hash        TEXT,
                    is_active         INTEGER NOT NULL DEFAULT 1,
                    rate_limit_id     TEXT,
                    provider_configs  TEXT,
                    team_alias        TEXT,
                    team_id           TEXT,
                    organization_id   TEXT,
                    access_group_id   TEXT,
                    user_email        TEXT,
                    user_id           TEXT,
                    created_at        INTEGER,
                    created_by        TEXT,
                    updated_at        INTEGER,
                    last_active       INTEGER,
                    expires_at        INTEGER
                )
                "#,
            )
            .execute(p)
            .await?;
        }

        Ok(Self { pool })
    }

    pub async fn list_keys(&self) -> Result<Vec<VirtualKeyConfig>, PylosError> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query("SELECT * FROM virtual_keys ORDER BY id")
                    .fetch_all(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to list virtual keys: {}", e))
                    })?;

                Ok(rows.iter().map(row_to_vk_config_sqlite).collect())
            }
            DbPool::Postgres(pool) => {
                let rows = sqlx::query::<sqlx::Postgres>("SELECT * FROM virtual_keys ORDER BY id")
                    .fetch_all(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to list virtual keys: {}", e))
                    })?;

                Ok(rows.iter().map(row_to_vk_config_pg).collect())
            }
        }
    }

    pub async fn get_key_by_value(
        &self,
        value: &str,
    ) -> Result<Option<VirtualKeyConfig>, PylosError> {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        let value_hash = format!("{:x}", hasher.finalize());
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let row = sqlx::query("SELECT * FROM virtual_keys WHERE value_hash = $1")
                    .bind(&value_hash)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to get virtual key: {}", e))
                    })?;

                Ok(row.as_ref().map(row_to_vk_config_sqlite))
            }
            DbPool::Postgres(pool) => {
                let row = sqlx::query::<sqlx::Postgres>(
                    "SELECT * FROM virtual_keys WHERE value_hash = $1",
                )
                .bind(&value_hash)
                .fetch_optional(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to get virtual key: {}", e)))?;

                Ok(row.as_ref().map(row_to_vk_config_pg))
            }
        }
    }

    /// Decrypt and reveal the raw key value for a given key (RBAC must be enforced by caller).
    pub async fn reveal_key_value(&self, id: &str) -> Result<Option<String>, PylosError> {
        let config = self.get_key_by_id(id).await?;
        match config {
            Some(vk) => {
                let encrypted = vk
                    .value
                    .as_ref()
                    .and_then(|v| v.resolve())
                    .unwrap_or_default();
                match key_decrypt(&encrypted) {
                    Ok(plaintext) => Ok(Some(plaintext)),
                    Err(e) => Err(PylosError::Internal(format!(
                        "Failed to decrypt key value — {e}"
                    ))),
                }
            }
            None => Ok(None),
        }
    }

    pub async fn get_key_by_id(&self, id: &str) -> Result<Option<VirtualKeyConfig>, PylosError> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let row = sqlx::query("SELECT * FROM virtual_keys WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to get virtual key: {}", e))
                    })?;

                Ok(row.as_ref().map(row_to_vk_config_sqlite))
            }
            DbPool::Postgres(pool) => {
                let row = sqlx::query::<sqlx::Postgres>("SELECT * FROM virtual_keys WHERE id = $1")
                    .bind(id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to get virtual key: {}", e))
                    })?;

                Ok(row.as_ref().map(row_to_vk_config_pg))
            }
        }
    }

    pub async fn upsert_key(&self, vk: &VirtualKeyConfig) -> Result<(), PylosError> {
        let raw_value =
            vk.value.as_ref().and_then(|v| v.resolve()).ok_or_else(|| {
                PylosError::InvalidRequest("Virtual key must have a value".into())
            })?;

        let encrypted_value = key_encrypt(&raw_value).map_err(|e| {
            PylosError::Internal(format!("Failed to encrypt key value: {e}"))
        })?;
        let value_hash = {
            let mut hasher = Sha256::new();
            hasher.update(raw_value.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        let provider_configs_json =
            serde_json::to_string(&vk.provider_configs).unwrap_or_else(|_| "[]".to_string());

        let now_ms = crate::log_store::now_ms();

        match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO virtual_keys
                        (id, name, description, value, value_hash, is_active, rate_limit_id, provider_configs,
                         team_alias, team_id, organization_id, access_group_id, user_email, user_id,
                         created_at, created_by, updated_at, last_active, expires_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                            $9, $10, $11, $12, $13, $14,
                            COALESCE($15, ?), $16, COALESCE($17, ?), $18, $19)
                    ON CONFLICT(id) DO UPDATE SET
                        name = excluded.name,
                        description = excluded.description,
                        value = excluded.value,
                        value_hash = excluded.value_hash,
                        is_active = excluded.is_active,
                        rate_limit_id = excluded.rate_limit_id,
                        provider_configs = excluded.provider_configs,
                        team_alias = excluded.team_alias,
                        team_id = excluded.team_id,
                        organization_id = excluded.organization_id,
                        access_group_id = excluded.access_group_id,
                        user_email = excluded.user_email,
                        user_id = excluded.user_id,
                        created_by = excluded.created_by,
                        updated_at = excluded.updated_at,
                        last_active = excluded.last_active,
                        expires_at = excluded.expires_at
                    "#,
                )
                .bind(&vk.id)
                .bind(&vk.name)
                .bind(&vk.description)
                .bind(&encrypted_value)
                .bind(&value_hash)
                .bind(if vk.is_active { 1 } else { 0 })
                .bind(&vk.rate_limit_id)
                .bind(&provider_configs_json)
                .bind(&vk.team_alias)
                .bind(&vk.team_id)
                .bind(&vk.organization_id)
                .bind(&vk.access_group_id)
                .bind(&vk.user_email)
                .bind(&vk.user_id)
                .bind(vk.created_at)
                .bind(&vk.created_by)
                .bind(vk.updated_at)
                .bind(vk.last_active)
                .bind(vk.expires_at)
                .bind(now_ms)
                .bind(now_ms)
                .execute(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to upsert virtual key: {}", e))
                })?;
            }
            DbPool::Postgres(pool) => {
                let provider_configs_val: serde_json::Value =
                    serde_json::from_str(&provider_configs_json)
                        .unwrap_or(serde_json::Value::Array(vec![]));

                sqlx::query::<sqlx::Postgres>(
                    r#"
                    INSERT INTO virtual_keys
                        (id, name, description, value, value_hash, is_active, rate_limit_id, provider_configs,
                         team_alias, team_id, organization_id, access_group_id, user_email, user_id,
                         created_at, created_by, updated_at, last_active, expires_at)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                            $9, $10, $11, $12, $13, $14,
                            COALESCE($15, $20), $16, COALESCE($17, $21), $18, $19)
                    ON CONFLICT(id) DO UPDATE SET
                        name = excluded.name,
                        description = excluded.description,
                        value = excluded.value,
                        value_hash = excluded.value_hash,
                        is_active = excluded.is_active,
                        rate_limit_id = excluded.rate_limit_id,
                        provider_configs = excluded.provider_configs,
                        team_alias = excluded.team_alias,
                        team_id = excluded.team_id,
                        organization_id = excluded.organization_id,
                        access_group_id = excluded.access_group_id,
                        user_email = excluded.user_email,
                        user_id = excluded.user_id,
                        created_by = excluded.created_by,
                        updated_at = excluded.updated_at,
                        last_active = excluded.last_active,
                        expires_at = excluded.expires_at
                    "#,
                )
                .bind(&vk.id)
                .bind(&vk.name)
                .bind(&vk.description)
                .bind(&encrypted_value)
                .bind(&value_hash)
                .bind(vk.is_active)
                .bind(&vk.rate_limit_id)
                .bind(&provider_configs_val)
                .bind(&vk.team_alias)
                .bind(&vk.team_id)
                .bind(&vk.organization_id)
                .bind(&vk.access_group_id)
                .bind(&vk.user_email)
                .bind(&vk.user_id)
                .bind(vk.created_at)
                .bind(&vk.created_by)
                .bind(vk.updated_at)
                .bind(vk.last_active)
                .bind(vk.expires_at)
                .bind(now_ms)
                .bind(now_ms)
                .execute(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to upsert virtual key: {}", e))
                })?;
            }
        }

        Ok(())
    }

    pub async fn delete_keys_by_user(
        &self,
        user_id: &str,
        user_email: &str,
    ) -> Result<Vec<String>, PylosError> {
        let ids = match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query(
                    "SELECT id FROM virtual_keys WHERE user_id = $1 OR user_email = $2",
                )
                .bind(user_id)
                .bind(user_email)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to query user virtual keys: {}", e))
                })?;
                let ids: Vec<String> = rows
                    .iter()
                    .map(|r| r.try_get("id").unwrap_or_default())
                    .collect();
                if !ids.is_empty() {
                    sqlx::query("DELETE FROM virtual_keys WHERE user_id = $1 OR user_email = $2")
                        .bind(user_id)
                        .bind(user_email)
                        .execute(pool)
                        .await
                        .map_err(|e| {
                            PylosError::Internal(format!(
                                "Failed to delete user virtual keys: {}",
                                e
                            ))
                        })?;
                }
                ids
            }
            DbPool::Postgres(pool) => {
                let rows = sqlx::query::<sqlx::Postgres>(
                    "SELECT id FROM virtual_keys WHERE user_id = $1 OR user_email = $2",
                )
                .bind(user_id)
                .bind(user_email)
                .fetch_all(pool)
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to query user virtual keys: {}", e))
                })?;
                let ids: Vec<String> = rows
                    .iter()
                    .map(|r| r.try_get("id").unwrap_or_default())
                    .collect();
                if !ids.is_empty() {
                    sqlx::query::<sqlx::Postgres>(
                        "DELETE FROM virtual_keys WHERE user_id = $1 OR user_email = $2",
                    )
                    .bind(user_id)
                    .bind(user_email)
                    .execute(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to delete user virtual keys: {}", e))
                    })?;
                }
                ids
            }
        };
        Ok(ids)
    }

    pub async fn delete_key(&self, id: &str) -> Result<bool, PylosError> {
        let rows_affected = match &self.pool {
            DbPool::Sqlite(pool) => sqlx::query("DELETE FROM virtual_keys WHERE id = $1")
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to delete virtual key: {}", e)))?
                .rows_affected(),
            DbPool::Postgres(pool) => {
                sqlx::query::<sqlx::Postgres>("DELETE FROM virtual_keys WHERE id = $1")
                    .bind(id)
                    .execute(pool)
                    .await
                    .map_err(|e| {
                        PylosError::Internal(format!("Failed to delete virtual key: {}", e))
                    })?
                    .rows_affected()
            }
        };

        Ok(rows_affected > 0)
    }
}

fn row_to_vk_config_sqlite(row: &sqlx::sqlite::SqliteRow) -> VirtualKeyConfig {
    let raw_prov_configs: String = row.try_get("provider_configs").unwrap_or_default();
    let provider_configs: Vec<VkProviderConfig> =
        serde_json::from_str(&raw_prov_configs).unwrap_or_default();
    let value_str: String = row.try_get("value").unwrap_or_default();

    VirtualKeyConfig {
        id: row.try_get("id").unwrap_or_default(),
        name: row.try_get("name").unwrap_or_default(),
        description: row.try_get("description").ok(),
        value: Some(EnvVar::Literal(value_str)),
        is_active: row.try_get::<i64, _>("is_active").unwrap_or(1) != 0,
        rate_limit_id: row.try_get("rate_limit_id").ok(),
        provider_configs,
        team_alias: row.try_get("team_alias").ok(),
        team_id: row.try_get("team_id").ok(),
        organization_id: row.try_get("organization_id").ok(),
        access_group_id: row.try_get("access_group_id").ok(),
        user_email: row.try_get("user_email").ok(),
        user_id: row.try_get("user_id").ok(),
        created_at: row.try_get("created_at").ok(),
        created_by: row.try_get("created_by").ok(),
        updated_at: row.try_get("updated_at").ok(),
        last_active: row.try_get("last_active").ok(),
        expires_at: row.try_get("expires_at").ok(),
    }
}

fn row_to_vk_config_pg(row: &sqlx::postgres::PgRow) -> VirtualKeyConfig {
    let prov_configs_val: serde_json::Value = row
        .try_get("provider_configs")
        .unwrap_or(serde_json::Value::Array(vec![]));
    let provider_configs: Vec<VkProviderConfig> =
        serde_json::from_value(prov_configs_val).unwrap_or_default();
    let value_str: String = row.try_get("value").unwrap_or_default();

    VirtualKeyConfig {
        id: row.try_get("id").unwrap_or_default(),
        name: row.try_get("name").unwrap_or_default(),
        description: row.try_get("description").ok(),
        value: Some(EnvVar::Literal(value_str)),
        is_active: row.try_get::<bool, _>("is_active").unwrap_or(true),
        rate_limit_id: row.try_get("rate_limit_id").ok(),
        provider_configs,
        team_alias: row.try_get("team_alias").ok(),
        team_id: row.try_get("team_id").ok(),
        organization_id: row.try_get("organization_id").ok(),
        access_group_id: row.try_get("access_group_id").ok(),
        user_email: row.try_get("user_email").ok(),
        user_id: row.try_get("user_id").ok(),
        created_at: row.try_get("created_at").ok(),
        created_by: row.try_get("created_by").ok(),
        updated_at: row.try_get("updated_at").ok(),
        last_active: row.try_get("last_active").ok(),
        expires_at: row.try_get("expires_at").ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_virtual_key_store_crud() {
        let store = VirtualKeyStore::in_memory().await.unwrap();
        let vk = VirtualKeyConfig {
            id: "vk-test-1".into(),
            name: "Test Key".into(),
            description: Some("Description test".into()),
            value: Some(EnvVar::Literal("sk-pylos-secret123".into())),
            is_active: true,
            rate_limit_id: Some("rl-1".into()),
            provider_configs: vec![VkProviderConfig {
                provider: "openai".into(),
                allowed_models: vec!["*".into()],
                key_names: vec!["*".into()],
                weight: 1.0,
            }],
            team_alias: None,
            team_id: None,
            organization_id: None,
            access_group_id: None,
            user_email: None,
            user_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            last_active: None,
            expires_at: None,
        };

        store.upsert_key(&vk).await.unwrap();

        let list = store.list_keys().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "vk-test-1");
        assert_eq!(list[0].name, "Test Key");
        assert_eq!(list[0].provider_configs.len(), 1);
        assert_eq!(list[0].provider_configs[0].provider, "openai");

        let fetched = store.get_key_by_value("sk-pylos-secret123").await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().id, "vk-test-1");

        let deleted = store.delete_key("vk-test-1").await.unwrap();
        assert!(deleted);

        let list_empty = store.list_keys().await.unwrap();
        assert_eq!(list_empty.len(), 0);
    }
}
