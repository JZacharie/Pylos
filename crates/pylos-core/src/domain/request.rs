use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Requête unifiée Pylos — équivalent de BifrostRequest en Go
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PylosRequest {
    ChatCompletion(crate::domain::openai::ChatCompletionRequest),
    TextCompletion(crate::domain::openai::TextCompletionRequest),
    Embedding(crate::domain::embedding::EmbeddingRequest),
    Image(crate::domain::image::ImageRequest),
}

impl PylosRequest {
    pub fn model(&self) -> &str {
        match self {
            PylosRequest::ChatCompletion(req) => &req.model,
            PylosRequest::TextCompletion(req) => &req.model,
            PylosRequest::Embedding(req) => &req.model,
            PylosRequest::Image(req) => &req.model,
        }
    }

    pub fn set_model(&mut self, new_model: String) {
        match self {
            PylosRequest::ChatCompletion(req) => req.model = new_model,
            PylosRequest::TextCompletion(req) => req.model = new_model,
            PylosRequest::Embedding(req) => req.model = new_model,
            PylosRequest::Image(req) => req.model = new_model,
        }
    }

    pub fn is_stream(&self) -> bool {
        match self {
            PylosRequest::ChatCompletion(req) => req.stream.unwrap_or(false),
            PylosRequest::TextCompletion(req) => req.stream.unwrap_or(false),
            PylosRequest::Embedding(_) => false,
            PylosRequest::Image(_) => false,
        }
    }
}

/// Réponse unifiée Pylos
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PylosResponse {
    ChatCompletion(crate::domain::openai::ChatCompletionResponse),
    TextCompletion(crate::domain::openai::TextCompletionResponse),
    Embedding(crate::domain::embedding::EmbeddingResponse),
    Image(crate::domain::image::ImageResponse),
}

/// Un chunk de streaming SSE
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<StreamChoice>,
    /// Usage réel si disponible (dernier chunk, compatible OpenAI stream_options.include_usage)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<crate::domain::openai::Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChoice {
    pub index: i32,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StreamDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// Contenu de raisonnement (DeepSeek R1, OpenAI o1/o3)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    /// Tool call chunks (format OpenAI streaming)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<StreamToolCallChunk>>,
}

/// Chunk d'un appel d'outil dans un stream SSE (format OpenAI)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToolCallChunk {
    pub index: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub call_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function: Option<StreamToolCallFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamToolCallFunction {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Arguments partiels (JSON fragmenté)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Metadata d'une requête en transit
#[derive(Debug, Clone, Default)]
pub struct RequestContext {
    pub headers: HashMap<String, String>,
    pub virtual_key: Option<String>,
    pub provider_configs: Vec<crate::domain::config::VkProviderConfig>,
    pub customer_id: Option<String>,
    pub team_id: Option<String>,
    pub trace_id: Option<String>,
    pub tried_providers: Vec<String>,
    /// Span tracing pour le call LLM en cours (utilisé par OtelPlugin)
    pub otel_span: Option<tracing::Span>,
    /// Vecteur d'embedding pour le cache sémantique (évite stockage dans headers HTTP)
    pub cache_query_vector: Option<Vec<f32>>,
    pub force_provider: Option<String>,
    pub force_model: Option<String>,
}
