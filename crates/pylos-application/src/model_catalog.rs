use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::postgres::{PgPool, PgPoolOptions};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;
use tracing::info;

#[derive(Clone)]
enum Pool {
    Sqlite(SqlitePool),
    Postgres(PgPool),
}

impl Pool {
    async fn run_migrations(&self) -> Result<(), sqlx::Error> {
        match self {
            Pool::Sqlite(pool) => sqlx::migrate!("./migrations")
                .run(pool)
                .await
                .map_err(|e| sqlx::Error::Migrate(Box::new(e))),
            Pool::Postgres(pool) => sqlx::migrate!("./migrations_postgres")
                .run(pool)
                .await
                .map_err(|e| sqlx::Error::Migrate(Box::new(e))),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub provider: String,
    pub model_id: String,
    pub display_name: Option<String>,
    pub context_window: u32,
    pub max_output_tokens: u32,
    pub input_price_per_1m_usd: f64,
    pub output_price_per_1m_usd: f64,
    pub supports_vision: bool,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_embeddings: bool,
    pub is_deprecated: bool,
    pub enabled: bool,

    // ── LiteLLM-style overrides ──────────────────────────────────────────
    pub api_base: Option<String>,
    pub tpm: Option<u32>,
    pub rpm: Option<u32>,
    pub max_retries: Option<u32>,
    pub timeout_secs: Option<u32>,
    pub stream_timeout_secs: Option<u32>,
    pub model_access_groups: Option<Vec<String>>,
    pub guardrails: Option<Vec<String>>,
    pub tags: Option<Vec<String>>,
    pub provider_params: Option<serde_json::Value>,
    pub organization_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelHealth {
    pub id: String,
    pub provider: String,
    pub model_id: String,
    pub health_status: String,
    pub error_details: Option<String>,
    pub last_check_ms: Option<i64>,
    pub last_success_ms: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PricingReloadStatus {
    pub source_url: String,
    pub last_reload_ms: Option<i64>,
    pub models_count: i32,
    pub periodic_schedule: Option<String>,
}

#[derive(Clone)]
pub struct ModelCatalog {
    pool: Pool,
}

impl ModelCatalog {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::new()
            .filename(db_path)
            .create_if_missing(true)
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        let pool = SqlitePoolOptions::new()
            .max_connections(4)
            .connect_with(options)
            .await?;

        let catalog = Self {
            pool: Pool::Sqlite(pool),
        };
        catalog.pool.run_migrations().await?;
        catalog.seed_builtin_models().await;

        info!(path = %db_path.display(), "Model catalog opened (SQLite)");
        Ok(catalog)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(database_url)
            .await?;

        let catalog = Self {
            pool: Pool::Postgres(pool),
        };
        catalog.pool.run_migrations().await?;
        catalog.seed_builtin_models().await;

        info!("Model catalog opened (PostgreSQL)");
        Ok(catalog)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = SqlitePoolOptions::new()
            .max_connections(2)
            .connect("sqlite::memory:")
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS model_catalog (
                id                      TEXT PRIMARY KEY,
                provider                TEXT NOT NULL,
                model_id                TEXT NOT NULL,
                display_name            TEXT,
                context_window          INTEGER NOT NULL DEFAULT 0,
                max_output_tokens       INTEGER NOT NULL DEFAULT 0,
                input_price_per_1m_usd  REAL    NOT NULL DEFAULT 0.0,
                output_price_per_1m_usd REAL    NOT NULL DEFAULT 0.0,
                supports_vision         INTEGER NOT NULL DEFAULT 0,
                supports_tools          INTEGER NOT NULL DEFAULT 1,
                supports_streaming      INTEGER NOT NULL DEFAULT 1,
                supports_embeddings     INTEGER NOT NULL DEFAULT 0,
                is_deprecated           INTEGER NOT NULL DEFAULT 0,
                enabled                 INTEGER NOT NULL DEFAULT 1,
                updated_at_ms           INTEGER NOT NULL,
                api_base                TEXT,
                tpm                     INTEGER,
                rpm                     INTEGER,
                max_retries             INTEGER,
                timeout_secs            INTEGER,
                stream_timeout_secs     INTEGER,
                model_access_groups     TEXT,
                guardrails              TEXT,
                tags                    TEXT,
                provider_params         TEXT,
                organization_id         TEXT
            );
            CREATE UNIQUE INDEX IF NOT EXISTS idx_catalog_uniq ON model_catalog(provider, model_id);
            CREATE TABLE IF NOT EXISTS model_health (
                id              TEXT PRIMARY KEY,
                provider        TEXT NOT NULL,
                model_id        TEXT NOT NULL,
                health_status   TEXT NOT NULL DEFAULT 'none',
                error_details   TEXT,
                last_check_ms   INTEGER,
                last_success_ms INTEGER
            );
        "#,
        )
        .execute(&pool)
        .await?;

        let catalog = Self {
            pool: Pool::Sqlite(pool),
        };
        catalog.seed_builtin_models().await;
        Ok(catalog)
    }

    pub async fn list_models(
        &self,
        provider: Option<&str>,
        include_deprecated: bool,
    ) -> Vec<ModelInfo> {
        let mut sql = String::from("SELECT * FROM model_catalog WHERE 1=1");
        let mut params: Vec<String> = vec![];

        if let Some(p) = provider {
            sql.push_str(&format!(" AND provider = ${}", params.len() + 1));
            params.push(p.to_string());
        }
        if !include_deprecated {
            sql.push_str(" AND is_deprecated = 0 AND enabled = 1");
        }
        sql.push_str(" ORDER BY provider, model_id");

        match &self.pool {
            Pool::Sqlite(pool) => {
                let mut q = sqlx::query(&sql);
                for p in &params {
                    q = q.bind(p);
                }
                q.fetch_all(pool)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .map(row_to_model_info_sqlite)
                    .collect()
            }
            Pool::Postgres(pool) => {
                let mut q = sqlx::query::<sqlx::Postgres>(&sql);
                for p in &params {
                    q = q.bind(p);
                }
                q.fetch_all(pool)
                    .await
                    .unwrap_or_default()
                    .iter()
                    .map(row_to_model_info_pg)
                    .collect()
            }
        }
    }

    pub async fn get_model(&self, provider: &str, model_id: &str) -> Option<ModelInfo> {
        match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query("SELECT * FROM model_catalog WHERE provider = $1 AND model_id = $2")
                    .bind(provider)
                    .bind(model_id)
                    .fetch_optional(pool)
                    .await
                    .ok()
                    .flatten()
                    .as_ref()
                    .map(row_to_model_info_sqlite)
            }
            Pool::Postgres(pool) => sqlx::query::<sqlx::Postgres>(
                "SELECT * FROM model_catalog WHERE provider = $1 AND model_id = $2",
            )
            .bind(provider)
            .bind(model_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .as_ref()
            .map(row_to_model_info_pg),
        }
    }

    pub async fn get_pricing(&self, provider: &str, model_id: &str) -> (f64, f64) {
        if let Some(info) = self.get_model(provider, model_id).await {
            return (info.input_price_per_1m_usd, info.output_price_per_1m_usd);
        }
        (1.0, 3.0)
    }

    pub async fn upsert_model(&self, model: &ModelInfo) -> Result<(), sqlx::Error> {
        let now = now_ms();
        let model_access_groups_json = model
            .model_access_groups
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let guardrails_json = model
            .guardrails
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let tags_json = model
            .tags
            .as_ref()
            .map(|v| serde_json::to_string(v).unwrap_or_default());
        let provider_params_json = model.provider_params.as_ref().map(|v| v.to_string());
        match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO model_catalog
                        (id, provider, model_id, display_name, context_window, max_output_tokens,
                         input_price_per_1m_usd, output_price_per_1m_usd,
                         supports_vision, supports_tools, supports_streaming, supports_embeddings,
                         is_deprecated, enabled, updated_at_ms,
                         api_base, tpm, rpm, max_retries, timeout_secs, stream_timeout_secs,
                         model_access_groups, guardrails, tags, provider_params, organization_id)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                            $16, $17, $18, $19, $20, $21, $22, $23, $24, $25, $26)
                    ON CONFLICT(id) DO UPDATE SET
                        display_name = excluded.display_name,
                        context_window = excluded.context_window,
                        max_output_tokens = excluded.max_output_tokens,
                        input_price_per_1m_usd = excluded.input_price_per_1m_usd,
                        output_price_per_1m_usd = excluded.output_price_per_1m_usd,
                        supports_vision = excluded.supports_vision,
                        supports_tools = excluded.supports_tools,
                        supports_streaming = excluded.supports_streaming,
                        supports_embeddings = excluded.supports_embeddings,
                        is_deprecated = excluded.is_deprecated,
                        enabled = excluded.enabled,
                        updated_at_ms = excluded.updated_at_ms,
                        api_base = excluded.api_base,
                        tpm = excluded.tpm,
                        rpm = excluded.rpm,
                        max_retries = excluded.max_retries,
                        timeout_secs = excluded.timeout_secs,
                        stream_timeout_secs = excluded.stream_timeout_secs,
                        model_access_groups = excluded.model_access_groups,
                        guardrails = excluded.guardrails,
                        tags = excluded.tags,
                        provider_params = excluded.provider_params,
                        organization_id = excluded.organization_id
                    "#,
                )
                .bind(&model.id)
                .bind(&model.provider)
                .bind(&model.model_id)
                .bind(&model.display_name)
                .bind(model.context_window)
                .bind(model.max_output_tokens)
                .bind(model.input_price_per_1m_usd)
                .bind(model.output_price_per_1m_usd)
                .bind(model.supports_vision as i32)
                .bind(model.supports_tools as i32)
                .bind(model.supports_streaming as i32)
                .bind(model.supports_embeddings as i32)
                .bind(model.is_deprecated as i32)
                .bind(model.enabled as i32)
                .bind(now)
                .bind(&model.api_base)
                .bind(model.tpm.map(|v| v as i64))
                .bind(model.rpm.map(|v| v as i64))
                .bind(model.max_retries.map(|v| v as i64))
                .bind(model.timeout_secs.map(|v| v as i64))
                .bind(model.stream_timeout_secs.map(|v| v as i64))
                .bind(&model_access_groups_json)
                .bind(&guardrails_json)
                .bind(&tags_json)
                .bind(&provider_params_json)
                .bind(&model.organization_id)
                .execute(pool)
                .await?;
            }
            Pool::Postgres(pool) => {
                sqlx::query::<sqlx::Postgres>(
                    r#"
                    INSERT INTO model_catalog
                        (id, provider, model_id, display_name, context_window, max_output_tokens,
                         input_price_per_1m_usd, output_price_per_1m_usd,
                         supports_vision, supports_tools, supports_streaming, supports_embeddings,
                         is_deprecated, enabled, updated_at_ms,
                         api_base, tpm, rpm, max_retries, timeout_secs, stream_timeout_secs,
                         model_access_groups, guardrails, tags, provider_params, organization_id)
                    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15,
                            $16, $17, $18, $19, $20, $21, $22::jsonb, $23::jsonb, $24::jsonb, $25::jsonb, $26)
                    ON CONFLICT(id) DO UPDATE SET
                        display_name = excluded.display_name,
                        context_window = excluded.context_window,
                        max_output_tokens = excluded.max_output_tokens,
                        input_price_per_1m_usd = excluded.input_price_per_1m_usd,
                        output_price_per_1m_usd = excluded.output_price_per_1m_usd,
                        supports_vision = excluded.supports_vision,
                        supports_tools = excluded.supports_tools,
                        supports_streaming = excluded.supports_streaming,
                        supports_embeddings = excluded.supports_embeddings,
                        is_deprecated = excluded.is_deprecated,
                        enabled = excluded.enabled,
                        updated_at_ms = excluded.updated_at_ms,
                        api_base = excluded.api_base,
                        tpm = excluded.tpm,
                        rpm = excluded.rpm,
                        max_retries = excluded.max_retries,
                        timeout_secs = excluded.timeout_secs,
                        stream_timeout_secs = excluded.stream_timeout_secs,
                        model_access_groups = excluded.model_access_groups,
                        guardrails = excluded.guardrails,
                        tags = excluded.tags,
                        provider_params = excluded.provider_params,
                        organization_id = excluded.organization_id
                    "#,
                )
                .bind(&model.id)
                .bind(&model.provider)
                .bind(&model.model_id)
                .bind(&model.display_name)
                .bind(model.context_window as i32)
                .bind(model.max_output_tokens as i32)
                .bind(model.input_price_per_1m_usd)
                .bind(model.output_price_per_1m_usd)
                .bind(model.supports_vision)
                .bind(model.supports_tools)
                .bind(model.supports_streaming)
                .bind(model.supports_embeddings)
                .bind(model.is_deprecated)
                .bind(model.enabled)
                .bind(now)
                .bind(&model.api_base)
                .bind(model.tpm.map(|v| v as i32))
                .bind(model.rpm.map(|v| v as i32))
                .bind(model.max_retries.map(|v| v as i32))
                .bind(model.timeout_secs.map(|v| v as i32))
                .bind(model.stream_timeout_secs.map(|v| v as i32))
                .bind(&model_access_groups_json)
                .bind(&guardrails_json)
                .bind(&tags_json)
                .bind(&provider_params_json)
                .bind(&model.organization_id)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    async fn seed_builtin_models(&self) {
        let models = builtin_models();
        for m in &models {
            if let Err(e) = self.upsert_model(m).await {
                tracing::warn!(model = %m.id, error = %e, "Failed to seed model");
            }
        }
        info!(count = models.len(), "Model catalog seeded");
    }

    pub async fn delete_model(&self, provider: &str, model_id: &str) -> Result<bool, sqlx::Error> {
        let rows_affected = match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query("DELETE FROM model_catalog WHERE provider = $1 AND model_id = $2")
                    .bind(provider)
                    .bind(model_id)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
            Pool::Postgres(pool) => sqlx::query::<sqlx::Postgres>(
                "DELETE FROM model_catalog WHERE provider = $1 AND model_id = $2",
            )
            .bind(provider)
            .bind(model_id)
            .execute(pool)
            .await?
            .rows_affected(),
        };
        Ok(rows_affected > 0)
    }

    pub async fn delete_models_by_provider(&self, provider: &str) -> Result<u64, sqlx::Error> {
        let rows_affected = match &self.pool {
            Pool::Sqlite(pool) => sqlx::query("DELETE FROM model_catalog WHERE provider = $1")
                .bind(provider)
                .execute(pool)
                .await?
                .rows_affected(),
            Pool::Postgres(pool) => {
                sqlx::query::<sqlx::Postgres>("DELETE FROM model_catalog WHERE provider = $1")
                    .bind(provider)
                    .execute(pool)
                    .await?
                    .rows_affected()
            }
        };
        Ok(rows_affected)
    }

    pub async fn get_pricing_status(&self) -> Result<PricingReloadStatus, sqlx::Error> {
        match &self.pool {
            Pool::Sqlite(pool) => {
                let row = sqlx::query("SELECT source_url, last_reload_ms, models_count, periodic_schedule FROM pricing_reload_status WHERE id = 1")
                    .fetch_one(pool)
                    .await?;
                Ok(PricingReloadStatus {
                    source_url: row.try_get("source_url")?,
                    last_reload_ms: row.try_get("last_reload_ms").ok(),
                    models_count: row.try_get::<i64, _>("models_count")? as i32,
                    periodic_schedule: row.try_get("periodic_schedule").ok(),
                })
            }
            Pool::Postgres(pool) => {
                let row = sqlx::query("SELECT source_url, last_reload_ms, models_count, periodic_schedule FROM pricing_reload_status WHERE id = 1")
                    .fetch_one(pool)
                    .await?;
                Ok(PricingReloadStatus {
                    source_url: row.try_get("source_url")?,
                    last_reload_ms: row.try_get("last_reload_ms").ok(),
                    models_count: row.try_get::<i32, _>("models_count")?,
                    periodic_schedule: row.try_get("periodic_schedule").ok(),
                })
            }
        }
    }

    pub async fn update_pricing_status(
        &self,
        source_url: &str,
        last_reload_ms: Option<i64>,
        models_count: i32,
        periodic_schedule: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO pricing_reload_status (id, source_url, last_reload_ms, models_count, periodic_schedule)
                    VALUES (1, $1, $2, $3, $4)
                    ON CONFLICT(id) DO UPDATE SET
                        source_url = excluded.source_url,
                        last_reload_ms = excluded.last_reload_ms,
                        models_count = excluded.models_count,
                        periodic_schedule = excluded.periodic_schedule
                    "#,
                )
                .bind(source_url)
                .bind(last_reload_ms)
                .bind(models_count as i64)
                .bind(periodic_schedule)
                .execute(pool)
                .await?;
            }
            Pool::Postgres(pool) => {
                sqlx::query(
                    r#"
                    INSERT INTO pricing_reload_status (id, source_url, last_reload_ms, models_count, periodic_schedule)
                    VALUES (1, $1, $2, $3, $4)
                    ON CONFLICT(id) DO UPDATE SET
                        source_url = excluded.source_url,
                        last_reload_ms = excluded.last_reload_ms,
                        models_count = excluded.models_count,
                        periodic_schedule = excluded.periodic_schedule
                    "#,
                )
                .bind(source_url)
                .bind(last_reload_ms)
                .bind(models_count)
                .bind(periodic_schedule)
                .execute(pool)
                .await?;
            }
        }
        Ok(())
    }

    pub async fn list_model_health(&self) -> Result<Vec<ModelHealth>, sqlx::Error> {
        let sql = "SELECT id, provider, model_id, health_status, error_details, last_check_ms, last_success_ms FROM model_health";
        match &self.pool {
            Pool::Sqlite(pool) => {
                let rows = sqlx::query(sql).fetch_all(pool).await?;
                Ok(rows
                    .iter()
                    .map(|r| ModelHealth {
                        id: r.try_get("id").unwrap_or_default(),
                        provider: r.try_get("provider").unwrap_or_default(),
                        model_id: r.try_get("model_id").unwrap_or_default(),
                        health_status: r
                            .try_get("health_status")
                            .unwrap_or_else(|_| "none".to_string()),
                        error_details: r.try_get("error_details").ok(),
                        last_check_ms: r.try_get::<Option<i64>, _>("last_check_ms").unwrap_or(None),
                        last_success_ms: r
                            .try_get::<Option<i64>, _>("last_success_ms")
                            .unwrap_or(None),
                    })
                    .collect())
            }
            Pool::Postgres(pool) => {
                let rows = sqlx::query::<sqlx::Postgres>(sql).fetch_all(pool).await?;
                Ok(rows
                    .iter()
                    .map(|r| ModelHealth {
                        id: r.try_get("id").unwrap_or_default(),
                        provider: r.try_get("provider").unwrap_or_default(),
                        model_id: r.try_get("model_id").unwrap_or_default(),
                        health_status: r
                            .try_get("health_status")
                            .unwrap_or_else(|_| "none".to_string()),
                        error_details: r.try_get("error_details").ok(),
                        last_check_ms: r.try_get::<Option<i64>, _>("last_check_ms").unwrap_or(None),
                        last_success_ms: r
                            .try_get::<Option<i64>, _>("last_success_ms")
                            .unwrap_or(None),
                    })
                    .collect())
            }
        }
    }

    pub async fn update_model_health(&self, health: &ModelHealth) -> Result<(), sqlx::Error> {
        let sql = r#"
            INSERT INTO model_health (id, provider, model_id, health_status, error_details, last_check_ms, last_success_ms)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            ON CONFLICT(id) DO UPDATE SET
                health_status = excluded.health_status,
                error_details = excluded.error_details,
                last_check_ms = excluded.last_check_ms,
                last_success_ms = excluded.last_success_ms
        "#;
        match &self.pool {
            Pool::Sqlite(pool) => {
                sqlx::query(sql)
                    .bind(&health.id)
                    .bind(&health.provider)
                    .bind(&health.model_id)
                    .bind(&health.health_status)
                    .bind(&health.error_details)
                    .bind(health.last_check_ms)
                    .bind(health.last_success_ms)
                    .execute(pool)
                    .await?;
            }
            Pool::Postgres(pool) => {
                sqlx::query(sql)
                    .bind(&health.id)
                    .bind(&health.provider)
                    .bind(&health.model_id)
                    .bind(&health.health_status)
                    .bind(&health.error_details)
                    .bind(health.last_check_ms)
                    .bind(health.last_success_ms)
                    .execute(pool)
                    .await?;
            }
        }
        Ok(())
    }
}

