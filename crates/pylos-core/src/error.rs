#[derive(Debug, thiserror::Error)]
pub enum PylosError {
    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Provider error from {provider}: {message}")]
    ProviderError { provider: String, message: String },

    #[error("All providers failed: {0}")]
    AllProvidersFailed(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Rate limit exceeded: {0}")]
    RateLimitExceeded(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Not supported: {0}")]
    Unsupported(String),

    #[error("Budget exceeded: {0}")]
    BudgetExceeded(String),
}

impl From<anyhow::Error> for PylosError {
    fn from(err: anyhow::Error) -> Self {
        PylosError::Internal(err.to_string())
    }
}

impl PylosError {
    /// Erreurs de gouvernance : elles doivent interrompre la requête.
    /// Les pre-hooks de plugins ne doivent JAMAIS les avaler.
    pub fn is_governance_violation(&self) -> bool {
        matches!(
            self,
            PylosError::BudgetExceeded(_)
                | PylosError::RateLimitExceeded(_)
                | PylosError::Unauthorized(_)
        )
    }

    /// Indique si l'erreur est retriable (fallback possible)
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            PylosError::ProviderError { .. }
                | PylosError::Timeout(_)
                | PylosError::RateLimitExceeded(_)
        )
    }

    /// Retourne le type d'erreur pour les labels Prometheus
    pub fn error_type(&self) -> &'static str {
        match self {
            PylosError::Internal(_) => "internal",
            PylosError::NotFound(_) => "not_found",
            PylosError::InvalidRequest(_) => "invalid_request",
            PylosError::ProviderError { .. } => "provider_error",
            PylosError::AllProvidersFailed(_) => "all_providers_failed",
            PylosError::Unauthorized(_) => "unauthorized",
            PylosError::RateLimitExceeded(_) => "rate_limit",
            PylosError::Timeout(_) => "timeout",
            PylosError::Unsupported(_) => "unsupported",
            PylosError::BudgetExceeded(_) => "budget_exceeded",
        }
    }
}
