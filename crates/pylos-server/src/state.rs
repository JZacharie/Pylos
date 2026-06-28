use std::path::PathBuf;
use std::sync::Arc;

use pylos_application::{
    BudgetPlugin, BudgetStore, ConfigStore, GuardrailsPlugin, InferenceOrchestrator, LogStore,
    McpServerStore, ModelCatalog, OrganizationStore, OtelConfig, PgLogStore, PrefixCachePlugin,
    RateLimitPlugin, RateLimitStore, SearchToolStore, SemanticCachePlugin, StructuredOutputPlugin,
    VirtualKeyStore,
};
use pylos_core::domain::traits::{LlmPlugin, Provider};

use crate::metrics::Metrics;

#[derive(Clone)]
pub enum LogStoreVariant {
    Sqlite(Arc<LogStore>),
    Postgres(Arc<PgLogStore>),
}

impl LogStoreVariant {
    pub async fn push(&self, entry: pylos_application::log_store::LogEntry) {
        match self {
            Self::Sqlite(s) => s.push(entry).await,
            Self::Postgres(s) => s.push(entry).await,
        }
    }

    pub async fn list(
        &self,
        limit: usize,
        offset: usize,
        filter: &pylos_application::log_store::LogFilter,
    ) -> (Vec<pylos_application::log_store::LogEntry>, u64) {
        match self {
            Self::Sqlite(s) => s.list(limit, offset, filter).await,
            Self::Postgres(s) => s.list(limit, offset, filter).await,
        }
    }

    pub async fn stats(
        &self,
        filter: &pylos_application::log_store::LogFilter,
    ) -> pylos_application::log_store::LogStats {
        match self {
            Self::Sqlite(s) => s.stats(filter).await,
            Self::Postgres(s) => s.stats(filter).await,
        }
    }

    pub async fn histogram(
        &self,
        filter: &pylos_application::log_store::LogFilter,
        bucket_secs: i64,
    ) -> Vec<pylos_application::log_store::HistogramBucket> {
        match self {
            Self::Sqlite(s) => s.histogram(filter, bucket_secs).await,
            Self::Postgres(s) => s.histogram(filter, bucket_secs).await,
        }
    }

    pub async fn token_histogram(
        &self,
        filter: &pylos_application::log_store::LogFilter,
        bucket_secs: i64,
    ) -> Vec<pylos_application::log_store::TokenBucket> {
        match self {
            Self::Sqlite(s) => s.token_histogram(filter, bucket_secs).await,
            Self::Postgres(s) => s.token_histogram(filter, bucket_secs).await,
        }
    }