fn parse_json_array(val: Option<&str>) -> Option<Vec<String>> {
    val.and_then(|v| serde_json::from_str(v).ok())
}

fn parse_json_value(val: Option<&str>) -> Option<serde_json::Value> {
    val.and_then(|v| serde_json::from_str(v).ok())
}

fn row_to_model_info_sqlite(row: &sqlx::sqlite::SqliteRow) -> ModelInfo {
    ModelInfo {
        id: row.try_get("id").unwrap_or_default(),
        provider: row.try_get("provider").unwrap_or_default(),
        model_id: row.try_get("model_id").unwrap_or_default(),
        display_name: row.try_get("display_name").ok(),
        context_window: row.try_get::<i64, _>("context_window").unwrap_or(0) as u32,
        max_output_tokens: row.try_get::<i64, _>("max_output_tokens").unwrap_or(0) as u32,
        input_price_per_1m_usd: row.try_get("input_price_per_1m_usd").unwrap_or(0.0),
        output_price_per_1m_usd: row.try_get("output_price_per_1m_usd").unwrap_or(0.0),
        supports_vision: row.try_get::<i64, _>("supports_vision").unwrap_or(0) != 0,
        supports_tools: row.try_get::<i64, _>("supports_tools").unwrap_or(1) != 0,
        supports_streaming: row.try_get::<i64, _>("supports_streaming").unwrap_or(1) != 0,
        supports_embeddings: row.try_get::<i64, _>("supports_embeddings").unwrap_or(0) != 0,
        is_deprecated: row.try_get::<i64, _>("is_deprecated").unwrap_or(0) != 0,
        enabled: row.try_get::<i64, _>("enabled").unwrap_or(1) != 0,
        api_base: row.try_get("api_base").ok(),
        tpm: row
            .try_get::<Option<i64>, _>("tpm")
            .unwrap_or(None)
            .map(|v| v as u32),
        rpm: row
            .try_get::<Option<i64>, _>("rpm")
            .unwrap_or(None)
            .map(|v| v as u32),
        max_retries: row
            .try_get::<Option<i64>, _>("max_retries")
            .unwrap_or(None)
            .map(|v| v as u32),
        timeout_secs: row
            .try_get::<Option<i64>, _>("timeout_secs")
            .unwrap_or(None)
            .map(|v| v as u32),
        stream_timeout_secs: row
            .try_get::<Option<i64>, _>("stream_timeout_secs")
            .unwrap_or(None)
            .map(|v| v as u32),
        model_access_groups: parse_json_array(row.try_get("model_access_groups").ok()),
        guardrails: parse_json_array(row.try_get("guardrails").ok()),
        tags: parse_json_array(row.try_get("tags").ok()),
        provider_params: parse_json_value(row.try_get("provider_params").ok()),
        organization_id: row.try_get("organization_id").ok(),
    }
}

