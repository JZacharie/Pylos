use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt; // for oneshot

use pylos_application::{
    BudgetStore, ConfigStore, InferenceOrchestrator, LogStore, ModelCatalog, OrganizationStore,
    RateLimitStore, SearchToolStore, SystemPromptStore,
};
use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionResponse, MessageRole, Usage,
};
use pylos_core::domain::provider::{ProviderConfig, ProviderKind};
use pylos_core::domain::request::{
    PylosRequest, PylosResponse, StreamChoice, StreamChunk, StreamDelta,
};
use pylos_core::domain::traits::{ChunkStream, Provider};
use pylos_core::error::PylosError;
use pylos_server::metrics::Metrics;
use pylos_server::routes::create_router;
use pylos_server::state::{AppState, LogStoreVariant};

use async_trait::async_trait;

struct MockProvider;

#[async_trait]
impl Provider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        _config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let model = request.model().to_string();
        Ok(PylosResponse::ChatCompletion(ChatCompletionResponse {
            id: "mock-id".into(),
            object: "chat.completion".into(),
            created: 123456789,
            model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: MessageRole::Assistant,
                    content: Some("Hello from Mock!".into()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    ..Default::default()
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                ..Default::default()
            }),
        }))
    }

    async fn stream(
        &self,
        request: &PylosRequest,
        _config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        let model = request.model().to_string();
        let chunk1 = StreamChunk {
            id: "mock-id".into(),
            object: "chat.completion.chunk".into(),
            created: 123456789,
            model: model.clone(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".into()),
                    content: None,
                    tool_calls: None,
                    ..Default::default()
                },
                finish_reason: None,
            }],
            usage: None,
        };
        let chunk2 = StreamChunk {
            id: "mock-id".into(),
            object: "chat.completion.chunk".into(),
            created: 123456789,
            model,
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: None,
                    content: Some("Hello".into()),
                    tool_calls: None,
                    ..Default::default()
                },
                finish_reason: Some("stop".into()),
            }],
            usage: None,
        };

        let stream = futures::stream::iter(vec![Ok(chunk1), Ok(chunk2)]);
        Ok(Box::pin(stream))
    }

    async fn generate_image(
        &self,
        request: &pylos_core::domain::image::ImageRequest,
        _config: &ProviderConfig,
    ) -> Result<pylos_core::domain::image::ImageResponse, PylosError> {
        Ok(pylos_core::domain::image::ImageResponse {
            created: 123456789,
            data: vec![pylos_core::domain::image::ImageData {
                url: Some("http://mock-url.com/image.png".to_string()),
                b64_json: None,
                revised_prompt: Some(request.prompt.clone()),
            }],
        })
    }
}

#[tokio::test]
async fn test_chat_completions_unary() {
    // 1. Setup Mock State
    let mock_provider = Arc::new(MockProvider);
    let mut config = ProviderConfig::new(ProviderKind::OpenAI, vec![]);
    config.timeout_ms = 1000;

    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![(mock_provider, config)],
        vec![],
    ));

    // Minimal mock stores
    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
    let log_store = Arc::new(LogStore::new(None, 1, 100)); // In-memory log store
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());

    let vk_store = Arc::new(
        pylos_application::VirtualKeyStore::in_memory()
            .await
            .unwrap(),
    );

    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let state = AppState {
        orchestrator,
        config_store,
        metrics,
        vk_registry,
        log_store: LogStoreVariant::Sqlite(log_store),
        model_catalog,
        budget_store,
        rate_limit_store,
        vk_store,
        system_prompt_store,
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(
            pylos_application::McpServerStore::in_memory()
                .await
                .unwrap(),
        ),
        admin_key_hash: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_secret".to_string(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
        max_concurrency: 100,
        max_queue_size: 1000,
        queue_timeout_ms: 30000,
    };

    // 2. Create Router
    let app = create_router(state);

    // 3. Send Request
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "Hi"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. Assertions
    assert_eq!(response.status(), StatusCode::OK);

    let body = ax_body_to_json(response.into_body()).await;
    assert_eq!(body["id"], "mock-id");
    assert_eq!(body["choices"][0]["message"]["content"], "Hello from Mock!");
}