    pub async fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &pylos_application::log_store::LogFilter,
    ) -> (Vec<pylos_application::log_store::LogEntry>, u64) {
        match self {
            Self::Sqlite(s) => s.list_guardrails(limit, offset, filter).await,
            Self::Postgres(s) => s.list_guardrails(limit, offset, filter).await,
        }
    }

    pub async fn guardrails_stats(
        &self,
        filter: &pylos_application::log_store::LogFilter,
    ) -> pylos_application::log_store::GuardrailsBreakdown {
        match self {
            Self::Sqlite(s) => s.guardrails_stats(filter).await,
            Self::Postgres(s) => s.guardrails_stats(filter).await,
        }
    }

    pub async fn guardrails_breakdown(
        &self,
        filter: &pylos_application::log_store::LogFilter,
    ) -> pylos_application::log_store::GuardrailsBreakdown {
        match self {
            Self::Sqlite(s) => s.guardrails_breakdown(filter).await,
            Self::Postgres(s) => s.guardrails_breakdown(filter).await,
        }
    }

    pub async fn guardrails_timeline(
        &self,
        filter: &pylos_application::log_store::LogFilter,
        bucket_secs: i64,
    ) -> Vec<pylos_application::log_store::GuardrailsTimeline> {
        match self {
            Self::Sqlite(s) => s.guardrails_timeline(filter, bucket_secs).await,
            Self::Postgres(s) => s.guardrails_timeline(filter, bucket_secs).await,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub orchestrator: Arc<InferenceOrchestrator>,
    pub config_store: Arc<ConfigStore>,
    pub metrics: Arc<Metrics>,
    pub vk_registry: Arc<pylos_core::domain::virtual_key::VirtualKeyRegistry>,
    pub log_store: LogStoreVariant,
    pub model_catalog: Arc<ModelCatalog>,
    pub budget_store: Arc<BudgetStore>,
    pub rate_limit_store: Arc<RateLimitStore>,
    pub vk_store: Arc<VirtualKeyStore>,
    pub system_prompt_store: Arc<pylos_application::SystemPromptStore>,
    pub org_store: Arc<OrganizationStore>,
    pub search_tool_store: Arc<SearchToolStore>,
    pub mcp_server_store: Arc<McpServerStore>,
    pub admin_key_hash: Arc<tokio::sync::RwLock<Option<String>>>,
    pub setup_required: Arc<tokio::sync::RwLock<bool>>,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_redirect_uri: Option<String>,
    pub jwt_secret: String,
    pub allowed_origins: Vec<String>,
    pub inference_semaphore: Arc<tokio::sync::Semaphore>,
    pub max_concurrency: usize,
    pub max_queue_size: usize,
    pub queue_timeout_ms: u64,
}

impl AppState {
    pub async fn from_config(config_path: Option<PathBuf>) -> anyhow::Result<Self> {
        Self::from_config_with_dir(config_path, None).await
    }

    async fn init_postgres_stores(
        db_url: &str,
        config_store: &ConfigStore,
    ) -> anyhow::Result<(
        LogStoreVariant,
        Arc<ModelCatalog>,
        Arc<BudgetStore>,
        Arc<RateLimitStore>,
        Arc<VirtualKeyStore>,
        Arc<pylos_application::SystemPromptStore>,
        Arc<OrganizationStore>,
        Arc<SearchToolStore>,
        Arc<McpServerStore>,
    )> {
        let db_scheme = db_url.split(':').next().unwrap_or("unknown");
        tracing::info!(database_url = %format!("{}://***@***/***", db_scheme), "Using PostgreSQL for all stores");

        let pg_log = Arc::new(
            PgLogStore::new(db_url, config_store.get().await.server.log_retention_days)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to connect PostgreSQL log store: {}", e))?,
        );
        let pg_catalog = Arc::new(
            ModelCatalog::open_postgres(db_url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open PostgreSQL model catalog: {}", e))?,
        );
        let pg_budget = Arc::new(
            BudgetStore::open_postgres(db_url)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open PostgreSQL budget store: {}", e))?,
        );
        let pg_rl =
            Arc::new(RateLimitStore::open_postgres(db_url).await.map_err(|e| {
                anyhow::anyhow!("Failed to open PostgreSQL rate limit store: {}", e)
            })?);
        let pg_vk =
            Arc::new(VirtualKeyStore::open_postgres(db_url).await.map_err(|e| {
                anyhow::anyhow!("Failed to open PostgreSQL virtual key store: {}", e)
            })?);
        let pg_prompts = Arc::new(
            pylos_application::SystemPromptStore::open_postgres(db_url)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to open PostgreSQL system prompt store: {}", e)
                })?,
        );
        let pg_org = Arc::new(
            OrganizationStore::open_postgres(db_url)
                .await
                .map_err(|e| {
                    anyhow::anyhow!("Failed to open PostgreSQL organization store: {}", e)
                })?,
        );
        let pg_search_tool =
            Arc::new(SearchToolStore::open_postgres(db_url).await.map_err(|e| {
                anyhow::anyhow!("Failed to open PostgreSQL search tool store: {}", e)
            })?);
        let pg_mcp =
            Arc::new(McpServerStore::open_postgres(db_url).await.map_err(|e| {
                anyhow::anyhow!("Failed to open PostgreSQL MCP server store: {}", e)
            })?);

        if let Err(e) = config_store.init_database(db_url).await {
            tracing::warn!(error = %e, "Failed to initialize PostgreSQL config store");
        }

        Ok((
            LogStoreVariant::Postgres(pg_log),
            pg_catalog,
            pg_budget,
            pg_rl,
            pg_vk,
            pg_prompts,
            pg_org,
            pg_search_tool,
            pg_mcp,
        ))
    }

    async fn init_sqlite_stores(
        data_dir: Option<PathBuf>,
        config_store: &ConfigStore,
    ) -> anyhow::Result<(
        LogStoreVariant,
        Arc<ModelCatalog>,
        Arc<BudgetStore>,
        Arc<RateLimitStore>,
        Arc<VirtualKeyStore>,
        Arc<pylos_application::SystemPromptStore>,
        Arc<OrganizationStore>,
        Arc<SearchToolStore>,
        Arc<McpServerStore>,
    )> {
        let data_dir = data_dir.unwrap_or_else(|| {
            std::env::var("PYLOS_DATA_DIR")
                .ok()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        });
        std::fs::create_dir_all(&data_dir).ok();

        let log_db_path = data_dir.join("pylos-logs.db");
        tracing::info!(path = %log_db_path.display(), "Log store path (SQLite)");
        let sqlite_log = Arc::new(LogStore::new(
            Some(log_db_path),
            config_store.get().await.server.log_retention_days,
            10_000,
        ));

        let catalog_db_path = data_dir.join("pylos-catalog.db");
        let sqlite_catalog = Arc::new(
            ModelCatalog::open(&catalog_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open model catalog: {}", e))?,
        );

        let budget_db_path = data_dir.join("pylos-budget.db");
        let sqlite_budget = Arc::new(
            BudgetStore::open(&budget_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open budget store: {}", e))?,
        );

        let rl_db_path = data_dir.join("pylos-ratelimit.db");
        let sqlite_rl = Arc::new(
            RateLimitStore::open(&rl_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open rate limit store: {}", e))?,
        );

        let vk_db_path = data_dir.join("pylos-virtualkeys.db");
        let sqlite_vk = Arc::new(
            VirtualKeyStore::open(&vk_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open virtual key store: {}", e))?,
        );

        let org_db_path = data_dir.join("pylos-org.db");
        let sqlite_org = Arc::new(
            OrganizationStore::open(&org_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open organization store: {}", e))?,
        );

        let prompts_db_path = data_dir.join("pylos-prompts.db");
        let sqlite_prompts = Arc::new(
            pylos_application::SystemPromptStore::open(&prompts_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open system prompt store: {}", e))?,
        );

        let st_db_path = data_dir.join("pylos-searchtools.db");
        let sqlite_search_tool = Arc::new(
            SearchToolStore::open(&st_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open search tool store: {}", e))?,
        );

        let mcp_db_path = data_dir.join("pylos-mcp.db");
        let sqlite_mcp = Arc::new(
            McpServerStore::open(&mcp_db_path)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to open MCP server store: {}", e))?,
        );

        let config_db_path = data_dir.join("pylos-config.db");
        let sqlite_config_db_url =
            format!("sqlite://{}?mode=rwc", config_db_path.to_string_lossy());
        if let Err(e) = config_store.init_database(&sqlite_config_db_url).await {
            tracing::warn!(error = %e, "Failed to initialize SQLite config store");
        }

        Ok((
            LogStoreVariant::Sqlite(sqlite_log),
            sqlite_catalog,
            sqlite_budget,
            sqlite_rl,
            sqlite_vk,
            sqlite_prompts,
            sqlite_org,
            sqlite_search_tool,
            sqlite_mcp,
        ))
    }

    fn find_provider_for_embedding(
        providers: &[(
            Arc<dyn Provider>,
            pylos_core::domain::provider::ProviderConfig,
        )],
        model: &str,
    ) -> Option<(
        Arc<dyn Provider>,
        pylos_core::domain::provider::ProviderConfig,
    )> {
        providers
            .iter()
            .find(|(_, config)| {
                if config.allowed_models.is_empty() {
                    return false;
                }
                config.allowed_models.iter().any(|allowed| {
                    allowed == "*"
                        || allowed == model
                        || (allowed.contains('*')
                            && model.starts_with(allowed.trim_end_matches('*')))
                })
            })
            .cloned()
    }

    fn register_plugins(
        cfg: &pylos_core::domain::config::PylosConfig,
        budget_store: &Arc<BudgetStore>,
        rate_limit_store: &Arc<RateLimitStore>,
        config_store: &Arc<ConfigStore>,
        providers: &[(
            Arc<dyn Provider>,
            pylos_core::domain::provider::ProviderConfig,
        )],
    ) -> Vec<Arc<dyn LlmPlugin>> {
        let mut plugins: Vec<Arc<dyn LlmPlugin>> = Vec::new();
        plugins.push(Arc::new(pylos_application::CacheAlignerPlugin::new()));

        plugins.push(Arc::new(BudgetPlugin::new(Arc::clone(budget_store))));
        tracing::info!("Budget plugin enabled");

        // StructuredOutputPlugin — check for optional correction config
        {
            let so_cfg = cfg
                .plugins
                .iter()
                .find(|p| p.name == "structured_output" && p.enabled);
            if let Some(so_cfg) = so_cfg {
                let correction_model = so_cfg
                    .config
                    .get("correction_model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("gpt-4o-mini")
                    .to_string();
                let max_retries = so_cfg
                    .config
                    .get("max_retries")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(1) as u32;
                let so_pylos_base_url = std::env::var("PYLOS_BASE_URL")
                    .unwrap_or_else(|_| "http://localhost:3000".to_string());
                let so_pylos_api_key = std::env::var("PYLOS_API_KEY").ok();
                plugins.push(Arc::new(StructuredOutputPlugin::with_correction(
                    correction_model.clone(),
                    max_retries,
                    so_pylos_base_url,
                    so_pylos_api_key,
                )));
                tracing::info!(name = "structured_output", correction_model = %correction_model, max_retries = max_retries, "Structured Output plugin with correction enabled");
            } else {
                plugins.push(Arc::new(StructuredOutputPlugin::new()));
                tracing::info!("Structured Output plugin enabled (no correction)");
            }
        }

        let has_rl = cfg
            .governance
            .rate_limits
            .iter()
            .any(|r| r.request_max_limit > 0 || r.token_max_limit > 0);
        if has_rl {
            plugins.push(Arc::new(RateLimitPlugin::new(Arc::clone(rate_limit_store))));
            tracing::info!("Rate limit plugin enabled");
        }

        for plugin_cfg in &cfg.plugins {
            if !plugin_cfg.enabled {
                continue;
            }
            match plugin_cfg.name.as_str() {
                "rag" => {
                    let qdrant_url = std::env::var("QDRANT_URL")
                        .unwrap_or_else(|_| "http://qdrant:6333".to_string());

                    // Build RagConfig from plugin config, falling back to defaults
                    let mut rag_config: pylos_application::RagConfig =
                        serde_json::from_value(plugin_cfg.config.clone()).unwrap_or_default();

                    // Allow env var overrides for backward compatibility
                    if let Ok(model) = std::env::var("PYLOS_EMBEDDING_MODEL") {
                        rag_config.embedding_model = model;
                    }
                    if let Ok(model) = std::env::var("PYLOS_MODEL") {
                        rag_config.pylos_model = model;
                    }

                    let rag_embed_provider =
                        Self::find_provider_for_embedding(providers, &rag_config.embedding_model);

                    plugins.push(Arc::new(pylos_application::RagPlugin::new(
                        qdrant_url,
                        rag_config,
                        rag_embed_provider,
                    )));
                    tracing::info!(name = "rag", "RAG plugin enabled");
                }
                "otel" => {
                    let otel_cfg = OtelConfig::from_plugin_config(&plugin_cfg.config);
                    plugins.push(Arc::new(otel_cfg.build_plugin()));
                    tracing::info!(name = "otel", "Plugin registered");
                }
                "semantic_cache" => {
                    let qdrant_url = std::env::var("QDRANT_URL")
                        .unwrap_or_else(|_| "http://qdrant:6333".to_string());
                    let collection_name = plugin_cfg
                        .config
                        .get("collection_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("pylos-cache")
                        .to_string();
                    let embedding_model = plugin_cfg
                        .config
                        .get("embedding_model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("nomic-embed-text-v2-moe-GGUF")
                        .to_string();
                    let similarity_threshold = plugin_cfg
                        .config
                        .get("similarity_threshold")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.9);
                    let ttl_secs = plugin_cfg
                        .config
                        .get("ttl_secs")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(86400);
                    let sc_embed_provider =
                        Self::find_provider_for_embedding(providers, &embedding_model);
                    plugins.push(Arc::new(SemanticCachePlugin::new(
                        qdrant_url,
                        collection_name,
                        embedding_model,
                        similarity_threshold,
                        ttl_secs,
                        sc_embed_provider,
                    )));
                    tracing::info!(name = "semantic_cache", "Semantic Cache plugin enabled");
                }
                "memory" => {
                    let memgraph_url = std::env::var("MEMGRAPH_URL")
                        .unwrap_or_else(|_| "127.0.0.1:7687".to_string());

                    // Since register_plugins is sync and MemoryPlugin::new is async, we use block_on or spawn.
                    // Because register_plugins is called in an async context (from_config_with_dir), we can't easily await inside sync.
                    // Let's create the runtime block here to await the connection:
                    if let Ok(handle) = tokio::runtime::Handle::try_current() {
                        let plugin_res = tokio::task::block_in_place(|| {
                            handle.block_on(async {
                                pylos_application::MemoryPlugin::new(memgraph_url).await
                            })
                        });

                        if let Ok(plugin) = plugin_res {
                            plugins.push(Arc::new(plugin));
                            tracing::info!(
                                name = "memory",
                                "Cross-Agent Memory (Memgraph) plugin enabled"
                            );
                        } else {
                            tracing::error!("Failed to connect to Memgraph");
                        }
                    }
                }
                "guardrails" => {
                    plugins.push(Arc::new(GuardrailsPlugin::new(Arc::clone(config_store))));
                    tracing::info!(name = "guardrails", "Guardrails plugin enabled");
                }
                "mem0" => {
                    let sidecar_url = std::env::var("MEM0_SIDECAR_URL")
                        .unwrap_or_else(|_| "http://mem0-sidecar:7577".to_string());
                    plugins.push(Arc::new(pylos_application::Mem0Plugin::new(sidecar_url)));
                    tracing::info!(name = "mem0", "Mem0 plugin enabled");
                }
                "prefix_cache" => {
                    let ttl_secs = plugin_cfg
                        .config
                        .get("ttl_secs")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(3600);
                    let max_capacity = plugin_cfg
                        .config
                        .get("max_capacity")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(10_000);
                    let min_prefix_len = plugin_cfg
                        .config
                        .get("min_prefix_len")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(1) as usize;
                    plugins.push(Arc::new(PrefixCachePlugin::new(
                        ttl_secs,
                        max_capacity,
                        min_prefix_len,
                    )));
                    tracing::info!(name = "prefix_cache", "Prefix Cache plugin enabled");
                }
                "batching" => {
                    let mode = plugin_cfg
                        .config
                        .get("mode")
                        .and_then(|v| v.as_str())
                        .unwrap_or("coalescing");
                    match mode {
                        "async_batch" => {
                            let openai_base_url = plugin_cfg
                                .config
                                .get("openai_base_url")
                                .and_then(|v| v.as_str())
                                .unwrap_or("https://api.openai.com/v1")
                                .to_string();
                            let api_key = plugin_cfg
                                .config
                                .get("api_key")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let batch_model = plugin_cfg
                                .config
                                .get("batch_model")
                                .and_then(|v| v.as_str())
                                .unwrap_or("gpt-4o-mini")
                                .to_string();
                            let poll_interval_secs = plugin_cfg
                                .config
                                .get("poll_interval_secs")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(60);
                            plugins.push(Arc::new(
                                pylos_application::BatchingPlugin::with_async_batch(
                                    openai_base_url,
                                    api_key,
                                    batch_model,
                                    poll_interval_secs,
                                ),
                            ));
                            tracing::info!(
                                name = "batching",
                                mode = "async_batch",
                                "Plugin registered"
                            );
                        }
                        _ => {
                            let delay_ms = plugin_cfg
                                .config
                                .get("delay_ms")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(200);
                            let max_batch_size = plugin_cfg
                                .config
                                .get("max_batch_size")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(50)
                                as usize;
                            plugins.push(Arc::new(
                                pylos_application::BatchingPlugin::with_coalescing(
                                    delay_ms,
                                    max_batch_size,
                                ),
                            ));
                            tracing::info!(
                                name = "batching",
                                mode = "coalescing",
                                delay_ms = delay_ms,
                                "Plugin registered"
                            );
                        }
                    }
                }
                name => tracing::debug!(name = %name, "Unknown plugin, skipping"),
            }
        }
        plugins
    }

    async fn register_virtual_keys(
        cfg: &pylos_core::domain::config::PylosConfig,
        vk_store: &VirtualKeyStore,
    ) -> Arc<pylos_core::domain::virtual_key::VirtualKeyRegistry> {
        let registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
        for vk_cfg in &cfg.governance.virtual_keys {
            if !vk_cfg.is_active {
                continue;
            }
            let key_value = vk_cfg
                .value
                .as_ref()
                .and_then(|v| v.resolve())
                .unwrap_or_else(|| {
                    format!("sk-pylos-{}", vk_cfg.id.replace(' ', "-").to_lowercase())
                });
            let rate_limit = cfg
                .governance
                .rate_limits
                .iter()
                .find(|rl| Some(&rl.id) == vk_cfg.rate_limit_id.as_ref())
                .map(|rl| rl.request_max_limit)
                .unwrap_or(0);
            let vk = pylos_core::domain::virtual_key::VirtualKey::new(key_value, &vk_cfg.name)
                .with_rpm(rate_limit);
            registry.register(vk).await;
        }
        if let Ok(db_vks) = vk_store.list_keys().await {
            for vk_cfg in db_vks {
                if !vk_cfg.is_active {
                    continue;
                }
                let key_value = vk_cfg
                    .value
                    .as_ref()
                    .map(|v| v.resolve().unwrap_or_default())
                    .unwrap_or_else(|| {
                        format!("sk-pylos-{}", vk_cfg.id.replace(' ', "-").to_lowercase())
                    });
                let rate_limit = cfg
                    .governance
                    .rate_limits
                    .iter()
                    .find(|rl| Some(&rl.id) == vk_cfg.rate_limit_id.as_ref())
                    .map(|rl| rl.request_max_limit)
                    .unwrap_or(0);
                let vk = pylos_core::domain::virtual_key::VirtualKey::new(key_value, &vk_cfg.name)
                    .with_rpm(rate_limit);
                registry.register(vk).await;
            }
        }
        registry
    }

    pub async fn from_config_with_dir(
        config_path: Option<PathBuf>,
        data_dir: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        let config_store = ConfigStore::load(config_path.as_deref()).await?;
        let config_store = Arc::new(config_store);

        let cfg = config_store.get().await;
        let providers = config_store.runtime_providers().await;

        let database_url = cfg.server.database_url.as_ref().and_then(|e| e.resolve());

        let (
            log_store,
            model_catalog,
            budget_store,
            rate_limit_store,
            vk_store,
            system_prompt_store,
            org_store,
            search_tool_store,
            mcp_server_store,
        ) = if let Some(ref db_url) = database_url {
            Self::init_postgres_stores(db_url, &config_store).await?
        } else {
            Self::init_sqlite_stores(data_dir, &config_store).await?
        };

        for budget_cfg in &cfg.governance.budgets {
            if let Some(vk_id) = &budget_cfg.virtual_key_id {
                if let Err(e) = budget_store.upsert_budget(vk_id, budget_cfg).await {
                    tracing::warn!(budget_id = %budget_cfg.id, error = %e, "Failed to init budget");
                }
            }
        }

        for vk_cfg in &cfg.governance.virtual_keys {
            if let Some(rl_id) = &vk_cfg.rate_limit_id {
                if let Some(rl_cfg) = cfg.governance.rate_limits.iter().find(|r| &r.id == rl_id) {
                    if let Err(e) = rate_limit_store.upsert_rate_limit(&vk_cfg.id, rl_cfg).await {
                        tracing::warn!(vk_id = %vk_cfg.id, error = %e, "Failed to init rate limit");
                    }
                }
            }
        }

        let plugins = Self::register_plugins(
            &cfg,
            &budget_store,
            &rate_limit_store,
            &config_store,
            &providers,
        );
        let orchestrator = Arc::new(InferenceOrchestrator::new(providers, plugins));
        let metrics = Arc::new(Metrics::new());
        let vk_registry = Self::register_virtual_keys(&cfg, &vk_store).await;

        let max_concurrency = cfg.server.queuing.max_concurrency;
        let max_queue_size = cfg.server.queuing.max_queue_size;
        let queue_timeout_ms = cfg.server.queuing.queue_timeout_ms;
        let inference_semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrency));

        let env_key = std::env::var("PYLOS_ADMIN_KEY").ok();
        let (resolved_hash, resolved_setup) = if let Some(key) = env_key {
            if !key.trim().is_empty() {
                (Some(hash_sha256(&key)), false)
            } else {
                match config_store.get_admin_key_hash().await.unwrap_or(None) {
                    Some(hash) => (Some(hash), false),
                    None => (None, true),
                }
            }
        } else {
            match config_store.get_admin_key_hash().await.unwrap_or(None) {
                Some(hash) => (Some(hash), false),
                None => (None, true),
            }
        };

        let admin_key_hash = Arc::new(tokio::sync::RwLock::new(resolved_hash));
        let setup_required = Arc::new(tokio::sync::RwLock::new(resolved_setup));

        Ok(Self {
            orchestrator,
            config_store,
            metrics,
            vk_registry,
            log_store,
            model_catalog,
            budget_store,
            rate_limit_store,
            vk_store,
            system_prompt_store,
            org_store,
            search_tool_store,
            mcp_server_store,
            admin_key_hash,
            setup_required,
            google_client_id: std::env::var("GOOGLE_CLIENT_ID").ok(),
            google_client_secret: std::env::var("GOOGLE_CLIENT_SECRET").ok(),
            google_redirect_uri: std::env::var("GOOGLE_REDIRECT_URI").ok(),
            jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| {
                // Generates a random session JWT key if none is provided
                let mut s = String::new();
                for _ in 0..32 {
                    s.push(fastrand::alphanumeric());
                }
                s
            }),
            allowed_origins: cfg.server.allowed_origins.clone(),
            inference_semaphore,
            max_concurrency,
            max_queue_size,
            queue_timeout_ms,
        })
    }
}

pub fn hash_sha256(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}
