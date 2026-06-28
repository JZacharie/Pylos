use std::sync::Arc;

use async_trait::async_trait;
use tracing::{error, info, warn};

use pylos_core::domain::embedding::{EmbeddingInput, EmbeddingRequest};
use pylos_core::domain::openai::{ChatCompletionMessage, MessageRole};
use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::{LlmPlugin, Provider};
use pylos_core::error::PylosError;

/// Configuration for a single RAG model routing.
/// Maps an incoming model name pattern to a Qdrant collection and prompt template.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RagModelRoute {
    /// Model name pattern to match (e.g. "graphon-rag", "my-rag-*")
    pub model_pattern: String,
    /// Qdrant collection name to search
    pub collection_name: String,
    /// Prompt template. Use {context} as placeholder for retrieved documents.
    pub system_prompt_template: String,
    /// Label for the result type (used in logging)
    pub result_type_label: String,
    /// Fields to extract from Qdrant payload (in order of display)
    pub payload_fields: Vec<String>,
}

impl RagModelRoute {
    pub fn matches(&self, model: &str) -> bool {
        if self.model_pattern.contains('*') {
            let prefix = self.model_pattern.trim_end_matches('*');
            model.starts_with(prefix)
        } else {
            model == self.model_pattern
        }
    }

    pub fn format_context(&self, points: &[QdrantPoint]) -> String {
        let mut context = String::new();
        for (i, point) in points.iter().enumerate() {
            if let Some(ref payload) = point.payload {
                context.push_str(&format!(
                    "--- {} #{i} ---\n",
                    self.result_type_label,
                    i = i + 1
                ));

                // Prefer parent_content (from parent-child chunking) over individual content
                let has_parent = payload
                    .get("parent_content")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| !s.is_empty());
                if has_parent {
                    if let Some(parent) = payload.get("parent_content").and_then(|v| v.as_str()) {
                        context.push_str(&format!("Content: {parent}\n"));
                    }
                } else {
                    for field in &self.payload_fields {
                        if let Some(value) =
                            payload.get(field).and_then(|v| v.as_str()).or_else(|| {
                                payload
                                    .get("metadata")
                                    .and_then(|m| m.get(field))
                                    .and_then(|v| v.as_str())
                            })
                        {
                            let label = match field.as_str() {
                                "sender" => "From",
                                "subject" => "Subject",
                                "file_name" => "File",
                                "source_path" => "Path",
                                "content" => "Content",
                                _ => field,
                            };
                            context.push_str(&format!("{label}: {value}\n"));
                        }
                    }
                }
                context.push('\n');
            }
        }
        context
    }
}

#[derive(serde::Deserialize)]
pub struct QdrantPoint {
    pub payload: Option<serde_json::Value>,
}

#[derive(serde::Deserialize)]
pub struct QdrantResponse {
    pub result: Vec<QdrantPoint>,
}

/// Configuration for query transformation (Query Expansion / HyDE).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct QueryTransformConfig {
    /// Enable query expansion (generate variant queries)
    pub expansion_enabled: bool,
    /// Enable HyDE (generate hypothetical document)
    pub hyde_enabled: bool,
    /// Number of query variants to generate for expansion
    pub num_variants: usize,
    /// Model to use for query transformation (e.g. "gpt-4o-mini", "gemini-2.0-flash")
    pub transform_model: String,
    /// Base URL for the LLM API
    pub transform_api_base: Option<String>,
    /// API key for the transform model
    pub transform_api_key: Option<String>,
}

impl Default for QueryTransformConfig {
    fn default() -> Self {
        Self {
            expansion_enabled: false,
            hyde_enabled: false,
            num_variants: 3,
            transform_model: "gpt-4o-mini".to_string(),
            transform_api_base: None,
            transform_api_key: None,
        }
    }
}

/// Configuration for Corrective RAG (CRAG) web search fallback.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CragConfig {
    /// Enable CRAG: if max Qdrant similarity score is below threshold, fall back to web search
    pub enabled: bool,
    /// Similarity score threshold (0.0 - 1.0). Below this triggers web search.
    pub threshold: f64,
    /// Web search API provider: "tavily", "searxng", or "custom"
    pub provider: String,
    /// API key for the web search provider
    pub api_key: Option<String>,
    /// Base URL for the web search API (required for searxng/custom)
    pub api_base: Option<String>,
    /// Number of web search results to include
    pub max_results: usize,
}

