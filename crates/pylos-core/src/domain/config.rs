use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// EnvVar — syntaxe "env.VAR_NAME" identique à bifrost
// ─────────────────────────────────────────────────────────────────────────────

/// Valeur pouvant être littérale ou référencer une variable d'env via "env.VAR"
/// Identique au type schemas.EnvVar de bifrost
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
pub enum EnvVar {
    Literal(String),
    /// Référence à une variable d'environnement (résolution différée)
    EnvRef(String),
}

impl EnvVar {
    /// Résout la valeur : "env.FOO" → valeur de $FOO, sinon valeur littérale
    pub fn resolve(&self) -> Option<String> {
        match self {
            EnvVar::Literal(s) => {
                if let Some(var_name) = s.strip_prefix("env.") {
                    std::env::var(var_name).ok()
                } else if s.is_empty() {
                    None
                } else {
                    Some(s.clone())
                }
            }
            EnvVar::EnvRef(var_name) => std::env::var(var_name).ok(),
        }
    }

    /// Version sûre pour les logs : masque la valeur si c'est une clé API
    pub fn redacted(&self) -> String {
        match self {
            EnvVar::Literal(s) if s.starts_with("env.") => s.clone(),
            EnvVar::Literal(s) if s.len() > 8 => {
                format!("{}****", &s[..4])
            }
            EnvVar::EnvRef(s) => format!("env.{}", s),
            EnvVar::Literal(_) => "****".into(),
        }
    }

    /// Crée une référence à une variable d'environnement
    pub fn from_env(var_name: impl Into<String>) -> Self {
        EnvVar::EnvRef(var_name.into())
    }
}

impl From<&str> for EnvVar {
    fn from(s: &str) -> Self {
        if let Some(var_name) = s.strip_prefix("env.") {
            EnvVar::EnvRef(var_name.to_string())
        } else {
            EnvVar::Literal(s.to_string())
        }
    }
}

