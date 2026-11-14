use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use tracing::{debug, error, warn};

use pylos_core::domain::embedding::{EmbeddingRequest, EmbeddingResponse};
use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse};
use pylos_core::domain::traits::{ChunkStream, Provider};
use pylos_core::error::PylosError;

use super::converters::{
    from_openai_response, from_openai_stream_chunk, from_openai_text_response,
    from_openai_text_stream_chunk, map_openai_error, to_openai_request, to_openai_text_request,
    OpenAIChatResponse, OpenAIStreamChunk, OpenAITextResponse,
};
use super::embedding::{
    from_openai_embedding_response, to_openai_embedding_request, OpenAIEmbeddingResponse,
};

const DEFAULT_BASE_URL: &str = "https://api.openai.com/v1";

/// Adapter OpenAI — implémente le trait Provider pour l'API OpenAI
/// Compatible aussi avec tout provider OpenAI-compatible (Groq, Ollama, OpenRouter, etc.)
pub struct OpenAIProvider {
    client: Client,
    name: String,
}

impl OpenAIProvider {
    pub fn new(name: String) -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client, name }
    }

    fn base_url<'a>(&self, config: &'a ProviderConfig) -> &'a str {
        config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL)
    }

    /// Sélectionne la meilleure clé API selon les poids configurés
    fn select_key<'a>(&self, config: &'a ProviderConfig) -> Option<&'a str> {
        if config.keys.is_empty() {
            return None;
        }

        // Weighted random selection — équivalent de keyselectors/ en Go
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

// OpenAIProvider no longer implements Default as it requires a name.

