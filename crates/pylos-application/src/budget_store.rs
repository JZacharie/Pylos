use std::path::Path;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::Row;
use tokio::sync::Mutex;
use tracing::{debug, info};

use crate::db_pool::DbPool;
use pylos_core::domain::config::BudgetConfig;
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct BudgetStore {
    pool: DbPool,
}

impl BudgetStore {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "budget_store", 4).await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "Budget store opened (SQLite)");
        Ok(store)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "budget_store").await?;
        let store = Self { pool };
        store.pool.run_migrations().await?;
        info!("Budget store opened (PostgreSQL)");
        Ok(store)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;

        if let Some(p) = pool.as_sqlite() {
            sqlx::query(
                r#"
                CREATE TABLE IF NOT EXISTS vk_budgets (
                    virtual_key_id TEXT NOT NULL,
                    period         TEXT NOT NULL,
                    max_usd        REAL NOT NULL,
                    current_usd    REAL NOT NULL DEFAULT 0.0,
                    reset_at       INTEGER NOT NULL,
                    PRIMARY KEY (virtual_key_id, period)
                )
                "#,
            )
            .execute(p)
            .await?;
        }

        Ok(Self { pool })
    }

    pub async fn upsert_budget(
        &self,
        vk_id: &str,
        budget: &BudgetConfig,
    ) -> Result<(), sqlx::Error> {
        let period = detect_period(&budget.reset_duration.0);
        let reset_at = next_reset_ms(&budget.reset_duration.0);
        let now = now_ms();

        match &self.pool {
            DbPool::Sqlite(pool) => {
                sqlx::query(
                    "INSERT INTO vk_budgets (virtual_key_id, period, max_usd, current_usd, reset_at) \
                     VALUES ($1, $2, $3, 0.0, $4) \
                     ON CONFLICT(virtual_key_id, period) DO UPDATE SET \
                     max_usd = excluded.max_usd, \
                     reset_at = CASE WHEN reset_at < $5 THEN excluded.reset_at ELSE reset_at END",
                )
                .bind(vk_id)
                .bind(&period)
                .bind(budget.max_limit)
                .bind(reset_at)
                .bind(now)
                .execute(pool)
                .await?;
            }
            DbPool::Postgres(pool) => {
                sqlx::query::<sqlx::Postgres>(
                    "INSERT INTO vk_budgets (virtual_key_id, period, max_usd, current_usd, reset_at) \
                     VALUES ($1, $2, $3, 0.0, $4) \
                     ON CONFLICT(virtual_key_id, period) DO UPDATE SET \
                     max_usd = excluded.max_usd, \
                     reset_at = CASE WHEN vk_budgets.reset_at < $5 THEN excluded.reset_at ELSE vk_budgets.reset_at END",
                )
                .bind(vk_id)
                .bind(&period)
                .bind(budget.max_limit)
                .bind(reset_at)
                .bind(now)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn check_budget(&self, vk_id: &str, estimated_cost: f64) -> Result<(), PylosError> {
        let now = now_ms();

        let budget_row = match &self.pool {
            DbPool::Sqlite(pool) => {
                let row: Option<sqlx::sqlite::SqliteRow> = sqlx::query(
                    "SELECT period, max_usd, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

                row.map(|r| {
                    let p: String = r.try_get("period").unwrap_or_default();
                    let max: f64 = r.try_get("max_usd").unwrap_or(f64::MAX);
                    let cur: f64 = r.try_get("current_usd").unwrap_or(0.0);
                    let res: i64 = r.try_get("reset_at").unwrap_or(0);
                    (p, max, cur, res)
                })
            }
            DbPool::Postgres(pool) => {
                let row: Option<sqlx::postgres::PgRow> = sqlx::query::<sqlx::Postgres>(
                    "SELECT period, max_usd, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

                row.map(|r| {
                    let p: String = r.try_get("period").unwrap_or_default();
                    let max: f64 = r.try_get("max_usd").unwrap_or(f64::MAX);
                    let cur: f64 = r.try_get("current_usd").unwrap_or(0.0);
                    let res: i64 = r.try_get("reset_at").unwrap_or(0);
                    (p, max, cur, res)
                })
            }
        };

        if let Some((_period, _max_usd, _current_usd, _reset_at)) = budget_row {
            if now >= _reset_at {
                debug!(vk_id = %vk_id, period = %_period, "Budget period expired, will reset on next charge");
                return Ok(());
            }

            if _current_usd + estimated_cost > _max_usd {
                return Err(PylosError::BudgetExceeded(format!(
                    "Virtual key '{}' has exceeded its {} budget: ${:.4} used / ${:.2} max",
                    vk_id, _period, _current_usd, _max_usd
                )));
            }
        }

        Ok(())
    }

    pub async fn record_usage(&self, vk_id: &str, cost_usd: f64) {
        if cost_usd <= 0.0 {
            return;
        }
        let now = now_ms();

        match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query(
                    "SELECT period, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                for row in &rows {
                    let period: String = row.try_get("period").unwrap_or_default();
                    let reset_at: i64 = row.try_get("reset_at").unwrap_or(0);

                    if now >= reset_at {
                        let new_reset = reset_at + period_ms(&period);
                        let _ = sqlx::query(
                            "UPDATE vk_budgets SET current_usd = $1, reset_at = $2 WHERE virtual_key_id = $3 AND period = $4",
                        )
                        .bind(cost_usd)
                        .bind(new_reset)
                        .bind(vk_id)
                        .bind(&period)
                        .execute(pool)
                        .await;
                    } else {
                        let _ = sqlx::query(
                            "UPDATE vk_budgets SET current_usd = current_usd + $1 WHERE virtual_key_id = $2 AND period = $3",
                        )
                        .bind(cost_usd)
                        .bind(vk_id)
                        .bind(&period)
                        .execute(pool)
                        .await;
                    }
                }
            }
            DbPool::Postgres(pool) => {
                let rows = sqlx::query::<sqlx::Postgres>(
                    "SELECT period, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                for row in &rows {
                    let period: String = row.try_get("period").unwrap_or_default();
                    let reset_at: i64 = row.try_get("reset_at").unwrap_or(0);

                    if now >= reset_at {
                        let new_reset = reset_at + period_ms(&period);
                        let _ = sqlx::query::<sqlx::Postgres>(
                            "UPDATE vk_budgets SET current_usd = $1, reset_at = $2 WHERE virtual_key_id = $3 AND period = $4",
                        )
                        .bind(cost_usd)
                        .bind(new_reset)
                        .bind(vk_id)
                        .bind(&period)
                        .execute(pool)
                        .await;
                    } else {
                        let _ = sqlx::query::<sqlx::Postgres>(
                            "UPDATE vk_budgets SET current_usd = current_usd + $1 WHERE virtual_key_id = $2 AND period = $3",
                        )
                        .bind(cost_usd)
                        .bind(vk_id)
                        .bind(&period)
                        .execute(pool)
                        .await;
                    }
                }
            }
        }
    }

    pub async fn get_usage(&self, vk_id: &str) -> Vec<BudgetUsage> {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let rows = sqlx::query(
                    "SELECT period, max_usd, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1 ORDER BY period",
                )
                .bind(vk_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                rows.iter()
                    .map(|r| BudgetUsage {
                        period: r.try_get("period").unwrap_or_default(),
                        max_usd: r.try_get("max_usd").unwrap_or(0.0),
                        current_usd: r.try_get("current_usd").unwrap_or(0.0),
                        reset_at_ms: r.try_get("reset_at").unwrap_or(0),
                    })
                    .collect()
            }
            DbPool::Postgres(pool) => {
                let rows = sqlx::query::<sqlx::Postgres>(
                    "SELECT period, max_usd, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1 ORDER BY period",
                )
                .bind(vk_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                rows.iter()
                    .map(|r| BudgetUsage {
                        period: r.try_get("period").unwrap_or_default(),
                        max_usd: r.try_get("max_usd").unwrap_or(0.0),
                        current_usd: r.try_get("current_usd").unwrap_or(0.0),
                        reset_at_ms: r.try_get("reset_at").unwrap_or(0),
                    })
                    .collect()
            }
        }
    }

    pub async fn delete_vk_entries(&self, vk_id: &str) {
        match &self.pool {
            DbPool::Sqlite(pool) => {
                let _ = sqlx::query("DELETE FROM vk_budgets WHERE virtual_key_id = $1")
                    .bind(vk_id)
                    .execute(pool)
                    .await;
            }
            DbPool::Postgres(pool) => {
                let _ = sqlx::query::<sqlx::Postgres>(
                    "DELETE FROM vk_budgets WHERE virtual_key_id = $1",
                )
                .bind(vk_id)
                .execute(pool)
                .await;
            }
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BudgetUsage {
    pub period: String,
    pub max_usd: f64,
    pub current_usd: f64,
    pub reset_at_ms: i64,
}

// ── BudgetPlugin ─────────────────────────────────────────────────

pub struct BudgetPlugin {
    store: Arc<BudgetStore>,
    pending_cost: Arc<Mutex<std::collections::HashMap<String, f64>>>,
}

impl BudgetPlugin {
    pub fn new(store: Arc<BudgetStore>) -> Self {
        Self {
            store,
            pending_cost: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }
}

#[async_trait::async_trait]
impl pylos_core::domain::traits::LlmPlugin for BudgetPlugin {
    fn name(&self) -> &str {
        "budget"
    }

    async fn pre_hook(
        &self,
        request: &mut pylos_core::domain::request::PylosRequest,
        ctx: &mut pylos_core::domain::request::RequestContext,
    ) -> Result<Option<pylos_core::domain::request::PylosResponse>, PylosError> {
        let vk_id = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(None),
        };

        let estimated = estimate_request_cost(request.model());
        self.store.check_budget(&vk_id, estimated).await?;

        let mut pending = self.pending_cost.lock().await;
        pending.insert(vk_id, estimated);

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

        let actual_cost = match response {
            pylos_core::domain::request::PylosResponse::ChatCompletion(r) => r
                .usage
                .as_ref()
                .map(|u| {
                    crate::log_store::estimate_cost_pub(
                        &guess_provider_from_model(&r.model),
                        &r.model,
                        u.prompt_tokens,
                        u.completion_tokens,
                    )
                })
                .unwrap_or(0.0),
            _ => 0.0,
        };

        let mut pending = self.pending_cost.lock().await;
        pending.remove(&vk_id);
        drop(pending);

        if actual_cost > 0.0 {
            self.store.record_usage(&vk_id, actual_cost).await;
            debug!(vk_id = %vk_id, cost_usd = actual_cost, "Budget usage recorded");
        }

        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn detect_period(duration_str: &str) -> String {
    let s = duration_str.trim();
    if s == "1w" || s == "7d" || (s.ends_with('w') && s[..s.len() - 1].parse::<u64>().is_ok()) {
        return "weekly".to_string();
    }
    if s.ends_with('d') && s[..s.len() - 1].parse::<u64>().unwrap_or(0) == 1 {
        "daily".to_string()
    } else if s.ends_with('M')
        || (s.ends_with('d') && s[..s.len() - 1].parse::<u64>().unwrap_or(0) >= 28)
    {
        "monthly".to_string()
    } else if s == "total" || s.ends_with('Y') {
        "total".to_string()
    } else {
        format!("window_{}", s)
    }
}

fn period_ms(period: &str) -> i64 {
    match period {
        "daily" => 86_400_000,
        "weekly" => 7 * 86_400_000,
        "monthly" => 30 * 86_400_000,
        "total" => i64::MAX / 2,
        _ => 3_600_000,
    }
}

fn next_reset_ms(duration_str: &str) -> i64 {
    let period = detect_period(duration_str);
    now_ms() + period_ms(&period)
}

fn estimate_request_cost(model: &str) -> f64 {
    let (input_per_1m, output_per_1m): (f64, f64) = if model.contains("gpt-4o") {
        (5.0, 15.0)
    } else if model.contains("claude-3-5") || model.contains("claude-3.5") {
        (3.0, 15.0)
    } else if model.contains("gemini-2.5-pro") {
        (7.0, 21.0)
    } else {
        (1.0, 3.0)
    };
    (1000.0 / 1_000_000.0) * input_per_1m + (500.0 / 1_000_000.0) * output_per_1m
}

fn guess_provider_from_model(model: &str) -> String {
    pylos_core::domain::provider::ProviderKind::guess_from_model(model).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::config::{BudgetConfig, Duration};

    async fn make_store() -> BudgetStore {
        BudgetStore::in_memory()
            .await
            .expect("in-memory budget store")
    }

    #[tokio::test]
    async fn test_upsert_and_check_budget_under_limit() {
        let store = make_store().await;
        let budget = BudgetConfig {
            id: "b1".into(),
            max_limit: 10.0,
            reset_duration: Duration("1d".into()),
            current_usage: 0.0,
            virtual_key_id: Some("vk-1".into()),
        };
        store.upsert_budget("vk-1", &budget).await.unwrap();

        let result = store.check_budget("vk-1", 1.0).await;
        assert!(result.is_ok(), "Should be within budget");
    }

    #[tokio::test]
    async fn test_budget_exceeded() {
        let store = make_store().await;
        let budget = BudgetConfig {
            id: "b1".into(),
            max_limit: 1.0,
            reset_duration: Duration("1d".into()),
            current_usage: 0.0,
            virtual_key_id: Some("vk-2".into()),
        };
        store.upsert_budget("vk-2", &budget).await.unwrap();

        store.record_usage("vk-2", 0.95).await;

        let result = store.check_budget("vk-2", 0.10).await;
        assert!(result.is_err(), "Should exceed budget");
        assert!(matches!(result.unwrap_err(), PylosError::BudgetExceeded(_)));
    }

    #[tokio::test]
    async fn test_no_vk_no_budget_check() {
        let store = make_store().await;
        let result = store.check_budget("vk-unknown", 1000.0).await;
        assert!(
            result.is_ok(),
            "Unknown VK should have no budget constraint"
        );
    }

    #[tokio::test]
    async fn test_get_usage() {
        let store = make_store().await;
        let budget = BudgetConfig {
            id: "b1".into(),
            max_limit: 5.0,
            reset_duration: Duration("1d".into()),
            current_usage: 0.0,
            virtual_key_id: Some("vk-3".into()),
        };
        store.upsert_budget("vk-3", &budget).await.unwrap();
        store.record_usage("vk-3", 1.50).await;

        let usages = store.get_usage("vk-3").await;
        assert_eq!(usages.len(), 1);
        assert!((usages[0].current_usd - 1.50).abs() < 0.001);
        assert!((usages[0].max_usd - 5.0).abs() < 0.001);
    }
}