impl From<String> for EnvVar {
    fn from(s: String) -> Self {
        if let Some(var_name) = s.strip_prefix("env.") {
            EnvVar::EnvRef(var_name.to_string())
        } else {
            EnvVar::Literal(s)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Durées — format string "5m", "1h", "30s", "1d" identique à bifrost
// ─────────────────────────────────────────────────────────────────────────────

/// Durée exprimée en string human-readable
/// Formats supportés : 30s, 5m, 1h, 1d, 1w, 1M, 1Y
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Duration(pub String);

impl Duration {
    pub fn as_secs(&self) -> u64 {
        let s = self.0.trim();
        let (n, unit) = s.split_at(s.len().saturating_sub(1));
        let n: u64 = n.parse().unwrap_or(0);
        match unit {
            "s" => n,
            "m" => n * 60,
            "h" => n * 3600,
            "d" => n * 86400,
            "w" => n * 604800,
            "M" => n * 2_592_000,  // ~30 jours
            "Y" => n * 31_536_000, // ~365 jours
            _ => n,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Config racine — pylos.json
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PylosConfig {
    /// Schéma JSON de validation (optionnel)
    #[serde(rename = "$schema", skip_serializing_if = "Option::is_none")]
    pub schema: Option<String>,

    /// Version du format (1 ou 2). v2 = tableaux vides → deny-all
    #[serde(default = "default_version")]
    pub version: u32,

    /// Configuration du serveur HTTP
    #[serde(default)]
    pub server: ServerConfig,

    /// Providers LLM configurés
    #[serde(default)]
    pub providers: HashMap<String, ProviderConfig>,

    /// Gouvernance : virtual keys, budgets, rate limits, routing rules
    #[serde(default)]
    pub governance: GovernanceConfig,

    /// Plugins activés
    #[serde(default)]
    pub plugins: Vec<PluginConfig>,
}

fn default_version() -> u32 {
    2
}

impl Default for PylosConfig {
    fn default() -> Self {
        Self {
            schema: None,
            version: 2,
            server: ServerConfig::default(),
            providers: HashMap::new(),
            governance: GovernanceConfig::default(),
            plugins: Vec::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ServerConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Port d'écoute
    #[serde(default = "default_port")]
    pub port: u16,

    /// Host de bind
    #[serde(default = "default_host")]
    pub host: String,

    /// Niveau de log : error | warn | info | debug | trace
    #[serde(default = "default_log_level")]
    pub log_level: String,

    /// Activer le logging des requêtes/réponses
    #[serde(default = "default_true")]
    pub enable_logging: bool,

    /// Ne pas logger le contenu (inputs/outputs) pour la confidentialité
    #[serde(default)]
    pub disable_content_logging: bool,

    /// Rétention des logs en jours
    #[serde(default = "default_log_retention")]
    pub log_retention_days: u32,

    /// Taille max du body en MB
    #[serde(default = "default_max_body_mb")]
    pub max_request_body_size_mb: u32,

    /// Origins CORS autorisés
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: Vec<String>,

    /// Auth obligatoire sur les endpoints d'inférence
    #[serde(default)]
    pub enforce_auth_on_inference: bool,

    /// Chemin vers la base SQLite pour la persistance des logs
    /// Si absent → log store in-memory (10k entrées max, perdu au restart)
    /// Exemple : "./pylos-logs.db"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_db_path: Option<String>,

    /// URL de connexion PostgreSQL
    /// Exemple : "postgresql://user:password@pg-prd/pylos"
    /// Si défini, remplace toutes les bases SQLite locales par PostgreSQL.
    /// Les bases de données suivantes doivent exister : pylos (prod), pylos-dev (dev)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub database_url: Option<EnvVar>,

    /// Configuration de la file d'attente de requêtes (queuing)
    #[serde(default)]
    pub queuing: QueuingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuingConfig {
    #[serde(default = "default_max_concurrency")]
    pub max_concurrency: usize,
    #[serde(default = "default_max_queue_size")]
    pub max_queue_size: usize,
    #[serde(default = "default_queue_timeout_ms")]
    pub queue_timeout_ms: u64,
}

fn default_max_concurrency() -> usize {
    100
}
fn default_max_queue_size() -> usize {
    1000
}
fn default_queue_timeout_ms() -> u64 {
    30_000
}

impl Default for QueuingConfig {
    fn default() -> Self {
        Self {
            max_concurrency: default_max_concurrency(),
            max_queue_size: default_max_queue_size(),
            queue_timeout_ms: default_queue_timeout_ms(),
        }
    }
}

fn default_port() -> u16 {
    3000
}
fn default_host() -> String {
    "0.0.0.0".into()
}
fn default_log_level() -> String {
    "info".into()
}
fn default_true() -> bool {
    true
}
fn default_log_retention() -> u32 {
    365
}
fn default_max_body_mb() -> u32 {
    100
}
fn default_allowed_origins() -> Vec<String> {
    vec!["*".into()]
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            host: default_host(),
            log_level: default_log_level(),
            enable_logging: true,
            disable_content_logging: false,
            log_retention_days: 365,
            max_request_body_size_mb: 100,
            allowed_origins: vec!["*".into()],
            enforce_auth_on_inference: false,
            log_db_path: None,
            database_url: None,
            queuing: QueuingConfig::default(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// ProviderConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProviderConfig {
    /// Clés API de ce provider (au moins 1 requise pour l'inférence)
    #[serde(default)]
    pub keys: Vec<ProviderKeyConfig>,

    /// Configuration réseau
    #[serde(default)]
    pub network: NetworkConfig,

    /// Concurrence et taille du buffer
    #[serde(default)]
    pub concurrency: ConcurrencyConfig,
}

impl ProviderConfig {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderKeyConfig {
    /// Nom interne (unique au sein du provider)
    pub name: String,

    /// Valeur de la clé API — supporte "env.VAR_NAME"
    /// Non requis pour Bedrock avec IAM role (laisser vide ou omettre)
    #[serde(default = "default_empty_envvar")]
    pub value: EnvVar,

    /// Modèles autorisés pour cette clé. ["*"] = tous, [] = deny-all (v2)
    #[serde(default = "default_wildcard")]
    pub models: Vec<String>,

    /// Poids pour le load-balancing pondéré (défaut: 1.0)
    #[serde(default = "default_weight")]
    pub weight: f64,

    /// Configuration spécifique AWS Bedrock
    /// Requis pour le provider "bedrock"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bedrock_key_config: Option<BedrockKeyConfig>,

    /// Configuration spécifique Azure OpenAI
    /// Requis pour le provider "azure"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_config: Option<AzureKeyConfig>,
}

fn default_empty_envvar() -> EnvVar {
    EnvVar::Literal(String::new())
}

/// Configuration des credentials AWS pour Bedrock
/// Identique à schemas.BedrockKeyConfig dans bifrost
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BedrockKeyConfig {
    /// AWS Access Key ID — supporte "env.AWS_ACCESS_KEY_ID"
    /// Si absent → utilise la chaîne de credentials par défaut AWS
    /// (IAM role, IRSA, profil ~/.aws, variables d'env)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_key_id: Option<EnvVar>,

    /// AWS Secret Access Key — supporte "env.AWS_SECRET_ACCESS_KEY"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_access_key: Option<EnvVar>,

    /// Session token STS — supporte "env.AWS_SESSION_TOKEN"
    /// Requis uniquement pour les credentials temporaires (AssumeRole résolu manuellement)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_token: Option<EnvVar>,

    /// Région AWS — supporte "env.AWS_REGION"
    /// Défaut : "us-east-1"
    #[serde(default = "default_aws_region")]
    pub region: String,

    /// ARN du rôle IAM à assumer via STS (cross-account ou permission séparée)
    /// supporte "env.AWS_ROLE_ARN"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role_arn: Option<EnvVar>,

    /// External ID pour l'AssumeRole (sécurité cross-account)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub external_id: Option<EnvVar>,

    /// Nom de la session STS (défaut: "pylos-session")
    #[serde(default = "default_session_name")]
    pub role_session_name: String,
}

fn default_aws_region() -> String {
    std::env::var("AWS_REGION")
        .or_else(|_| std::env::var("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|_| "us-east-1".into())
}

fn default_session_name() -> String {
    "pylos-session".into()
}

// ─────────────────────────────────────────────────────────────────────────────
// AzureKeyConfig — alias de AzureConfig pour la désérialisation JSON
// ─────────────────────────────────────────────────────────────────────────────

/// Configuration spécifique Azure OpenAI par clé
/// Bifrost source: core/providers/azure/types.go AzureKeyConfig
/// Alias de pylos_core::domain::provider::AzureConfig pour éviter la duplication
pub use crate::domain::provider::AzureConfig as AzureKeyConfig;

impl Default for BedrockKeyConfig {
    fn default() -> Self {
        Self {
            access_key_id: None,
            secret_access_key: None,
            session_token: None,
            region: default_aws_region(),
            role_arn: None,
            external_id: None,
            role_session_name: default_session_name(),
        }
    }
}

fn default_wildcard() -> Vec<String> {
    vec!["*".into()]
}
fn default_weight() -> f64 {
    1.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    /// URL de base (requis pour Ollama, vLLM, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,

    /// Timeout en secondes (défaut: 30)
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,

    /// Nombre max de retries (défaut: 3)
    #[serde(default = "default_retries")]
    pub max_retries: u32,

    /// Backoff initial en ms (défaut: 100)
    #[serde(default = "default_backoff_initial")]
    pub retry_backoff_initial_ms: u64,

    /// Backoff max en ms (défaut: 5000)
    #[serde(default = "default_backoff_max")]
    pub retry_backoff_max_ms: u64,

    /// Headers additionnels vers le provider
    #[serde(default)]
    pub extra_headers: HashMap<String, String>,
}

fn default_timeout() -> u64 {
    30
}
fn default_retries() -> u32 {
    3
}
fn default_backoff_initial() -> u64 {
    100
}
fn default_backoff_max() -> u64 {
    5_000
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            base_url: None,
            timeout_secs: 30,
            max_retries: 3,
            retry_backoff_initial_ms: 100,
            retry_backoff_max_ms: 5_000,
            extra_headers: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyConfig {
    /// Nb de workers concurrents par provider (défaut: 100)
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,

    /// Taille du buffer de la queue (défaut: 1000)
    #[serde(default = "default_buffer")]
    pub buffer_size: u32,
}

fn default_concurrency() -> u32 {
    100
}
fn default_buffer() -> u32 {
    1_000
}

impl Default for ConcurrencyConfig {
    fn default() -> Self {
        Self {
            concurrency: 100,
            buffer_size: 1_000,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GovernanceConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GovernanceConfig {
    /// Virtual keys
    #[serde(default)]
    pub virtual_keys: Vec<VirtualKeyConfig>,

    /// Budgets
    #[serde(default)]
    pub budgets: Vec<BudgetConfig>,

    /// Rate limits
    #[serde(default)]
    pub rate_limits: Vec<RateLimitConfig>,

    /// Règles de routing (CEL)
    #[serde(default)]
    pub routing_rules: Vec<RoutingRuleConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtualKeyConfig {
    /// Identifiant stable
    pub id: String,

    /// Nom lisible
    pub name: String,

    /// Description optionnelle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Valeur de la clé — supporte "env.VAR". Préfixe auto "sk-pylos-" si absent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<EnvVar>,

    /// Actif/inactif (défaut: true)
    #[serde(default = "default_true")]
    pub is_active: bool,

    /// Rate limit ID associé
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rate_limit_id: Option<String>,

    /// Providers autorisés pour cette VK
    #[serde(default)]
    pub provider_configs: Vec<VkProviderConfig>,

    /// Outils autorisés pour cette Virtual Key (S-MCP-12)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_tools: Option<Vec<String>>,

    /// Outils explicitement interdits pour cette Virtual Key (S-MCP-12)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denied_tools: Option<Vec<String>>,

    /// Alias de l'équipe propriétaire
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_alias: Option<String>,

    /// ID de l'équipe propriétaire
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,

    /// ID de l'organisation propriétaire
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organization_id: Option<String>,

    /// ID du groupe d'accès associé
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub access_group_id: Option<String>,

    /// Email de l'utilisateur propriétaire
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_email: Option<String>,

    /// ID de l'utilisateur propriétaire
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,

    /// Timestamp de création (ms)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<i64>,

    /// Nom de l'entité ayant créé la clé
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,

    /// Timestamp de dernière mise à jour (ms)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<i64>,

    /// Timestamp de dernière utilisation (ms)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_active: Option<i64>,

    /// Timestamp d'expiration (ms)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
}

impl Default for VirtualKeyConfig {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            description: None,
            value: None,
            is_active: true,
            rate_limit_id: None,
            provider_configs: Vec::new(),
            allowed_tools: None,
            denied_tools: None,
            team_alias: None,
            team_id: None,
            organization_id: None,
            access_group_id: None,
            user_email: None,
            user_id: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            last_active: None,
            expires_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VkProviderConfig {
    /// Nom du provider (ex: "openai", "anthropic")
    pub provider: String,

    /// Modèles autorisés. ["*"] = tous, [] = deny-all (v2)
    #[serde(default = "default_wildcard")]
    pub allowed_models: Vec<String>,

    /// Noms de clés autorisées. ["*"] = toutes
    #[serde(default = "default_wildcard")]
    pub key_names: Vec<String>,

    /// Poids pour load-balancing entre providers (défaut: 1.0)
    #[serde(default = "default_weight")]
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetConfig {
    pub id: String,
    /// Limite en USD
    pub max_limit: f64,
    /// Période de reset : "30s"|"5m"|"1h"|"1d"|"1w"|"1M"|"1Y"
    pub reset_duration: Duration,
    #[serde(default)]
    pub current_usage: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtual_key_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    pub id: String,
    /// Tokens max par fenêtre (0 = illimité)
    #[serde(default)]
    pub token_max_limit: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_reset_duration: Option<Duration>,
    /// Requêtes max par fenêtre (0 = illimité)
    #[serde(default)]
    pub request_max_limit: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_reset_duration: Option<Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRuleConfig {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Expression CEL pour matcher les requêtes
    pub cel_expression: String,
    /// Cibles de routing avec poids (doivent sommer à 1.0)
    pub targets: Vec<RoutingTarget>,
    /// Fallback chain si toutes les cibles échouent
    #[serde(default)]
    pub fallbacks: Vec<String>,
    /// Priorité : plus petit = évalué en premier
    #[serde(default)]
    pub priority: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingTarget {
    /// Provider cible (None = provider de la requête)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Modèle override (None = modèle de la requête)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Poids pour routing probabiliste (doit être > 0)
    pub weight: f64,
}

// ─────────────────────────────────────────────────────────────────────────────
// PluginConfig
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    /// Nom du plugin : "telemetry" | "logging" | "governance" | "otel" | ...
    pub name: String,
    /// Activer/désactiver
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Configuration spécifique au plugin (JSON libre)
    #[serde(default)]
    pub config: serde_json::Value,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_var_literal() {
        let v = EnvVar::from("sk-test-123");
        assert_eq!(v.resolve(), Some("sk-test-123".into()));
    }

    #[test]
    fn test_env_var_env_syntax() {
        std::env::set_var("PYLOS_TEST_KEY", "my-secret");
        let v = EnvVar::from("env.PYLOS_TEST_KEY");
        assert_eq!(v.resolve(), Some("my-secret".into()));
    }

    #[test]
    fn test_env_var_missing() {
        let v = EnvVar::from("env.PYLOS_NONEXISTENT_XYZ");
        assert_eq!(v.resolve(), None);
    }

    #[test]
    fn test_env_var_empty() {
        let v = EnvVar::from("");
        assert_eq!(v.resolve(), None);
    }

    #[test]
    fn test_env_var_redacted() {
        let v = EnvVar::from("env.SECRET_KEY");
        assert_eq!(v.redacted(), "env.SECRET_KEY");

        let v2 = EnvVar::from("sk-abc123456");
        assert!(v2.redacted().ends_with("****"));
        assert!(v2.redacted().starts_with("sk-a"));
    }

    #[test]
    fn test_env_var_env_ref() {
        std::env::set_var("PYLOS_TEST_REF", "resolved-value");
        let v = EnvVar::from_env("PYLOS_TEST_REF");
        assert_eq!(v.resolve(), Some("resolved-value".into()));
        assert_eq!(v.redacted(), "env.PYLOS_TEST_REF");
    }

    #[test]
    fn test_env_var_from_str_with_env_prefix() {
        std::env::set_var("PYLOS_VAR_FROM_STR", "hello");
        let v = EnvVar::from("env.PYLOS_VAR_FROM_STR");
        assert_eq!(v.resolve(), Some("hello".into()));
    }

    #[test]
    fn test_env_var_env_ref_missing() {
        let v = EnvVar::from_env("PYLOS_NONEXISTENT_REF");
        assert_eq!(v.resolve(), None);
    }

    #[test]
    fn test_duration_parse() {
        assert_eq!(Duration("30s".into()).as_secs(), 30);
        assert_eq!(Duration("5m".into()).as_secs(), 300);
        assert_eq!(Duration("1h".into()).as_secs(), 3600);
        assert_eq!(Duration("1d".into()).as_secs(), 86400);
    }

    #[test]
    fn test_config_deserialize_minimal() {
        let json = r#"{
            "providers": {
                "openai": {
                    "keys": [{"name": "prod", "value": "env.OPENAI_API_KEY"}]
                }
            }
        }"#;
        let cfg: PylosConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.providers.contains_key("openai"));
        let key = &cfg.providers["openai"].keys[0];
        assert_eq!(key.name, "prod");
        assert_eq!(key.models, vec!["*"]);
        assert!((key.weight - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_config_full_deserialize() {
        let json = r#"{
            "$schema": "https://pylos.ai/schema",
            "version": 2,
            "server": {"port": 8080, "log_level": "debug"},
            "providers": {
                "openai": {
                    "keys": [
                        {"name": "k1", "value": "sk-test", "models": ["gpt-4o"], "weight": 0.8},
                        {"name": "k2", "value": "env.OPENAI_KEY_2", "weight": 0.2}
                    ],
                    "network": {"timeout_secs": 60, "max_retries": 5}
                }
            },
            "governance": {
                "virtual_keys": [
                    {
                        "id": "vk-1",
                        "name": "Team Alpha",
                        "value": "sk-pylos-alpha123",
                        "is_active": true,
                        "provider_configs": [
                            {"provider": "openai", "allowed_models": ["gpt-4o"]}
                        ]
                    }
                ],
                "rate_limits": [
                    {"id": "rl-1", "request_max_limit": 1000, "request_reset_duration": "1h"}
                ]
            },
            "plugins": [
                {"name": "telemetry", "enabled": true}
            ]
        }"#;
        let cfg: PylosConfig = serde_json::from_str(json).unwrap();
        assert_eq!(cfg.server.port, 8080);
        assert_eq!(cfg.providers["openai"].keys.len(), 2);
        assert_eq!(cfg.governance.virtual_keys.len(), 1);
        assert_eq!(cfg.governance.rate_limits.len(), 1);
        assert_eq!(cfg.plugins.len(), 1);
    }
}
