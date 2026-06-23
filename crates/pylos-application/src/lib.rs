pub mod budget_store;
pub mod cache_aligner;
pub mod config_store;
pub(crate) mod db_pool;
pub mod guardrails;
pub mod log_store;
pub mod mem0_plugin;
pub mod memory_plugin;
pub mod model_catalog;
pub mod otel_plugin;
pub mod pg_log_store;
pub mod rag_plugin;
pub mod rate_limit_store;
pub mod semantic_cache;
pub mod system_prompt_store;

pub mod batching;
pub mod mcp_server_store;
pub mod organization_store;
pub mod prefix_cache;
pub mod prompt_registry;
pub mod search_tool_store;
pub mod structured_output;
pub mod use_cases;
pub mod virtual_key_store;

pub use mcp_server_store::McpServerStore;
pub use organization_store::OrganizationStore;
pub use search_tool_store::SearchToolStore;
pub use system_prompt_store::SystemPromptStore;
pub use virtual_key_store::VirtualKeyStore;

pub use batching::BatchingPlugin;
pub use budget_store::{BudgetPlugin, BudgetStore};
pub use cache_aligner::CacheAlignerPlugin;
pub use config_store::ConfigStore;
pub use guardrails::GuardrailsPlugin;
pub use log_store::{
    build_log_entry, build_log_entry_full, generate_log_id, now_ms, GuardrailsBreakdown,
    GuardrailsTimeline, KeywordCount, LogEntry, LogFilter, LogStats, LogStatus, LogStore,
};
pub use mem0_plugin::Mem0Plugin;
pub use memory_plugin::MemoryPlugin;
pub use model_catalog::{ModelCatalog, ModelHealth, ModelInfo, PricingReloadStatus};
pub use otel_plugin::{OtelConfig, OtelPlugin};
pub use pg_log_store::PgLogStore;
pub use prefix_cache::PrefixCachePlugin;
pub use prompt_registry::PromptRegistryPlugin;
pub use rag_plugin::{RagConfig, RagModelRoute, RagPlugin};
pub use rate_limit_store::{RateLimitPlugin, RateLimitStatus, RateLimitStore};
pub use semantic_cache::SemanticCachePlugin;

pub use structured_output::StructuredOutputPlugin;
pub use use_cases::InferenceOrchestrator;

pub fn init() {
    tracing::debug!("Initializing Pylos Application");
}