#[async_trait]
impl Provider for OpenAIProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let api_key = self
            .select_key(config)
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for OpenAI".into()))?;

        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let openai_req = to_openai_request(req, false);
                let url = format!("{}/chat/completions", base_url);

                debug!(provider = %self.name, model = %req.model, url = %url, "Sending chat completion request");

                let response = self
                    .client
                    .post(&url)
                    .bearer_auth(api_key)
                    .json(&openai_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "openai".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(provider = %self.name, status = status, body = %body, "Provider returned error");
                    return Err(map_openai_error(status, &body));
                }

                let openai_resp: OpenAIChatResponse = response.json().await.map_err(|e| {
                    PylosError::Internal(format!("Failed to parse OpenAI response: {}", e))
                })?;

                debug!(provider = %self.name, id = %openai_resp.id, "Chat completion successful");
                Ok(from_openai_response(openai_resp))
            }
            PylosRequest::TextCompletion(req) => {
                let openai_req = to_openai_text_request(req, false);
                let url = format!("{}/completions", base_url);

                debug!(provider = %self.name, model = %req.model, url = %url, "Sending text completion request");

                let response = self
                    .client
                    .post(&url)
                    .bearer_auth(api_key)
                    .json(&openai_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "openai".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    return Err(map_openai_error(status, &body));
                }

                let openai_resp: OpenAITextResponse = response.json().await.map_err(|e| {
                    PylosError::Internal(format!("Failed to parse OpenAI response: {}", e))
                })?;

                Ok(from_openai_text_response(openai_resp))
            }
            PylosRequest::Embedding(_) => Err(PylosError::InvalidRequest(
                "Use the embed() method for embedding requests".into(),
            )),
            PylosRequest::Image(_) => Err(PylosError::InvalidRequest(
                "Use the generate_image() method for image requests".into(),
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
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for OpenAI".into()))?;

        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let openai_req = to_openai_request(req, true);
                let url = format!("{}/chat/completions", base_url);

                debug!(provider = %self.name, model = %req.model, url = %url, "Sending streaming chat completion");

                let response = self
                    .client
                    .post(&url)
                    .bearer_auth(api_key)
                    .json(&openai_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "openai".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(
                        provider = %self.name,
                        status = status,
                        "Stream request failed"
                    );
                    return Err(map_openai_error(status, &body));
                }

                // Décode le flux SSE ligne par ligne
                let stream = response
                    .bytes_stream()
                    .eventsource()
                    .filter_map(|event| async move {
                        match event {
                            Ok(e) => {
                                let data = e.data.trim().to_string();
                                // Sentinel de fin de stream OpenAI
                                if data == "[DONE]" {
                                    return None;
                                }
                                // Vérifie d'abord si le chunk est une erreur Ollama/Provider
                                if let Ok(err_body) = serde_json::from_str::<
                                    super::converters::OpenAIErrorBody,
                                >(&data)
                                {
                                    return Some(Err(PylosError::ProviderError {
                                        provider: "openai".into(),
                                        message: err_body.error.message,
                                    }));
                                }
                                match serde_json::from_str::<OpenAIStreamChunk>(&data) {
                                    Ok(chunk) => Some(Ok(from_openai_stream_chunk(chunk))),
                                    Err(parse_err) => {
                                        // Ignore les lignes vides ou malformées
                                        if data.is_empty() {
                                            None
                                        } else {
                                            error!(
                                                provider = "openai",
                                                data = %data,
                                                error = %parse_err,
                                                "Failed to parse SSE chunk"
                                            );
                                            None
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                error!(provider = "openai", error = %e, "SSE stream error");
                                Some(Err(PylosError::ProviderError {
                                    provider: "openai".into(),
                                    message: e.to_string(),
                                }))
                            }
                        }
                    });

                Ok(Box::pin(stream))
            }
            PylosRequest::TextCompletion(req) => {
                let openai_req = to_openai_text_request(req, true);
                let url = format!("{}/completions", base_url);

                debug!(provider = "openai", model = %req.model, url = %url, "Sending streaming text completion");

                let response = self
                    .client
                    .post(&url)
                    .bearer_auth(api_key)
                    .json(&openai_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "openai".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    return Err(map_openai_error(status, &body));
                }

                let stream = response
                    .bytes_stream()
                    .eventsource()
                    .filter_map(|event| async move {
                        match event {
                            Ok(e) => {
                                let data = e.data.trim().to_string();
                                if data == "[DONE]" {
                                    return None;
                                }
                                match serde_json::from_str::<OpenAITextResponse>(&data) {
                                    Ok(resp) => Some(Ok(from_openai_text_stream_chunk(resp))),
                                    Err(_) => None,
                                }
                            }
                            Err(_) => None,
                        }
                    });

                Ok(Box::pin(stream))
            }
            PylosRequest::Embedding(_) => Err(PylosError::InvalidRequest(
                "Use the /v1/embeddings endpoint for embedding requests".into(),
            )),
            PylosRequest::Image(_) => Err(PylosError::Unsupported(
                "Image requests do not support streaming".into(),
            )),
        }
    }

    /// Embeddings — POST /v1/embeddings (OpenAI)
    async fn embed(
        &self,
        request: &EmbeddingRequest,
        config: &ProviderConfig,
    ) -> Result<EmbeddingResponse, PylosError> {
        let api_key = self
            .select_key(config)
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for OpenAI".into()))?;

        let base_url = self.base_url(config);
        let url = format!("{}/embeddings", base_url);
        let openai_req = to_openai_embedding_request(request);

        debug!(provider = "openai", model = %request.model, url = %url, "Sending embedding request");

        let response = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .json(&openai_req)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    PylosError::Timeout(e.to_string())
                } else {
                    PylosError::ProviderError {
                        provider: "openai".into(),
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(provider = "openai", status = status, body = %body, "Embedding request failed");
            return Err(map_openai_error(status, &body));
        }

        let openai_resp: OpenAIEmbeddingResponse = response.json().await.map_err(|e| {
            PylosError::Internal(format!("Failed to parse OpenAI embedding response: {}", e))
        })?;

        debug!(provider = "openai", model = %openai_resp.model, "Embedding successful");
        Ok(from_openai_embedding_response(openai_resp))
    }

    /// Image Generation — POST /v1/images/generations (OpenAI)
    async fn generate_image(
        &self,
        request: &pylos_core::domain::image::ImageRequest,
        config: &ProviderConfig,
    ) -> Result<pylos_core::domain::image::ImageResponse, PylosError> {
        let api_key = self
            .select_key(config)
            .ok_or_else(|| PylosError::InvalidRequest("No API key configured for OpenAI".into()))?;

        let base_url = self.base_url(config);
        let url = format!("{}/images/generations", base_url);

        debug!(provider = %self.name, model = %request.model, url = %url, "Sending image generation request");

        let response = self
            .client
            .post(&url)
            .bearer_auth(api_key)
            .json(request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    PylosError::Timeout(e.to_string())
                } else {
                    PylosError::ProviderError {
                        provider: self.name.clone(),
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status().as_u16();

        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(provider = %self.name, status = status, body = %body, "Image generation failed");
            return Err(map_openai_error(status, &body));
        }

        let resp: pylos_core::domain::image::ImageResponse =
            response.json().await.map_err(|e| {
                PylosError::Internal(format!("Failed to parse OpenAI image response: {}", e))
            })?;

        Ok(resp)
    }

    async fn health_check(&self, config: &ProviderConfig) -> Result<(), PylosError> {
        let api_key = self.select_key(config).ok_or_else(|| {
            PylosError::InvalidRequest(format!("No API key configured for {}", self.name))
        })?;

        let base_url = self.base_url(config);
        let url = format!("{}/models", base_url);

        debug!(provider = %self.name, url = %url, "Testing provider connectivity");

        let response = self
            .client
            .get(&url)
            .bearer_auth(api_key)
            .send()
            .await
            .map_err(|e| PylosError::ProviderError {
                provider: self.name.clone(),
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            if (status == 404 || status == 405) && self.name != "openai" {
                debug!(provider = %self.name, status = status, "Health check returned 404/405 for custom provider, assuming OK");
                return Ok(());
            }
            warn!(provider = %self.name, status = status, body = %body, "Health check failed");
            return Err(map_openai_error(status, &body));
        }

        Ok(())
    }
}
