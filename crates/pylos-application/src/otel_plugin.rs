use async_trait::async_trait;
use tracing::{debug, info};

use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

// ─────────────────────────────────────────────────────────────────────────────
// OtelPlugin — plugin LLM OpenTelemetry
// Bifrost source: plugins/otel/
//
// Enregistre des spans gen_ai.* pour chaque appel LLM via tracing.
// Compatible avec opentelemetry-tracing-subscriber pour export OTLP.
//
// Attributs gen_ai emis (OpenTelemetry Semantic Conventions) :
//   gen_ai.system       = "openai" | "anthropic" | "gemini" | ...
//   gen_ai.request.model
//   gen_ai.response.model
//   gen_ai.usage.input_tokens
//   gen_ai.usage.output_tokens
//   gen_ai.operation.name = "chat" | "text_completion" | "embeddings"
// ─────────────────────────────────────────────────────────────────────────────

pub struct OtelPlugin {
    /// Nom du service à utiliser dans les spans
    service_name: String,
    /// Si true, log aussi les requêtes/réponses (attention: données sensibles)
    log_content: bool,
}

impl OtelPlugin {
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
            log_content: false,
        }
    }

    pub fn with_log_content(mut self, enabled: bool) -> Self {
        self.log_content = enabled;
        self
    }
}

/// Contexte partagé entre pre_hook et post_hook pour calculer la latence
/// Stocké dans RequestContext.headers comme side-channel
const OTEL_START_KEY: &str = "__otel_start_ns";

#[async_trait]
impl LlmPlugin for OtelPlugin {
    fn name(&self) -> &str {
        "otel"
    }

    fn priority(&self) -> i32 {
        100
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        let operation = operation_name(request);
        let model = request.model().to_string();
        let system = guess_system(request.model());
        let service = self.service_name.clone();

        // Stocke le timestamp de début pour calculer la latence dans post_hook
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
            .to_string();
        ctx.headers.insert(OTEL_START_KEY.to_string(), now_ns);

        // Crée un span tracing — sera exporté via OTLP par tracing-opentelemetry
        let span = tracing::info_span!(
            target: "pylos.otel",
            "llm.{}", operation,
            gen_ai_operation = operation,
            gen_ai_request_model = model.as_str(),
            gen_ai_system = system,
            service_name = service.as_str(),
            vk = ctx.virtual_key.as_deref().unwrap_or(""),
        );

        // Enregistre l'événement de début dans le span
        span.in_scope(|| {
            debug!(
                service = %service,
                operation = %operation,
                model = %model,
                "OTel: LLM call started"
            );
        });

        ctx.otel_span = Some(span);

        Ok(None)
    }

    async fn post_hook(
        &self,
        request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        // Calcule la latence
        let latency_ms = if let Some(start_ns_str) = ctx.headers.get(OTEL_START_KEY) {
            let start_ns: u128 = start_ns_str.parse().unwrap_or(0);
            let now_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            (now_ns.saturating_sub(start_ns) / 1_000_000) as f64
        } else {
            0.0
        };
        ctx.headers.remove(OTEL_START_KEY);

        let operation = operation_name(request);
        let model = request.model();

        let (response_model, input_tokens, output_tokens) = extract_usage(response);

        // Enregistre la complétion dans le span et le ferme
        if let Some(span) = ctx.otel_span.take() {
            span.in_scope(|| {
                tracing::info!(
                    target: "pylos.otel",
                    gen_ai_response_model = %response_model,
                    gen_ai_usage_input_tokens = input_tokens,
                    gen_ai_usage_output_tokens = output_tokens,
                    gen_ai_latency_ms = latency_ms,
                    "llm.call.complete"
                );

                debug!(
                    operation = %operation,
                    model = %model,
                    latency_ms = %latency_ms,
                    input_tokens = %input_tokens,
                    output_tokens = %output_tokens,
                    "OTel: LLM call complete"
                );
            });
            // span tombe ici → on_close → fin du span OTel
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// OtelInitializer — configure le tracer global depuis la config plugin
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct OtelConfig {
    /// OTLP endpoint (ex: "http://localhost:4317")
    pub endpoint: Option<String>,
    /// Nom du service
    #[serde(default = "default_service_name")]
    pub service_name: String,
    /// Log le contenu des requêtes/réponses
    #[serde(default)]
    pub log_content: bool,
    /// Sample rate (0.0 - 1.0, défaut: 1.0)
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
}

fn default_service_name() -> String {
    "pylos".to_string()
}
fn default_sample_rate() -> f64 {
    1.0
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            endpoint: None,
            service_name: default_service_name(),
            log_content: false,
            sample_rate: 1.0,
        }
    }
}

impl OtelConfig {
    pub fn from_plugin_config(config: &serde_json::Value) -> Self {
        serde_json::from_value(config.clone()).unwrap_or_default()
    }

