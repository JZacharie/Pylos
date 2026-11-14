use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::Row;
use tracing::{debug, info};

use crate::db_pool::DbPool;
use pylos_core::domain::config::RateLimitConfig;
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

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;

        if let Some(p) = pool.as_sqlite() {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS vk_rate_limits (
                    virtual_key_id     TEXT    NOT NULL,
                    window_type        TEXT    NOT NULL,
                    max_value          INTEGER NOT NULL,
                    current_value      INTEGER NOT NULL DEFAULT 0,
                    window_start_ms    INTEGER NOT NULL,
                    window_duration_ms INTEGER NOT NULL,
                    PRIMARY KEY (virtual_key_id, window_type)
                )
                "#,
            )
            .execute(p)
            .await?;
        }

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

            match &self.pool {
                DbPool::Sqlite(pool) => {
                    sqlx::query(
                        "INSERT INTO vk_rate_limits (virtual_key_id, window_type, max_value, current_value, window_start_ms, window_duration_ms) \
                         VALUES ($1, 'requests', $2, 0, $3, $4) \
                         ON CONFLICT(virtual_key_id, window_type) DO UPDATE SET \
                         max_value = excluded.max_value, \
                         window_duration_ms = excluded.window_duration_ms",
                    )
                    .bind(vk_id)
                    .bind(config.request_max_limit)
                    .bind(now)
                    .bind(window_ms)
                    .execute(pool)
                    .await?;
                }
                DbPool::Postgres(pool) => {
                    sqlx::query::<sqlx::Postgres>(
                        "INSERT INTO vk_rate_limits (virtual_key_id, window_type, max_value, current_value, window_start_ms, window_duration_ms) \
                         VALUES ($1, 'requests', $2, 0, $3, $4) \
                         ON CONFLICT(virtual_key_id, window_type) DO UPDATE SET \
                         max_value = excluded.max_value, \
                         window_duration_ms = excluded.window_duration_ms",
                    )
                    .bind(vk_id)
                    .bind(config.request_max_limit as i32)
                    .bind(now)
                    .bind(window_ms)
                    .execute(pool)
                    .await?;
                }
            }
        }

        if config.token_max_limit > 0 {
            let window_ms = config
                .token_reset_duration
                .as_ref()
                .map(|d| (d.as_secs() * 1000) as i64)
                .unwrap_or(60_000);

            match &self.pool {
                DbPool::Sqlite(pool) => {
                    sqlx::query(
                        "INSERT INTO vk_rate_limits (virtual_key_id, window_type, max_value, current_value, window_start_ms, window_duration_ms) \
                         VALUES ($1, 'tokens', $2, 0, $3, $4) \
                         ON CONFLICT(virtual_key_id, window_type) DO UPDATE SET \
                         max_value = excluded.max_value, \
                         window_duration_ms = excluded.window_duration_ms",
                    )
                    .bind(vk_id)
                    .bind(config.token_max_limit as i64)
                    .bind(now)
                    .bind(window_ms)
                    .execute(pool)
                    .await?;
                }
                DbPool::Postgres(pool) => {
                    sqlx::query::<sqlx::Postgres>(
                        "INSERT INTO vk_rate_limits (virtual_key_id, window_type, max_value, current_value, window_start_ms, window_duration_ms) \
                         VALUES ($1, 'tokens', $2, 0, $3, $4) \
                         ON CONFLICT(virtual_key_id, window_type) DO UPDATE SET \
                         max_value = excluded.max_value, \
                         window_duration_ms = excluded.window_duration_ms",
                    )
                    .bind(vk_id)
                    .bind(config.token_max_limit as i64)
                    .bind(now)
                    .bind(window_ms)
                    .execute(pool)
                    .await?;
                }
            }
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
        let vk_id = vk_id.to_string();
        let window_type = window_type.to_string();

        match &self.pool {
            DbPool::Sqlite(pool) => {
                let mut tx = pool.begin().await.map_err(|e| {
                    PylosError::Internal(format!("Rate limit tx begin failed: {}", e))
                })?;

                let row = sqlx::query(
                    "SELECT max_value, current_value, window_start_ms, window_duration_ms \
                     FROM vk_rate_limits WHERE virtual_key_id = $1 AND window_type = $2",
                )
                .bind(&vk_id)
                .bind(&window_type)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);

                let Some(row) = row else {
                    let _ = tx.commit().await;
                    return Ok(());
                };

                let max_value: i64 = row.try_get("max_value").unwrap_or(0);
                let current_value: i64 = row.try_get("current_value").unwrap_or(0);
                let window_start_ms: i64 = row.try_get("window_start_ms").unwrap_or(now);
                let window_duration_ms: i64 = row.try_get("window_duration_ms").unwrap_or(60_000);

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

                let _ = sqlx::query(
                    "UPDATE vk_rate_limits SET current_value = $1, window_start_ms = $2 \
                     WHERE virtual_key_id = $3 AND window_type = $4",
                )
                .bind(new_current + increment)
                .bind(new_start)
                .bind(&vk_id)
                .bind(&window_type)
                .execute(&mut *tx)
                .await;

                tx.commit().await.map_err(|e| {
                    PylosError::Internal(format!("Rate limit tx commit failed: {}", e))
                })?;

                Ok(())
            }
            DbPool::Postgres(pool) => {
                let mut tx = pool.begin().await.map_err(|e| {
                    PylosError::Internal(format!("Rate limit tx begin failed: {}", e))
                })?;

                let row = sqlx::query::<sqlx::Postgres>(
                    "SELECT max_value, current_value, window_start_ms, window_duration_ms \
                     FROM vk_rate_limits WHERE virtual_key_id = $1 AND window_type = $2",
                )
                .bind(&vk_id)
                .bind(&window_type)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);

                let Some(row) = row else {
                    let _ = tx.commit().await;
                    return Ok(());
                };

                let max_value: i64 = row.try_get("max_value").unwrap_or(0);
                let current_value: i64 = row.try_get("current_value").unwrap_or(0);
                let window_start_ms: i64 = row.try_get("window_start_ms").unwrap_or(now);
                let window_duration_ms: i64 = row.try_get("window_duration_ms").unwrap_or(60_000);

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

                let _ = sqlx::query::<sqlx::Postgres>(
                    "UPDATE vk_rate_limits SET current_value = $1, window_start_ms = $2 \
                     WHERE virtual_key_id = $3 AND window_type = $4",
                )
                .bind(new_current + increment)
                .bind(new_start)
                .bind(&vk_id)
                .bind(&window_type)
                .execute(&mut *tx)
                .await;

                tx.commit().await.map_err(|e| {
                    PylosError::Internal(format!("Rate limit tx commit failed: {}", e))
                })?;

                Ok(())
            }
        }
    }

    pub async fn get_status(&self, vk_id: &str) -> Vec<RateLimitStatus> {
        let now = now_ms();

        match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query(
                    "SELECT window_type, max_value, current_value, window_start_ms, window_duration_ms FROM vk_rate_limits WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                rows.iter()
                    .map(|row| {
                        let window_start: i64 = row.try_get("window_start_ms").unwrap_or(0);
                        let window_dur: i64 = row.try_get("window_duration_ms").unwrap_or(60_000);
                        let current: i64 = row.try_get("current_value").unwrap_or(0);
                        let max: i64 = row.try_get("max_value").unwrap_or(0);
                        let effective_current = if now - window_start >= window_dur {
                            0
                        } else {
                            current
                        };

                        RateLimitStatus {
                            window_type: row.try_get("window_type").unwrap_or_default(),
                            max_value: max as u64,
                            current_value: effective_current as u64,
                            reset_at_ms: window_start + window_dur,
                        }
                    })
                    .collect()
            }
            DbPool::Postgres(pool) => {
                let rows = sqlx::query::<sqlx::Postgres>(
                    "SELECT window_type, max_value, current_value, window_start_ms, window_duration_ms FROM vk_rate_limits WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                rows.iter()
                    .map(|row| {
                        let window_start: i64 = row.try_get("window_start_ms").unwrap_or(0);
                        let window_dur: i64 = row.try_get("window_duration_ms").unwrap_or(60_000);
                        let current: i64 = row.try_get("current_value").unwrap_or(0);
                        let max: i64 = row.try_get("max_value").unwrap_or(0);
                        let effective_current = if now - window_start >= window_dur {
                            0
                        } else {
                            current
                        };

                        RateLimitStatus {
                            window_type: row.try_get("window_type").unwrap_or_default(),
                            max_value: max as u64,
                            current_value: effective_current as u64,
                            reset_at_ms: window_start + window_dur,
                        }
                    })
                    .collect()
            }
        }
    }

    pub async fn delete_vk_entries(&self, vk_id: &str) {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let _ = sqlx::query("DELETE FROM vk_rate_limits WHERE virtual_key_id = $1")
                    .bind(vk_id)
                    .execute(pool)
                    .await;
            }
            DbPool::Postgres(pool) => {
                let _ = sqlx::query::<sqlx::Postgres>(
                    "DELETE FROM vk_rate_limits WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .execute(pool)
                .await;
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RateLimitStatus {
    pub window_type: String,
    pub max_value: u64,
    pub current_value: u64,
    pub reset_at_ms: i64,
}

pub struct RateLimitPlugin {
    store: Arc<RateLimitStore>,
}

impl RateLimitPlugin {
    pub fn new(store: Arc<RateLimitStore>) -> Self {
        Self { store }
    }
}

#[async_trait::async_trait]
impl pylos_core::domain::traits::LlmPlugin for RateLimitPlugin {
    fn name(&self) -> &str {
        "rate_limit"
    }

    fn priority(&self) -> i32 {
        -80
    }

    async fn pre_hook(
        &self,
        _request: &mut pylos_core::domain::request::PylosRequest,
        ctx: &mut pylos_core::domain::request::RequestContext,
    ) -> Result<Option<pylos_core::domain::request::PylosResponse>, PylosError> {
        let vk_id = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(None),
        };

        self.store.check_and_increment_requests(&vk_id).await?;
        Ok(None)
    }

    async fn post_hook(
        &self,
        _request: &pylos_core::domain::request::PylosRequest,
        response: &mut pylos_core::domain::request::PylosResponse,
        ctx: &mut pylos_core::domain::request::RequestContext,
    ) -> Result<(), PylosError> {
        let vk_id = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(()),
        };

        let tokens = match response {
            pylos_core::domain::request::PylosResponse::ChatCompletion(r) => {
                r.usage.as_ref().map(|u| u.total_tokens as i64).unwrap_or(0)
            }
            pylos_core::domain::request::PylosResponse::Embedding(r) => {
                r.usage.prompt_tokens as i64
            }
            _ => 0,
        };

        if tokens > 0 {
            self.store.record_tokens(&vk_id, tokens).await;
        }

        Ok(())
    }
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
    async fn test_request_under_limit() {
        let store = make_store().await;
        store
            .upsert_rate_limit("vk-1", &make_config(10, "1m"))
            .await
            .unwrap();

        for _ in 0..10 {
            store.check_and_increment_requests("vk-1").await.unwrap();
        }
    }

    #[tokio::test]
    async fn test_request_exceeds_limit() {
        let store = make_store().await;
        store
            .upsert_rate_limit("vk-2", &make_config(3, "1m"))
            .await
            .unwrap();

        for _ in 0..3 {
            store.check_and_increment_requests("vk-2").await.unwrap();
        }
        let result = store.check_and_increment_requests("vk-2").await;
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            PylosError::RateLimitExceeded(_)
        ));
    }

    #[tokio::test]
    async fn test_no_limit_configured() {
        let store = make_store().await;
        for _ in 0..1000 {
            store
                .check_and_increment_requests("vk-unknown")
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn test_get_status() {
        let store = make_store().await;
        store
            .upsert_rate_limit("vk-3", &make_config(100, "1m"))
            .await
            .unwrap();
        store.check_and_increment_requests("vk-3").await.unwrap();
        store.check_and_increment_requests("vk-3").await.unwrap();

        let status = store.get_status("vk-3").await;
        assert_eq!(status.len(), 1);
        assert_eq!(status[0].current_value, 2);
        assert_eq!(status[0].max_value, 100);
    }
}
