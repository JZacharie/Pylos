/// Identifiant unique d'un provider AI
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    OpenAI,
    Anthropic,
    Bedrock,
    Azure,
    Gemini,
    Cohere,
    Groq,
    Mistral,
    Cerebras,
    Perplexity,
    Fireworks,
    XAI,
    Nebius,
    #[serde(rename = "ollama-jo3")]
    Ollama,
    OpenRouter,
    Vertex,
    DeepSeek,
    Lemonade,
    /// Provider custom via URL de base configurable
    Custom(String),
}

impl std::fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderKind::OpenAI => write!(f, "openai"),
            ProviderKind::Anthropic => write!(f, "anthropic"),
            ProviderKind::Bedrock => write!(f, "bedrock"),
            ProviderKind::Azure => write!(f, "azure"),
            ProviderKind::Gemini => write!(f, "gemini"),
            ProviderKind::Cohere => write!(f, "cohere"),
            ProviderKind::Groq => write!(f, "groq"),
            ProviderKind::Mistral => write!(f, "mistral"),
            ProviderKind::Cerebras => write!(f, "cerebras"),
            ProviderKind::Perplexity => write!(f, "perplexity"),
            ProviderKind::Fireworks => write!(f, "fireworks"),
            ProviderKind::XAI => write!(f, "xai"),
            ProviderKind::Nebius => write!(f, "nebius"),
            ProviderKind::Ollama => write!(f, "ollama-jo3"),
            ProviderKind::OpenRouter => write!(f, "openrouter"),
            ProviderKind::Vertex => write!(f, "vertex"),
            ProviderKind::DeepSeek => write!(f, "deepseek"),
            ProviderKind::Lemonade => write!(f, "lemonade"),
            ProviderKind::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

impl ProviderKind {
    /// Déduit le provider depuis le nom du modèle.
    pub fn guess_from_model(model: &str) -> Self {
        if model.starts_with("us.")
            || model.starts_with("eu.")
            || model.starts_with("ap.")
            || model.starts_with("amazon.")
            || model.contains("nova")
            || model.contains("titan")
            || model.starts_with("anthropic.")
        {
            return ProviderKind::Bedrock;
        }
        if model.contains("claude") {
            return ProviderKind::Anthropic;
        }
        if model.starts_with("gpt")
            || model.starts_with("o1")
            || model.starts_with("o3")
            || model.starts_with("o4")
            || model.starts_with("dall-e")
            || model.starts_with("text-embedding")
        {
            return ProviderKind::OpenAI;
        }
        if model.starts_with("gemini") || model.starts_with("google/") || model.contains("learnlm")
        {
            return ProviderKind::Gemini;
        }
        if model.starts_with("command")
            || model.starts_with("embed-")
            || model.starts_with("rerank-")
        {
            return ProviderKind::Cohere;
        }
        if model.starts_with("grok") {
            return ProviderKind::XAI;
        }
        if model.starts_with("deepseek") {
            return ProviderKind::DeepSeek;
        }
        if (model.starts_with("mistral")
            || model.starts_with("codestral")
            || model.starts_with("open-")
            || model.starts_with("pixtral"))
            && !model.contains(':')
        {
            return ProviderKind::Mistral;
        }
        if model.contains('/') {
            return ProviderKind::OpenRouter;
        }
        if model.contains("sonar") || (model.starts_with("llama-") && !model.contains(":")) {
            return ProviderKind::Perplexity;
        }
        if model.contains("firefunction")
            || model.contains("fireworks")
            || model.starts_with("accounts/fireworks/")
        {
            return ProviderKind::Fireworks;
        }
        if (model.starts_with("llama") || model.starts_with("qwen")) && !model.contains('-') {
            return ProviderKind::Cerebras;
        }
        if model.contains(':')
            || model.contains("starcoder")
            || model.contains("nomic")
            || model.contains("phi")
        {
            return ProviderKind::Ollama;
        }
        if model.contains("lemonade") {
            return ProviderKind::Lemonade;
        }
        ProviderKind::Custom("unknown".into())
    }

