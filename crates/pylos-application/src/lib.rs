#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod batching;
pub mod budget_plugin;
pub mod cache_aligner;
pub mod config_store;
pub mod guardrails;
pub mod mcp;
pub mod mem0_plugin;
#[cfg(feature = "memgraph")]
pub mod memory_plugin;
pub mod otel_plugin;
pub mod prefix_cache;
pub mod prompt_registry;
pub mod provider_availability;
pub mod quota_guard;
pub mod rag_plugin;
pub mod rate_limit_plugin;
pub mod semantic_cache;
pub mod smart_router;
pub mod structured_output;
pub mod use_cases;

pub use mcp::{
    create_mcp_client, execute_tool_calls_parallel, execute_web_fetch, is_tool_allowed,
    run_agent_loop, web_fetch_tool_def, AgentLoopConfig, HttpMcpClient, McpClient, McpToolRegistry,
    StdioMcpClient, StreamToolCallAccumulator, WEB_FETCH_TOOL_NAME,
};

pub use batching::BatchingPlugin;
pub use budget_plugin::BudgetPlugin;
pub use cache_aligner::CacheAlignerPlugin;
pub use config_store::ConfigStore;
pub use guardrails::GuardrailsPlugin;
pub use mem0_plugin::Mem0Plugin;
#[cfg(feature = "memgraph")]
pub use memory_plugin::MemoryPlugin;
pub use otel_plugin::{OtelConfig, OtelPlugin};
pub use prefix_cache::PrefixCachePlugin;
pub use prompt_registry::PromptRegistryPlugin;
pub use provider_availability::{
    AvailabilityStatus, BillingTier, FinOpsRoutingStats, ProviderAvailabilityService,
    ProviderStatusSnapshot,
};
pub use quota_guard::{AccountQuotaSnapshot, QuotaGuard, QuotaOverview, QuotaStatus, RequestPriority};
pub use rag_plugin::{RagConfig, RagPlugin};
pub use rate_limit_plugin::RateLimitPlugin;
pub use semantic_cache::SemanticCachePlugin;
pub use smart_router::SmartRouterPlugin;
pub use structured_output::StructuredOutputPlugin;
pub use use_cases::InferenceOrchestrator;

pub fn init() {
    tracing::debug!("Initializing Pylos Application");
}
