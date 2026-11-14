use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tracing::{debug, info};

use super::db_pool::{DbPool, DbValue};
use pylos_core::domain::config::BudgetConfig;
use pylos_core::domain::repositories::BudgetRepository;
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

    pub async fn from_pool(pool: DbPool) -> Result<Self, sqlx::Error> {
        let store = Self { pool };
        store.pool.run_migrations().await?;
        Ok(store)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        pool.execute(
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
            vec![],
        )
        .await?;
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

        self.pool
            .execute(
                "INSERT INTO vk_budgets (virtual_key_id, period, max_usd, current_usd, reset_at) \
                 VALUES ($1, $2, $3, 0.0, $4) \
                 ON CONFLICT(virtual_key_id, period) DO UPDATE SET \
                 max_usd = excluded.max_usd, \
                 reset_at = CASE WHEN vk_budgets.reset_at < $5 THEN excluded.reset_at ELSE vk_budgets.reset_at END",
                vec![
                    DbValue::Text(vk_id.to_string()),
                    DbValue::Text(period),
                    DbValue::F64(budget.max_limit),
                    DbValue::I64(reset_at),
                    DbValue::I64(now),
                ],
            )
            .await?;

        Ok(())
    }

    pub async fn check_budget(&self, vk_id: &str, estimated_cost: f64) -> Result<(), PylosError> {
        let now = now_ms();

        let row = self
            .pool
            .fetch_optional(
                "SELECT period, max_usd, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1",
                vec![DbValue::Text(vk_id.to_string())],
            )
            .await
            .ok()
            .flatten();

        if let Some(r) = row {
            let period = r.get_str("period");
            let max_usd = r.get_opt_f64("max_usd").unwrap_or(f64::MAX);
            let current_usd = r.get_f64("current_usd");
            let reset_at = r.get_i64("reset_at");

            if now >= reset_at {
                debug!(vk_id = %vk_id, period = %period, "Budget period expired, will reset on next charge");
                return Ok(());
            }

            if current_usd + estimated_cost > max_usd {
                return Err(PylosError::BudgetExceeded(format!(
                    "Virtual key '{}' has exceeded its {} budget: ${:.4} used / ${:.2} max",
                    vk_id, period, current_usd, max_usd
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

        let rows = self
            .pool
            .fetch_all(
                "SELECT period, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1",
                vec![DbValue::Text(vk_id.to_string())],
            )
            .await
            .unwrap_or_default();

        for row in &rows {
            let period = row.get_str("period");
            let reset_at = row.get_i64("reset_at");

            if now >= reset_at {
                let new_reset = reset_at + period_ms(&period);
                let _ = self
                    .pool
                    .execute(
                        "UPDATE vk_budgets SET current_usd = $1, reset_at = $2 WHERE virtual_key_id = $3 AND period = $4",
                        vec![
                            DbValue::F64(cost_usd),
                            DbValue::I64(new_reset),
                            DbValue::Text(vk_id.to_string()),
                            DbValue::Text(period),
                        ],
                    )
                    .await;
            } else {
                let _ = self
                    .pool
                    .execute(
                        "UPDATE vk_budgets SET current_usd = current_usd + $1 WHERE virtual_key_id = $2 AND period = $3",
                        vec![
                            DbValue::F64(cost_usd),
                            DbValue::Text(vk_id.to_string()),
                            DbValue::Text(period),
                        ],
                    )
                    .await;
            }
        }
    }

    pub async fn get_usage(&self, vk_id: &str) -> Vec<BudgetUsage> {
        let rows = self
            .pool
            .fetch_all(
                "SELECT period, max_usd, current_usd, reset_at FROM vk_budgets WHERE virtual_key_id = $1 ORDER BY period",
                vec![DbValue::Text(vk_id.to_string())],
            )
            .await
            .unwrap_or_default();

        rows.iter()
            .map(|r| BudgetUsage {
                period: r.get_str("period"),
                max_usd: r.get_f64("max_usd"),
                current_usd: r.get_f64("current_usd"),
                reset_at_ms: r.get_i64("reset_at"),
            })
            .collect()
    }

    pub async fn delete_vk_entries(&self, vk_id: &str) {
        let _ = self
            .pool
            .execute(
                "DELETE FROM vk_budgets WHERE virtual_key_id = $1",
                vec![DbValue::Text(vk_id.to_string())],
            )
            .await;
    }
}

#[async_trait]
impl BudgetRepository for BudgetStore {
    async fn check_budget(&self, vk_id: &str, estimated_cost: f64) -> Result<(), PylosError> {
        self.check_budget(vk_id, estimated_cost).await
    }

    async fn record_usage(&self, vk_id: &str, cost_usd: f64) {
        self.record_usage(vk_id, cost_usd).await
    }

    async fn upsert_budget(&self, vk_id: &str, budget: &BudgetConfig) -> Result<(), PylosError> {
        self.upsert_budget(vk_id, budget)
            .await
            .map_err(|e| PylosError::Internal(format!("Budget upsert failed: {}", e)))
    }

    async fn delete_budget(&self, vk_id: &str) -> Result<(), PylosError> {
        self.delete_vk_entries(vk_id).await;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct BudgetUsage {
    pub period: String,
    pub max_usd: f64,
    pub current_usd: f64,
    pub reset_at_ms: i64,
}

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
        "total" => 100 * 365 * 86_400_000,
        _ => 86_400_000,
    }
}

fn next_reset_ms(duration_str: &str) -> i64 {
    let period = detect_period(duration_str);
    now_ms() + period_ms(&period)
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
        assert!(result.is_ok(), "Keys without a budget should pass");
    }
}