    /// Détermine si ce provider supporte un modèle donné.
    /// Utilisé pour le tri des providers lors du fallback.
    pub fn supports_model(&self, model: &str) -> bool {
        match self {
            ProviderKind::Bedrock => {
                model.starts_with("us.")
                    || model.starts_with("eu.")
                    || model.starts_with("ap.")
                    || model.starts_with("amazon.")
                    || model.starts_with("anthropic.")
                    || model.contains("nova")
                    || model.contains("titan")
            }
            ProviderKind::OpenAI => {
                model.starts_with("gpt")
                    || model.starts_with("o1")
                    || model.starts_with("o3")
                    || model.starts_with("o4")
                    || model.starts_with("dall-e")
            }
            ProviderKind::Anthropic => model.contains("claude"),
            ProviderKind::Gemini => {
                model.starts_with("gemini")
                    || model.starts_with("google/")
                    || model.contains("learnlm")
            }
            ProviderKind::Cohere => {
                model.starts_with("command")
                    || model.starts_with("embed-")
                    || model.starts_with("rerank-")
            }
            ProviderKind::Groq => {
                model.contains("llama")
                    || model.contains("mixtral")
                    || model.contains("whisper")
                    || model.contains("gemma")
            }
            ProviderKind::Mistral => {
                model.starts_with("mistral")
                    || model.starts_with("codestral")
                    || model.starts_with("open-")
                    || model.starts_with("pixtral")
            }
            ProviderKind::Cerebras => model.starts_with("llama") || model.starts_with("qwen"),
            ProviderKind::Perplexity => model.contains("sonar") || model.starts_with("llama-"),
            ProviderKind::Fireworks => {
                model.contains("firefunction")
                    || model.contains("fireworks")
                    || model.starts_with("accounts/fireworks/")
            }
            ProviderKind::XAI => model.starts_with("grok"),
            ProviderKind::DeepSeek => model.starts_with("deepseek"),
            ProviderKind::Nebius => {
                model.contains("llama") || model.contains("qwen") || model.contains("deepseek")
            }
            ProviderKind::Ollama => {
                !model.starts_with("gpt")
                    && !model.starts_with("claude")
                    && !model.starts_with("gemini")
                    && !model.starts_with("command")
                    && !model.starts_with("deepseek-v4")
                    && !model.contains('/')
                    && !model.starts_with("us.")
                    && !model.starts_with("amazon.")
                    && !model.starts_with("anthropic.")
            }
            ProviderKind::OpenRouter => model.contains('/'),
            ProviderKind::Vertex | ProviderKind::Lemonade | ProviderKind::Custom(_) => true,
            _ => true,
        }
    }

    /// Retourne le nom du système pour les conventions OTel gen_ai.system
    pub fn otel_system_name(&self) -> &'static str {
        match self {
            ProviderKind::OpenAI => "openai",
            ProviderKind::Anthropic => "anthropic",
            ProviderKind::Bedrock => "aws_bedrock",
            ProviderKind::Azure => "azure_openai",
            ProviderKind::Gemini => "google",
            ProviderKind::Cohere => "cohere",
            ProviderKind::Groq => "groq",
            ProviderKind::Mistral => "mistral",
            ProviderKind::Cerebras => "cerebras",
            ProviderKind::Perplexity => "perplexity",
            ProviderKind::Fireworks => "fireworks",
            ProviderKind::XAI => "xai",
            ProviderKind::Nebius => "nebius",
            ProviderKind::Ollama => "ollama-jo3",
            ProviderKind::OpenRouter => "openrouter",
            ProviderKind::Vertex => "vertex",
            ProviderKind::DeepSeek => "deepseek",
            ProviderKind::Lemonade => "lemonade",
            ProviderKind::Custom(_) => "custom",
        }
    }
}

/// URLs de base par défaut pour les providers OpenAI-compatibles
pub fn default_base_url(kind: &ProviderKind) -> Option<&'static str> {
    match kind {
        ProviderKind::Groq => Some("https://api.groq.com/openai/v1"),
        ProviderKind::Mistral => Some("https://api.mistral.ai/v1"),
        ProviderKind::Cerebras => Some("https://api.cerebras.ai/v1"),
        ProviderKind::Perplexity => Some("https://api.perplexity.ai"),
        ProviderKind::Fireworks => Some("https://api.fireworks.ai/inference/v1"),
        ProviderKind::XAI => Some("https://api.x.ai/v1"),
        ProviderKind::Nebius => Some("https://api.studio.nebius.ai/v1"),
        ProviderKind::OpenRouter => Some("https://openrouter.ai/api/v1"),
        ProviderKind::Gemini => Some("https://generativelanguage.googleapis.com/v1beta"),
        ProviderKind::Cohere => Some("https://api.cohere.ai"),
        ProviderKind::DeepSeek => Some("https://api.deepseek.com"),
        ProviderKind::Ollama => Some("http://localhost:11434"),
        _ => None,
    }
}

