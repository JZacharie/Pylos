use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// ─────────────────────────────────────────────────────────────────────────────
// Types publics
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: i64,
    pub provider: String,
    pub model: String,
    pub object: String,
    pub status: LogStatus,
    pub latency_ms: f64,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub cost_usd: f64,
    pub finish_reason: Option<String>,
    pub error_message: Option<String>,
    pub virtual_key: Option<String>,
    pub is_stream: bool,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub compression_saved_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogStatus {
    Success,
    Error,
}

impl LogStatus {
    fn as_str(&self) -> &'static str {
        match self {
            LogStatus::Success => "success",
            LogStatus::Error => "error",
        }
    }
    fn from_str(s: &str) -> Self {
        if s == "success" {
            LogStatus::Success
        } else {
            LogStatus::Error
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogStats {
    pub total_requests: u64,
    pub success_rate: f64,
    pub average_latency_ms: f64,
    pub total_tokens: i64,
    pub total_cost_usd: f64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_compression_saved_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub timestamp: i64,
    pub count: u64,
    pub success: u64,
    pub error: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBucket {
    pub timestamp: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<LogStatus>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub virtual_key: Option<String>,
    pub finish_reason: Option<String>,
}

impl LogFilter {
    pub fn bucket_size_secs(&self) -> i64 {
        let range_ms = match (self.since_ms, self.until_ms) {
            (Some(s), Some(u)) => u - s,
            (Some(s), None) => now_ms() - s,
            _ => 3_600_000,
        };
        match range_ms / 1000 {
            0..=7200 => 60,
            7201..=86400 => 600,
            86401..=259200 => 3600,
            _ => 86400,
        }
    }

    fn matches(&self, entry: &LogEntry) -> bool {
        if let Some(ref p) = self.provider {
            if &entry.provider != p {
                return false;
            }
        }
        if let Some(ref m) = self.model {
            if !entry.model.contains(m.as_str()) {
                return false;
            }
        }
        if let Some(ref s) = self.status {
            if &entry.status != s {
                return false;
            }
        }
        if let Some(since) = self.since_ms {
            if entry.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until_ms {
            if entry.timestamp > until {
                return false;
            }
        }
        if let Some(ref vk) = self.virtual_key {
            match &entry.virtual_key {
                Some(k) if k == vk => {}
                _ => return false,
            }
        }
        if let Some(ref fr) = self.finish_reason {
            match &entry.finish_reason {
                Some(f) if f == fr => {}
                _ => return false,
            }
        }
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Backend SQLite (synchrone — exécuté dans spawn_blocking)
// ─────────────────────────────────────────────────────────────────────────────

struct SqliteBackend {
    conn: Connection,
    retention_days: u32,
}

impl SqliteBackend {
    fn open(path: &PathBuf, retention_days: u32) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;
        let b = Self {
            conn,
            retention_days,
        };
        b.migrate()?;
        // Tentative d'ajout de la colonne si elle n'existe pas (migration à chaud)
        let _ = b.conn.execute(
            "ALTER TABLE logs ADD COLUMN compression_saved_bytes INTEGER NOT NULL DEFAULT 0",
            [],
        );
        Ok(b)
    }

    fn migrate(&self) -> rusqlite::Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS logs (
                id                TEXT    PRIMARY KEY,
                timestamp         INTEGER NOT NULL,
                provider          TEXT    NOT NULL DEFAULT '',
                model             TEXT    NOT NULL DEFAULT '',
                object            TEXT    NOT NULL DEFAULT '',
                status            TEXT    NOT NULL DEFAULT 'success',
                latency_ms        REAL    NOT NULL DEFAULT 0,
                prompt_tokens     INTEGER NOT NULL DEFAULT 0,
                completion_tokens INTEGER NOT NULL DEFAULT 0,
                total_tokens      INTEGER NOT NULL DEFAULT 0,
                cost_usd          REAL    NOT NULL DEFAULT 0,
                finish_reason     TEXT,
                error_message     TEXT,
                virtual_key       TEXT,
                is_stream         INTEGER NOT NULL DEFAULT 0,
                input_preview     TEXT,
                output_preview    TEXT,
                compression_saved_bytes INTEGER NOT NULL DEFAULT 0
            );
            CREATE INDEX IF NOT EXISTS idx_ts ON logs(timestamp DESC);
            CREATE INDEX IF NOT EXISTS idx_provider ON logs(provider);
            CREATE INDEX IF NOT EXISTS idx_status ON logs(status);",
        )
    }

    fn insert(&self, e: &LogEntry) -> rusqlite::Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO logs VALUES
             (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18)",
            params![
                e.id,
                e.timestamp,
                e.provider,
                e.model,
                e.object,
                e.status.as_str(),
                e.latency_ms,
                e.prompt_tokens,
                e.completion_tokens,
                e.total_tokens,
                e.cost_usd,
                e.finish_reason,
                e.error_message,
                e.virtual_key,
                e.is_stream as i32,
                e.input_preview,
                e.output_preview,
                e.compression_saved_bytes as i32
            ],
        )?;
        Ok(())
    }

    fn prune_old(&self) -> rusqlite::Result<()> {
        let cutoff = now_ms() - (self.retention_days as i64 * 86_400_000);
        self.conn
            .execute("DELETE FROM logs WHERE timestamp < ?1", params![cutoff])?;
        Ok(())
    }

    fn query(
        &self,
        filter: &LogFilter,
        limit: usize,
        offset: usize,
    ) -> rusqlite::Result<(Vec<LogEntry>, u64)> {
        // Build dynamic WHERE from filter
        let mut conditions = Vec::<String>::new();

        if filter.provider.is_some() {
            conditions.push("provider = ?1".into());
        }
        if filter.model.is_some() {
            conditions.push("model LIKE ?2".into());
        }
        if filter.status.is_some() {
            conditions.push("status = ?3".into());
        }
        if filter.since_ms.is_some() {
            conditions.push("timestamp >= ?4".into());
        }
        if filter.until_ms.is_some() {
            conditions.push("timestamp <= ?5".into());
        }
        if filter.virtual_key.is_some() {
            conditions.push("virtual_key = ?6".into());
        }
        if filter.finish_reason.is_some() {
            conditions.push("finish_reason = ?7".into());
        }

        let where_str = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        // Named params via a fixed-position approach: always pass all 6, using NULL for absent
        let p1: Option<String> = filter.provider.clone();
        let p2: Option<String> = filter.model.as_ref().map(|m| format!("%{m}%"));
        let p3: Option<String> = filter.status.as_ref().map(|s| s.as_str().to_string());
        let p4: Option<i64> = filter.since_ms;
        let p5: Option<i64> = filter.until_ms;
        let p6: Option<String> = filter.virtual_key.clone();
        let p7: Option<String> = filter.finish_reason.clone();

        let count_sql = format!("SELECT COUNT(*) FROM logs {where_str}");
        let total: u64 = self
            .conn
            .query_row(&count_sql, params![p1, p2, p3, p4, p5, p6, p7], |r| r.get(0))?;

        let rows_sql = format!(
            "SELECT id,timestamp,provider,model,object,status,latency_ms,
                    prompt_tokens,completion_tokens,total_tokens,cost_usd,
                    finish_reason,error_message,virtual_key,is_stream,
                    input_preview,output_preview,compression_saved_bytes
             FROM logs {where_str}
             ORDER BY timestamp DESC LIMIT ?8 OFFSET ?9"
        );

        let mut stmt = self.conn.prepare(&rows_sql)?;
        let entries = stmt
            .query_map(
                params![p1, p2, p3, p4, p5, p6, p7, limit as i64, offset as i64],
                row_to_entry,
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok((entries, total))
    }

    fn stats(&self, filter: &LogFilter) -> rusqlite::Result<LogStats> {
        let mut conditions = Vec::<String>::new();
        if filter.provider.is_some() {
            conditions.push("provider = ?1".into());
        }
        if filter.model.is_some() {
            conditions.push("model LIKE ?2".into());
        }
        if filter.status.is_some() {
            conditions.push("status = ?3".into());
        }
        if filter.since_ms.is_some() {
            conditions.push("timestamp >= ?4".into());
        }
        if filter.until_ms.is_some() {
            conditions.push("timestamp <= ?5".into());
        }
        if filter.virtual_key.is_some() {
            conditions.push("virtual_key = ?6".into());
        }
        if filter.finish_reason.is_some() {
            conditions.push("finish_reason = ?7".into());
        }

        let where_str = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let p1: Option<String> = filter.provider.clone();
        let p2: Option<String> = filter.model.as_ref().map(|m| format!("%{m}%"));
        let p3: Option<String> = filter.status.as_ref().map(|s| s.as_str().to_string());
        let p4: Option<i64> = filter.since_ms;
        let p5: Option<i64> = filter.until_ms;
        let p6: Option<String> = filter.virtual_key.clone();
        let p7: Option<String> = filter.finish_reason.clone();

        let sql = format!(
            "SELECT COUNT(*),
                    SUM(CASE WHEN status='success' THEN 1 ELSE 0 END),
                    AVG(latency_ms),
                    SUM(CAST(total_tokens AS INTEGER)),
                    SUM(cost_usd),
                    SUM(CAST(prompt_tokens AS INTEGER)),
                    SUM(CAST(completion_tokens AS INTEGER)),
                    SUM(CAST(compression_saved_bytes AS INTEGER))
             FROM logs {where_str}"
        );

        self.conn
            .query_row(&sql, params![p1, p2, p3, p4, p5, p6, p7], |r| {
                let total: u64 = r.get(0)?;
                let success: u64 = r.get::<_, Option<i64>>(1)?.unwrap_or(0) as u64;
                Ok(LogStats {
                    total_requests: total,
                    success_rate: if total > 0 {
                        success as f64 / total as f64 * 100.0
                    } else {
                        0.0
                    },
                    average_latency_ms: r.get::<_, Option<f64>>(2)?.unwrap_or(0.0),
                    total_tokens: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    total_cost_usd: r.get::<_, Option<f64>>(4)?.unwrap_or(0.0),
                    total_prompt_tokens: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                    total_completion_tokens: r.get::<_, Option<i64>>(6)?.unwrap_or(0),
                    total_compression_saved_bytes: r.get::<_, Option<i64>>(7)?.unwrap_or(0),
                })
            })
    }

    fn histogram(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> rusqlite::Result<Vec<HistogramBucket>> {
        let bucket_ms = bucket_secs * 1000;
        let mut conditions = Vec::<String>::new();
        if filter.provider.is_some() {
            conditions.push("provider = ?1".into());
        }
        if filter.model.is_some() {
            conditions.push("model LIKE ?2".into());
        }
        if filter.status.is_some() {
            conditions.push("status = ?3".into());
        }
        if filter.since_ms.is_some() {
            conditions.push("timestamp >= ?4".into());
        }
        if filter.until_ms.is_some() {
            conditions.push("timestamp <= ?5".into());
        }
        if filter.virtual_key.is_some() {
            conditions.push("virtual_key = ?6".into());
        }
        if filter.finish_reason.is_some() {
            conditions.push("finish_reason = ?7".into());
        }
        let where_str = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let p1: Option<String> = filter.provider.clone();
        let p2: Option<String> = filter.model.as_ref().map(|m| format!("%{m}%"));
        let p3: Option<String> = filter.status.as_ref().map(|s| s.as_str().to_string());
        let p4: Option<i64> = filter.since_ms;
        let p5: Option<i64> = filter.until_ms;
        let p6: Option<String> = filter.virtual_key.clone();
        let p7: Option<String> = filter.finish_reason.clone();

        let sql = format!(
            "SELECT (timestamp/{b})*{b} AS ts,
                    COUNT(*),
                    SUM(CASE WHEN status='success' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN status='error' THEN 1 ELSE 0 END)
             FROM logs {where_str}
             GROUP BY ts ORDER BY ts ASC",
            b = bucket_ms
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![p1, p2, p3, p4, p5, p6, p7], |r| {
                Ok(HistogramBucket {
                    timestamp: r.get(0)?,
                    count: r.get::<_, i64>(1)? as u64,
                    success: r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                    error: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn token_histogram(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> rusqlite::Result<Vec<TokenBucket>> {
        let bucket_ms = bucket_secs * 1000;
        let mut conditions = Vec::<String>::new();
        if filter.provider.is_some() {
            conditions.push("provider = ?1".into());
        }
        if filter.model.is_some() {
            conditions.push("model LIKE ?2".into());
        }
        if filter.status.is_some() {
            conditions.push("status = ?3".into());
        }
        if filter.since_ms.is_some() {
            conditions.push("timestamp >= ?4".into());
        }
        if filter.until_ms.is_some() {
            conditions.push("timestamp <= ?5".into());
        }
        if filter.virtual_key.is_some() {
            conditions.push("virtual_key = ?6".into());
        }
        if filter.finish_reason.is_some() {
            conditions.push("finish_reason = ?7".into());
        }
        let where_str = if conditions.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", conditions.join(" AND "))
        };

        let p1: Option<String> = filter.provider.clone();
        let p2: Option<String> = filter.model.as_ref().map(|m| format!("%{m}%"));
        let p3: Option<String> = filter.status.as_ref().map(|s| s.as_str().to_string());
        let p4: Option<i64> = filter.since_ms;
        let p5: Option<i64> = filter.until_ms;
        let p6: Option<String> = filter.virtual_key.clone();
        let p7: Option<String> = filter.finish_reason.clone();

        let sql = format!(
            "SELECT (timestamp/{b})*{b} AS ts,
                    SUM(CAST(prompt_tokens AS INTEGER)),
                    SUM(CAST(completion_tokens AS INTEGER)),
                    SUM(CAST(total_tokens AS INTEGER))
             FROM logs {where_str}
             GROUP BY ts ORDER BY ts ASC",
            b = bucket_ms
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt
            .query_map(params![p1, p2, p3, p4, p5, p6, p7], |r| {
                Ok(TokenBucket {
                    timestamp: r.get(0)?,
                    prompt_tokens: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                    completion_tokens: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                    total_tokens: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }
}

fn row_to_entry(r: &rusqlite::Row) -> rusqlite::Result<LogEntry> {
    Ok(LogEntry {
        id: r.get(0)?,
        timestamp: r.get(1)?,
        provider: r.get(2)?,
        model: r.get(3)?,
        object: r.get(4)?,
        status: LogStatus::from_str(&r.get::<_, String>(5)?),
        latency_ms: r.get(6)?,
        prompt_tokens: r.get(7)?,
        completion_tokens: r.get(8)?,
        total_tokens: r.get(9)?,
        cost_usd: r.get(10)?,
        finish_reason: r.get(11)?,
        error_message: r.get(12)?,
        virtual_key: r.get(13)?,
        is_stream: r.get::<_, i32>(14)? != 0,
        input_preview: r.get(15)?,
        output_preview: r.get(16)?,
        compression_saved_bytes: r.get::<_, Option<i32>>(17)?.unwrap_or(0) as usize,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// LogStore public — SQLite (primaire) + ring buffer (fallback mémoire)
// ─────────────────────────────────────────────────────────────────────────────

enum Backend {
    Sqlite(SqliteBackend),
    Memory { buf: VecDeque<LogEntry>, max: usize },
}

pub struct LogStore {
    inner: Arc<Mutex<Backend>>,
}

impl LogStore {
    pub fn new(db_path: Option<PathBuf>, retention_days: u32, mem_size: usize) -> Self {
        let backend = match db_path {
            Some(path) => match SqliteBackend::open(&path, retention_days) {
                Ok(db) => {
                    tracing::info!(path = %path.display(), "Log store: SQLite");
                    Backend::Sqlite(db)
                }
                Err(e) => {
                    tracing::warn!(error = %e, "SQLite failed, using memory ring buffer");
                    Backend::Memory {
                        buf: VecDeque::with_capacity(mem_size),
                        max: mem_size,
                    }
                }
            },
            None => {
                tracing::info!(size = mem_size, "Log store: in-memory ring buffer");
                Backend::Memory {
                    buf: VecDeque::with_capacity(mem_size),
                    max: mem_size,
                }
            }
        };
        Self {
            inner: Arc::new(Mutex::new(backend)),
        }
    }

    pub async fn push(&self, entry: LogEntry) {
        // Determine backend type without holding the lock
        let is_sqlite = {
            let g = self.inner.lock().await;
            matches!(&*g, Backend::Sqlite(_))
        };

        if is_sqlite {
            let entry = entry.clone();
            let inner = self.inner.clone();
            tokio::task::spawn_blocking(move || {
                let mut g = inner.blocking_lock();
                if let Backend::Sqlite(db) = &mut *g {
                    if let Err(e) = db.insert(&entry) {
                        tracing::warn!(error = %e, "Log insert failed");
                    }
                    if fastrand::u8(..) == 0 {
                        let _ = db.prune_old();
                    }
                }
            })
            .await
            .ok();
        } else {
            let mut g = self.inner.lock().await;
            if let Backend::Memory { buf, max } = &mut *g {
                if buf.len() >= *max {
                    buf.pop_front();
                }
                buf.push_back(entry);
            }
        }
    }

    pub async fn list(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        let g = self.inner.lock().await;
        match &*g {
            Backend::Sqlite(db) => db.query(filter, limit, offset).unwrap_or_default(),
            Backend::Memory { buf, .. } => {
                let filtered: Vec<&LogEntry> =
                    buf.iter().rev().filter(|e| filter.matches(e)).collect();
                let total = filtered.len() as u64;
                let page = filtered
                    .into_iter()
                    .skip(offset)
                    .take(limit)
                    .cloned()
                    .collect();
                (page, total)
            }
        }
    }

    pub async fn stats(&self, filter: &LogFilter) -> LogStats {
        let g = self.inner.lock().await;
        match &*g {
            Backend::Sqlite(db) => db.stats(filter).unwrap_or_default(),
            Backend::Memory { buf, .. } => {
                let entries: Vec<&LogEntry> = buf.iter().filter(|e| filter.matches(e)).collect();
                memory_stats(&entries)
            }
        }
    }

    pub async fn histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<HistogramBucket> {
        let g = self.inner.lock().await;
        match &*g {
            Backend::Sqlite(db) => db.histogram(filter, bucket_secs).unwrap_or_default(),
            Backend::Memory { buf, .. } => {
                let entries: Vec<&LogEntry> = buf.iter().filter(|e| filter.matches(e)).collect();
                memory_histogram(&entries, bucket_secs)
            }
        }
    }

    pub async fn token_histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<TokenBucket> {
        let g = self.inner.lock().await;
        match &*g {
            Backend::Sqlite(db) => db.token_histogram(filter, bucket_secs).unwrap_or_default(),
            Backend::Memory { buf, .. } => {
                let entries: Vec<&LogEntry> = buf.iter().filter(|e| filter.matches(e)).collect();
                memory_token_histogram(&entries, bucket_secs)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Calculs in-memory (fallback)
// ─────────────────────────────────────────────────────────────────────────────

fn memory_stats(entries: &[&LogEntry]) -> LogStats {
    let total = entries.len() as u64;
    if total == 0 {
        return LogStats::default();
    }
    let success = entries
        .iter()
        .filter(|e| e.status == LogStatus::Success)
        .count() as u64;
    LogStats {
        total_requests: total,
        success_rate: success as f64 / total as f64 * 100.0,
        average_latency_ms: entries.iter().map(|e| e.latency_ms).sum::<f64>() / total as f64,
        total_tokens: entries.iter().map(|e| e.total_tokens as i64).sum(),
        total_cost_usd: entries.iter().map(|e| e.cost_usd).sum(),
        total_prompt_tokens: entries.iter().map(|e| e.prompt_tokens as i64).sum(),
        total_completion_tokens: entries.iter().map(|e| e.completion_tokens as i64).sum(),
        total_compression_saved_bytes: entries
            .iter()
            .map(|e| e.compression_saved_bytes as i64)
            .sum(),
    }
}

fn memory_histogram(entries: &[&LogEntry], bucket_secs: i64) -> Vec<HistogramBucket> {
    let mut map: std::collections::BTreeMap<i64, HistogramBucket> = Default::default();
    let bms = bucket_secs * 1000;
    for e in entries {
        let ts = (e.timestamp / bms) * bms;
        let b = map.entry(ts).or_insert(HistogramBucket {
            timestamp: ts,
            count: 0,
            success: 0,
            error: 0,
        });
        b.count += 1;
        match e.status {
            LogStatus::Success => b.success += 1,
            LogStatus::Error => b.error += 1,
        }
    }
    map.into_values().collect()
}

fn memory_token_histogram(entries: &[&LogEntry], bucket_secs: i64) -> Vec<TokenBucket> {
    let mut map: std::collections::BTreeMap<i64, TokenBucket> = Default::default();
    let bms = bucket_secs * 1000;
    for e in entries {
        let ts = (e.timestamp / bms) * bms;
        let b = map.entry(ts).or_insert(TokenBucket {
            timestamp: ts,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });
        b.prompt_tokens += e.prompt_tokens as i64;
        b.completion_tokens += e.completion_tokens as i64;
        b.total_tokens += e.total_tokens as i64;
    }
    map.into_values().collect()
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers publics
// ─────────────────────────────────────────────────────────────────────────────

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn generate_log_id() -> String {
    format!("log_{}", fastrand::u64(..))
}

#[allow(clippy::too_many_arguments)]
pub fn build_log_entry(
    provider: &str,
    model: &str,
    is_stream: bool,
    status: LogStatus,
    latency_ms: f64,
    usage: Option<&pylos_core::domain::openai::Usage>,
    finish_reason: Option<String>,
    error_message: Option<String>,
    input_preview: Option<String>,
    output_preview: Option<String>,
    virtual_key: Option<String>,
    compression_saved_bytes: usize,
) -> LogEntry {
    let (prompt_tokens, completion_tokens, total_tokens) = usage
        .map(|u| (u.prompt_tokens, u.completion_tokens, u.total_tokens))
        .unwrap_or((0, 0, 0));

    LogEntry {
        id: generate_log_id(),
        timestamp: now_ms(),
        provider: provider.to_string(),
        model: model.to_string(),
        object: if is_stream {
            "chat.completion.stream".into()
        } else {
            "chat.completion".into()
        },
        status,
        latency_ms,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cost_usd: estimate_cost(provider, model, prompt_tokens, completion_tokens),
        finish_reason,
        error_message,
        virtual_key,
        is_stream,
        input_preview: input_preview.map(|s| truncate(&s, 200)),
        output_preview: output_preview.map(|s| truncate(&s, 200)),
        compression_saved_bytes,
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

pub(crate) fn estimate_cost_pub(provider: &str, model: &str, prompt: i32, completion: i32) -> f64 {
    estimate_cost(provider, model, prompt, completion)
}

fn estimate_cost(provider: &str, model: &str, prompt: i32, completion: i32) -> f64 {
    let (in_m, out_m): (f64, f64) = match provider {
        "ollama-jo3" => (0.0, 0.0),
        "openai" | "openrouter" => {
            if model.contains("gpt-4o-mini") {
                (0.15, 0.60)
            } else if model.contains("gpt-4o") {
                (5.0, 15.0)
            } else if model.contains("o4-mini") || model.contains("o3-mini") {
                (1.1, 4.4)
            } else if model.contains("o3") {
                (10.0, 40.0)
            } else if model.contains("o1-mini") {
                (3.0, 12.0)
            } else if model.contains("o1") {
                (15.0, 60.0)
            } else if model.starts_with("text-embedding") {
                (0.1, 0.0)
            } else {
                (1.0, 3.0)
            }
        }
        "anthropic" => {
            if model.contains("haiku-3-5") || model.contains("haiku-4") {
                (0.8, 4.0)
            } else if model.contains("haiku") {
                (0.25, 1.25)
            } else if model.contains("sonnet-4")
                || model.contains("sonnet-3-7")
                || model.contains("sonnet")
            {
                (3.0, 15.0)
            } else {
                // opus and unknown
                (15.0, 75.0)
            }
        }
        "gemini" => {
            if model.contains("2.5-pro") {
                (7.0, 21.0)
            } else if model.contains("2.5-flash") {
                (0.3, 2.5)
            } else if model.contains("2.0-flash") {
                (0.1, 0.4)
            } else if model.contains("1.5-pro") {
                (3.5, 10.5)
            } else if model.contains("1.5-flash") {
                (0.075, 0.3)
            } else if model.contains("embedding") {
                (0.025, 0.0)
            } else {
                (0.5, 1.5)
            }
        }
        "cohere" => {
            if model.contains("command-a") {
                (2.5, 10.0)
            } else if model.contains("command-r-plus") {
                (3.0, 15.0)
            } else if model.contains("command-r") {
                (0.15, 0.60)
            } else if model.contains("embed") {
                (0.1, 0.0)
            } else {
                (1.0, 3.0)
            }
        }
        "groq" => {
            if model.contains("llama-3.3-70b") || model.contains("llama-3.1-70b") {
                (0.59, 0.79)
            } else if model.contains("llama-3.1-8b") {
                (0.05, 0.08)
            } else if model.contains("mixtral") {
                (0.24, 0.24)
            } else {
                (0.2, 0.2)
            }
        }
        "mistral" => {
            if model.contains("large") {
                (3.0, 9.0)
            } else if model.contains("codestral") {
                (0.3, 0.9)
            } else {
                (0.2, 0.6)
            }
        }
        "xai" => {
            if model.contains("grok-3-mini") {
                (0.3, 0.5)
            } else {
                (5.0, 15.0)
            }
        }
        "deepseek" => {
            if model.contains("reasoner") || model.contains("r1") {
                (0.55, 2.19)
            } else {
                (0.14, 0.28)
            }
        }
        "bedrock" => {
            if model.contains("nova-lite") {
                (0.06, 0.24)
            } else if model.contains("nova-pro") {
                (0.80, 3.20)
            } else if model.contains("nova-micro") {
                (0.035, 0.14)
            } else if model.contains("haiku") {
                (0.25, 1.25)
            } else if model.contains("sonnet") {
                (3.0, 15.0)
            } else if model.contains("claude") {
                (15.0, 75.0)
            } else {
                (0.50, 1.50)
            }
        }
        _ => (1.0, 3.0),
    };
    let cost = (prompt as f64 / 1_000_000.0) * in_m + (completion as f64 / 1_000_000.0) * out_m;
    (cost * 1_000_000.0).round() / 1_000_000.0
}
