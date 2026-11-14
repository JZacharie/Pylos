#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]

pub mod persistence;
pub mod providers;

pub use persistence::*;
pub use providers::AnthropicProvider;
pub use providers::AntigravityProvider;
pub use providers::AzureProvider;
pub use providers::BedrockProvider;
pub use providers::CohereProvider;
pub use providers::GeminiProvider;
pub use providers::OpenAIProvider;

pub fn init() {
    tracing::debug!("Initializing Pylos Infrastructure");
}
