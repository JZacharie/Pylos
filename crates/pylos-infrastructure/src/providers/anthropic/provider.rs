use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use tracing::{debug, error, warn};

use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse};
use pylos_core::domain::traits::{ChunkStream, Provider};
use pylos_core::error::PylosError;

use super::converters::{
    from_anthropic_response, from_anthropic_stream_event, map_anthropic_error,
    to_anthropic_request, AnthropicResponse, AnthropicStreamEvent, StreamContext,
};

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com/v1";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Adapter Anthropic — implémente le trait Provider pour l'API Anthropic Messages
/// Équivalent de core/providers/anthropic/ en Go (bifrost)
pub struct AnthropicProvider {
    client: Client,
}

impl AnthropicProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    fn base_url<'a>(&self, config: &'a ProviderConfig) -> &'a str {
        config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL)
    }

    fn select_key<'a>(&self, config: &'a ProviderConfig) -> Option<&'a str> {
        if config.keys.is_empty() {
            return None;
        }
        let total_weight: f64 = config.keys.iter().map(|k| k.weight).sum();
        if total_weight <= 0.0 {
            return Some(&config.keys[0].value);
        }
        let mut rng_val = fastrand::f64() * total_weight;
        for key in &config.keys {
            rng_val -= key.weight;
            if rng_val <= 0.0 {
                return Some(&key.value);
            }
        }
        config.keys.last().map(|k| k.value.as_str())
    }
}

impl Default for AnthropicProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let api_key = self.select_key(config).ok_or_else(|| {
            PylosError::InvalidRequest("No API key configured for Anthropic".into())
        })?;

        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let anthropic_req = to_anthropic_request(req, false);
                let url = format!("{}/messages", base_url);

                debug!(provider = "anthropic", model = %req.model, "Sending messages request");

                let response = self
                    .client
                    .post(&url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("content-type", "application/json")
                    .json(&anthropic_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "anthropic".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(provider = "anthropic", status = status, body = %body, "Provider error");
                    return Err(map_anthropic_error(status, &body));
                }

                let anthropic_resp: AnthropicResponse = response.json().await.map_err(|e| {
                    PylosError::Internal(format!("Failed to parse Anthropic response: {}", e))
                })?;

                debug!(provider = "anthropic", id = %anthropic_resp.id, "Messages request successful");
                Ok(from_anthropic_response(anthropic_resp, &req.model))
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::Unsupported(
                "Anthropic does not support this request type".into(),
            )),
        }
    }

    async fn stream(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        let api_key = self.select_key(config).ok_or_else(|| {
            PylosError::InvalidRequest("No API key configured for Anthropic".into())
        })?;

        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let anthropic_req = to_anthropic_request(req, true);
                let url = format!("{}/messages", base_url);

                debug!(provider = "anthropic", model = %req.model, "Sending streaming messages request");

                let response = self
                    .client
                    .post(&url)
                    .header("x-api-key", api_key)
                    .header("anthropic-version", ANTHROPIC_VERSION)
                    .header("content-type", "application/json")
                    .json(&anthropic_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "anthropic".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(
                        provider = "anthropic",
                        status = status,
                        "Stream request failed"
                    );
                    return Err(map_anthropic_error(status, &body));
                }

                // Contexte partagé pour accumuler les métadonnées (id, model, tool state)
                let ctx = std::sync::Arc::new(tokio::sync::Mutex::new(StreamContext {
                    message_id: format!("msg_{}", fastrand::u64(..)),
                    model: req.model.clone(),
                    created: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs() as i64,
                    ..Default::default()
                }));

                let stream = response
                    .bytes_stream()
                    .eventsource()
                    .filter_map(move |event| {
                        let ctx = ctx.clone();
                        async move {
                            match event {
                                Ok(e) => {
                                    let data = e.data.trim().to_string();
                                    if data.is_empty() {
                                        return None;
                                    }

                                    match serde_json::from_str::<AnthropicStreamEvent>(&data) {
                                        Ok(stream_event) => {
                                            // Mise à jour du contexte si c'est un message_start
                                            if stream_event.event_type == "message_start" {
                                                if let Some(msg) = &stream_event.message {
                                                    let mut c = ctx.lock().await;
                                                    c.message_id = msg.id.clone();
                                                    c.model = msg.model.clone();
                                                }
                                                return None;
                                            }

                                            let mut c = ctx.lock().await;
                                            from_anthropic_stream_event(stream_event, &mut c)
                                                .map(Ok)
                                        }
                                        Err(parse_err) => {
                                            if !data.is_empty() {
                                                error!(
                                                    provider = "anthropic",
                                                    data = %data,
                                                    error = %parse_err,
                                                    "Failed to parse SSE event"
                                                );
                                            }
                                            None
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!(provider = "anthropic", error = %e, "SSE stream error");
                                    Some(Err(PylosError::ProviderError {
                                        provider: "anthropic".into(),
                                        message: e.to_string(),
                                    }))
                                }
                            }
                        }
                    });

                Ok(Box::pin(stream))
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::Unsupported(
                "Streaming not supported for this request type".into(),
            )),
        }
    }

    async fn health_check(&self, config: &ProviderConfig) -> Result<(), PylosError> {
        let api_key = self.select_key(config).ok_or_else(|| {
            PylosError::InvalidRequest("No API key configured for Anthropic".into())
        })?;

        let base_url = self.base_url(config);
        let url = format!("{}/models", base_url);

        debug!(provider = "anthropic", url = %url, "Testing provider connectivity");

        let response = self
            .client
            .get(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .send()
            .await
            .map_err(|e| PylosError::ProviderError {
                provider: "anthropic".into(),
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(provider = "anthropic", status = status, body = %body, "Health check failed");
            return Err(map_anthropic_error(status, &body));
        }

        Ok(())
    }
}
