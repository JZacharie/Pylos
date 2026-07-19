use std::sync::Arc;

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::Row;
use tokio::sync::Mutex;
use tracing::info;

use crate::log_store::{
    now_ms, HistogramBucket, LogEntry, LogFilter, LogStats, LogStatus, TokenBucket,
};

#[derive(Clone)]
pub struct PgLogStore {
    pool: PgPool,
    retention_days: u32,
    #[allow(dead_code)]
    pending_purge: Arc<Mutex<bool>>,
}

impl PgLogStore {
    pub async fn new(database_url: &str, retention_days: u32) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(8)
            .connect(database_url)
            .await?;

        sqlx::migrate!("./migrations_postgres").run(&pool).await?;

        info!("PostgreSQL log store opened");

        Ok(Self {
            pool,
            retention_days,
            pending_purge: Arc::new(Mutex::new(false)),
        })
    }

    pub async fn push(&self, entry: LogEntry) {
        let status_str = match entry.status {
            LogStatus::Success => "success",
            LogStatus::Error => "error",
        };

        if let Err(e) = sqlx::query(
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
        )
        .bind(&entry.id)
        .bind(entry.timestamp)
        .bind(&entry.provider)
        .bind(&entry.model)
        .bind(&entry.object)
        .bind(status_str)
        .bind(entry.latency_ms)
        .bind(entry.prompt_tokens)
        .bind(entry.completion_tokens)
        .bind(entry.total_tokens)
        .bind(entry.cost_usd)
        .bind(&entry.finish_reason)
        .bind(&entry.error_message)
        .bind(&entry.virtual_key)
        .bind(entry.is_stream as i16)
        .bind(&entry.input_preview)
        .bind(&entry.output_preview)
        .bind(entry.compression_saved_bytes as i32)
        .bind(entry.guardrail_triggered)
        .bind(&entry.guardrail_type)
        .bind(&entry.guardrail_detail)
        .execute(&self.pool)
        .await
        {
            tracing::warn!(error = %e, "Log insert failed");
        }

        // Purge aléatoire ~1/256
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

        let count_sql = format!("SELECT COUNT(*) FROM requests {}", where_clause);
        let mut count_q = sqlx::query(&count_sql);
        for p in &params {
            count_q = count_q.bind(p);
        }
        let total: u64 = count_q
            .fetch_one(&self.pool)
            .await
            .map(|r| r.try_get::<i64, _>(0).unwrap_or(0) as u64)
            .unwrap_or(0);

        let limit_idx = params.len() + 1;
        let offset_idx = params.len() + 2;
        let data_sql = format!(
            "SELECT * FROM requests {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
            where_clause, limit_idx, offset_idx
        );
        let mut data_q = sqlx::query(&data_sql);
        for p in &params {
            data_q = data_q.bind(p);
        }
        data_q = data_q.bind(limit as i64).bind(offset as i64);
        let rows = data_q
            .fetch_all(&self.pool)
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
                COUNT(*)                    AS total,
                SUM(CASE WHEN status='success' THEN 1 ELSE 0 END) AS successes,
                AVG(latency_ms)             AS avg_latency,
                SUM(total_tokens)           AS total_tokens,
                SUM(prompt_tokens)          AS total_prompt,
                SUM(completion_tokens)      AS total_completion,
                SUM(cost_usd)               AS total_cost,
                SUM(compression_saved_bytes) AS total_compression_saved_bytes
            FROM requests {}
            "#,
            where_clause
        );

        let mut q = sqlx::query(&sql);
        for p in &params {
            q = q.bind(p);
        }

        match q.fetch_one(&self.pool).await {
            Ok(row) => {
                let total: i64 = row.try_get(0).unwrap_or(0);
                if total == 0 {
                    return LogStats::default();
                }
                let successes: i64 = row.try_get(1).unwrap_or(0);
                let avg_latency: f64 = row.try_get(2).unwrap_or(0.0);
                let total_tokens: i64 = row.try_get(3).unwrap_or(0);
                let total_prompt: i64 = row.try_get(4).unwrap_or(0);
                let total_completion: i64 = row.try_get(5).unwrap_or(0);
                let total_cost: f64 = row.try_get(6).unwrap_or(0.0);

                LogStats {
                    total_requests: total as u64,
                    success_rate: (successes as f64 / total as f64) * 100.0,
                    average_latency_ms: avg_latency,
                    total_tokens,
                    total_cost_usd: total_cost,
                    total_prompt_tokens: total_prompt,
                    total_completion_tokens: total_completion,
                    total_compression_saved_bytes: row.try_get::<i64, _>(7).unwrap_or(0),
                }
            }
            Err(_) => LogStats::default(),
        }
    }

    pub async fn histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<HistogramBucket> {
        let bucket_ms = bucket_secs * 1000;
        let (where_clause, params) = build_where_clause(filter);

        let sql = format!(
            r#"
            SELECT
                (timestamp / {bucket_ms}) * {bucket_ms}  AS bucket_ts,
                COUNT(*)                                  AS total,
                SUM(CASE WHEN status='success' THEN 1 ELSE 0 END) AS successes,
                SUM(CASE WHEN status='error'   THEN 1 ELSE 0 END) AS errors
            FROM requests {where_clause}
            GROUP BY bucket_ts
            ORDER BY bucket_ts ASC
            "#,
            bucket_ms = bucket_ms,
            where_clause = where_clause
        );

        let mut q = sqlx::query(&sql);
        for p in &params {
            q = q.bind(p);
        }

        match q.fetch_all(&self.pool).await {
            Ok(rows) => rows
                .iter()
                .map(|row| HistogramBucket {
                    timestamp: row.try_get::<i64, _>(0).unwrap_or(0),
                    count: row.try_get::<i64, _>(1).unwrap_or(0) as u64,
                    success: row.try_get::<i64, _>(2).unwrap_or(0) as u64,
                    error: row.try_get::<i64, _>(3).unwrap_or(0) as u64,
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    pub async fn token_histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<TokenBucket> {
        let bucket_ms = bucket_secs * 1000;
        let (where_clause, params) = build_where_clause(filter);

        let sql = format!(
            r#"
            SELECT
                (timestamp / {bucket_ms}) * {bucket_ms}  AS bucket_ts,
                SUM(prompt_tokens)                        AS prompt_tokens,
                SUM(completion_tokens)                    AS completion_tokens,
                SUM(total_tokens)                         AS total_tokens
            FROM requests {where_clause}
            GROUP BY bucket_ts
            ORDER BY bucket_ts ASC
            "#,
            bucket_ms = bucket_ms,
            where_clause = where_clause
        );

        let mut q = sqlx::query(&sql);
        for p in &params {
            q = q.bind(p);
        }

        match q.fetch_all(&self.pool).await {
            Ok(rows) => rows
                .iter()
                .map(|row| TokenBucket {
                    timestamp: row.try_get::<i64, _>(0).unwrap_or(0),
                    prompt_tokens: row.try_get::<i64, _>(1).unwrap_or(0),
                    completion_tokens: row.try_get::<i64, _>(2).unwrap_or(0),
                    total_tokens: row.try_get::<i64, _>(3).unwrap_or(0),
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    pub async fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        let (where_clause, params) = build_where_clause(filter);
        let guard_where = if where_clause.is_empty() {
            "WHERE guardrail_triggered = TRUE".to_string()
        } else {
            format!("{} AND guardrail_triggered = TRUE", where_clause)
        };

        let count_sql = format!("SELECT COUNT(*) FROM requests {}", guard_where);
        let mut count_q = sqlx::query(&count_sql);
        for p in &params {
            count_q = count_q.bind(p);
        }
        let total: u64 = count_q
            .fetch_one(&self.pool)
            .await
            .map(|r| r.try_get::<i64, _>(0).unwrap_or(0) as u64)
            .unwrap_or(0);

        let limit_idx = params.len() + 1;
        let offset_idx = params.len() + 2;
        let data_sql = format!(
            "SELECT * FROM requests {} ORDER BY timestamp DESC LIMIT ${} OFFSET ${}",
            guard_where, limit_idx, offset_idx
        );
        let mut data_q = sqlx::query(&data_sql);
        for p in &params {
            data_q = data_q.bind(p);
        }
        data_q = data_q.bind(limit as i64).bind(offset as i64);
        let rows = data_q
            .fetch_all(&self.pool)
            .await
            .unwrap_or_default()
            .iter()
            .map(row_to_log_entry)
            .collect();

        (rows, total)
    }

    pub async fn guardrails_stats(
        &self,
        filter: &LogFilter,
    ) -> crate::log_store::GuardrailsBreakdown {
        let (where_clause, params) = build_where_clause(filter);
        let guard_where = if where_clause.is_empty() {
            "WHERE guardrail_triggered = TRUE".to_string()
        } else {
            format!("{} AND guardrail_triggered = TRUE", where_clause)
        };

        let total_sql = format!(
            "SELECT COUNT(*) FROM requests {} AND guardrail_type IS NOT NULL",
            guard_where
        );
        let mut q = sqlx::query(&total_sql);
        for p in &params {
            q = q.bind(p);
        }
        let total_blocks: u64 = q
            .fetch_one(&self.pool)
            .await
            .map(|r| r.try_get::<i64, _>(0).unwrap_or(0) as u64)
            .unwrap_or(0);

        crate::log_store::GuardrailsBreakdown {
            total_blocks,
            keyword_blocks: 0,
            prompt_injection_blocks: 0,
            content_filter_blocks: 0,
            top_keywords: vec![],
        }
    }

    pub async fn guardrails_breakdown(
        &self,
        filter: &LogFilter,
    ) -> crate::log_store::GuardrailsBreakdown {
        self.guardrails_stats(filter).await
    }

    pub async fn guardrails_timeline(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> Vec<crate::log_store::GuardrailsTimeline> {
        let bucket_ms = bucket_secs * 1000;
        let (where_clause, params) = build_where_clause(filter);
        let guard_where = if where_clause.is_empty() {
            "WHERE guardrail_triggered = TRUE".to_string()
        } else {
            format!("{} AND guardrail_triggered = TRUE", where_clause)
        };

        let sql = format!(
            "SELECT (timestamp / {b}) * {b} AS ts,
                    COUNT(*) as total,
                    COALESCE(SUM(CASE WHEN guardrail_type = 'keyword_block' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN guardrail_type = 'prompt_injection' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN guardrail_type = 'content_filter' THEN 1 ELSE 0 END), 0)
             FROM requests {guard_where}
             GROUP BY ts ORDER BY ts ASC",
            b = bucket_ms
        );

        let mut q = sqlx::query(&sql);
        for p in &params {
            q = q.bind(p);
        }
        match q.fetch_all(&self.pool).await {
            Ok(rows) => rows
                .iter()
                .map(|row| crate::log_store::GuardrailsTimeline {
                    timestamp: row.try_get::<i64, _>(0).unwrap_or(0),
                    total: row.try_get::<i64, _>(1).unwrap_or(0) as u64,
                    keyword_blocks: row.try_get::<i64, _>(2).unwrap_or(0) as u64,
                    prompt_injection: row.try_get::<i64, _>(3).unwrap_or(0) as u64,
                    content_filter: row.try_get::<i64, _>(4).unwrap_or(0) as u64,
                })
                .collect(),
            Err(_) => vec![],
        }
    }

    async fn purge_old_logs(&self) {
        let cutoff = now_ms() - (self.retention_days as i64 * 86_400_000);
        match sqlx::query("DELETE FROM requests WHERE timestamp < $1")
            .bind(cutoff)
            .execute(&self.pool)
            .await
        {
            Ok(result) => {
                let deleted = result.rows_affected();
                if deleted > 0 {
                    tracing::info!(
                        deleted = deleted,
                        retention_days = self.retention_days,
                        "Purged old log entries"
                    );
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to purge old log entries");
            }
        }
    }
}

fn build_where_clause(filter: &LogFilter) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = vec![];
    let mut params: Vec<String> = vec![];

    if let Some(ref p) = filter.provider {
        conditions.push(format!("provider = ${}", params.len() + 1));
        params.push(p.clone());
    }
    if let Some(ref m) = filter.model {
        conditions.push(format!("model LIKE ${}", params.len() + 1));
        params.push(format!("%{}%", m));
    }
    if let Some(ref s) = filter.status {
        let s_str = match s {
            LogStatus::Success => "success",
            LogStatus::Error => "error",
        };
        conditions.push(format!("status = ${}", params.len() + 1));
        params.push(s_str.to_string());
    }
    if let Some(since) = filter.since_ms {
        conditions.push(format!("timestamp >= ${}::bigint", params.len() + 1));
        params.push(since.to_string());
    }
    if let Some(until) = filter.until_ms {
        conditions.push(format!("timestamp <= ${}::bigint", params.len() + 1));
        params.push(until.to_string());
    }
    if let Some(ref vk) = filter.virtual_key {
        conditions.push(format!("virtual_key = ${}", params.len() + 1));
        params.push(vk.clone());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, params)
}

fn row_to_log_entry(row: &sqlx::postgres::PgRow) -> LogEntry {
    let status_str: String = row.try_get("status").unwrap_or_default();
    let status = if status_str == "success" {
        LogStatus::Success
    } else {
        LogStatus::Error
    };
    let is_stream_int: i16 = row.try_get("is_stream").unwrap_or(0);

    LogEntry {
        id: row.try_get("id").unwrap_or_default(),
        timestamp: row.try_get("timestamp").unwrap_or(0),
        provider: row.try_get("provider").unwrap_or_default(),
        model: row.try_get("model").unwrap_or_default(),
        object: row.try_get("object").unwrap_or_default(),
        status,
        latency_ms: row.try_get("latency_ms").unwrap_or(0.0),
        prompt_tokens: row.try_get("prompt_tokens").unwrap_or(0),
        completion_tokens: row.try_get("completion_tokens").unwrap_or(0),
        total_tokens: row.try_get("total_tokens").unwrap_or(0),
        cost_usd: row.try_get("cost_usd").unwrap_or(0.0),
        finish_reason: row.try_get("finish_reason").ok(),
        error_message: row.try_get("error_message").ok(),
        virtual_key: row.try_get("virtual_key").ok(),
        is_stream: is_stream_int != 0,
        input_preview: row.try_get("input_preview").unwrap_or_default(),
        output_preview: row.try_get("output_preview").unwrap_or_default(),
        compression_saved_bytes: row
            .try_get::<i32, _>("compression_saved_bytes")
            .unwrap_or(0) as usize,
        guardrail_triggered: row.try_get::<bool, _>("guardrail_triggered").ok(),
        guardrail_type: row.try_get("guardrail_type").ok(),
        guardrail_detail: row.try_get("guardrail_detail").ok(),
        obfuscated_input: None,
        pii_mapping: None,
    }
}
