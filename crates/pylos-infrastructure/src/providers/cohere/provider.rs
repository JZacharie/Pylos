use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use tracing::{debug, warn};

use pylos_core::domain::embedding::{EmbeddingRequest, EmbeddingResponse};
use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse};
use pylos_core::domain::traits::{ChunkStream, Provider};
use pylos_core::error::PylosError;

use super::converters::{
    from_cohere_embed_response, from_cohere_response, from_cohere_stream_event, map_cohere_error,
    to_cohere_embed_request, to_cohere_request, CohereChatResponse, CohereStreamEvent,
};

const DEFAULT_BASE_URL: &str = "https://api.cohere.ai";

// ─────────────────────────────────────────────────────────────────────────────
// CohereProvider — Cohere API v2
// Bifrost source: core/providers/cohere/cohere.go
//
// Auth: "Authorization: Bearer <key>"
// Chat: POST /v2/chat
// Embed: POST /v2/embed
// ─────────────────────────────────────────────────────────────────────────────

pub struct CohereProvider {
    client: Client,
}

impl CohereProvider {
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

impl Default for CohereProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for CohereProvider {
    fn name(&self) -> &str {
        "cohere"
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let api_key = self
            .select_key(config)
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for Cohere".into()))?;

        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let url = format!("{}/v2/chat", base_url);
                let cohere_req = to_cohere_request(req, false);

                debug!(provider = "cohere", model = %req.model, url = %url, "Sending Cohere chat request");

                let response = self
                    .client
                    .post(&url)
                    .bearer_auth(api_key)
                    .json(&cohere_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "cohere".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();
                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(provider = "cohere", status = status, body = %body, "Cohere returned error");
                    return Err(map_cohere_error(status, &body));
                }

                let cohere_resp: CohereChatResponse = response.json().await.map_err(|e| {
                    PylosError::Internal(format!("Failed to parse Cohere response: {}", e))
                })?;

                // Injecte le modèle utilisé dans la réponse
                let model = req.model.clone();
                debug!(provider = "cohere", model = %model, "Cohere chat successful");

                // Patch model dans la réponse PylosResponse
                let mut pylos_resp = from_cohere_response(cohere_resp);
                if let pylos_core::domain::request::PylosResponse::ChatCompletion(ref mut r) =
                    pylos_resp
                {
                    r.model = model;
                }
                Ok(pylos_resp)
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::InvalidRequest(
                "Request type not supported by complete() on Cohere".into(),
            )),
        }
    }

    async fn stream(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        let api_key = self
            .select_key(config)
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for Cohere".into()))?;

        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let url = format!("{}/v2/chat", base_url);
                let cohere_req = to_cohere_request(req, true);
                let model = req.model.clone();

                debug!(provider = "cohere", model = %model, url = %url, "Sending Cohere streaming request");

                let response = self
                    .client
                    .post(&url)
                    .bearer_auth(api_key)
                    .json(&cohere_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "cohere".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();
                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    return Err(map_cohere_error(status, &body));
                }

                // Cohere envoie des lignes JSON (NDJSON), pas du SSE standard
                // On lit ligne par ligne via bytes_stream
                let model_clone = model.clone();
                let stream = response
                    .bytes_stream()
                    .map(move |chunk_result| {
                        let m = model_clone.clone();
                        match chunk_result {
                            Ok(bytes) => {
                                // Parse chaque ligne JSON
                                let text = String::from_utf8_lossy(&bytes);
                                let mut results = Vec::new();
                                for line in text.lines() {
                                    let line = line.trim();
                                    if line.is_empty() {
                                        continue;
                                    }
                                    if let Ok(event) =
                                        serde_json::from_str::<CohereStreamEvent>(line)
                                    {
                                        if let Some(chunk) = from_cohere_stream_event(event, &m) {
                                            results.push(Ok(chunk));
                                        }
                                    }
                                }
                                futures::stream::iter(results)
                            }
                            Err(e) => futures::stream::iter(vec![Err(PylosError::ProviderError {
                                provider: "cohere".into(),
                                message: e.to_string(),
                            })]),
                        }
                    })
                    .flatten();

                Ok(Box::pin(stream))
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::InvalidRequest(
                "Streaming not supported for this request type".into(),
            )),
        }
    }

    /// Cohere embeddings — POST /v2/embed
    async fn embed(
        &self,
        request: &EmbeddingRequest,
        config: &ProviderConfig,
    ) -> Result<EmbeddingResponse, PylosError> {
        let api_key = self
            .select_key(config)
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for Cohere".into()))?;

        let base_url = self.base_url(config);
        let url = format!("{}/v2/embed", base_url);
        let cohere_req = to_cohere_embed_request(request);

        debug!(provider = "cohere", model = %request.model, "Sending Cohere embed request");

        let response = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .json(&cohere_req)
            .send()
            .await
            .map_err(|e| PylosError::ProviderError {
                provider: "cohere".into(),
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(map_cohere_error(status, &body));
        }

        let cohere_resp = response.json().await.map_err(|e| {
            PylosError::Internal(format!("Failed to parse Cohere embed response: {}", e))
        })?;

        Ok(from_cohere_embed_response(cohere_resp, &request.model))
    }

    async fn health_check(&self, config: &ProviderConfig) -> Result<(), PylosError> {
        let api_key = self
            .select_key(config)
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for Cohere".into()))?;

        let base_url = self.base_url(config);
        let url = format!("{}/v1/models", base_url);

        debug!(provider = "cohere", url = %url, "Testing provider connectivity");

        let response = self
            .client
            .get(&url)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|e| PylosError::ProviderError {
                provider: "cohere".into(),
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(provider = "cohere", status = status, body = %body, "Health check failed");
            return Err(map_cohere_error(status, &body));
        }

        Ok(())
    }
}
