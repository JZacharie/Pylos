use std::path::Path;

use async_trait::async_trait;
use pylos_core::domain::repositories::ConfigRepository;
use pylos_core::error::PylosError;

use super::db_pool::{DbPool, DbValue};

/// Implémentation SQL unifiée (SQLite / PostgreSQL) du port `ConfigRepository`.
#[derive(Clone)]
pub struct SqlConfigRepository {
    pool: DbPool,
}

impl SqlConfigRepository {
    pub async fn open(db_url: &str) -> Result<Self, PylosError> {
        let pool = if db_url.starts_with("postgres") || db_url.starts_with("postgresql") {
            DbPool::open_postgres(db_url, "config_store")
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to connect config Postgres: {}", e)))?
        } else {
            let path = db_url
                .strip_prefix("sqlite://")
                .unwrap_or(db_url)
                .split('?')
                .next()
                .unwrap_or("");
            DbPool::open_sqlite(Path::new(path), "config_store", 2)
                .await
                .map_err(|e| PylosError::Internal(format!("Failed to connect config SQLite: {}", e)))?
        };
        let repo = Self { pool };
        repo.ensure_schema().await;
        Ok(repo)
    }

    pub async fn from_pool(pool: DbPool) -> Self {
        let repo = Self { pool };
        repo.ensure_schema().await;
        repo
    }

    async fn ensure_schema(&self) {
        let (cfg_type, backup_pk) = match &self.pool {
            DbPool::Sqlite(_) => ("TEXT", "INTEGER PRIMARY KEY AUTOINCREMENT"),
            DbPool::Postgres(_) => ("JSONB", "SERIAL PRIMARY KEY"),
        };
        let _ = self
            .pool
            .execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS gateway_config (id TEXT PRIMARY KEY, config {} NOT NULL, config_version INTEGER NOT NULL DEFAULT 2)",
                    cfg_type
                ),
                vec![],
            )
            .await;
        let _ = self
            .pool
            .execute(
                "ALTER TABLE gateway_config ADD COLUMN config_version INTEGER NOT NULL DEFAULT 1",
                vec![],
            )
            .await;
        let _ = self
            .pool
            .execute(
                &format!(
                    "CREATE TABLE IF NOT EXISTS gateway_config_backups (id {}, source_version INTEGER NOT NULL, target_version INTEGER NOT NULL, config {} NOT NULL, backed_up_at TEXT NOT NULL)",
                    backup_pk, cfg_type
                ),
                vec![],
            )
            .await;
        let _ = self
            .pool
            .execute(
                "CREATE TABLE IF NOT EXISTS server_secrets (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
                vec![],
            )
            .await;
    }
}

#[async_trait]
impl ConfigRepository for SqlConfigRepository {
    async fn load_config_json(&self) -> Result<Option<(String, i64)>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT config, config_version FROM gateway_config WHERE id = 'pylos'",
                vec![],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to load gateway_config: {}", e)))?;

        Ok(row.map(|r| {
            let cfg_str = r
                .get_json::<serde_json::Value>("config")
                .map(|v| v.to_string())
                .unwrap_or_else(|| r.get_str("config"));
            let ver = r.get_opt_i64("config_version").unwrap_or(1);
            (cfg_str, ver)
        }))
    }

    async fn save_config_json(&self, json: &str, version: i64) -> Result<(), PylosError> {
        let val: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        self.pool
            .execute(
                "INSERT INTO gateway_config (id, config, config_version) VALUES ('pylos', $1, $2) \
                 ON CONFLICT(id) DO UPDATE SET config = excluded.config, config_version = excluded.config_version",
                vec![DbValue::Json(val), DbValue::I64(version)],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to save gateway_config: {}", e)))?;
        Ok(())
    }

    async fn backup_config_json(
        &self,
        json: &str,
        source_version: i64,
        target_version: i64,
    ) -> Result<(), PylosError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs().to_string())
            .unwrap_or_else(|_| "0".to_string());
        let val: serde_json::Value = serde_json::from_str(json).unwrap_or_default();
        self.pool
            .execute(
                "INSERT INTO gateway_config_backups (source_version, target_version, config, backed_up_at) \
                 VALUES ($1, $2, $3, $4)",
                vec![
                    DbValue::I64(source_version),
                    DbValue::I64(target_version),
                    DbValue::Json(val),
                    DbValue::Text(now),
                ],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to backup gateway_config: {}", e)))?;
        Ok(())
    }

    async fn get_admin_key_hash(&self) -> Result<Option<String>, PylosError> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT value FROM server_secrets WHERE key = 'admin_key_hash'",
                vec![],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to read admin_key_hash: {}", e)))?;
        Ok(row.map(|r| r.get_str("value")))
    }

    async fn save_admin_key_hash(&self, hash: &str) -> Result<(), PylosError> {
        self.pool
            .execute(
                "INSERT INTO server_secrets (key, value) VALUES ('admin_key_hash', $1) \
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                vec![DbValue::Text(hash.to_string())],
            )
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to save admin_key_hash: {}", e)))?;
        Ok(())
    }
}