#[tokio::test]
async fn test_chat_completions_stream() {
    // 1. Setup Mock State
    let mock_provider = Arc::new(MockProvider);
    let config = ProviderConfig::new(ProviderKind::OpenAI, vec![]);

    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![(mock_provider, config)],
        vec![],
    ));

    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
    let log_store = Arc::new(LogStore::new(None, 1, 100));
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());

    let vk_store = Arc::new(
        pylos_application::VirtualKeyStore::in_memory()
            .await
            .unwrap(),
    );

    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let state = AppState {
        orchestrator,
        config_store,
        metrics,
        vk_registry,
        log_store: LogStoreVariant::Sqlite(log_store),
        model_catalog,
        budget_store,
        rate_limit_store,
        vk_store,
        system_prompt_store,
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(
            pylos_application::McpServerStore::in_memory()
                .await
                .unwrap(),
        ),
        admin_key_hash: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_secret".to_string(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
        max_concurrency: 100,
        max_queue_size: 1000,
        queue_timeout_ms: 30000,
    };

    // 2. Create Router
    let app = create_router(state);

    // 3. Send Request
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "Hi"}],
                        "stream": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. Assertions
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "text/event-stream");

    let body = response.into_body();
    let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();

    // Check SSE format
    assert!(text.contains("data: {\"id\":\"mock-id\""));
    assert!(text.contains("data: [DONE]"));
    assert!(text.contains("content\":\"Hello\""));
}

#[tokio::test]
async fn test_image_generations() {
    // 1. Setup Mock State
    let mock_provider = Arc::new(MockProvider);
    let mut config = ProviderConfig::new(ProviderKind::OpenAI, vec![]);
    config.timeout_ms = 1000;

    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![(mock_provider, config)],
        vec![],
    ));

    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
    let log_store = Arc::new(LogStore::new(None, 1, 100));
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());

    let vk_store = Arc::new(
        pylos_application::VirtualKeyStore::in_memory()
            .await
            .unwrap(),
    );

    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let state = AppState {
        orchestrator,
        config_store,
        metrics,
        vk_registry,
        log_store: LogStoreVariant::Sqlite(log_store),
        model_catalog,
        budget_store,
        rate_limit_store,
        vk_store,
        system_prompt_store,
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(
            pylos_application::McpServerStore::in_memory()
                .await
                .unwrap(),
        ),
        admin_key_hash: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_secret".to_string(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
        max_concurrency: 100,
        max_queue_size: 1000,
        queue_timeout_ms: 30000,
    };

    // 2. Create Router
    let app = create_router(state);

    // 3. Send Request
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/images/generations")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "prompt": "a futuristic city",
                        "model": "dall-e-3",
                        "n": 1
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // 4. Assertions
    assert_eq!(response.status(), StatusCode::OK);

    let body = ax_body_to_json(response.into_body()).await;
    assert_eq!(body["created"], 123456789);
    assert_eq!(body["data"][0]["url"], "http://mock-url.com/image.png");
    assert_eq!(body["data"][0]["revised_prompt"], "a futuristic city");
}

async fn ax_body_to_json(body: Body) -> Value {
    let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

struct CustomMockProvider {
    name: String,
    response_text: String,
}

#[async_trait]
impl Provider for CustomMockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        _config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let model = request.model().to_string();
        Ok(PylosResponse::ChatCompletion(ChatCompletionResponse {
            id: "mock-id".into(),
            object: "chat.completion".into(),
            created: 123456789,
            model,
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: MessageRole::Assistant,
                    content: Some(self.response_text.clone()),
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                    ..Default::default()
                },
                finish_reason: Some("stop".into()),
            }],
            usage: Some(Usage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                ..Default::default()
            }),
        }))
    }

    async fn stream(
        &self,
        _request: &PylosRequest,
        _config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        Err(PylosError::Unsupported("Not implemented".into()))
    }
}

