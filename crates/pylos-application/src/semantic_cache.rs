use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, error, info, warn};

use pylos_core::domain::embedding::{EmbeddingInput, EmbeddingRequest};
use pylos_core::domain::openai::{ChatCompletionResponse, MessageRole};
use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::{LlmPlugin, Provider};
use pylos_core::error::PylosError;

pub struct SemanticCachePlugin {
    qdrant_url: String,
    collection_name: String,
    embedding_model: String,
    similarity_threshold: f64,
    ttl_secs: u64,
    client: reqwest::Client,
    embedding_provider: Option<(Arc<dyn Provider>, ProviderConfig)>,
}

#[allow(clippy::too_many_arguments)]
impl SemanticCachePlugin {
    pub fn new(
        qdrant_url: String,
        collection_name: String,
        embedding_model: String,
        similarity_threshold: f64,
        ttl_secs: u64,
        embedding_provider: Option<(Arc<dyn Provider>, ProviderConfig)>,
    ) -> Self {
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(key) = std::env::var("QDRANT_API_KEY") {
            if !key.is_empty() {
                if let Ok(val) = reqwest::header::HeaderValue::from_str(&key) {
                    headers.insert("api-key", val);
                }
            }
        }
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .default_headers(headers)
            .build()
            .unwrap_or_default();
        Self {
            qdrant_url,
            collection_name,
            embedding_model,
            similarity_threshold,
            ttl_secs,
            client,
            embedding_provider,
        }
    }

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>, PylosError> {
        // Direct provider call if available
        if let Some((ref provider, ref config)) = self.embedding_provider {
            let req = EmbeddingRequest {
                model: self.embedding_model.clone(),
                input: EmbeddingInput::Single(text.to_string()),
                encoding_format: None,
                dimensions: None,
                user: None,
            };
            let resp = provider.embed(&req, config).await.map_err(|e| {
                error!("SemanticCachePlugin: Embedding failed: {:?}", e);
                PylosError::Internal(format!("Embedding failed: {}", e))
            })?;
            return resp
                .data
                .into_iter()
                .next()
                .map(|d| d.embedding)
                .ok_or_else(|| {
                    error!("SemanticCachePlugin: Empty embedding returned");
                    PylosError::Internal("Empty embedding returned".into())
                });
        }

        Err(PylosError::Internal(
            "SemanticCachePlugin: No embedding provider configured".into(),
        ))
    }

    async fn ensure_collection(&self, vector_size: usize) {
        let collection_url = format!(
            "{}/collections/{}",
            self.qdrant_url.trim_end_matches('/'),
            self.collection_name
        );

        let res = self
            .client
            .put(&collection_url)
            .json(&json!({
                "vectors": {
                    "size": vector_size,
                    "distance": "Cosine"
                }
            }))
            .send()
            .await;

        match res {
            Ok(resp) => {
                if resp.status().is_success() {
                    info!(collection = %self.collection_name, "SemanticCachePlugin: Created Qdrant collection");
                }
            }
            Err(e) => {
                debug!("SemanticCachePlugin: Collection check/creation failed (might already exist): {:?}", e);
            }
        }
    }
}

