pub mod domain;
pub mod error;

pub use error::PylosError;
pub type Result<T> = std::result::Result<T, PylosError>;

// Re-exports pratiques
pub use domain::auth::{OidcAuthenticator, OidcClaims};
pub use domain::config::{
    BedrockKeyConfig, BudgetConfig, ConcurrencyConfig, EnvVar, GovernanceConfig, NetworkConfig,
    PluginConfig, ProviderConfig, ProviderKeyConfig, PylosConfig, RateLimitConfig,
    RoutingRuleConfig, ServerConfig, VirtualKeyConfig,
};
pub use domain::encryption::{
    decrypt as key_decrypt, encrypt as key_encrypt, init as init_encryption,
};
pub use domain::provider::ProviderConfig as RuntimeProviderConfig;
pub use domain::provider::{ProviderKey, ProviderKind, RoutingTarget};
pub use domain::request::{PylosRequest, PylosResponse, RequestContext, StreamChunk};
pub use domain::search_tool::SearchToolConfig;
pub use domain::traits::{ChunkStream, LlmPlugin, Provider};
pub use domain::virtual_key::{VirtualKey, VirtualKeyRegistry, VIRTUAL_KEY_PREFIX};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = PylosError::NotFound("something".into());
        assert_eq!(format!("{}", err), "Not found: something");
    }

    #[test]
    fn test_error_retriable() {
        assert!(PylosError::Timeout("slow".into()).is_retriable());
        assert!(PylosError::RateLimitExceeded("429".into()).is_retriable());
        assert!(!PylosError::InvalidRequest("bad".into()).is_retriable());
        assert!(!PylosError::NotFound("missing".into()).is_retriable());
    }

    #[test]
    fn test_provider_kind_display() {
        assert_eq!(ProviderKind::OpenAI.to_string(), "openai");
        assert_eq!(ProviderKind::Anthropic.to_string(), "anthropic");
        assert_eq!(
            ProviderKind::Custom("myprovider".into()).to_string(),
            "custom:myprovider"
        );
    }

    #[test]
    fn test_provider_key_weight() {
        let key = ProviderKey::new("sk-test").with_weight(0.7);
        assert_eq!(key.value, "sk-test");
        assert!((key.weight - 0.7).abs() < f64::EPSILON);
    }
}
