use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tracing::{debug, info};

use super::db_pool::{DbPool, DbValue};
use pylos_core::domain::config::RateLimitConfig;
use pylos_core::domain::repositories::RateLimitRepository;
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct RateLimitStore {
    pool: DbPool,
}

impl RateLimitStore {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "rate_limit_store", 4).await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "Rate limit store opened (SQLite)");
        Ok(store)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "rate_limit_store").await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!("Rate limit store opened (PostgreSQL)");
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
            CREATE TABLE IF NOT EXISTS vk_rate_limits (
                virtual_key_id     TEXT NOT NULL,
                window_type        TEXT NOT NULL,
                max_value          INTEGER NOT NULL,
                current_value      INTEGER NOT NULL DEFAULT 0,
                window_start_ms    INTEGER NOT NULL,
                window_duration_ms INTEGER NOT NULL,
                PRIMARY KEY (virtual_key_id, window_type)
            )
            "#,
            vec![],
        )
        .await?;
        Ok(Self { pool })
    }

    pub async fn upsert_rate_limit(
        &self,
        vk_id: &str,
        config: &RateLimitConfig,
    ) -> Result<(), sqlx::Error> {
        let now = now_ms();

        if config.request_max_limit > 0 {
            let window_ms = config
                .request_reset_duration
                .as_ref()
                .map(|d| (d.as_secs() * 1000) as i64)
                .unwrap_or(60_000);

            self.pool
                .execute(
                    "INSERT INTO vk_rate_limits (virtual_key_id, window_type, max_value, current_value, window_start_ms, window_duration_ms) \
                     VALUES ($1, 'requests', $2, 0, $3, $4) \
                     ON CONFLICT(virtual_key_id, window_type) DO UPDATE SET \
                     max_value = excluded.max_value, \
                     window_duration_ms = excluded.window_duration_ms",
                    vec![
                        DbValue::Text(vk_id.to_string()),
                        DbValue::I64(i64::from(config.request_max_limit)),
                        DbValue::I64(now),
                        DbValue::I64(window_ms),
                    ],
                )
                .await?;
        }

        if config.token_max_limit > 0 {
            let window_ms = config
                .token_reset_duration
                .as_ref()
                .map(|d| (d.as_secs() * 1000) as i64)
                .unwrap_or(60_000);

            self.pool
                .execute(
                    "INSERT INTO vk_rate_limits (virtual_key_id, window_type, max_value, current_value, window_start_ms, window_duration_ms) \
                     VALUES ($1, 'tokens', $2, 0, $3, $4) \
                     ON CONFLICT(virtual_key_id, window_type) DO UPDATE SET \
                     max_value = excluded.max_value, \
                     window_duration_ms = excluded.window_duration_ms",
                    vec![
                        DbValue::Text(vk_id.to_string()),
                        DbValue::I64(config.token_max_limit as i64),
                        DbValue::I64(now),
                        DbValue::I64(window_ms),
                    ],
                )
                .await?;
        }

        Ok(())
    }

    pub async fn check_and_increment_requests(&self, vk_id: &str) -> Result<(), PylosError> {
        self.check_and_increment(vk_id, "requests", 1).await
    }

    pub async fn record_tokens(&self, vk_id: &str, tokens: i64) {
        if tokens <= 0 {
            return;
        }
        if let Err(e) = self.check_and_increment(vk_id, "tokens", tokens).await {
            debug!(error = %e, vk_id = %vk_id, "Token rate limit would be exceeded (post-hoc)");
        }
    }

    async fn check_and_increment(
        &self,
        vk_id: &str,
        window_type: &str,
        increment: i64,
    ) -> Result<(), PylosError> {
        let now = now_ms();
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PylosError::Internal(format!("Rate limit tx begin failed: {}", e)))?;

        let row = tx
            .fetch_optional(
                "SELECT max_value, current_value, window_start_ms, window_duration_ms \
                 FROM vk_rate_limits WHERE virtual_key_id = $1 AND window_type = $2",
                vec![
                    DbValue::Text(vk_id.to_string()),
                    DbValue::Text(window_type.to_string()),
                ],
            )
            .await
            .ok()
            .flatten();

        let Some(row) = row else {
            let _ = tx.commit().await;
            return Ok(());
        };

        let max_value = row.get_i64("max_value");
        let current_value = row.get_i64("current_value");
        let window_start_ms = row.get_opt_i64("window_start_ms").unwrap_or(now);
        let window_duration_ms = row.get_opt_i64("window_duration_ms").unwrap_or(60_000);

        let (new_current, new_start) = if now - window_start_ms >= window_duration_ms {
            (0i64, now)
        } else {
            (current_value, window_start_ms)
        };

        if max_value > 0 && new_current + increment > max_value {
            let reset_in_secs = (window_duration_ms - (now - new_start)).max(0) / 1000;
            let _ = tx.rollback().await;
            return Err(PylosError::RateLimitExceeded(format!(
                "Rate limit exceeded for {} ({}): {}/{} — resets in {}s",
                vk_id, window_type, new_current, max_value, reset_in_secs
            )));
        }

        let _ = tx
            .execute(
                "UPDATE vk_rate_limits SET current_value = $1, window_start_ms = $2 \
                 WHERE virtual_key_id = $3 AND window_type = $4",
                vec![
                    DbValue::I64(new_current + increment),
                    DbValue::I64(new_start),
                    DbValue::Text(vk_id.to_string()),
                    DbValue::Text(window_type.to_string()),
                ],
            )
            .await;

        tx.commit()
            .await
            .map_err(|e| PylosError::Internal(format!("Rate limit tx commit failed: {}", e)))?;

        Ok(())
    }

    pub async fn get_status(&self, vk_id: &str) -> Vec<RateLimitStatus> {
        let now = now_ms();
        let rows = self
            .pool
            .fetch_all(
                "SELECT window_type, max_value, current_value, window_start_ms, window_duration_ms FROM vk_rate_limits WHERE virtual_key_id = $1",
                vec![DbValue::Text(vk_id.to_string())],
            )
            .await
            .unwrap_or_default();

        rows.iter()
            .map(|row| {
                let window_start = row.get_i64("window_start_ms");
                let window_dur = row.get_opt_i64("window_duration_ms").unwrap_or(60_000);
                let current = row.get_i64("current_value");
                let max = row.get_i64("max_value");
                let effective_current = if now - window_start >= window_dur {
                    0
                } else {
                    current
                };

                RateLimitStatus {
                    window_type: row.get_str("window_type"),
                    max_value: max as u64,
                    current_value: effective_current as u64,
                    reset_at_ms: window_start + window_dur,
                }
            })
            .collect()
    }

    pub async fn delete_vk_entries(&self, vk_id: &str) {
        let _ = self
            .pool
            .execute(
                "DELETE FROM vk_rate_limits WHERE virtual_key_id = $1",
                vec![DbValue::Text(vk_id.to_string())],
            )
            .await;
    }
}

