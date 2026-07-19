use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tracing::info;

use super::{
    log_store::{HistogramBucket, TokenBucket},
    LogEntry, LogFilter, LogStats, LogStatus,
};

// ─────────────────────────────────────────────────────────────────────────────
// SqliteLogStore — persistance durable des logs via SQLite
// Compatible API avec le LogStore in-memory (même méthodes publiques)
// Bifrost source: framework/logstore/logstore.go
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct SqliteLogStore {
    pool: SqlitePool,
}

impl SqliteLogStore {
    /// Ouvre (ou crée) la base SQLite et applique les migrations
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal) // WAL pour meilleures perfs concurrentes
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(8)
            .connect_with(options)
            .await?;

        // Applique les migrations inline
        sqlx::migrate!("./migrations").run(&pool).await.map_err(|e| {
            sqlx::Error::Migrate(Box::new(e))
        })?;

        info!(path = %db_path.display(), "SQLite log store opened");
        Ok(Self { pool })
    }

    /// Pour les tests : base en mémoire
    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect("sqlite::memory:")
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS requests (
                id              TEXT    PRIMARY KEY,
                timestamp       INTEGER NOT NULL,
                provider        TEXT    NOT NULL,
                model           TEXT    NOT NULL,
                object          TEXT    NOT NULL,
                status          TEXT    NOT NULL,
                latency_ms      REAL    NOT NULL,
                prompt_tokens   INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens    INTEGER NOT NULL DEFAULT 0,
                cost_usd        REAL    NOT NULL DEFAULT 0.0,
                finish_reason   TEXT,
                error_message   TEXT,
                virtual_key     TEXT,
                is_stream       INTEGER NOT NULL DEFAULT 0,
                input_preview   TEXT,
                output_preview  TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_requests_timestamp   ON requests(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_requests_provider    ON requests(provider);
            CREATE INDEX IF NOT EXISTS idx_requests_model       ON requests(model);
            CREATE INDEX IF NOT EXISTS idx_requests_status      ON requests(status);
            CREATE INDEX IF NOT EXISTS idx_requests_virtual_key ON requests(virtual_key);
            "#,
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    /// Insère une entrée de log
    pub async fn push(&self, entry: LogEntry) {
        let status_str = match entry.status {
            LogStatus::Success => "success",
            LogStatus::Error => "error",
        };
        let is_stream_int = if entry.is_stream { 1i64 } else { 0i64 };

        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO requests
                (id, timestamp, provider, model, object, status, latency_ms,
                 prompt_tokens, completion_tokens, total_tokens, cost_usd,
                 finish_reason, error_message, virtual_key, is_stream,
                 input_preview, output_preview)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
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
        .bind(is_stream_int)
        .bind(&entry.input_preview)
        .bind(&entry.output_preview)
        .execute(&self.pool)
        .await;

        if let Err(e) = result {
            tracing::error!(error = %e, "Failed to persist log entry to SQLite");
        }
    }

    /// Liste les logs avec filtres et pagination (les plus récents en premier)
    pub async fn list(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        let (where_clause, params) = build_where_clause(filter);

        // Compte total
        let count_sql = format!(
            "SELECT COUNT(*) FROM requests {}",
            where_clause
        );
        let total: u64 = self
            .query_count(&count_sql, &params)
            .await
            .unwrap_or(0);

        // Données paginées
        let data_sql = format!(
            "SELECT * FROM requests {} ORDER BY timestamp DESC LIMIT ? OFFSET ?",
            where_clause
        );
        let rows = self.query_rows_with_offset(&data_sql, &params, limit, offset).await;

        (rows, total)
    }

    /// Stats agrégées sur la fenêtre filtrée
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
                SUM(cost_usd)               AS total_cost
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
                }
            }
            Err(_) => LogStats::default(),
        }
    }

    /// Histogramme de requêtes par bucket temporel
    pub async fn histogram(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> Vec<HistogramBucket> {
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

    /// Histogramme de tokens par bucket temporel
    pub async fn token_histogram(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> Vec<TokenBucket> {
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

    /// Supprime les logs plus anciens que `retention_days` jours (cleanup job)
    pub async fn purge_old_logs(&self, retention_days: u32) {
        let cutoff_ms = crate::log_store::now_ms()
            - (retention_days as i64 * 86_400 * 1_000);

        match sqlx::query("DELETE FROM requests WHERE timestamp < ?")
            .bind(cutoff_ms)
            .execute(&self.pool)
            .await
        {
            Ok(result) => {
                let deleted = result.rows_affected();
                if deleted > 0 {
                    info!(deleted = deleted, retention_days = retention_days, "Purged old log entries");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to purge old log entries");
            }
        }
    }

    /// Nombre total d'entrées
    pub async fn total_count(&self) -> usize {
        let row = sqlx::query("SELECT COUNT(*) FROM requests")
            .fetch_one(&self.pool)
            .await;
        row.map(|r| r.try_get::<i64, _>(0).unwrap_or(0) as usize)
            .unwrap_or(0)
    }

    // ── helpers internes ──────────────────────────────────────────────────────

    async fn query_count(&self, sql: &str, params: &[String]) -> Result<u64, sqlx::Error> {
        let mut q = sqlx::query(sql);
        for p in params {
            q = q.bind(p);
        }
        let row = q.fetch_one(&self.pool).await?;
        Ok(row.try_get::<i64, _>(0).unwrap_or(0) as u64)
    }

    async fn query_rows_with_offset(
        &self,
        sql: &str,
        params: &[String],
        limit: usize,
        offset: usize,
    ) -> Vec<LogEntry> {
        let mut q = sqlx::query(sql);
        for p in params {
            q = q.bind(p);
        }
        q = q.bind(limit as i64).bind(offset as i64);
        match q.fetch_all(&self.pool).await {
            Ok(rows) => rows.iter().map(row_to_log_entry).collect(),
            Err(e) => {
                tracing::error!(error = %e, "Failed to query log entries");
                vec![]
            }
        }
    }

    #[allow(dead_code)]
    async fn query_rows(&self, sql: &str, params: &[String]) -> Vec<LogEntry> {
        let mut q = sqlx::query(sql);
        for p in params {
            q = q.bind(p);
        }
        match q.fetch_all(&self.pool).await {
            Ok(rows) => rows.iter().map(row_to_log_entry).collect(),
            Err(e) => {
                tracing::error!(error = %e, "Failed to query log entries");
                vec![]
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers SQL
// ─────────────────────────────────────────────────────────────────────────────

/// Construit la clause WHERE et les paramètres depuis un LogFilter
/// Retourne (clause_sql, vec_of_string_params)
fn build_where_clause(filter: &LogFilter) -> (String, Vec<String>) {
    let mut conditions: Vec<String> = vec![];
    let mut params: Vec<String> = vec![];

    if let Some(ref p) = filter.provider {
        conditions.push("provider = ?".into());
        params.push(p.clone());
    }
    if let Some(ref m) = filter.model {
        conditions.push("model LIKE ?".into());
        params.push(format!("%{}%", m));
    }
    if let Some(ref s) = filter.status {
        let s_str = match s {
            LogStatus::Success => "success",
            LogStatus::Error => "error",
        };
        conditions.push("status = ?".into());
        params.push(s_str.to_string());
    }
    if let Some(since) = filter.since_ms {
        conditions.push("timestamp >= ?".into());
        params.push(since.to_string());
    }
    if let Some(until) = filter.until_ms {
        conditions.push("timestamp <= ?".into());
        params.push(until.to_string());
    }
    if let Some(ref vk) = filter.virtual_key {
        conditions.push("virtual_key = ?".into());
        params.push(vk.clone());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_clause, params)
}

/// Convertit une ligne SQLite en LogEntry
fn row_to_log_entry(row: &sqlx::sqlite::SqliteRow) -> LogEntry {
    let status_str: String = row.try_get("status").unwrap_or_default();
    let status = if status_str == "success" {
        LogStatus::Success
    } else {
        LogStatus::Error
    };
    let is_stream_int: i64 = row.try_get("is_stream").unwrap_or(0);

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
        input_preview: row.try_get("input_preview").ok(),
        output_preview: row.try_get("output_preview").ok(),
        compression_saved_bytes: 0,
        guardrail_triggered: None,
        guardrail_type: None,
        guardrail_detail: None,
        obfuscated_input: None,
        pii_mapping: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::log_store::{build_log_entry, generate_log_id, now_ms};

    async fn make_store() -> SqliteLogStore {
        SqliteLogStore::in_memory().await.expect("in-memory SQLite should work")
    }

    #[tokio::test]
    async fn test_push_and_list() {
        let store = make_store().await;

        let entry = build_log_entry(
            "openai", "gpt-4o", false, LogStatus::Success,
            123.0, None, Some("stop".into()), None,
            Some("hello".into()), Some("world".into()), None,
        );
        store.push(entry).await;

        let (entries, total) = store.list(10, 0, &LogFilter::default()).await;
        assert_eq!(total, 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].provider, "openai");
        assert_eq!(entries[0].model, "gpt-4o");
        assert_eq!(entries[0].status, LogStatus::Success);
    }

    #[tokio::test]
    async fn test_list_filter_by_provider() {
        let store = make_store().await;

        store.push(build_log_entry("openai", "gpt-4o", false, LogStatus::Success, 10.0, None, None, None, None, None, None)).await;
        store.push(build_log_entry("anthropic", "claude-3", false, LogStatus::Success, 20.0, None, None, None, None, None, None)).await;

        let filter = LogFilter { provider: Some("openai".into()), ..Default::default() };
        let (entries, total) = store.list(10, 0, &filter).await;
        assert_eq!(total, 1);
        assert_eq!(entries[0].provider, "openai");
    }

    #[tokio::test]
    async fn test_list_filter_by_virtual_key() {
        let store = make_store().await;

        let mut entry1 = build_log_entry("openai", "gpt-4o", false, LogStatus::Success, 10.0, None, None, None, None, None, None);
        entry1.virtual_key = Some("sk-pylos-abc".into());
        store.push(entry1).await;
        store.push(build_log_entry("openai", "gpt-4o", false, LogStatus::Success, 10.0, None, None, None, None, None, None)).await;

        let filter = LogFilter { virtual_key: Some("sk-pylos-abc".into()), ..Default::default() };
        let (entries, total) = store.list(10, 0, &filter).await;
        assert_eq!(total, 1);
        assert_eq!(entries[0].virtual_key, Some("sk-pylos-abc".into()));
    }

    #[tokio::test]
    async fn test_stats_aggregation() {
        let store = make_store().await;

        for _ in 0..3 {
            store.push(build_log_entry("openai", "gpt-4o", false, LogStatus::Success, 100.0, None, None, None, None, None, None)).await;
        }
        store.push(build_log_entry("openai", "gpt-4o", false, LogStatus::Error, 50.0, None, None, Some("err".into()), None, None, None)).await;

        let stats = store.stats(&LogFilter::default()).await;
        assert_eq!(stats.total_requests, 4);
        assert!((stats.success_rate - 75.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_purge_old_logs() {
        let store = make_store().await;

        // Insère une vieille entrée (il y a 400 jours)
        let old_ts = now_ms() - (400i64 * 86_400 * 1_000);
        let mut old_entry = build_log_entry("openai", "gpt-4o", false, LogStatus::Success, 10.0, None, None, None, None, None, None);
        old_entry.id = generate_log_id();
        old_entry.timestamp = old_ts;
        store.push(old_entry).await;

        // Insère une entrée récente
        store.push(build_log_entry("openai", "gpt-4o", false, LogStatus::Success, 10.0, None, None, None, None, None, None)).await;

        assert_eq!(store.total_count().await, 2);
        store.purge_old_logs(365).await;
        assert_eq!(store.total_count().await, 1, "Old entry should have been purged");
    }

    #[tokio::test]
    async fn test_logs_survive_beyond_10k() {
        let store = make_store().await;

        // Vérifie qu'il n'y a pas de ring buffer (contrairement au store in-memory)
        for i in 0..10_001usize {
            let mut e = build_log_entry("openai", "gpt-4o", false, LogStatus::Success, 10.0, None, None, None, None, None, None);
            e.id = format!("log_{}", i);
            store.push(e).await;
        }

        let count = store.total_count().await;
        assert_eq!(count, 10_001, "SQLite store should not drop entries like the ring buffer");
    }
}
