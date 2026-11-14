use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use tracing::info;

use super::db_pool::{DbPool, DbRow, DbValue};

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
    pool: DbPool,
}

impl ModelCatalog {
    pub async fn open(db_path: &Path) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_sqlite(db_path, "model_catalog", 4).await?;
        let catalog = Self { pool };
        catalog.pool.run_migrations().await?;
        catalog.seed_builtin_models().await;
        info!(path = %db_path.display(), "Model catalog opened (SQLite)");
        Ok(catalog)
    }

    pub async fn open_postgres(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = DbPool::open_postgres(database_url, "model_catalog").await?;
        let catalog = Self { pool };
        catalog.pool.run_migrations().await?;
        catalog.seed_builtin_models().await;
        info!("Model catalog opened (PostgreSQL)");
        Ok(catalog)
    }

    pub async fn from_pool(pool: DbPool) -> Result<Self, sqlx::Error> {
        let catalog = Self { pool };
        catalog.pool.run_migrations().await?;
        catalog.seed_builtin_models().await;
        Ok(catalog)
    }

    pub async fn in_memory() -> Result<Self, sqlx::Error> {
        let pool = DbPool::in_memory(2).await?;
        let ddls = [
            r#"CREATE TABLE IF NOT EXISTS model_catalog (
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
            )"#,
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_catalog_uniq ON model_catalog(provider, model_id)",
            r#"CREATE TABLE IF NOT EXISTS model_health (
                id              TEXT PRIMARY KEY,
                provider        TEXT NOT NULL,
                model_id        TEXT NOT NULL,
                health_status   TEXT NOT NULL DEFAULT 'none',
                error_details   TEXT,
                last_check_ms   INTEGER,
                last_success_ms INTEGER
            )"#,
            r#"CREATE TABLE IF NOT EXISTS pricing_reload_status (
                id                INTEGER PRIMARY KEY,
                source_url        TEXT NOT NULL,
                last_reload_ms    INTEGER,
                models_count      INTEGER NOT NULL DEFAULT 0,
                periodic_schedule TEXT
            )"#,
        ];
        for ddl in ddls {
            pool.execute(ddl, vec![]).await?;
        }
        let catalog = Self { pool };
        catalog.seed_builtin_models().await;
        Ok(catalog)
    }

    pub async fn list_models(
        &self,
        provider: Option<&str>,
        include_deprecated: bool,
    ) -> Vec<ModelInfo> {
        let mut sql = String::from("SELECT * FROM model_catalog WHERE 1=1");
        let mut params: Vec<DbValue> = vec![];

        if let Some(p) = provider {
            sql.push_str(&format!(" AND provider = ${}", params.len() + 1));
            params.push(DbValue::Text(p.to_string()));
        }
        if !include_deprecated {
            sql.push_str(&format!(
                " AND is_deprecated = ${} AND enabled = ${}",
                params.len() + 1,
                params.len() + 2
            ));
            params.push(DbValue::Bool(false));
            params.push(DbValue::Bool(true));
        }
        sql.push_str(" ORDER BY provider, model_id");

        self.pool
            .fetch_all(&sql, params)
            .await
            .unwrap_or_default()
            .iter()
            .map(row_to_model_info)
            .collect()
    }

    pub async fn get_model(&self, provider: &str, model_id: &str) -> Option<ModelInfo> {
        self.pool
            .fetch_optional(
                "SELECT * FROM model_catalog WHERE provider = $1 AND model_id = $2",
                vec![
                    DbValue::Text(provider.to_string()),
                    DbValue::Text(model_id.to_string()),
                ],
            )
            .await
            .ok()
            .flatten()
            .as_ref()
            .map(row_to_model_info)
    }

    pub async fn get_pricing(&self, provider: &str, model_id: &str) -> (f64, f64) {
        if let Some(info) = self.get_model(provider, model_id).await {
            return (info.input_price_per_1m_usd, info.output_price_per_1m_usd);
        }
        (1.0, 3.0)
    }

    pub async fn upsert_model(&self, model: &ModelInfo) -> Result<(), sqlx::Error> {
        let now = now_ms();
        let model_access_groups_val = model
            .model_access_groups
            .as_ref()
            .and_then(|v| serde_json::to_value(v).ok());
        let guardrails_val = model
            .guardrails
            .as_ref()
            .and_then(|v| serde_json::to_value(v).ok());
        let tags_val = model
            .tags
            .as_ref()
            .and_then(|v| serde_json::to_value(v).ok());

        self.pool
            .execute(
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
                vec![
                    DbValue::Text(model.id.clone()),
                    DbValue::Text(model.provider.clone()),
                    DbValue::Text(model.model_id.clone()),
                    DbValue::OptText(model.display_name.clone()),
                    DbValue::I64(i64::from(model.context_window)),
                    DbValue::I64(i64::from(model.max_output_tokens)),
                    DbValue::F64(model.input_price_per_1m_usd),
                    DbValue::F64(model.output_price_per_1m_usd),
                    DbValue::Bool(model.supports_vision),
                    DbValue::Bool(model.supports_tools),
                    DbValue::Bool(model.supports_streaming),
                    DbValue::Bool(model.supports_embeddings),
                    DbValue::Bool(model.is_deprecated),
                    DbValue::Bool(model.enabled),
                    DbValue::I64(now),
                    DbValue::OptText(model.api_base.clone()),
                    DbValue::OptI64(model.tpm.map(i64::from)),
                    DbValue::OptI64(model.rpm.map(i64::from)),
                    DbValue::OptI64(model.max_retries.map(i64::from)),
                    DbValue::OptI64(model.timeout_secs.map(i64::from)),
                    DbValue::OptI64(model.stream_timeout_secs.map(i64::from)),
                    DbValue::OptJson(model_access_groups_val),
                    DbValue::OptJson(guardrails_val),
                    DbValue::OptJson(tags_val),
                    DbValue::OptJson(model.provider_params.clone()),
                    DbValue::OptText(model.organization_id.clone()),
                ],
            )
            .await?;
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
        let rows_affected = self
            .pool
            .execute(
                "DELETE FROM model_catalog WHERE provider = $1 AND model_id = $2",
                vec![
                    DbValue::Text(provider.to_string()),
                    DbValue::Text(model_id.to_string()),
                ],
            )
            .await?;
        Ok(rows_affected > 0)
    }

    pub async fn delete_models_by_provider(&self, provider: &str) -> Result<u64, sqlx::Error> {
        self.pool
            .execute(
                "DELETE FROM model_catalog WHERE provider = $1",
                vec![DbValue::Text(provider.to_string())],
            )
            .await
    }

    pub async fn get_pricing_status(&self) -> Result<PricingReloadStatus, sqlx::Error> {
        let row = self
            .pool
            .fetch_optional(
                "SELECT source_url, last_reload_ms, models_count, periodic_schedule FROM pricing_reload_status WHERE id = 1",
                vec![],
            )
            .await?
            .ok_or(sqlx::Error::RowNotFound)?;
        Ok(PricingReloadStatus {
            source_url: row.get_str("source_url"),
            last_reload_ms: row.get_opt_i64("last_reload_ms"),
            models_count: row.get_i64("models_count") as i32,
            periodic_schedule: row.get_opt_str("periodic_schedule"),
        })
    }

    pub async fn update_pricing_status(
        &self,
        source_url: &str,
        last_reload_ms: Option<i64>,
        models_count: i32,
        periodic_schedule: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        self.pool
            .execute(
                r#"
                INSERT INTO pricing_reload_status (id, source_url, last_reload_ms, models_count, periodic_schedule)
                VALUES (1, $1, $2, $3, $4)
                ON CONFLICT(id) DO UPDATE SET
                    source_url = excluded.source_url,
                    last_reload_ms = excluded.last_reload_ms,
                    models_count = excluded.models_count,
                    periodic_schedule = excluded.periodic_schedule
                "#,
                vec![
                    DbValue::Text(source_url.to_string()),
                    DbValue::OptI64(last_reload_ms),
                    DbValue::I64(i64::from(models_count)),
                    DbValue::OptText(periodic_schedule.map(str::to_string)),
                ],
            )
            .await?;
        Ok(())
    }

    pub async fn list_model_health(&self) -> Result<Vec<ModelHealth>, sqlx::Error> {
        let rows = self
            .pool
            .fetch_all(
                "SELECT id, provider, model_id, health_status, error_details, last_check_ms, last_success_ms FROM model_health",
                vec![],
            )
            .await?;
        Ok(rows
            .iter()
            .map(|r| ModelHealth {
                id: r.get_str("id"),
                provider: r.get_str("provider"),
                model_id: r.get_str("model_id"),
                health_status: r
                    .get_opt_str("health_status")
                    .unwrap_or_else(|| "none".to_string()),
                error_details: r.get_opt_str("error_details"),
                last_check_ms: r.get_opt_i64("last_check_ms"),
                last_success_ms: r.get_opt_i64("last_success_ms"),
            })
            .collect())
    }

    pub async fn update_model_health(&self, health: &ModelHealth) -> Result<(), sqlx::Error> {
        self.pool
            .execute(
                r#"
                INSERT INTO model_health (id, provider, model_id, health_status, error_details, last_check_ms, last_success_ms)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT(id) DO UPDATE SET
                    health_status = excluded.health_status,
                    error_details = excluded.error_details,
                    last_check_ms = excluded.last_check_ms,
                    last_success_ms = excluded.last_success_ms
                "#,
                vec![
                    DbValue::Text(health.id.clone()),
                    DbValue::Text(health.provider.clone()),
                    DbValue::Text(health.model_id.clone()),
                    DbValue::Text(health.health_status.clone()),
                    DbValue::OptText(health.error_details.clone()),
                    DbValue::OptI64(health.last_check_ms),
                    DbValue::OptI64(health.last_success_ms),
                ],
            )
            .await?;
        Ok(())
    }
}