impl Default for CragConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: 0.75,
            provider: "tavily".to_string(),
            api_key: None,
            api_base: None,
            max_results: 3,
        }
    }
}

/// Configuration for the RAG plugin.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RagConfig {
    /// Embedding model to use
    pub embedding_model: String,
    /// Fallback LLM model for completion when not injecting context
    pub pylos_model: String,
    /// Routes: model pattern → Qdrant collection
    pub routes: Vec<RagModelRoute>,
    /// Default prompt used when no context is found
    pub default_system_prompt: Option<String>,
    /// Query transformation configuration
    pub query_transform: QueryTransformConfig,
    /// Corrective RAG configuration
    pub crag: CragConfig,
}

impl Default for RagConfig {
    fn default() -> Self {
        Self {
            embedding_model: std::env::var("PYLOS_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "nomic-embed-text-v2-moe-GGUF".to_string()),
            pylos_model: std::env::var("PYLOS_MODEL")
                .unwrap_or_else(|_| "deepseek-coder-v2:16b".to_string()),
            routes: vec![
                RagModelRoute {
                    model_pattern: "graphon-rag".to_string(),
                    collection_name: std::env::var("QDRANT_COLLECTION")
                        .unwrap_or_else(|_| "emails".to_string()),
                    system_prompt_template: "Use the following relevant documents to answer the user accurately and concisely:\n\n{context}".to_string(),
                    result_type_label: "DOCUMENT".to_string(),
                    payload_fields: vec!["sender".to_string(), "subject".to_string(), "content".to_string()],
                },
                RagModelRoute {
                    model_pattern: "graphon-rag-files".to_string(),
                    collection_name: std::env::var("QDRANT_FILES_COLLECTION")
                        .unwrap_or_else(|_| "mnemosyne_docs".to_string()),
                    system_prompt_template: "Use the following relevant documents to answer the user accurately and concisely:\n\n{context}".to_string(),
                    result_type_label: "DOCUMENT".to_string(),
                    payload_fields: vec!["file_name".to_string(), "source_path".to_string(), "content".to_string()],
                },
                RagModelRoute {
                    model_pattern: "mnemosyne-search".to_string(),
                    collection_name: std::env::var("QDRANT_FILES_COLLECTION")
                        .unwrap_or_else(|_| "mnemosyne_docs".to_string()),
                    system_prompt_template: "Use the following relevant documents to answer the user accurately and concisely:\n\n{context}".to_string(),
                    result_type_label: "DOCUMENT".to_string(),
                    payload_fields: vec!["file_name".to_string(), "source_path".to_string(), "content".to_string()],
                },
            ],
            default_system_prompt: None,
            query_transform: QueryTransformConfig::default(),
            crag: CragConfig::default(),
        }
    }
}

pub struct RagPlugin {
    qdrant_url: String,
    config: RagConfig,
    client: reqwest::Client,
    embedding_provider: Option<(Arc<dyn Provider>, ProviderConfig)>,
    llm_client: Option<reqwest::Client>,
}

impl RagPlugin {
    pub fn new(
        qdrant_url: String,
        config: RagConfig,
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
        let llm_client =
            if config.query_transform.expansion_enabled || config.query_transform.hyde_enabled {
                Some(
                    reqwest::Client::builder()
                        .connect_timeout(std::time::Duration::from_secs(10))
                        .timeout(std::time::Duration::from_secs(30))
                        .build()
                        .unwrap_or_default(),
                )
            } else {
                None
            };

        Self {
            qdrant_url,
            config,
            client,
            embedding_provider,
            llm_client,
        }
    }

