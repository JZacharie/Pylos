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
    pub guardrail_triggered: Option<bool>,
    pub guardrail_type: Option<String>,
    pub guardrail_detail: Option<String>,
    pub obfuscated_input: Option<String>,
    pub pii_mapping: Option<String>,
    pub moderation_scores: Option<String>,
    pub caveman_input: Option<String>,
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
        let _ = b.conn.execute(
            "ALTER TABLE logs ADD COLUMN guardrail_triggered INTEGER",
            [],
        );
        let _ = b
            .conn
            .execute("ALTER TABLE logs ADD COLUMN guardrail_type TEXT", []);
        let _ = b
            .conn
            .execute("ALTER TABLE logs ADD COLUMN guardrail_detail TEXT", []);
        let _ = b
            .conn
            .execute("ALTER TABLE logs ADD COLUMN obfuscated_input TEXT", []);
        let _ = b
            .conn
            .execute("ALTER TABLE logs ADD COLUMN pii_mapping TEXT", []);
        let _ = b
            .conn
            .execute("ALTER TABLE logs ADD COLUMN moderation_scores TEXT", []);
        let _ = b
            .conn
            .execute("ALTER TABLE logs ADD COLUMN caveman_input TEXT", []);
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
             (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25)",
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
                e.compression_saved_bytes as i32,
                e.guardrail_triggered,
                e.guardrail_type,
                e.guardrail_detail,
                e.obfuscated_input,
                e.pii_mapping,
                e.moderation_scores,
                e.caveman_input,
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
        let (where_str, params_vec) = build_where_clause(filter);
        let idx = params_vec.len();

        let count_sql = format!("SELECT COUNT(*) FROM logs {where_str}");
        let total: u64 = self
            .conn
            .query_row(
                &count_sql,
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| r.get(0),
            )?;

        let rows_sql = format!(
            "SELECT id,timestamp,provider,model,object,status,latency_ms,
                    prompt_tokens,completion_tokens,total_tokens,cost_usd,
                    finish_reason,error_message,virtual_key,is_stream,
                    input_preview,output_preview,compression_saved_bytes,
                    guardrail_triggered,guardrail_type,guardrail_detail,
                    obfuscated_input,pii_mapping,moderation_scores,
                    caveman_input
             FROM logs {where_str}
             ORDER BY timestamp DESC LIMIT ?{} OFFSET ?{}",
            idx + 1,
            idx + 2,
        );

        let mut all_params = params_vec;
        all_params.push(Box::new(limit as i64));
        all_params.push(Box::new(offset as i64));

        let mut stmt = self.conn.prepare(&rows_sql)?;
        let entries = stmt
            .query_map(
                rusqlite::params_from_iter(all_params.iter().map(|p| p.as_ref())),
                row_to_entry,
            )?
            .filter_map(|r| r.ok())
            .collect();

        Ok((entries, total))
    }

    fn stats(&self, filter: &LogFilter) -> rusqlite::Result<LogStats> {
        let (where_str, params_vec) = build_where_clause(filter);

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
            .query_row(
                &sql,
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| {
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
                },
            )
    }

    fn histogram(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> rusqlite::Result<Vec<HistogramBucket>> {
        let bucket_ms = bucket_secs * 1000;
        let (where_str, params_vec) = build_where_clause(filter);

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
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| {
                    Ok(HistogramBucket {
                        timestamp: r.get(0)?,
                        count: r.get::<_, i64>(1)? as u64,
                        success: r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                        error: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                    })
                },
            )?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }

    fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> rusqlite::Result<(Vec<LogEntry>, u64)> {
        let mut conditions = vec!["guardrail_triggered = 1".to_string()];
        let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
        let mut idx = 0;

        if let Some(ref provider) = filter.provider {
            idx += 1;
            conditions.push(format!("provider = ?{idx}"));
            params_vec.push(Box::new(provider.clone()));
        }
        if let Some(ref model) = filter.model {
            idx += 1;
            conditions.push(format!("model LIKE ?{idx}"));
            params_vec.push(Box::new(format!("%{model}%")));
        }
        if let Some(ref _status) = filter.status {
            idx += 1;
            conditions.push(format!("status = ?{idx}"));
            params_vec.push(Box::new(_status.as_str().to_string()));
        }
        if let Some(since) = filter.since_ms {
            idx += 1;
            conditions.push(format!("timestamp >= ?{idx}"));
            params_vec.push(Box::new(since));
        }
        if let Some(until) = filter.until_ms {
            idx += 1;
            conditions.push(format!("timestamp <= ?{idx}"));
            params_vec.push(Box::new(until));
        }
        if let Some(ref vk) = filter.virtual_key {
            idx += 1;
            conditions.push(format!("virtual_key = ?{idx}"));
            params_vec.push(Box::new(vk.clone()));
        }

        let where_clause = conditions.join(" AND ");
        let count_sql = format!("SELECT COUNT(*) FROM logs WHERE {where_clause}");
        let total: u64 = self
            .conn
            .query_row(
                &count_sql,
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| r.get(0),
            )
            .unwrap_or(0);

        let rows_sql = format!(
            "SELECT id,timestamp,provider,model,object,status,latency_ms,
                    prompt_tokens,completion_tokens,total_tokens,cost_usd,
                    finish_reason,error_message,virtual_key,is_stream,
                    input_preview,output_preview,compression_saved_bytes,
                    guardrail_triggered,guardrail_type,guardrail_detail,
                    obfuscated_input,pii_mapping
             FROM logs WHERE {where_clause}
             ORDER BY timestamp DESC LIMIT ?{} OFFSET ?{}",
            idx + 1,
            idx + 2,
        );

        let limit_i64 = limit as i64;
        let offset_i64 = offset as i64;
        let mut all_params: Vec<Box<dyn rusqlite::types::ToSql>> = params_vec;
        all_params.push(Box::new(limit_i64));
        all_params.push(Box::new(offset_i64));

        let mut stmt = self.conn.prepare(&rows_sql)?;
        let mapped = stmt.query_map(
            rusqlite::params_from_iter(all_params.iter().map(|p| p.as_ref())),
            row_to_entry,
        )?;
        let entries: Vec<LogEntry> = mapped.filter_map(|r| r.ok()).collect();

        Ok((entries, total))
    }

    fn guardrails_breakdown(&self, filter: &LogFilter) -> rusqlite::Result<GuardrailsBreakdown> {
        let (where_clause, params_vec) = build_guardrails_where(filter);

        let total_sql = format!(
            "SELECT COUNT(*) FROM logs WHERE {where_clause} AND guardrail_type IS NOT NULL"
        );
        let total_blocks: u64 = self
            .conn
            .query_row(
                &total_sql,
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| r.get(0),
            )
            .unwrap_or(0);

        let keyword_sql = format!(
            "SELECT COUNT(*) FROM logs WHERE {where_clause} AND guardrail_type = 'keyword_block'"
        );
        let keyword_blocks: u64 = self
            .conn
            .query_row(
                &keyword_sql,
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| r.get(0),
            )
            .unwrap_or(0);

        let injection_sql = format!("SELECT COUNT(*) FROM logs WHERE {where_clause} AND guardrail_type = 'prompt_injection'");
        let prompt_injection_blocks: u64 = self
            .conn
            .query_row(
                &injection_sql,
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| r.get(0),
            )
            .unwrap_or(0);

        let content_sql = format!(
            "SELECT COUNT(*) FROM logs WHERE {where_clause} AND guardrail_type = 'content_filter'"
        );
        let content_filter_blocks: u64 = self
            .conn
            .query_row(
                &content_sql,
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| r.get(0),
            )
            .unwrap_or(0);

        let top_keywords_sql = format!(
            "SELECT guardrail_detail, COUNT(*) as cnt FROM logs
             WHERE {where_clause} AND guardrail_type = 'keyword_block' AND guardrail_detail IS NOT NULL
             GROUP BY guardrail_detail ORDER BY cnt DESC LIMIT 10"
        );
        let top_keywords = {
            let mut stmt = self.conn.prepare(&top_keywords_sql)?;
            let rows = stmt.query_map(
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| {
                    Ok(KeywordCount {
                        keyword: r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                        count: r.get::<_, i64>(1)? as u64,
                    })
                },
            )?;
            rows.filter_map(|r| r.ok()).collect()
        };

        Ok(GuardrailsBreakdown {
            total_blocks,
            keyword_blocks,
            prompt_injection_blocks,
            content_filter_blocks,
            top_keywords,
        })
    }

    fn guardrails_timeline(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> rusqlite::Result<Vec<GuardrailsTimeline>> {
        let bucket_ms = bucket_secs * 1000;
        let (where_clause, params_vec) = build_guardrails_where(filter);

        let sql = format!(
            "SELECT (timestamp/{b})*{b} AS ts,
                    COUNT(*) as total,
                    SUM(CASE WHEN guardrail_type = 'keyword_block' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN guardrail_type = 'prompt_injection' THEN 1 ELSE 0 END),
                    SUM(CASE WHEN guardrail_type = 'content_filter' THEN 1 ELSE 0 END)
             FROM logs WHERE {where_clause} AND guardrail_triggered = 1
             GROUP BY ts ORDER BY ts ASC",
            b = bucket_ms
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let mapped = stmt.query_map(
            rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
            |r| {
                Ok(GuardrailsTimeline {
                    timestamp: r.get(0)?,
                    total: r.get::<_, i64>(1)? as u64,
                    keyword_blocks: r.get::<_, Option<i64>>(2)?.unwrap_or(0) as u64,
                    prompt_injection: r.get::<_, Option<i64>>(3)?.unwrap_or(0) as u64,
                    content_filter: r.get::<_, Option<i64>>(4)?.unwrap_or(0) as u64,
                })
            },
        )?;
        let rows: Vec<GuardrailsTimeline> = mapped.filter_map(|r| r.ok()).collect();
        Ok(rows)
    }

    fn token_histogram(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> rusqlite::Result<Vec<TokenBucket>> {
        let bucket_ms = bucket_secs * 1000;
        let (where_str, params_vec) = build_where_clause(filter);

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
            .query_map(
                rusqlite::params_from_iter(params_vec.iter().map(|p| p.as_ref())),
                |r| {
                    Ok(TokenBucket {
                        timestamp: r.get(0)?,
                        prompt_tokens: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
                        completion_tokens: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                        total_tokens: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                    })
                },
            )?
            .filter_map(|r| r.ok())
            .collect();
        Ok(rows)
    }
}

fn build_guardrails_where(filter: &LogFilter) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut conditions = vec!["guardrail_triggered = 1".to_string()];
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 0;

    if let Some(ref _provider) = filter.provider {
        idx += 1;
        conditions.push(format!("provider = ?{idx}"));
        params_vec.push(Box::new(_provider.clone()));
    }
    if let Some(ref _model) = filter.model {
        idx += 1;
        conditions.push(format!("model LIKE ?{idx}"));
        params_vec.push(Box::new(format!("%{_model}%")));
    }
    if let Some(ref _status) = filter.status {
        idx += 1;
        conditions.push(format!("status = ?{idx}"));
        params_vec.push(Box::new(_status.as_str().to_string()));
    }
    if let Some(since) = filter.since_ms {
        idx += 1;
        conditions.push(format!("timestamp >= ?{idx}"));
        params_vec.push(Box::new(since));
    }
    if let Some(until) = filter.until_ms {
        idx += 1;
        conditions.push(format!("timestamp <= ?{idx}"));
        params_vec.push(Box::new(until));
    }
    if let Some(ref vk) = filter.virtual_key {
        idx += 1;
        conditions.push(format!("virtual_key = ?{idx}"));
        params_vec.push(Box::new(vk.clone()));
    }

    (conditions.join(" AND "), params_vec)
}

fn build_where_clause(filter: &LogFilter) -> (String, Vec<Box<dyn rusqlite::types::ToSql>>) {
    let mut conditions = Vec::<String>::new();
    let mut params_vec: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();
    let mut idx = 0;

    if let Some(ref provider) = filter.provider {
        idx += 1;
        conditions.push(format!("provider = ?{idx}"));
        params_vec.push(Box::new(provider.clone()));
    }
    if let Some(ref model) = filter.model {
        idx += 1;
        conditions.push(format!("model LIKE ?{idx}"));
        params_vec.push(Box::new(format!("%{model}%")));
    }
    if let Some(ref status) = filter.status {
        idx += 1;
        conditions.push(format!("status = ?{idx}"));
        params_vec.push(Box::new(status.as_str().to_string()));
    }
    if let Some(since) = filter.since_ms {
        idx += 1;
        conditions.push(format!("timestamp >= ?{idx}"));
        params_vec.push(Box::new(since));
    }
    if let Some(until) = filter.until_ms {
        idx += 1;
        conditions.push(format!("timestamp <= ?{idx}"));
        params_vec.push(Box::new(until));
    }
    if let Some(ref vk) = filter.virtual_key {
        idx += 1;
        conditions.push(format!("virtual_key = ?{idx}"));
        params_vec.push(Box::new(vk.clone()));
    }

    let where_str = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    (where_str, params_vec)
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
        guardrail_triggered: r.get::<_, Option<i32>>(18)?.map(|v| v != 0),
        guardrail_type: r.get(19)?,
        guardrail_detail: r.get(20)?,
        obfuscated_input: r.get(21).ok(),
        pii_mapping: r.get(22).ok(),
        moderation_scores: r.get(23).ok(),
        caveman_input: r.get(24).ok(),
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

    pub async fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        let g = self.inner.lock().await;
        match &*g {
            Backend::Sqlite(db) => db
                .list_guardrails(limit, offset, filter)
                .unwrap_or_default(),
            Backend::Memory { buf, .. } => {
                let filtered: Vec<&LogEntry> = buf
                    .iter()
                    .rev()
                    .filter(|e| e.guardrail_triggered == Some(true) && filter.matches(e))
                    .collect();
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

    pub async fn guardrails_stats(&self, filter: &LogFilter) -> GuardrailsBreakdown {
        let g = self.inner.lock().await;
        match &*g {
            Backend::Sqlite(db) => db.guardrails_breakdown(filter).unwrap_or_default(),
            Backend::Memory { buf, .. } => {
                let entries: Vec<&LogEntry> = buf
                    .iter()
                    .filter(|e| e.guardrail_triggered == Some(true) && filter.matches(e))
                    .collect();
                let total_blocks = entries.len() as u64;
                let keyword_blocks = entries
                    .iter()
                    .filter(|e| e.guardrail_type.as_deref() == Some("keyword_block"))
                    .count() as u64;
                let prompt_injection_blocks = entries
                    .iter()
                    .filter(|e| e.guardrail_type.as_deref() == Some("prompt_injection"))
                    .count() as u64;
                let content_filter_blocks = entries
                    .iter()
                    .filter(|e| e.guardrail_type.as_deref() == Some("content_filter"))
                    .count() as u64;
                GuardrailsBreakdown {
                    total_blocks,
                    keyword_blocks,
                    prompt_injection_blocks,
                    content_filter_blocks,
                    top_keywords: vec![],
                }
            }
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
        let g = self.inner.lock().await;
        match &*g {
            Backend::Sqlite(db) => db
                .guardrails_timeline(filter, bucket_secs)
                .unwrap_or_default(),
            Backend::Memory { buf, .. } => {
                let entries: Vec<&LogEntry> = buf
                    .iter()
                    .filter(|e| e.guardrail_triggered == Some(true) && filter.matches(e))
                    .collect();
                memory_guardrails_timeline(&entries, bucket_secs)
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

fn memory_guardrails_timeline(entries: &[&LogEntry], bucket_secs: i64) -> Vec<GuardrailsTimeline> {
    let mut map: std::collections::BTreeMap<i64, GuardrailsTimeline> = Default::default();
    let bms = bucket_secs * 1000;
    for e in entries {
        let ts = (e.timestamp / bms) * bms;
        let b = map.entry(ts).or_insert(GuardrailsTimeline {
            timestamp: ts,
            total: 0,
            keyword_blocks: 0,
            prompt_injection: 0,
            content_filter: 0,
        });
        b.total += 1;
        match e.guardrail_type.as_deref() {
            Some("keyword_block") => b.keyword_blocks += 1,
            Some("prompt_injection") => b.prompt_injection += 1,
            _ => b.content_filter += 1,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GuardrailsBreakdown {
    pub total_blocks: u64,
    pub keyword_blocks: u64,
    pub prompt_injection_blocks: u64,
    pub content_filter_blocks: u64,
    pub top_keywords: Vec<KeywordCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordCount {
    pub keyword: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailsTimeline {
    pub timestamp: i64,
    pub total: u64,
    pub keyword_blocks: u64,
    pub prompt_injection: u64,
    pub content_filter: u64,
}

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
    build_log_entry_full(
        provider,
        model,
        is_stream,
        status,
        latency_ms,
        usage,
        finish_reason,
        error_message,
        input_preview,
        output_preview,
        virtual_key,
        compression_saved_bytes,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_log_entry_full(
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
    guardrail_triggered: Option<bool>,
    guardrail_type: Option<String>,
    guardrail_detail: Option<String>,
    obfuscated_input: Option<String>,
    pii_mapping: Option<String>,
    moderation_scores: Option<String>,
    caveman_input: Option<String>,
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
        input_preview,
        output_preview,
        compression_saved_bytes,
        guardrail_triggered,
        guardrail_type,
        guardrail_detail,
        obfuscated_input,
        pii_mapping,
        moderation_scores,
        caveman_input,
    }
}

#[allow(dead_code)]
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
    if provider.starts_with("ollama") {
        return 0.0;
    }
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