/// Environnement variable par défaut pour la clé API d'un provider
pub fn default_env_var(kind: &ProviderKind) -> Option<&'static str> {
    match kind {
        ProviderKind::OpenAI => Some("OPENAI_API_KEY"),
        ProviderKind::Anthropic => Some("ANTHROPIC_API_KEY"),
        ProviderKind::Groq => Some("GROQ_API_KEY"),
        ProviderKind::Mistral => Some("MISTRAL_API_KEY"),
        ProviderKind::DeepSeek => Some("DEEPSEEK_API_KEY"),
        ProviderKind::Gemini => Some("GEMINI_API_KEY"),
        ProviderKind::Cohere => Some("COHERE_API_KEY"),
        ProviderKind::Perplexity => Some("PERPLEXITY_API_KEY"),
        ProviderKind::XAI => Some("XAI_API_KEY"),
        ProviderKind::Cerebras => Some("CEREBRAS_API_KEY"),
        ProviderKind::Nebius => Some("NEBIUS_API_KEY"),
        ProviderKind::Fireworks => Some("FIREWORKS_API_KEY"),
        ProviderKind::OpenRouter => Some("OPENROUTER_API_KEY"),
        ProviderKind::Lemonade => Some("LEMONADE_API_KEY"),
        _ => None,
    }
}

/// Configuration spécifique Azure OpenAI
/// Bifrost source: core/providers/azure/types.go AzureKeyConfig
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AzureConfig {
    /// Nom de la ressource Azure : {resource_name}.openai.azure.com
    pub resource_name: String,
    /// Nom du déploiement Azure (correspond au modèle déployé)
    pub deployment_name: String,
    /// Version de l'API Azure OpenAI (ex: "2024-02-01")
    #[serde(default = "default_azure_api_version")]
    pub api_version: String,
}

fn default_azure_api_version() -> String {
    "2024-02-01".to_string()
}

/// Configuration d'une clé API pour un provider
#[derive(Debug, Clone)]
pub struct ProviderKey {
    pub value: String,
    /// Poids pour le load balancing pondéré (0.0 à 1.0)
    pub weight: f64,
    /// Metadata (région AWS, projet GCP, etc.)
    pub metadata: std::collections::HashMap<String, String>,
}

impl ProviderKey {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            weight: 1.0,
            metadata: Default::default(),
        }
    }

    pub fn with_weight(mut self, weight: f64) -> Self {
        self.weight = weight;
        self
    }
}

/// Configuration d'un provider
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub kind: ProviderKind,
    pub keys: Vec<ProviderKey>,
    /// URL de base optionnelle (pour custom providers, Ollama, etc.)
    pub base_url: Option<String>,
    /// Timeout HTTP en millisecondes
    pub timeout_ms: u64,
    /// Nombre max de retries
    pub max_retries: u32,
    /// Délai initial de backoff en ms
    pub retry_backoff_initial_ms: u64,
    /// Délai maximum de backoff en ms
    pub retry_backoff_max_ms: u64,
    /// Configuration Bedrock spécifique (region, credentials, role_arn…)
    pub bedrock: Option<crate::domain::config::BedrockKeyConfig>,
    /// Configuration Azure OpenAI spécifique
    pub azure: Option<AzureConfig>,
    pub allowed_models: Vec<String>,
}

impl ProviderConfig {
    pub fn new(kind: ProviderKind, keys: Vec<ProviderKey>) -> Self {
        Self {
            kind,
            keys,
            base_url: None,
            timeout_ms: 30_000,
            max_retries: 3,
            retry_backoff_initial_ms: 100,
            retry_backoff_max_ms: 5_000,
            bedrock: None,
            azure: None,
            allowed_models: Vec::new(),
        }
    }
}

/// Cible de routing : provider + modèle optionnel
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RoutingTarget {
    pub provider: ProviderKind,
    /// Modèle override (si None, utilise le modèle de la requête)
    pub model_override: Option<String>,
    /// Poids pour routing probabiliste
    pub weight: f64,
}

impl RoutingTarget {
    pub fn new(provider: ProviderKind) -> Self {
        Self {
            provider,
            model_override: None,
            weight: 1.0,
        }
    }
}