    async fn get_embedding(&self, text: &str) -> Result<Vec<f32>, PylosError> {
        if let Some((ref provider, ref config)) = self.embedding_provider {
            let req = EmbeddingRequest {
                model: self.config.embedding_model.clone(),
                input: EmbeddingInput::Single(text.to_string()),
                encoding_format: None,
                dimensions: None,
                user: None,
            };
            let resp = provider.embed(&req, config).await.map_err(|e| {
                error!("RagPlugin: Embedding failed: {:?}", e);
                PylosError::Internal(format!("Embedding failed: {}", e))
            })?;
            return resp
                .data
                .into_iter()
                .next()
                .map(|d| d.embedding)
                .ok_or_else(|| {
                    error!("RagPlugin: Empty embedding returned");
                    PylosError::Internal("Empty embedding returned".into())
                });
        }

        Err(PylosError::Internal(
            "RagPlugin: No embedding provider configured".into(),
        ))
    }

    /// Call an LLM for query transformation (expansion or HyDE).
    async fn call_llm(&self, system_prompt: &str, user_prompt: &str) -> Result<String, PylosError> {
        let client = self.llm_client.as_ref().ok_or_else(|| {
            PylosError::Internal("RagPlugin: No LLM client for query transformation".into())
        })?;

        let api_base = self
            .config
            .query_transform
            .transform_api_base
            .as_deref()
            .unwrap_or("http://localhost:3000");
        let url = format!("{}/v1/chat/completions", api_base.trim_end_matches('/'));

        let body = serde_json::json!({
            "model": self.config.query_transform.transform_model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": user_prompt}
            ],
            "temperature": 0.7,
        });

        let mut req = client.post(&url).json(&body);
        if let Some(ref key) = self.config.query_transform.transform_api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let resp = req.send().await.map_err(|e| {
            error!("RagPlugin: Transform LLM call failed: {:?}", e);
            PylosError::Internal(format!("Transform LLM call failed: {}", e))
        })?;

        if !resp.status().is_success() {
            let err = resp.text().await.unwrap_or_default();
            return Err(PylosError::Internal(format!(
                "Transform LLM API error: {}",
                err
            )));
        }

        #[derive(serde::Deserialize)]
        struct LlmResponse {
            choices: Vec<LlmChoice>,
        }
        #[derive(serde::Deserialize)]
        struct LlmChoice {
            message: LlmMessage,
        }
        #[derive(serde::Deserialize)]
        struct LlmMessage {
            content: Option<String>,
        }

        let data: LlmResponse = resp.json().await.map_err(|e| {
            error!("RagPlugin: Failed to parse LLM response: {:?}", e);
            PylosError::Internal(format!("Failed to parse LLM response: {}", e))
        })?;