#[tokio::test]
async fn test_a2a_allowed_models_routing() {
    let mock_default = Arc::new(CustomMockProvider {
        name: "custom-default".into(),
        response_text: "Default Provider Response".into(),
    });
    let mock_mnemosyne = Arc::new(CustomMockProvider {
        name: "custom-mnemosyne".into(),
        response_text: "Mnemosyne Search Response".into(),
    });

    let mut config_default =
        ProviderConfig::new(ProviderKind::Custom("custom-default".into()), vec![]);
    config_default.allowed_models = vec!["gpt-4o".into()];

    let mut config_mnemosyne =
        ProviderConfig::new(ProviderKind::Custom("custom-mnemosyne".into()), vec![]);
    config_mnemosyne.allowed_models = vec!["mnemosyne-search".into()];

    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![
            (mock_default, config_default),
            (mock_mnemosyne, config_mnemosyne),
        ],
        vec![],
    ));

    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
    let log_store = Arc::new(LogStore::new(None, 1, 100));
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());
    let vk_store = Arc::new(
        pylos_application::VirtualKeyStore::in_memory()
            .await
            .unwrap(),
    );

    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let state = AppState {
        orchestrator,
        config_store,
        metrics,
        vk_registry,
        log_store: LogStoreVariant::Sqlite(log_store),
        model_catalog,
        budget_store,
        rate_limit_store,
        vk_store,
        system_prompt_store,
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(
            pylos_application::McpServerStore::in_memory()
                .await
                .unwrap(),
        ),
        admin_key_hash: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_secret".to_string(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
        max_concurrency: 100,
        max_queue_size: 1000,
        queue_timeout_ms: 30000,
    };

    let app = create_router(state);

    // 1. Request mnemosyne-search. Should route to custom-mnemosyne provider.
    let response_mnemosyne = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "mnemosyne-search",
                        "messages": [{"role": "user", "content": "Search for AI models"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_mnemosyne.status(), StatusCode::OK);
    let body_mnemosyne = ax_body_to_json(response_mnemosyne.into_body()).await;
    assert_eq!(
        body_mnemosyne["choices"][0]["message"]["content"],
        "Mnemosyne Search Response"
    );

    // 2. Request gpt-4o. Should route to custom-default provider.
    let response_default = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "Hello"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response_default.status(), StatusCode::OK);
    let body_default = ax_body_to_json(response_default.into_body()).await;
    assert_eq!(
        body_default["choices"][0]["message"]["content"],
        "Default Provider Response"
    );
}

