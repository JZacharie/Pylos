use std::path::Path;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use tracing::info;

use super::db_pool::{DbPool, DbRow, DbValue};
use pylos_core::domain::config::{EnvVar, VirtualKeyConfig, VkProviderConfig};
use pylos_core::domain::logging::now_ms;
use pylos_core::domain::repositories::VirtualKeyRepository;
use pylos_core::error::PylosError;
use pylos_core::{key_decrypt, key_encrypt};

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

    pub async fn from_pool(pool: DbPool) -> Result<Self, sqlx::Error> {
        let store = Self { pool };
        store.pool.run_migrations().await?;
        Ok(store)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        pool.execute(
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
                expires_at        INTEGER,
                allowed_tools     TEXT,
                denied_tools      TEXT
            )
            "#,
            vec![],
        )
        .await?;
        Ok(Self { pool })
    }

    pub async fn list_keys(&self) -> Result<Vec<VirtualKeyConfig>, PylosError> {
        let rows = self
            .pool
            .fetch_all("SELECT * FROM virtual_keys ORDER BY id", vec![])
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to list virtual keys: {}", e)))?;
        Ok(rows.iter().map(row_to_vk_config).collect())
    }

    pub async fn get_key_by_value(
        &self,
        value: &str,
    ) -> Result<Option<VirtualKeyConfig>, PylosError> {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        let value_hash = format!("{:x}", hasher.finalize());

        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM virtual_keys WHERE value_hash = $1",
                vec![DbValue::Text(value_hash)],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get virtual key: {}", e)))?;
        Ok(row.as_ref().map(row_to_vk_config))
    }

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
        let row = self
            .pool
            .fetch_optional(
                "SELECT * FROM virtual_keys WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to get virtual key: {}", e)))?;
        Ok(row.as_ref().map(row_to_vk_config))
    }

    pub async fn upsert_key(&self, vk: &VirtualKeyConfig) -> Result<(), PylosError> {
        let raw_value =
            vk.value.as_ref().and_then(|v| v.resolve()).ok_or_else(|| {
                PylosError::InvalidRequest("Virtual key must have a value".into())
            })?;

        let encrypted_value = key_encrypt(&raw_value)
            .map_err(|e| PylosError::Internal(format!("Failed to encrypt key value: {e}")))?;
        let value_hash = {
            let mut hasher = Sha256::new();
            hasher.update(raw_value.as_bytes());
            format!("{:x}", hasher.finalize())
        };

        let provider_configs_val = serde_json::to_value(&vk.provider_configs)
            .unwrap_or(serde_json::Value::Array(vec![]));
        let allowed_tools_val = vk
            .allowed_tools
            .as_ref()
            .and_then(|t| serde_json::to_value(t).ok());
        let denied_tools_val = vk
            .denied_tools
            .as_ref()
            .and_then(|t| serde_json::to_value(t).ok());

        let now = now_ms();
        let created_at = vk.created_at.unwrap_or(now);
        let updated_at = vk.updated_at.unwrap_or(now);

        self.pool
            .execute(
                r#"
                INSERT INTO virtual_keys
                    (id, name, description, value, value_hash, is_active, rate_limit_id, provider_configs,
                     team_alias, team_id, organization_id, access_group_id, user_email, user_id,
                     created_at, created_by, updated_at, last_active, expires_at, allowed_tools, denied_tools)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8,
                        $9, $10, $11, $12, $13, $14,
                        $15, $16, $17, $18, $19, $20, $21)
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
                    expires_at = excluded.expires_at,
                    allowed_tools = excluded.allowed_tools,
                    denied_tools = excluded.denied_tools
                "#,
                vec![
                    DbValue::Text(vk.id.clone()),
                    DbValue::Text(vk.name.clone()),
                    DbValue::OptText(vk.description.clone()),
                    DbValue::Text(encrypted_value),
                    DbValue::Text(value_hash),
                    DbValue::Bool(vk.is_active),
                    DbValue::OptText(vk.rate_limit_id.clone()),
                    DbValue::Json(provider_configs_val),
                    DbValue::OptText(vk.team_alias.clone()),
                    DbValue::OptText(vk.team_id.clone()),
                    DbValue::OptText(vk.organization_id.clone()),
                    DbValue::OptText(vk.access_group_id.clone()),
                    DbValue::OptText(vk.user_email.clone()),
                    DbValue::OptText(vk.user_id.clone()),
                    DbValue::I64(created_at),
                    DbValue::OptText(vk.created_by.clone()),
                    DbValue::I64(updated_at),
                    DbValue::OptI64(vk.last_active),
                    DbValue::OptI64(vk.expires_at),
                    DbValue::OptJson(allowed_tools_val),
                    DbValue::OptJson(denied_tools_val),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to upsert virtual key: {}", e)))?;

        Ok(())
    }

    pub async fn delete_keys_by_user(
        &self,
        user_id: &str,
        user_email: &str,
    ) -> Result<Vec<String>, PylosError> {
        let rows = self
            .pool
            .fetch_all(
                "SELECT id FROM virtual_keys WHERE user_id = $1 OR user_email = $2",
                vec![
                    DbValue::Text(user_id.to_string()),
                    DbValue::Text(user_email.to_string()),
                ],
            )
            .await
            .map_err(|e| {
                PylosError::Internal(format!("Failed to query user virtual keys: {}", e))
            })?;
        let ids: Vec<String> = rows.iter().map(|r| r.get_str("id")).collect();
        if !ids.is_empty() {
            self.pool
                .execute(
                    "DELETE FROM virtual_keys WHERE user_id = $1 OR user_email = $2",
                    vec![
                        DbValue::Text(user_id.to_string()),
                        DbValue::Text(user_email.to_string()),
                    ],
                )
                .await
                .map_err(|e| {
                    PylosError::Internal(format!("Failed to delete user virtual keys: {}", e))
                })?;
        }
        Ok(ids)
    }

    pub async fn delete_key(&self, id: &str) -> Result<bool, PylosError> {
        let rows_affected = self
            .pool
            .execute(
                "DELETE FROM virtual_keys WHERE id = $1",
                vec![DbValue::Text(id.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to delete virtual key: {}", e)))?;
        Ok(rows_affected > 0)
    }
}

#[async_trait]
impl VirtualKeyRepository for VirtualKeyStore {
    async fn list_keys(&self) -> Result<Vec<VirtualKeyConfig>, PylosError> {
        self.list_keys().await
    }

    async fn get_key(&self, id: &str) -> Result<Option<VirtualKeyConfig>, PylosError> {
        self.get_key_by_id(id).await
    }

    async fn find_by_raw_value(
        &self,
        raw_value: &str,
    ) -> Result<Option<VirtualKeyConfig>, PylosError> {
        self.get_key_by_value(raw_value).await
    }

    async fn upsert_key(&self, vk: &VirtualKeyConfig) -> Result<(), PylosError> {
        self.upsert_key(vk).await
    }

    async fn delete_key(&self, id: &str) -> Result<bool, PylosError> {
        self.delete_key(id).await
    }
}

fn row_to_vk_config(row: &DbRow) -> VirtualKeyConfig {
    let provider_configs: Vec<VkProviderConfig> =
        row.get_json("provider_configs").unwrap_or_default();
    let value_str = row.get_str("value");

    VirtualKeyConfig {
        id: row.get_str("id"),
        name: row.get_str("name"),
        description: row.get_opt_str("description"),
        value: Some(EnvVar::Literal(value_str)),
        is_active: row.get_bool("is_active"),
        rate_limit_id: row.get_opt_str("rate_limit_id"),
        provider_configs,
        team_alias: row.get_opt_str("team_alias"),
        team_id: row.get_opt_str("team_id"),
        organization_id: row.get_opt_str("organization_id"),
        access_group_id: row.get_opt_str("access_group_id"),
        user_email: row.get_opt_str("user_email"),
        user_id: row.get_opt_str("user_id"),
        created_at: row.get_opt_i64("created_at"),
        created_by: row.get_opt_str("created_by"),
        updated_at: row.get_opt_i64("updated_at"),
        last_active: row.get_opt_i64("last_active"),
        expires_at: row.get_opt_i64("expires_at"),
        allowed_tools: row.get_json("allowed_tools"),
        denied_tools: row.get_json("denied_tools"),
    }
}