    pub fn build_plugin(&self) -> OtelPlugin {
        if let Some(ref endpoint) = self.endpoint {
            info!(endpoint = %endpoint, service = %self.service_name, "OTel tracing configured");
        } else {
            info!(service = %self.service_name, "OTel plugin initialized (no OTLP endpoint — tracing to logs only)");
        }
        OtelPlugin::new(self.service_name.clone()).with_log_content(self.log_content)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn operation_name(request: &PylosRequest) -> &'static str {
    match request {
        PylosRequest::ChatCompletion(_) => "chat",
        PylosRequest::TextCompletion(_) => "text_completion",
        PylosRequest::Embedding(_) => "embeddings",
        PylosRequest::Image(_) => "images",
    }
}

fn guess_system(model: &str) -> &'static str {
    // Heuristique OTel gen_ai.system — différent de ProviderKind::guess_from_model
    // qui identifie le provider de déploiement, pas la famille du modèle.
    if model.starts_with("gpt")
        || model.starts_with("o1")
        || model.starts_with("o3")
        || model.starts_with("o4")
    {
        "openai"
    } else if model.contains("claude") {
        "anthropic"
    } else if model.starts_with("gemini") {
        "google"
    } else if model.starts_with("command") {
        "cohere"
    } else if model.starts_with("grok") {
        "xai"
    } else if model.contains("llama") || model.contains("mixtral") {
        "meta"
    } else if model.starts_with("mistral") || model.starts_with("codestral") {
        "mistral"
    } else {
        "unknown"
    }
}

fn extract_usage(response: &PylosResponse) -> (String, i32, i32) {
    match response {
        PylosResponse::ChatCompletion(r) => {
            let model = r.model.clone();
            let (inp, out) = r
                .usage
                .as_ref()
                .map(|u| (u.prompt_tokens, u.completion_tokens))
                .unwrap_or((0, 0));
            (model, inp, out)
        }
        PylosResponse::TextCompletion(r) => {
            let model = r.model.clone();
            let (inp, out) = r
                .usage
                .as_ref()
                .map(|u| (u.prompt_tokens, u.completion_tokens))
                .unwrap_or((0, 0));
            (model, inp, out)
        }
        PylosResponse::Embedding(r) => (r.model.clone(), r.usage.prompt_tokens, 0),
        PylosResponse::Image(_) => ("".to_string(), 0, 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_guess_system() {
        assert_eq!(guess_system("gpt-4o"), "openai");
        assert_eq!(guess_system("claude-3-5-sonnet"), "anthropic");
        assert_eq!(guess_system("gemini-2.0-flash"), "google");
        assert_eq!(guess_system("command-r"), "cohere");
        assert_eq!(guess_system("grok-3"), "xai");
        assert_eq!(guess_system("llama-3.3-70b"), "meta");
    }

    #[test]
    fn test_otel_config_defaults() {
        let cfg = OtelConfig::default();
        assert_eq!(cfg.service_name, "pylos");
        assert!((cfg.sample_rate - 1.0).abs() < f64::EPSILON);
        assert!(cfg.endpoint.is_none());
    }

    #[test]
    fn test_otel_config_from_json() {
        let json = serde_json::json!({
            "endpoint": "http://jaeger:4317",
            "service_name": "my-gateway",
            "sample_rate": 0.5
        });
        let cfg = OtelConfig::from_plugin_config(&json);
        assert_eq!(cfg.endpoint, Some("http://jaeger:4317".to_string()));
        assert_eq!(cfg.service_name, "my-gateway");
        assert!((cfg.sample_rate - 0.5).abs() < f64::EPSILON);
    }
}