#[tokio::test]
async fn test_semantic_caching_flow() {
    use axum::routing::{post, put};
    use std::sync::Mutex as StdMutex;

    // 1. Set up standard mock DB state to hold points
    #[derive(Clone, Default)]
    struct MockDb {
        points: Arc<StdMutex<Vec<serde_json::Value>>>,
    }
    let db = MockDb::default();
    let db_search = db.clone();
    let db_upsert = db.clone();

    // 2. Build mock server router
    let mock_server_app = axum::Router::new()
        .route(
            "/v1/embeddings",
            post(|axum::Json(_body): axum::Json<serde_json::Value>| async {
                axum::Json(json!({
                    "data": [
                        {
                            "embedding": [0.1, 0.2, 0.3]
                        }
                    ]
                }))
            }),
        )
        .route("/collections/test_cache", put(|| async { StatusCode::OK }))
        .route(
            "/collections/test_cache/points/search",
            post(
                move |axum::Json(_body): axum::Json<serde_json::Value>| async move {
                    let guard = db_search.points.lock().unwrap();
                    if let Some(pt) = guard.first() {
                        axum::Json(json!({
                            "result": [
                                {
                                    "score": 0.99,
                                    "payload": pt["payload"]
                                }
                            ]
                        }))
                    } else {
                        axum::Json(json!({
                            "result": []
                        }))
                    }
                },
            ),
        )
        .route(
            "/collections/test_cache/points",
            post(
                move |axum::Json(body): axum::Json<serde_json::Value>| async move {
                    let mut guard = db_upsert.points.lock().unwrap();
                    if let Some(pts) = body["points"].as_array() {
                        for p in pts {
                            guard.push(p.clone());
                        }
                    }
                    StatusCode::OK
                },
            ),
        );

    // 3. Start mock HTTP server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    let mock_server_url = format!("http://{}", local_addr);

    tokio::spawn(async move {
        axum::serve(listener, mock_server_app).await.unwrap();
    });

    // 4. Create SemanticCachePlugin and register in orchestrator
    let cache_plugin = Arc::new(pylos_application::SemanticCachePlugin::new(
        mock_server_url.clone(),  // qdrant_url
        "test_cache".to_string(), // collection_name
        "text-embedding-3-small".to_string(),
        0.90, // score threshold
        3600, // ttl
        None, // no direct embedding provider
    ));

    // Spin up mock provider
    let mock_provider = Arc::new(CustomMockProvider {
        name: "mock-for-cache".into(),
        response_text: "Fresh Response From Provider".into(),
    });
    let config = ProviderConfig::new(ProviderKind::OpenAI, vec![]);

    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![(mock_provider, config)],
        vec![cache_plugin],
    ));

    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
    let log_store = Arc::new(LogStore::new(None, 1, 100));
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());
    let vk_store = Arc::new(
        pylos_application::VirtualKeyStore::in_memory()
            .await
            .unwrap(),
    );

    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let state = AppState {
        orchestrator,
        config_store,
        metrics,
        vk_registry,
        log_store: LogStoreVariant::Sqlite(log_store),
        model_catalog,
        budget_store,
        rate_limit_store,
        vk_store,
        system_prompt_store,
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(
            pylos_application::McpServerStore::in_memory()
                .await
                .unwrap(),
        ),
        admin_key_hash: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_secret".to_string(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
        max_concurrency: 100,
        max_queue_size: 1000,
        queue_timeout_ms: 30000,
    };

    let app = create_router(state);

    // 5. Send FIRST request (Cache MISS) -> Should call provider and return "Fresh Response From Provider"
    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "How's the weather today?"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = ax_body_to_json(response1.into_body()).await;
    assert_eq!(
        body1["choices"][0]["message"]["content"],
        "Fresh Response From Provider"
    );

    // Wait a brief moment to ensure post-hook's async task/calls are processed if any,
    // though post_hook is awaited in the orchestrator pipeline.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 6. Send SECOND request (Cache HIT) -> Should hit semantic cache and return "Fresh Response From Provider"
    // even if we change the mock provider response now!
    let response2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "How is the weather today?"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = ax_body_to_json(response2.into_body()).await;
    assert_eq!(
        body2["choices"][0]["message"]["content"],
        "Fresh Response From Provider"
    );
}