#[async_trait]
impl LlmPlugin for SemanticCachePlugin {
    fn name(&self) -> &str {
        "semantic_cache"
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        let chat_req = match request {
            PylosRequest::ChatCompletion(ref req) => req,
            _ => return Ok(None),
        };

        // Don't cache streaming requests
        if chat_req.stream.unwrap_or(false) {
            return Ok(None);
        }

        // 1. Get user query
        let user_query = chat_req
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, MessageRole::User))
            .and_then(|m| m.content.clone())
            .unwrap_or_default();

        if user_query.is_empty() {
            return Ok(None);
        }

        // 2. Fetch embedding
        let query_vector = match self.get_embedding(&user_query).await {
            Ok(v) => v,
            Err(e) => {
                warn!(
                    "SemanticCachePlugin: Skipping cache check due to embedding failure: {:?}",
                    e
                );
                return Ok(None);
            }
        };

        // Ensure collection exists
        self.ensure_collection(query_vector.len()).await;

        // 3. Search Qdrant
        let search_url = format!(
            "{}/collections/{}/points/search",
            self.qdrant_url.trim_end_matches('/'),
            self.collection_name
        );
        let search_body = json!({
            "vector": query_vector,
            "limit": 1,
            "with_payload": true,
            "score_threshold": self.similarity_threshold
        });

        let search_resp = match self
            .client
            .post(&search_url)
            .json(&search_body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                warn!("SemanticCachePlugin: Qdrant search failed: {:?}", e);
                return Ok(None);
            }
        };

        if !search_resp.status().is_success() {
            debug!(
                "SemanticCachePlugin: Qdrant search status error: {}",
                search_resp.status()
            );
            return Ok(None);
        }

        #[derive(serde::Deserialize)]
        struct QdrantMatch {
            score: f64,
            payload: Option<serde_json::Value>,
        }
        #[derive(serde::Deserialize)]
        struct QdrantSearchResponse {
            result: Vec<QdrantMatch>,
        }

        let search_data: QdrantSearchResponse = match search_resp.json().await {
            Ok(d) => d,
            Err(_) => return Ok(None),
        };

        if let Some(best_match) = search_data.result.first() {
            if let Some(ref payload) = best_match.payload {
                // Check TTL
                let created_at = payload
                    .get("created_at")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                if now.saturating_sub(created_at) > self.ttl_secs {
                    debug!("SemanticCachePlugin: Cached entry expired, skipping");
                    return Ok(None);
                }

                // Retrieve cached response
                if let Some(response_str) = payload.get("response").and_then(|v| v.as_str()) {
                    if let Ok(cached_resp) =
                        serde_json::from_str::<ChatCompletionResponse>(response_str)
                    {
                        info!(
                            score = best_match.score,
                            "SemanticCachePlugin: Cache HIT (Similarity = {:.4})", best_match.score
                        );
                        // Save the query vector in context to prevent re-computing in post_hook
                        ctx.headers
                            .insert("x-cache-hit".to_string(), "true".to_string());
                        return Ok(Some(PylosResponse::ChatCompletion(cached_resp)));
                    }
                }
            }
        }

        // Cache miss: save the query vector/text in RequestContext so post_hook doesn't re-embed
        ctx.headers
            .insert("x-cache-query-text".to_string(), user_query);
        ctx.cache_query_vector = Some(query_vector);

        Ok(None)
    }

    async fn post_hook(
        &self,
        request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        // If it was a cache HIT, do nothing
        if ctx.headers.contains_key("x-cache-hit") {
            return Ok(());
        }

        let chat_resp = match response {
            PylosResponse::ChatCompletion(ref resp) => resp,
            _ => return Ok(()),
        };

        let user_query = match ctx.headers.get("x-cache-query-text") {
            Some(q) => q,
            None => return Ok(()),
        };

        let query_vector = match ctx.cache_query_vector.take() {
            Some(v) => v,
            None => return Ok(()),
        };

        // Ensure collection exists
        self.ensure_collection(query_vector.len()).await;

        // Upsert into Qdrant
        let upsert_url = format!(
            "{}/collections/{}/points?wait=true",
            self.qdrant_url.trim_end_matches('/'),
            self.collection_name
        );

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let point_id = fastrand::u64(..);
        let serialized_response = serde_json::to_string(chat_resp).unwrap_or_default();

        let upsert_body = json!({
            "points": [
                {
                    "id": point_id,
                    "vector": query_vector,
                    "payload": {
                        "query": user_query,
                        "response": serialized_response,
                        "model": request.model(),
                        "created_at": now
                    }
                }
            ]
        });

        match self
            .client
            .post(&upsert_url)
            .json(&upsert_body)
            .send()
            .await
        {
            Ok(resp) => {
                if resp.status().is_success() {
                    debug!(
                        "SemanticCachePlugin: Saved query and response to semantic cache Qdrant"
                    );
                } else {
                    let err = resp.text().await.unwrap_or_default();
                    warn!("SemanticCachePlugin: Qdrant upsert error: {}", err);
                }
            }
            Err(e) => {
                warn!(
                    "SemanticCachePlugin: Failed to save to semantic cache Qdrant: {:?}",
                    e
                );
            }
        }

        Ok(())
    }
}
