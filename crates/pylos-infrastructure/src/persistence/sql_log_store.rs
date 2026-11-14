use std::path::Path;

use async_trait::async_trait;
use tracing::info;

use super::db_pool::{DbPool, DbRow, DbValue};
use pylos_core::domain::logging::{
    now_ms, GuardrailsBreakdown, GuardrailsTimeline, HistogramBucket, LogEntry, LogFilter,
    LogStats, LogStatus, TokenBucket,
};
use pylos_core::domain::repositories::LogRepository;

/// Store SQL unifié pour les logs d'inférence (SQLite et PostgreSQL via `DbPool`).
/// Remplace les anciens `SqliteLogStore` et `PgLogStore` dupliqués.
#[derive(Clone)]
pub struct SqlLogStore {
    pool: DbPool,
    retention_days: u32,
}

pub type PgLogStore = SqlLogStore;
pub type SqliteLogStore = SqlLogStore;

impl SqlLogStore {
    pub async fn new(database_url: &str, retention_days: u32) -> Result<Self, sqlx::Error> {
        let pool = if database_url.starts_with("postgres") || database_url.starts_with("postgresql")
        {
            DbPool::open_postgres(database_url, "log_store").await?
        } else {
            let path = database_url.strip_prefix("sqlite://").unwrap_or(database_url);
            DbPool::open_sqlite(Path::new(path), "log_store", 4).await?
        };
        let store = Self {
            pool,
            retention_days,
        };
        store.pool.run_migrations().await?;
        info!("Unified SQL log store opened");
        Ok(store)
    }

    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "log_store", 4).await?;
        let store = Self {
            pool,
            retention_days: 30,
        };
        store.pool.run_migrations().await?;
        info!(path = %db_path.display(), "SQL log store opened (SQLite)");
        Ok(store)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        Self::new(database_url, 30).await
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        pool.execute(
            r#"
            CREATE TABLE IF NOT EXISTS requests (
                id TEXT PRIMARY KEY,
                timestamp INTEGER NOT NULL,
                provider TEXT NOT NULL,
                model TEXT NOT NULL,
                object TEXT NOT NULL DEFAULT 'chat.completion',
                status TEXT NOT NULL,
                latency_ms REAL NOT NULL,
                prompt_tokens INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens INTEGER NOT NULL DEFAULT 0,
                cost_usd REAL NOT NULL DEFAULT 0.0,
                finish_reason TEXT,
                error_message TEXT,
                virtual_key TEXT,
                is_stream INTEGER NOT NULL DEFAULT 0,
                input_preview TEXT,
                output_preview TEXT,
                compression_saved_bytes INTEGER NOT NULL DEFAULT 0,
                guardrail_triggered INTEGER,
                guardrail_type TEXT,
                guardrail_detail TEXT
            )
            "#,
            vec![],
        )
        .await?;
        Ok(Self {
            pool,
            retention_days: 30,
        })
    }

    pub async fn push(&self, entry: LogEntry) {
        let _ = self
            .pool
            .execute(
                r#"
                INSERT INTO requests
                    (id, timestamp, provider, model, object, status, latency_ms,
                     prompt_tokens, completion_tokens, total_tokens, cost_usd,
                     finish_reason, error_message, virtual_key, is_stream,
                     input_preview, output_preview, compression_saved_bytes,
                     guardrail_triggered, guardrail_type, guardrail_detail)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19, $20, $21)
                ON CONFLICT(id) DO NOTHING
                "#,
                vec![
                    DbValue::Text(entry.id),
                    DbValue::I64(entry.timestamp),
                    DbValue::Text(entry.provider),
                    DbValue::Text(entry.model),
                    DbValue::Text(entry.object),
                    DbValue::Text(entry.status.as_str().to_string()),
                    DbValue::F64(entry.latency_ms),
                    DbValue::I64(i64::from(entry.prompt_tokens)),
                    DbValue::I64(i64::from(entry.completion_tokens)),
                    DbValue::I64(i64::from(entry.total_tokens)),
                    DbValue::F64(entry.cost_usd),
                    DbValue::OptText(entry.finish_reason),
                    DbValue::OptText(entry.error_message),
                    DbValue::OptText(entry.virtual_key),
                    DbValue::I64(if entry.is_stream { 1 } else { 0 }),
                    DbValue::OptText(entry.input_preview),
                    DbValue::OptText(entry.output_preview),
                    DbValue::I64(entry.compression_saved_bytes as i64),
                    DbValue::Bool(entry.guardrail_triggered.unwrap_or(false)),
                    DbValue::OptText(entry.guardrail_type),
                    DbValue::OptText(entry.guardrail_detail),
                ],
            )
            .await;

        if fastrand::u8(..) == 0 {
            self.purge_old_logs().await;
        }
    }

    pub async fn list(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        let (where_clause, params) = build_where_clause(filter);

        let count_sql = format!("SELECT COUNT(*) AS cnt FROM requests {}", where_clause);
        let total = self
            .pool
            .fetch_optional(&count_sql, params.clone())
            .await
            .ok()
            .flatten()
            .map(|r| r.get_i64("cnt") as u64)
            .unwrap_or(0);

        let limit_idx = params.len() + 1;
        let offset_idx = params.len() + 2;
        let data_sql = format!(
            "SELECT * FROM requests {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
            where_clause, limit_idx, offset_idx
        );
        let mut data_params = params;
        data_params.push(DbValue::I64(limit as i64));
        data_params.push(DbValue::I64(offset as i64));

        let rows = self
            .pool
            .fetch_all(&data_sql, data_params)
            .await
            .unwrap_or_default()
            .iter()
            .map(row_to_log_entry)
            .collect();

        (rows, total)
    }

    pub async fn stats(&self, filter: &LogFilter) -> LogStats {
        let (where_clause, params) = build_where_clause(filter);
        let sql = format!(
            r#"
            SELECT
                COUNT(*) AS total,
                COALESCE(SUM(CASE WHEN status='success' THEN 1 ELSE 0 END), 0) AS successes,
                COALESCE(AVG(latency_ms), 0.0) AS avg_latency,
                COALESCE(SUM(total_tokens), 0) AS total_tokens,
                COALESCE(SUM(prompt_tokens), 0) AS total_prompt,
                COALESCE(SUM(completion_tokens), 0) AS total_completion,
                COALESCE(SUM(cost_usd), 0.0) AS total_cost,
                COALESCE(SUM(compression_saved_bytes), 0) AS total_compression_saved_bytes
            FROM requests {}
            "#,
            where_clause
        );

        let Some(row) = self.pool.fetch_optional(&sql, params).await.ok().flatten() else {
            return LogStats::default();
        };
        let total = row.get_i64("total");
        if total == 0 {
            return LogStats::default();
        }
        let successes = row.get_i64("successes");
        LogStats {
            total_requests: total as u64,
            success_rate: (successes as f64 / total as f64) * 100.0,
            average_latency_ms: row.get_f64("avg_latency"),
            total_tokens: row.get_i64("total_tokens"),
            total_cost_usd: row.get_f64("total_cost"),
            total_prompt_tokens: row.get_i64("total_prompt"),
            total_completion_tokens: row.get_i64("total_completion"),
            total_compression_saved_bytes: row.get_i64("total_compression_saved_bytes"),
        }
    }

    pub async fn histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<HistogramBucket> {
        let bucket_ms = bucket_secs * 1000;
        let (where_clause, params) = build_where_clause(filter);
        let sql = format!(
            r#"
            SELECT
                (timestamp / {bucket_ms}) * {bucket_ms} AS bucket_ts,
                COUNT(*) AS total,
                COALESCE(SUM(CASE WHEN status='success' THEN 1 ELSE 0 END), 0) AS successes,
                COALESCE(SUM(CASE WHEN status='error' THEN 1 ELSE 0 END), 0) AS errors
            FROM requests {where_clause}
            GROUP BY bucket_ts
            ORDER BY bucket_ts ASC
            "#,
        );

        self.pool
            .fetch_all(&sql, params)
            .await
            .unwrap_or_default()
            .iter()
            .map(|row| HistogramBucket {
                timestamp: row.get_i64("bucket_ts"),
                count: row.get_i64("total") as u64,
                success: row.get_i64("successes") as u64,
                error: row.get_i64("errors") as u64,
            })
            .collect()
    }

    pub async fn token_histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<TokenBucket> {
        let bucket_ms = bucket_secs * 1000;
        let (where_clause, params) = build_where_clause(filter);
        let sql = format!(
            r#"
            SELECT
                (timestamp / {bucket_ms}) * {bucket_ms} AS bucket_ts,
                COALESCE(SUM(prompt_tokens), 0) AS prompt_tokens,
                COALESCE(SUM(completion_tokens), 0) AS completion_tokens,
                COALESCE(SUM(total_tokens), 0) AS total_tokens
            FROM requests {where_clause}
            GROUP BY bucket_ts
            ORDER BY bucket_ts ASC
            "#,
        );

        self.pool
            .fetch_all(&sql, params)
            .await
            .unwrap_or_default()
            .iter()
            .map(|row| TokenBucket {
                timestamp: row.get_i64("bucket_ts"),
                prompt_tokens: row.get_i64("prompt_tokens"),
                completion_tokens: row.get_i64("completion_tokens"),
                total_tokens: row.get_i64("total_tokens"),
            })
            .collect()
    }

    pub async fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        let (where_clause, mut params) = build_where_clause(filter);
        let flag_idx = params.len() + 1;
        params.push(DbValue::Bool(true));
        let guard_where = if where_clause.is_empty() {
            format!("WHERE guardrail_triggered = ${}", flag_idx)
        } else {
            format!("{} AND guardrail_triggered = ${}", where_clause, flag_idx)
        };

        let count_sql = format!("SELECT COUNT(*) AS cnt FROM requests {}", guard_where);
        let total = self
            .pool
            .fetch_optional(&count_sql, params.clone())
            .await
            .ok()
            .flatten()
            .map(|r| r.get_i64("cnt") as u64)
            .unwrap_or(0);

        let limit_idx = params.len() + 1;
        let offset_idx = params.len() + 2;
        let data_sql = format!(
            "SELECT * FROM requests {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
            guard_where, limit_idx, offset_idx
        );
        params.push(DbValue::I64(limit as i64));
        params.push(DbValue::I64(offset as i64));

        let rows = self
            .pool
            .fetch_all(&data_sql, params)
            .await
            .unwrap_or_default()
            .iter()
            .map(row_to_log_entry)
            .collect();

        (rows, total)
    }

    pub async fn guardrails_stats(&self, filter: &LogFilter) -> GuardrailsBreakdown {
        let (where_clause, mut params) = build_where_clause(filter);
        let flag_idx = params.len() + 1;
        params.push(DbValue::Bool(true));
        let guard_where = if where_clause.is_empty() {
            format!("WHERE guardrail_triggered = ${}", flag_idx)
        } else {
            format!("{} AND guardrail_triggered = ${}", where_clause, flag_idx)
        };

        let total_sql = format!(
            "SELECT COUNT(*) AS cnt FROM requests {} AND guardrail_type IS NOT NULL",
            guard_where
        );
        let total_blocks = self
            .pool
            .fetch_optional(&total_sql, params)
            .await
            .ok()
            .flatten()
            .map(|r| r.get_i64("cnt") as u64)
            .unwrap_or(0);

        GuardrailsBreakdown {
            total_blocks,
            keyword_blocks: 0,
            prompt_injection_blocks: 0,
            content_filter_blocks: 0,
            top_keywords: vec![],
        }
    }

    pub async fn guardrails_breakdown(&self, filter: &LogFilter) -> GuardrailsBreakdown {
        self.guardrails_stats(filter).await
    }

    pub async fn guardrails_timeline(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> Vec<GuardrailsTimeline> {
        let bucket_ms = bucket_secs * 1000;
        let (where_clause, mut params) = build_where_clause(filter);
        let flag_idx = params.len() + 1;
        params.push(DbValue::Bool(true));
        let guard_where = if where_clause.is_empty() {
            format!("WHERE guardrail_triggered = ${}", flag_idx)
        } else {
            format!("{} AND guardrail_triggered = ${}", where_clause, flag_idx)
        };

        let sql = format!(
            "SELECT (timestamp / {b}) * {b} AS ts,
                    COUNT(*) AS total,
                    COALESCE(SUM(CASE WHEN guardrail_type = 'keyword_block' THEN 1 ELSE 0 END), 0) AS keyword_blocks,
                    COALESCE(SUM(CASE WHEN guardrail_type = 'prompt_injection' THEN 1 ELSE 0 END), 0) AS prompt_injection,
                    COALESCE(SUM(CASE WHEN guardrail_type = 'content_filter' THEN 1 ELSE 0 END), 0) AS content_filter
             FROM requests {guard_where}
             GROUP BY ts ORDER BY ts ASC",
            b = bucket_ms
        );

        self.pool
            .fetch_all(&sql, params)
            .await
            .unwrap_or_default()
            .iter()
            .map(|row| GuardrailsTimeline {
                timestamp: row.get_i64("ts"),
                total: row.get_i64("total") as u64,
                keyword_blocks: row.get_i64("keyword_blocks") as u64,
                prompt_injection: row.get_i64("prompt_injection") as u64,
                content_filter: row.get_i64("content_filter") as u64,
            })
            .collect()
    }

    async fn purge_old_logs(&self) {
        let cutoff = now_ms() - (i64::from(self.retention_days) * 86_400_000);
        let _ = self
            .pool
            .execute(
                "DELETE FROM requests WHERE timestamp < $1",
                vec![DbValue::I64(cutoff)],
            )
            .await;
    }
}