        data.choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| PylosError::Internal("Empty LLM response".into()))
    }

    /// Generate query variants for Query Expansion.
    async fn expand_query(
        &self,
        query: &str,
        num_variants: usize,
    ) -> Result<Vec<String>, PylosError> {
        let system = "You are a search query expansion assistant. Generate diverse reformulations of the user's query to improve search recall. Return one query per line, no numbering, no explanation.";
        let user = format!(
            "Generate {} different reformulations of this query:\n\n{}",
            num_variants, query
        );

        let response = self.call_llm(system, &user).await?;
        let variants: Vec<String> = response
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| {
                !l.is_empty() && !l.starts_with('-') && !l.starts_with(|c: char| c.is_ascii_digit())
            })
            .take(num_variants)
            .collect();

        if variants.is_empty() {
            Ok(vec![query.to_string()])
        } else {
            let mut all = vec![query.to_string()];
            all.extend(variants);
            Ok(all)
        }
    }

    /// Generate a hypothetical document (HyDE) from the query.
    async fn generate_hyde(&self, query: &str) -> Result<String, PylosError> {
        let system = "You are a search engine. Generate a hypothetical ideal document that would be the perfect answer to the user's query. Write it as a factual document excerpt.";
        let user = format!(
            "Generate a hypothetical document that would perfectly answer this query:\n\n{}",
            query
        );

        self.call_llm(system, &user).await
    }

    /// Search Qdrant for the given query vector and return (context_string, max_score).
    async fn search_qdrant(
        &self,
        query_vector: &[f32],
        collection_name: &str,
        route: &RagModelRoute,
    ) -> Result<(String, f64), PylosError> {
        let search_url = format!(
            "{}/collections/{}/points/search",
            self.qdrant_url.trim_end_matches('/'),
            collection_name
        );
        let search_body = serde_json::json!({
            "vector": query_vector,
            "limit": 5,
            "with_payload": true
        });

        let search_resp = self
            .client
            .post(&search_url)
            .json(&search_body)
            .send()
            .await
            .map_err(|e| {
                error!("RagPlugin: Failed to connect to Qdrant: {:?}", e);
                PylosError::Internal(format!("Failed to connect to Qdrant: {}", e))
            })?;

        if !search_resp.status().is_success() {
            warn!(
                "RagPlugin: Qdrant search returned status: {}",
                search_resp.status()
            );
            return Ok((String::new(), 0.0));
        }

        #[derive(serde::Deserialize)]
        struct QdrantSearchPoint {
            score: Option<f64>,
            payload: Option<serde_json::Value>,
        }
        #[derive(serde::Deserialize)]
        struct QdrantSearchResponse {
            result: Vec<QdrantSearchPoint>,
        }

        let qdrant_resp: QdrantSearchResponse = search_resp.json().await.map_err(|e| {
            error!("RagPlugin: Failed to parse Qdrant response: {:?}", e);
            PylosError::Internal(format!("Failed to parse Qdrant response: {}", e))
        })?;

        if qdrant_resp.result.is_empty() {
            return Ok((String::new(), 0.0));
        }

        let max_score = qdrant_resp
            .result
            .iter()
            .filter_map(|p| p.score)
            .fold(0.0_f64, f64::max);

        let points: Vec<QdrantPoint> = qdrant_resp
            .result
            .into_iter()
            .map(|p| QdrantPoint { payload: p.payload })
            .collect();

        let context = route.format_context(&points);
        Ok((context, max_score))
    }

    /// Search the web using the configured CRAG provider.
    async fn web_search(&self, query: &str) -> Result<String, PylosError> {
        let crag = &self.config.crag;
        let context = match crag.provider.as_str() {
            "tavily" => {
                let api_key = crag
                    .api_key
                    .as_deref()
                    .ok_or_else(|| PylosError::Internal("Tavily API key not configured".into()))?;
                let url = "https://api.tavily.com/search";
                let body = serde_json::json!({
                    "api_key": api_key,
                    "query": query,
                    "max_results": crag.max_results,
                    "include_answer": false,
                });
                let resp = self
                    .client
                    .post(url)
                    .json(&body)
                    .send()
                    .await
                    .map_err(|e| PylosError::Internal(format!("Tavily search failed: {}", e)))?;
                if !resp.status().is_success() {
                    let err = resp.text().await.unwrap_or_default();
                    return Err(PylosError::Internal(format!("Tavily API error: {}", err)));
                }
                #[derive(serde::Deserialize)]
                struct TavilyResult {
                    title: String,
                    content: String,
                    url: String,
                }
                #[derive(serde::Deserialize)]
                struct TavilyResponse {
                    results: Vec<TavilyResult>,
                }
                let data: TavilyResponse = resp
                    .json()
                    .await
                    .map_err(|e| PylosError::Internal(format!("Tavily parse error: {}", e)))?;
                let mut text = String::new();
                for (i, r) in data.results.iter().enumerate() {
                    text.push_str(&format!(
                        "--- WEB RESULT #{} ---\nTitle: {}\nURL: {}\nContent:\n{}\n\n",
                        i + 1,
                        r.title,
                        r.url,
                        r.content
                    ));
                }
                text
            }
            _ => {
                return Err(PylosError::Internal(format!(
                    "Unsupported CRAG provider: {}",
                    crag.provider
                )));
            }
        };
        Ok(context)
    }

    /// Build a system message with the RAG context.
    fn build_context_message(
        route: &RagModelRoute,
        context_text: &str,
    ) -> Option<ChatCompletionMessage> {
        if context_text.is_empty() {
            return None;
        }
        let system_prompt = route
            .system_prompt_template
            .replace("{context}", context_text);
        Some(ChatCompletionMessage {
            role: MessageRole::System,
            content: Some(system_prompt),
            ..Default::default()
        })
    }
}

/// Merge multiple (context, score) results, deduplicating by line.
fn merge_contexts(results: &[(String, f64)]) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut merged = String::new();
    for (ctx, _score) in results {
        for line in ctx.lines() {
            if seen.insert(line.to_string()) {
                merged.push_str(line);
                merged.push('\n');
            }
        }
    }
    merged
}

