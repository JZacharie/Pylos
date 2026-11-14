use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use tracing::{debug, warn};

use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse};
use pylos_core::domain::traits::{ChunkStream, Provider};
use pylos_core::error::PylosError;

use crate::providers::openai::converters::{
    from_openai_response, from_openai_stream_chunk, to_openai_request, OpenAIChatResponse,
    OpenAIStreamChunk,
};

// ─────────────────────────────────────────────────────────────────────────────
// AzureProvider — Azure OpenAI Service
// Bifrost source: core/providers/azure/azure.go
//
// Spécificités vs OpenAI standard :
//  - URL : https://{resource}.openai.azure.com/openai/deployments/{deployment}/chat/completions?api-version=YYYY-MM-DD
//  - Auth : header "api-key" (pas "Authorization: Bearer")
//  - Le nom du modèle dans la requête est ignoré (c'est le deployment name dans l'URL)
//  - Format JSON de la requête/réponse : identique à OpenAI
// ─────────────────────────────────────────────────────────────────────────────

pub struct AzureProvider {
    client: Client,
}

impl AzureProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());
        Self { client }
    }

    /// Construit l'URL Azure depuis la config du provider
    /// Format : https://{resource_name}.openai.azure.com/openai/deployments/{deployment_name}/chat/completions?api-version={api_version}
    fn build_url(&self, config: &ProviderConfig) -> Result<String, PylosError> {
        let azure = config.azure.as_ref().ok_or_else(|| {
            PylosError::InvalidRequest("Azure provider requires azure_config in pylos.json".into())
        })?;

        Ok(format!(
            "https://{}.openai.azure.com/openai/deployments/{}/chat/completions?api-version={}",
            azure.resource_name, azure.deployment_name, azure.api_version
        ))
    }

    /// Sélectionne la clé API selon les poids (identique à OpenAI provider)
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

impl Default for AzureProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for AzureProvider {
    fn name(&self) -> &str {
        "azure"
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let api_key = self.select_key(config).ok_or_else(|| {
            PylosError::InvalidRequest("No API key configured for Azure OpenAI".into())
        })?;

        let url = self.build_url(config)?;

        match request {
            PylosRequest::ChatCompletion(req) => {
                // Construit la requête au format OpenAI (identique au wire format)
                let azure_req = to_openai_request(req, false);

                debug!(provider = "azure", model = %req.model, url = %url, "Sending Azure chat completion request");

                let response = self
                    .client
                    .post(&url)
                    // Azure utilise "api-key" header, pas "Authorization: Bearer"
                    .header("api-key", api_key)
                    .json(&azure_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "azure".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(provider = "azure", status = status, body = %body, "Azure returned error");
                    return Err(map_azure_error(status, &body));
                }

                let resp: OpenAIChatResponse = response.json().await.map_err(|e| {
                    PylosError::Internal(format!("Failed to parse Azure response: {}", e))
                })?;

                debug!(provider = "azure", id = %resp.id, "Azure chat completion successful");
                Ok(from_openai_response(resp))
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::ProviderError {
                provider: "azure".into(),
                message: "Request type not supported by complete() on Azure".into(),
            }),
        }
    }