#[async_trait]
impl LogRepository for SqlLogStore {
    async fn push(&self, entry: LogEntry) {
        self.push(entry).await;
    }

    async fn list(&self, limit: usize, offset: usize, filter: &LogFilter) -> (Vec<LogEntry>, u64) {
        self.list(limit, offset, filter).await
    }

    async fn stats(&self, filter: &LogFilter) -> LogStats {
        self.stats(filter).await
    }

    async fn histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<HistogramBucket> {
        self.histogram(filter, bucket_secs).await
    }

    async fn token_histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<TokenBucket> {
        self.token_histogram(filter, bucket_secs).await
    }

    async fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        self.list_guardrails(limit, offset, filter).await
    }

    async fn guardrails_breakdown(&self, filter: &LogFilter) -> GuardrailsBreakdown {
        self.guardrails_breakdown(filter).await
    }

    async fn guardrails_timeline(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> Vec<GuardrailsTimeline> {
        self.guardrails_timeline(filter, bucket_secs).await
    }
}

fn build_where_clause(filter: &LogFilter) -> (String, Vec<DbValue>) {
    let mut conditions: Vec<String> = vec![];
    let mut params: Vec<DbValue> = vec![];

    if let Some(ref p) = filter.provider {
        conditions.push(format!("provider = ${}", params.len() + 1));
        params.push(DbValue::Text(p.clone()));
    }
    if let Some(ref m) = filter.model {
        conditions.push(format!("model LIKE ${}", params.len() + 1));
        params.push(DbValue::Text(format!("%{}%", m)));
    }
    if let Some(ref s) = filter.status {
        conditions.push(format!("status = ${}", params.len() + 1));
        params.push(DbValue::Text(s.as_str().to_string()));
    }
    if let Some(since) = filter.since_ms {
        conditions.push(format!("timestamp >= ${}", params.len() + 1));
        params.push(DbValue::I64(since));
    }
    if let Some(until) = filter.until_ms {
        conditions.push(format!("timestamp <= ${}", params.len() + 1));
        params.push(DbValue::I64(until));
    }
    if let Some(ref vk) = filter.virtual_key {
        conditions.push(format!("virtual_key = ${}", params.len() + 1));
        params.push(DbValue::Text(vk.clone()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, params)
}

fn row_to_log_entry(row: &DbRow) -> LogEntry {
    let status = LogStatus::parse(&row.get_str("status"));
    let is_stream = row.get_bool("is_stream");
    let guardrail_triggered = if row.get_bool("guardrail_triggered") {
        Some(true)
    } else {
        None
    };

    LogEntry {
        id: row.get_str("id"),
        timestamp: row.get_i64("timestamp"),
        provider: row.get_str("provider"),
        model: row.get_str("model"),
        object: row.get_str("object"),
        status,
        latency_ms: row.get_f64("latency_ms"),
        prompt_tokens: row.get_i64("prompt_tokens") as i32,
        completion_tokens: row.get_i64("completion_tokens") as i32,
        total_tokens: row.get_i64("total_tokens") as i32,
        cost_usd: row.get_f64("cost_usd"),
        finish_reason: row.get_opt_str("finish_reason"),
        error_message: row.get_opt_str("error_message"),
        virtual_key: row.get_opt_str("virtual_key"),
        is_stream,
        input_preview: row.get_opt_str("input_preview"),
        output_preview: row.get_opt_str("output_preview"),
        compression_saved_bytes: row.get_i64("compression_saved_bytes") as usize,
        guardrail_triggered,
        guardrail_type: row.get_opt_str("guardrail_type"),
        guardrail_detail: row.get_opt_str("guardrail_detail"),
        obfuscated_input: None,
        pii_mapping: None,
        moderation_scores: None,
        caveman_input: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::logging::build_log_entry;

    #[tokio::test]
    async fn test_sql_log_store_push_and_query() {
        let store = SqlLogStore::in_memory().await.unwrap();
        let entry = build_log_entry(
            "openai",
            "gpt-4o",
            false,
            LogStatus::Success,
            12.5,
            None,
            Some("stop".into()),
            None,
            None,
            None,
            None,
            0,
        );
        store.push(entry).await;
        let (items, total) = store.list(10, 0, &LogFilter::default()).await;
        assert_eq!(total, 1);
        assert_eq!(items[0].model, "gpt-4o");
    }
}