#[async_trait]
impl LlmPlugin for RagPlugin {
    fn name(&self) -> &str {
        "rag"
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        _ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        let chat_req = match request {
            PylosRequest::ChatCompletion(ref req) => req,
            _ => return Ok(None),
        };

        // Find matching route
        let route = match self
            .config
            .routes
            .iter()
            .find(|r| r.matches(&chat_req.model))
        {
            Some(r) => r,
            None => return Ok(None),
        };

        let collection_name = &route.collection_name;

        info!(
            "RagPlugin: Intercepted {} request (targeting collection: {})",
            chat_req.model, collection_name
        );

        // 1. Extract the latest user query
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

        // 2. Build search queries (with optional transformation)
        let search_texts: Vec<String> = if self.config.query_transform.expansion_enabled {
            match self
                .expand_query(&user_query, self.config.query_transform.num_variants)
                .await
            {
                Ok(variants) => {
                    info!("RagPlugin: Query expanded to {} variants", variants.len());
                    variants
                }
                Err(e) => {
                    warn!("RagPlugin: Query expansion failed: {:?}, using original", e);
                    vec![user_query.clone()]
                }
            }
        } else {
            vec![user_query.clone()]
        };

        // 3. Generate HyDE document and add to search texts
        let mut hyde_text: Option<String> = None;
        if self.config.query_transform.hyde_enabled {
            match self.generate_hyde(&user_query).await {
                Ok(hd) => {
                    hyde_text = Some(hd.clone());
                    info!("RagPlugin: HyDE document generated ({} chars)", hd.len());
                }
                Err(e) => {
                    warn!("RagPlugin: HyDE generation failed: {:?}", e);
                }
            }
        }

        // 4. Fetch embeddings for all search texts (including HyDE)
        let mut all_vectors: Vec<Vec<f32>> = Vec::with_capacity(search_texts.len());
        for text in &search_texts {
            match self.get_embedding(text).await {
                Ok(v) => all_vectors.push(v),
                Err(e) => warn!("RagPlugin: Embedding failed for variant: {:?}", e),
            }
        }
        if let Some(ref hd) = hyde_text {
            if let Ok(v) = self.get_embedding(hd).await {
                all_vectors.push(v);
            }
        }

        if all_vectors.is_empty() {
            return Ok(None);
        }

        // 5. Search Qdrant in parallel for all vectors
        use futures::future::join_all;
        let searches: Vec<_> = all_vectors
            .iter()
            .map(|vec| self.search_qdrant(vec, collection_name, route))
            .collect();

        let search_results: Vec<(String, f64)> = join_all(searches)
            .await
            .into_iter()
            .filter_map(|r| r.ok())
            .collect();

        // 6. Check max score for CRAG (Corrective RAG)
        let max_score = search_results
            .iter()
            .map(|(_, score)| *score)
            .fold(0.0_f64, f64::max);

        let final_context = if self.config.crag.enabled && max_score < self.config.crag.threshold {
            info!(
                "RagPlugin: CRAG triggered — max Qdrant score {:.4} below threshold {:.4}, searching web",
                max_score,
                self.config.crag.threshold
            );
            match self.web_search(&user_query).await {
                Ok(web_ctx) => {
                    if web_ctx.is_empty() {
                        info!("RagPlugin: CRAG web search returned no results");
                        String::new()
                    } else {
                        web_ctx
                    }
                }
                Err(e) => {
                    warn!(
                        "RagPlugin: CRAG web search failed: {:?}, using Qdrant results",
                        e
                    );
                    // Fall back to Qdrant context
                    merge_contexts(&search_results)
                }
            }
        } else {
            if self.config.crag.enabled {
                info!(
                    "RagPlugin: CRAG not triggered — max score {:.4} >= threshold {:.4}",
                    max_score, self.config.crag.threshold
                );
            }
            merge_contexts(&search_results)
        };

        // 7. Build augmented messages and change model
        if let PylosRequest::ChatCompletion(ref mut req) = request {
            if !final_context.is_empty() {
                if let Some(ctx_msg) = Self::build_context_message(route, &final_context) {
                    req.messages.insert(0, ctx_msg);
                }
            }
            req.model = self.config.pylos_model.clone();
        }

        Ok(None)
    }
}