fn row_to_model_info(row: &DbRow) -> ModelInfo {
    ModelInfo {
        id: row.get_str("id"),
        provider: row.get_str("provider"),
        model_id: row.get_str("model_id"),
        display_name: row.get_opt_str("display_name"),
        context_window: row.get_i64("context_window") as u32,
        max_output_tokens: row.get_i64("max_output_tokens") as u32,
        input_price_per_1m_usd: row.get_f64("input_price_per_1m_usd"),
        output_price_per_1m_usd: row.get_f64("output_price_per_1m_usd"),
        supports_vision: row.get_bool("supports_vision"),
        supports_tools: row.get_bool("supports_tools"),
        supports_streaming: row.get_bool("supports_streaming"),
        supports_embeddings: row.get_bool("supports_embeddings"),
        is_deprecated: row.get_bool("is_deprecated"),
        enabled: row.get_bool("enabled"),
        api_base: row.get_opt_str("api_base"),
        tpm: row.get_opt_i64("tpm").map(|v| v as u32),
        rpm: row.get_opt_i64("rpm").map(|v| v as u32),
        max_retries: row.get_opt_i64("max_retries").map(|v| v as u32),
        timeout_secs: row.get_opt_i64("timeout_secs").map(|v| v as u32),
        stream_timeout_secs: row.get_opt_i64("stream_timeout_secs").map(|v| v as u32),
        model_access_groups: row.get_json("model_access_groups"),
        guardrails: row.get_json("guardrails"),
        tags: row.get_json("tags"),
        provider_params: row.get_json("provider_params"),
        organization_id: row.get_opt_str("organization_id"),
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
        // Google Antigravity
        m(
            "antigravity",
            "gemini-3.8-flash-medium",
            "Gemini 3.8 Flash Medium (Antigravity)",
            1_048_576,
            65_536,
            0.15,
            0.60,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "gemini-3.8-flash-high",
            "Gemini 3.8 Flash High (Antigravity)",
            1_048_576,
            65_536,
            0.15,
            0.60,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "gemini-3.8-flash-low",
            "Gemini 3.8 Flash Low (Antigravity)",
            1_048_576,
            65_536,
            0.15,
            0.60,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "gemini-3.8-flash",
            "Gemini 3.8 Flash (Antigravity)",
            1_048_576,
            65_536,
            0.15,
            0.60,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "gemini-4-argon-medium",
            "Gemini 4 Argon Medium (Antigravity)",
            2_097_152,
            65_536,
            1.25,
            5.00,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "gemini-4-argon",
            "Gemini 4 Argon (Antigravity)",
            2_097_152,
            65_536,
            1.25,
            5.00,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "claude-opus-4-6-thinking",
            "Claude Opus 4.6 Thinking (Antigravity)",
            500_000,
            32_768,
            15.0,
            75.0,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "claude-opus-5-5",
            "Claude Opus 5.5 (Antigravity)",
            500_000,
            32_768,
            15.0,
            75.0,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "claude-sonnet-4-6",
            "Claude Sonnet 4.6 (Antigravity)",
            500_000,
            32_768,
            3.0,
            15.0,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "claude-sonnet-5-5",
            "Claude Sonnet 5.5 (Antigravity)",
            500_000,
            32_768,
            3.0,
            15.0,
            true,
            true,
            false,
        ),
        m(
            "antigravity",
            "gpt-oss-120b-medium",
            "GPT-OSS 120B Medium (Antigravity)",
            131_072,
            16_384,
            0.20,
            0.80,
            false,
            true,
            false,
        ),
        m(
            "antigravity",
            "gpt-oss-120b",
            "GPT-OSS 120B (Antigravity)",
            131_072,
            16_384,
            0.20,
            0.80,
            false,
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