    async fn stream(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        let api_key = self.select_key(config).ok_or_else(|| {
            PylosError::InvalidRequest("No API key configured for Azure OpenAI".into())
        })?;

        let url = self.build_url(config)?;

        match request {
            PylosRequest::ChatCompletion(req) => {
                let azure_req = to_openai_request(req, true);

                debug!(provider = "azure", model = %req.model, url = %url, "Sending Azure streaming request");

                let response = self
                    .client
                    .post(&url)
                    .header("api-key", api_key)
                    .json(&azure_req)
                    .send()
                    .await
                    .map_err(|e| {
                        if e.is_timeout() {
                            PylosError::Timeout(e.to_string())
                        } else {
                            PylosError::ProviderError {
                                provider: "azure".into(),
                                message: e.to_string(),
                            }
                        }
                    })?;

                let status = response.status().as_u16();

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(provider = "azure", status = status, "Azure stream failed");
                    return Err(map_azure_error(status, &body));
                }

                // Format SSE identique à OpenAI
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
                                match serde_json::from_str::<OpenAIStreamChunk>(&data) {
                                    Ok(chunk) => Some(Ok(from_openai_stream_chunk(chunk))),
                                    Err(_) if data.is_empty() => None,
                                    Err(err) => {
                                        tracing::error!(
                                            provider = "azure",
                                            data = %data,
                                            error = %err,
                                            "Failed to parse Azure SSE chunk"
                                        );
                                        None
                                    }
                                }
                            }
                            Err(e) => Some(Err(PylosError::ProviderError {
                                provider: "azure".into(),
                                message: e.to_string(),
                            })),
                        }
                    });

                Ok(Box::pin(stream))
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::ProviderError {
                provider: "azure".into(),
                message: "Streaming not supported for this request type".into(),
            }),
        }
    }

    async fn health_check(&self, config: &ProviderConfig) -> Result<(), PylosError> {
        let api_key = self.select_key(config).ok_or_else(|| {
            PylosError::InvalidRequest("No API key configured for Azure OpenAI".into())
        })?;

        let azure = config.azure.as_ref().ok_or_else(|| {
            PylosError::InvalidRequest("Azure provider requires azure_config in pylos.json".into())
        })?;

        let url = format!(
            "https://{}.openai.azure.com/openai/deployments?api-version={}",
            azure.resource_name, azure.api_version
        );

        debug!(provider = "azure", url = %url, "Testing provider connectivity");

        let response = self
            .client
            .get(&url)
            .header("api-key", api_key)
            .send()
            .await
            .map_err(|e| PylosError::ProviderError {
                provider: "azure".into(),
                message: e.to_string(),
            })?;

        let status = response.status().as_u16();
        if !response.status().is_success() {
            let body = response.text().await.unwrap_or_default();
            warn!(provider = "azure", status = status, body = %body, "Health check failed");
            return Err(map_azure_error(status, &body));
        }

        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Gestion des erreurs Azure
// Azure retourne les mêmes codes HTTP qu'OpenAI avec un format légèrement différent
// ─────────────────────────────────────────────────────────────────────────────

fn map_azure_error(status: u16, body: &str) -> PylosError {
    // Azure peut wrapper l'erreur dans {"error": {...}} comme OpenAI
    // ou retourner {"error": {"innererror": ...}}
    #[derive(serde::Deserialize)]
    struct AzureError {
        error: AzureErrorDetail,
    }
    #[derive(serde::Deserialize)]
    struct AzureErrorDetail {
        message: String,
    }

    let message = serde_json::from_str::<AzureError>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| body.to_string());

    match status {
        401 => PylosError::Unauthorized(message),
        429 => PylosError::RateLimitExceeded(message),
        408 | 504 => PylosError::Timeout(message),
        _ => PylosError::ProviderError {
            provider: "azure".into(),
            message,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests unitaires
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::provider::{AzureConfig, ProviderKey, ProviderKind};

    fn make_azure_config() -> ProviderConfig {
        let mut cfg = ProviderConfig::new(
            ProviderKind::Azure,
            vec![ProviderKey::new("test-api-key").with_weight(1.0)],
        );
        cfg.azure = Some(AzureConfig {
            resource_name: "my-resource".into(),
            deployment_name: "gpt-4-deployment".into(),
            api_version: "2024-02-01".into(),
        });
        cfg
    }

    #[test]
    fn test_build_url() {
        let provider = AzureProvider::new();
        let config = make_azure_config();
        let url = provider.build_url(&config).unwrap();
        assert_eq!(
            url,
            "https://my-resource.openai.azure.com/openai/deployments/gpt-4-deployment/chat/completions?api-version=2024-02-01"
        );
    }

    #[test]
    fn test_build_url_without_azure_config() {
        let provider = AzureProvider::new();
        let config = ProviderConfig::new(ProviderKind::Azure, vec![]);
        let result = provider.build_url(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("azure_config"));
    }

    #[test]
    fn test_map_azure_error_401() {
        let err = map_azure_error(401, r#"{"error": {"message": "Unauthorized"}}"#);
        assert!(matches!(err, PylosError::Unauthorized(_)));
    }

    #[test]
    fn test_map_azure_error_429() {
        let err = map_azure_error(429, r#"{"error": {"message": "Rate limit exceeded"}}"#);
        assert!(matches!(err, PylosError::RateLimitExceeded(_)));
    }

    #[test]
    fn test_key_weighted_selection() {
        let provider = AzureProvider::new();
        let config = ProviderConfig::new(
            ProviderKind::Azure,
            vec![
                ProviderKey::new("key-a").with_weight(9.0),
                ProviderKey::new("key-b").with_weight(1.0),
            ],
        );

        let mut count_a = 0usize;
        let mut count_b = 0usize;
        for _ in 0..1_000 {
            match provider.select_key(&config) {
                Some("key-a") => count_a += 1,
                Some("key-b") => count_b += 1,
                _ => {}
            }
        }
        // ~90% key-a, ~10% key-b (tolérance statistique)
        assert!(
            count_a > 800,
            "Expected ~900 selections for key-a, got {}",
            count_a
        );
        assert!(
            count_b > 50,
            "Expected ~100 selections for key-b, got {}",
            count_b
        );
    }
}