#[async_trait]
impl RateLimitRepository for RateLimitStore {
    async fn check_and_increment_requests(&self, vk_id: &str) -> Result<(), PylosError> {
        self.check_and_increment_requests(vk_id).await
    }

    async fn record_tokens(&self, vk_id: &str, tokens: i64) {
        self.record_tokens(vk_id, tokens).await
    }

    async fn upsert_rate_limit(
        &self,
        vk_id: &str,
        rl: &RateLimitConfig,
    ) -> Result<(), PylosError> {
        self.upsert_rate_limit(vk_id, rl)
            .await
            .map_err(|e| PylosError::Internal(format!("Rate limit upsert failed: {}", e)))
    }

    async fn delete_rate_limit(&self, vk_id: &str) -> Result<(), PylosError> {
        self.delete_vk_entries(vk_id).await;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RateLimitStatus {
    pub window_type: String,
    pub max_value: u64,
    pub current_value: u64,
    pub reset_at_ms: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::config::{Duration, RateLimitConfig};

    async fn make_store() -> RateLimitStore {
        RateLimitStore::in_memory()
            .await
            .expect("in-memory rate limit store")
    }

    fn make_config(req_limit: u32, window: &str) -> RateLimitConfig {
        RateLimitConfig {
            id: "rl-test".into(),
            token_max_limit: 0,
            token_reset_duration: None,
            request_max_limit: req_limit,
            request_reset_duration: Some(Duration(window.into())),
        }
    }

    #[tokio::test]
    async fn test_request_rate_limit_within_and_exceeded() {
        let store = make_store().await;
        store
            .upsert_rate_limit("vk-rl-1", &make_config(2, "1m"))
            .await
            .unwrap();

        assert!(store.check_and_increment_requests("vk-rl-1").await.is_ok());
        assert!(store.check_and_increment_requests("vk-rl-1").await.is_ok());
        let err = store.check_and_increment_requests("vk-rl-1").await.unwrap_err();
        assert!(matches!(err, PylosError::RateLimitExceeded(_)));
    }
}