fn row_to_model_info_pg(row: &sqlx::postgres::PgRow) -> ModelInfo {
    ModelInfo {
        id: row.try_get("id").unwrap_or_default(),
        provider: row.try_get("provider").unwrap_or_default(),
        model_id: row.try_get("model_id").unwrap_or_default(),
        display_name: row.try_get("display_name").ok(),
        context_window: row.try_get::<i32, _>("context_window").unwrap_or(0) as u32,
        max_output_tokens: row.try_get::<i32, _>("max_output_tokens").unwrap_or(0) as u32,
        input_price_per_1m_usd: row.try_get("input_price_per_1m_usd").unwrap_or(0.0),
        output_price_per_1m_usd: row.try_get("output_price_per_1m_usd").unwrap_or(0.0),
        supports_vision: row.try_get::<bool, _>("supports_vision").unwrap_or(false),
        supports_tools: row.try_get::<bool, _>("supports_tools").unwrap_or(true),
        supports_streaming: row.try_get::<bool, _>("supports_streaming").unwrap_or(true),
        supports_embeddings: row
            .try_get::<bool, _>("supports_embeddings")
            .unwrap_or(false),
        is_deprecated: row.try_get::<bool, _>("is_deprecated").unwrap_or(false),
        enabled: row.try_get::<bool, _>("enabled").unwrap_or(true),
        api_base: row.try_get("api_base").ok(),
        tpm: row
            .try_get::<Option<i32>, _>("tpm")
            .unwrap_or(None)
            .map(|v| v as u32),
        rpm: row
            .try_get::<Option<i32>, _>("rpm")
            .unwrap_or(None)
            .map(|v| v as u32),
        max_retries: row
            .try_get::<Option<i32>, _>("max_retries")
            .unwrap_or(None)
            .map(|v| v as u32),
        timeout_secs: row
            .try_get::<Option<i32>, _>("timeout_secs")
            .unwrap_or(None)
            .map(|v| v as u32),
        stream_timeout_secs: row
            .try_get::<Option<i32>, _>("stream_timeout_secs")
            .unwrap_or(None)
            .map(|v| v as u32),
        model_access_groups: parse_json_array(row.try_get("model_access_groups").ok()),
        guardrails: parse_json_array(row.try_get("guardrails").ok()),
        tags: parse_json_array(row.try_get("tags").ok()),
        provider_params: parse_json_value(row.try_get("provider_params").ok()),
        organization_id: row.try_get("organization_id").ok(),
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[allow(clippy::too_many_arguments)]
fn m(
    provider: &str,
    model_id: &str,
    display: &str,
    ctx: u32,
    max_out: u32,
    in_price: f64,
    out_price: f64,
    vision: bool,
    tools: bool,
    embeddings: bool,
) -> ModelInfo {
    ModelInfo {
        id: format!("{}/{}", provider, model_id),
        provider: provider.to_string(),
        model_id: model_id.to_string(),
        display_name: Some(display.to_string()),
        context_window: ctx,
        max_output_tokens: max_out,
        input_price_per_1m_usd: in_price,
        output_price_per_1m_usd: out_price,
        supports_vision: vision,
        supports_tools: tools,
        supports_streaming: true,
        supports_embeddings: embeddings,
        is_deprecated: false,
        enabled: true,
        api_base: None,
        tpm: None,
        rpm: None,
        max_retries: None,
        timeout_secs: None,
        stream_timeout_secs: None,
        model_access_groups: None,
        guardrails: None,
        tags: None,
        provider_params: None,
        organization_id: None,
    }
}

fn builtin_models() -> Vec<ModelInfo> {
    vec![
        m(
            "openai", "gpt-4o", "GPT-4o", 128_000, 16_384, 5.0, 15.0, true, true, false,
        ),
        m(
            "openai",
            "gpt-4o-mini",
            "GPT-4o Mini",
            128_000,
            16_384,
            0.15,
            0.60,
            true,
            true,
            false,
        ),
        m(
            "openai",
            "gpt-4-turbo",
            "GPT-4 Turbo",
            128_000,
            4_096,
            10.0,
            30.0,
            true,
            true,
            false,
        ),
        m(
            "openai", "o1", "o1", 200_000, 100_000, 15.0, 60.0, false, true, false,
        ),
        m(
            "openai", "o1-mini", "o1-mini", 128_000, 65_536, 3.0, 12.0, false, false, false,
        ),
        m(
            "openai", "o3-mini", "o3-mini", 200_000, 100_000, 1.1, 4.4, false, true, false,
        ),
        m(
            "openai",
            "text-embedding-3-small",
            "Embedding 3 Small",
            8_191,
            0,
            0.02,
            0.0,
            false,
            false,
            true,
        ),
        m(
            "openai",
            "text-embedding-3-large",
            "Embedding 3 Large",
            8_191,
            0,
            0.13,
            0.0,
            false,
            false,
            true,
        ),
        m(
            "anthropic",
            "claude-opus-4-5",
            "Claude Opus 4.5",
            200_000,
            32_000,
            15.0,
            75.0,
            true,
            true,
            false,
        ),
        m(
            "anthropic",
            "claude-sonnet-4-5",
            "Claude Sonnet 4.5",
            200_000,
            64_000,
            3.0,
            15.0,
            true,
            true,
            false,
        ),
        m(
            "anthropic",
            "claude-haiku-3-5",
            "Claude Haiku 3.5",
            200_000,
            8_096,
            0.8,
            4.0,
            true,
            true,
            false,
        ),
        m(
            "anthropic",
            "claude-3-5-sonnet-20241022",
            "Claude 3.5 Sonnet",
            200_000,
            8_096,
            3.0,
            15.0,
            true,
            true,
            false,
        ),
        m(
            "anthropic",
            "claude-3-opus-20240229",
            "Claude 3 Opus",
            200_000,
            4_096,
            15.0,
            75.0,
            true,
            true,
            false,
        ),
        m(
            "anthropic",
            "claude-3-haiku-20240307",
            "Claude 3 Haiku",
            200_000,
            4_096,
            0.25,
            1.25,
            true,
            true,
            false,
        ),
        m(
            "gemini",
            "gemini-3.5-pro",
            "Gemini 3.5 Pro",
            1_000_000,
            65_536,
            7.0,
            21.0,
            true,
            true,
            false,
        ),
        m(
            "gemini",
            "gemini-3.5-flash",
            "Gemini 3.5 Flash",
            1_000_000,
            65_536,
            0.3,
            2.5,
            true,
            true,
            false,
        ),
        m(
            "gemini",
            "gemini-2.5-pro",
            "Gemini 2.5 Pro",
            1_000_000,
            65_536,
            7.0,
            21.0,
            true,
            true,
            false,
        ),
        m(
            "gemini",
            "gemini-2.5-flash",
            "Gemini 2.5 Flash",
            1_000_000,
            65_536,
            0.3,
            2.5,
            true,
            true,
            false,
        ),
        m(
            "gemini",
            "gemini-2.0-flash",
            "Gemini 2.0 Flash",
            1_000_000,
            8_192,
            0.1,
            0.4,
            true,
            true,
            false,
        ),
        m(
            "gemini",
            "gemini-1.5-pro",
            "Gemini 1.5 Pro",
            2_000_000,
            8_192,
            3.5,
            10.5,
            true,
            true,
            false,
        ),
        m(
            "gemini",
            "text-embedding-004",
            "Gemini Embedding",
            2_048,
            0,
            0.025,
            0.0,
            false,
            false,
            true,
        ),
        m(
            "gemini",
            "gemini-embedding-001",
            "Gemini Embedding 001",
            2_048,
            0,
            0.0,
            0.0,
            false,
            false,
            true,
        ),
        m(
            "cohere",
            "command-a-03-2025",
            "Command A",
            256_000,
            8_000,
            2.5,
            10.0,
            true,
            true,
            false,
        ),
        m(
            "cohere",
            "command-r-plus",
            "Command R+",
            128_000,
            4_096,
            3.0,
            15.0,
            false,
            true,
            false,
        ),
        m(
            "cohere",
            "command-r",
            "Command R",
            128_000,
            4_096,
            0.15,
            0.60,
            false,
            true,
            false,
        ),
        m(
            "cohere",
            "embed-v4.0",
            "Embed v4",
            512,
            0,
            0.1,
            0.0,
            false,
            false,
            true,
        ),
        m(
            "groq",
            "llama-3.3-70b-versatile",
            "Llama 3.3 70B",
            128_000,
            32_768,
            0.59,
            0.79,
            false,
            true,
            false,
        ),
        m(
            "groq",
            "llama-3.1-8b-instant",
            "Llama 3.1 8B",
            128_000,
            8_192,
            0.05,
            0.08,
            false,
            true,
            false,
        ),
        m(
            "groq",
            "gemma2-9b-it",
            "Gemma 2 9B",
            8_192,
            8_192,
            0.2,
            0.2,
            false,
            true,
            false,
        ),
        m(
            "groq",
            "mixtral-8x7b-32768",
            "Mixtral 8x7B",
            32_768,
            32_768,
            0.24,
            0.24,
            false,
            true,
            false,
        ),
        m(
            "mistral",
            "mistral-large-latest",
            "Mistral Large",
            128_000,
            8_192,
            3.0,
            9.0,
            true,
            true,
            false,
        ),
        m(
            "mistral",
            "mistral-small-latest",
            "Mistral Small",
            128_000,
            8_192,
            0.2,
            0.6,
            false,
            true,
            false,
        ),
        m(
            "mistral",
            "codestral-latest",
            "Codestral",
            256_000,
            8_192,
            0.3,
            0.9,
            false,
            true,
            false,
        ),
        m(
            "xai", "grok-3", "Grok 3", 131_072, 8_192, 5.0, 15.0, true, true, false,
        ),
        m(
            "xai",
            "grok-3-mini",
            "Grok 3 Mini",
            131_072,
            8_192,
            0.3,
            0.5,
            true,
            true,
            false,
        ),
        m(
            "bedrock",
            "anthropic.claude-3-5-sonnet-20241022-v2:0",
            "Claude 3.5 Sonnet (Bedrock)",
            200_000,
            8_096,
            3.0,
            15.0,
            true,
            true,
            false,
        ),
        m(
            "bedrock",
            "amazon.nova-pro-v1:0",
            "Nova Pro",
            300_000,
            5_120,
            0.8,
            3.2,
            true,
            true,
            false,
        ),
        m(
            "bedrock",
            "amazon.nova-lite-v1:0",
            "Nova Lite",
            300_000,
            5_120,
            0.06,
            0.24,
            true,
            true,
            false,
        ),
        m(
            "bedrock",
            "amazon.nova-micro-v1:0",
            "Nova Micro",
            128_000,
            5_120,
            0.035,
            0.14,
            false,
            false,
            false,
        ),
        m(
            "deepseek",
            "deepseek-v4-pro",
            "DeepSeek V4 Pro",
            1_000_000,
            128_000,
            0.435,
            0.87,
            true,
            true,
            false,
        ),
        m(
            "deepseek",
            "deepseek-v4-flash",
            "DeepSeek V4 Flash",
            1_000_000,
            128_000,
            0.14,
            0.28,
            true,
            true,
            false,
        ),
        m(
            "deepseek",
            "deepseek-r1-v4",
            "DeepSeek R1 V4",
            128_000,
            16_384,
            0.55,
            2.19,
            false,
            true,
            false,
        ),
        m(
            "deepseek",
            "deepseek-chat",
            "DeepSeek V3",
            64_000,
            8_192,
            0.14,
            0.28,
            false,
            true,
            false,
        ),
        m(
            "opencode-zen",
            "deepseek-v4-flash-free",
            "DeepSeek V4 Flash Free",
            1_000_000,
            384_000,
            0.0,
            0.0,
            false,
            true,
            false,
        ),
        m(
            "graphon-rag",
            "graphon-rag",
            "Graphon RAG",
            8192,
            2048,
            0.0,
            0.0,
            false,
            false,
            false,
        ),
        m(
            "mnemosyne",
            "mnemosyne-search",
            "Mnemosyne Search Engine",
            8192,
            2048,
            0.0,
            0.0,
            false,
            false,
            false,
        ),
        // Ollama — modèles locaux populaires
        m(
            "ollama",
            "llama3.2:3b",
            "Llama 3.2 3B",
            128_000,
            8_192,
            0.0,
            0.0,
            false,
            true,
            false,
        ),
        m(
            "ollama",
            "llama3.2:1b",
            "Llama 3.2 1B",
            128_000,
            8_192,
            0.0,
            0.0,
            false,
            true,
            false,
        ),
        m(
            "ollama",
            "llama3.1:8b",
            "Llama 3.1 8B",
            128_000,
            8_192,
            0.0,
            0.0,
            false,
            true,
            false,
        ),
        m(
            "ollama",
            "mistral:7b",
            "Mistral 7B",
            32_768,
            8_192,
            0.0,
            0.0,
            false,
            true,
            false,
        ),
        m(
            "ollama",
            "gemma2:9b",
            "Gemma 2 9B",
            8_192,
            8_192,
            0.0,
            0.0,
            false,
            true,
            false,
        ),
        m(
            "ollama",
            "codellama:7b",
            "CodeLlama 7B",
            16_384,
            8_192,
            0.0,
            0.0,
            false,
            true,
            false,
        ),
        m(
            "ollama",
            "nomic-embed-text-v2-moe-GGUF",
            "Nomic Embed Text",
            8_192,
            0,
            0.0,
            0.0,
            false,
            false,
            true,
        ),
        // Lemonade
        m(
            "lemonade",
            "lemonade-optimus",
            "Lemonade Optimus",
            128_000,
            16_384,
            0.0,
            0.0,
            true,
            true,
            false,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_seed_and_list() {
        let catalog = ModelCatalog::in_memory().await.unwrap();
        let models = catalog.list_models(None, false).await;
        assert!(!models.is_empty());
    }

    #[tokio::test]
    async fn test_list_by_provider() {
        let catalog = ModelCatalog::in_memory().await.unwrap();
        let models = catalog.list_models(Some("openai"), false).await;
        assert!(!models.is_empty());
        assert!(models.iter().all(|m| m.provider == "openai"));
    }

    #[tokio::test]
    async fn test_get_model() {
        let catalog = ModelCatalog::in_memory().await.unwrap();
        let info = catalog.get_model("openai", "gpt-4o").await;
        assert!(info.is_some());
        let info = info.unwrap();
        assert_eq!(info.input_price_per_1m_usd, 5.0);
        assert!(info.supports_vision);
    }

    #[tokio::test]
    async fn test_get_pricing() {
        let catalog = ModelCatalog::in_memory().await.unwrap();
        let (inp, out) = catalog
            .get_pricing("anthropic", "claude-3-5-sonnet-20241022")
            .await;
        assert_eq!(inp, 3.0);
        assert_eq!(out, 15.0);
    }

    #[tokio::test]
    async fn test_upsert_custom_model() {
        let catalog = ModelCatalog::in_memory().await.unwrap();
        let custom = m(
            "ollama-jo3",
            "llama3.2:3b",
            "Llama 3.2 3B",
            128_000,
            8_192,
            0.0,
            0.0,
            false,
            true,
            false,
        );
        catalog.upsert_model(&custom).await.unwrap();
        let found = catalog.get_model("ollama-jo3", "llama3.2:3b").await;
        assert!(found.is_some());
    }
}