#[tokio::test]
async fn test_structured_outputs_validation() {
    // 1. Mock provider returning invalid JSON format
    struct BadSchemaMock;
    #[async_trait]
    impl Provider for BadSchemaMock {
        fn name(&self) -> &str {
            "bad-schema"
        }
        async fn complete(
            &self,
            request: &PylosRequest,
            _config: &ProviderConfig,
        ) -> Result<PylosResponse, PylosError> {
            let model = request.model().to_string();
            Ok(PylosResponse::ChatCompletion(ChatCompletionResponse {
                id: "mock-id".into(),
                object: "chat.completion".into(),
                created: 123456789,
                model,
                choices: vec![ChatCompletionChoice {
                    index: 0,
                    message: ChatCompletionMessage {
                        role: MessageRole::Assistant,
                        content: Some("{\"age\": \"not-a-number\"}".into()),
                        ..Default::default()
                    },
                    finish_reason: Some("stop".into()),
                }],
                usage: None,
            }))
        }
        async fn stream(
            &self,
            _r: &PylosRequest,
            _c: &ProviderConfig,
        ) -> Result<ChunkStream, PylosError> {
            Err(PylosError::Unsupported("Not implemented".into()))
        }
    }

    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![(
            Arc::new(BadSchemaMock),
            ProviderConfig::new(ProviderKind::OpenAI, vec![]),
        )],
        vec![Arc::new(pylos_application::StructuredOutputPlugin::new())],
    ));

    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
    let log_store = Arc::new(LogStore::new(None, 1, 100));
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());
    let vk_store = Arc::new(
        pylos_application::VirtualKeyStore::in_memory()
            .await
            .unwrap(),
    );

    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let state = AppState {
        orchestrator,
        config_store,
        metrics,
        vk_registry,
        log_store: LogStoreVariant::Sqlite(log_store),
        model_catalog,
        budget_store,
        rate_limit_store,
        vk_store,
        system_prompt_store,
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(
            pylos_application::McpServerStore::in_memory()
                .await
                .unwrap(),
        ),
        admin_key_hash: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_secret".to_string(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(10)),
        max_concurrency: 10,
        max_queue_size: 100,
        queue_timeout_ms: 30000,
    };

    let app = create_router(state);

    // Send request requiring schema: age must be integer
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "Tell me age"}],
                        "response_format": {
                            "type": "json_schema",
                            "json_schema": {
                                "type": "object",
                                "properties": {
                                    "age": { "type": "integer" }
                                },
                                "required": ["age"]
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Must be BAD_REQUEST (400) because validation fails
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_inference_queuing_and_timeout() {
    // Mock slow provider
    struct SlowMock;
    #[async_trait]
    impl Provider for SlowMock {
        fn name(&self) -> &str {
            "slow"
        }
        async fn complete(
            &self,
            request: &PylosRequest,
            _config: &ProviderConfig,
        ) -> Result<PylosResponse, PylosError> {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            let model = request.model().to_string();
            Ok(PylosResponse::ChatCompletion(ChatCompletionResponse {
                id: "slow-id".into(),
                object: "chat.completion".into(),
                created: 123456789,
                model,
                choices: vec![ChatCompletionChoice {
                    index: 0,
                    message: ChatCompletionMessage {
                        role: MessageRole::Assistant,
                        content: Some("Slow Hello".into()),
                        ..Default::default()
                    },
                    finish_reason: Some("stop".into()),
                }],
                usage: None,
            }))
        }
        async fn stream(
            &self,
            _r: &PylosRequest,
            _c: &ProviderConfig,
        ) -> Result<ChunkStream, PylosError> {
            Err(PylosError::Unsupported("Not implemented".into()))
        }
    }

    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![(
            Arc::new(SlowMock),
            ProviderConfig::new(ProviderKind::OpenAI, vec![]),
        )],
        vec![],
    ));

    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(pylos_core::domain::virtual_key::VirtualKeyRegistry::new());
    let log_store = Arc::new(LogStore::new(None, 1, 100));
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());
    let vk_store = Arc::new(
        pylos_application::VirtualKeyStore::in_memory()
            .await
            .unwrap(),
    );

    // Concurrency limit: 1, Max queue size: 1, Queue timeout: 100ms
    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let state = AppState {
        orchestrator,
        config_store,
        metrics,
        vk_registry,
        log_store: LogStoreVariant::Sqlite(log_store),
        model_catalog,
        budget_store,
        rate_limit_store,
        vk_store,
        system_prompt_store,
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(
            pylos_application::McpServerStore::in_memory()
                .await
                .unwrap(),
        ),
        admin_key_hash: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_secret".to_string(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(1)),
        max_concurrency: 1,
        max_queue_size: 1,
        queue_timeout_ms: 100,
    };

    let app = create_router(state);

    // 1. Send first request (occupies the single permit)
    let app1 = app.clone();
    let handle1 = tokio::spawn(async move {
        app1.oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "Req 1"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
    });

    // Let the first request start processing
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // 2. Send second request (will queue, queue size is 1)
    let app2 = app.clone();
    let handle2 = tokio::spawn(async move {
        app2.oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "Req 2"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
    });

    // Let the second request queue
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // 3. Send third request (should be immediately rejected because queue size is 1 and already full)
    let response3 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "model": "gpt-4o",
                        "messages": [{"role": "user", "content": "Req 3"}]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response3.status(), StatusCode::TOO_MANY_REQUESTS);

    // Wait for the first and second to finish
    let resp1 = handle1.await.unwrap();
    let resp2 = handle2.await.unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    // The second request should timeout because 100ms queue timeout < 300ms execution time of req 1
    assert_eq!(resp2.status(), StatusCode::GATEWAY_TIMEOUT);
}
