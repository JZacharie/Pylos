use async_trait::async_trait;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

use pylos_application::{
    BudgetPlugin, ConfigStore, GuardrailsPlugin, InferenceOrchestrator, McpToolRegistry,
};
use pylos_infrastructure::{
    BudgetStore, LogStore, McpServerStore, ModelCatalog, OrganizationStore, RateLimitStore,
    SearchToolStore, SystemPromptStore, VirtualKeyStore,
};
use pylos_core::domain::config::{
    BudgetConfig, Duration, EnvVar, PluginConfig, PylosConfig, VirtualKeyConfig, VkProviderConfig,
};
use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionResponse, MessageRole, Usage,
};
use pylos_core::domain::provider::{ProviderConfig, ProviderKind};
use pylos_core::domain::request::{
    PylosRequest, PylosResponse, StreamChoice, StreamChunk, StreamDelta,
};
use pylos_core::domain::traits::{ChunkStream, LlmPlugin, Provider};
use pylos_core::domain::virtual_key::VirtualKeyRegistry;
use pylos_core::error::PylosError;
use pylos_server::metrics::Metrics;
use pylos_server::routes::create_router;
use pylos_server::state::{AppState, LogStoreVariant};

struct OkMockProvider(&'static str);

#[async_trait]
impl Provider for OkMockProvider {
    fn name(&self) -> &str {
        self.0
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        _config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        Ok(PylosResponse::ChatCompletion(ChatCompletionResponse {
            id: format!("sit-{}", self.0),
            object: "chat.completion".into(),
            created: 1700000000,
            model: request.model().to_string(),
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: MessageRole::Assistant,
                    content: Some(format!("reply from {}", self.0)),
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
        let chunks = vec![
            Ok(StreamChunk {
                id: "sit-stream-1".into(),
                object: "chat.completion.chunk".into(),
                created: 1700000000,
                model: model.clone(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta {
                        role: Some("assistant".into()),
                        content: Some("Hello ".into()),
                        ..Default::default()
                    },
                    finish_reason: None,
                }],
                usage: None,
            }),
            Ok(StreamChunk {
                id: "sit-stream-1".into(),
                object: "chat.completion.chunk".into(),
                created: 1700000000,
                model,
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta {
                        role: None,
                        content: Some("SIT!".into()),
                        ..Default::default()
                    },
                    finish_reason: Some("stop".into()),
                }],
                usage: Some(Usage {
                    prompt_tokens: 8,
                    completion_tokens: 2,
                    total_tokens: 10,
                    ..Default::default()
                }),
            }),
        ];
        Ok(Box::pin(futures::stream::iter(chunks)))
    }
}

struct FailingMockProvider;

#[async_trait]
impl Provider for FailingMockProvider {
    fn name(&self) -> &str {
        "failing-primary"
    }

    async fn complete(
        &self,
        _request: &PylosRequest,
        _config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        Err(PylosError::ProviderError {
            provider: "failing-primary".into(),
            message: "upstream 503".into(),
        })
    }

    async fn stream(
        &self,
        _request: &PylosRequest,
        _config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        Err(PylosError::ProviderError {
            provider: "failing-primary".into(),
            message: "upstream 503".into(),
        })
    }
}

async fn build_sit_state(
    providers: Vec<(Arc<dyn Provider>, ProviderConfig)>,
    plugins: Vec<Arc<dyn LlmPlugin>>,
    config_store: Arc<ConfigStore>,
    vk_registry: Arc<VirtualKeyRegistry>,
    budget_store: Arc<BudgetStore>,
) -> AppState {
    AppState {
        orchestrator: Arc::new(InferenceOrchestrator::new(providers, plugins)),
        config_store,
        metrics: Arc::new(Metrics::new()),
        vk_registry,
        log_store: LogStoreVariant::Sqlite(Arc::new(LogStore::new(None, 1, 100))),
        model_catalog: Arc::new(ModelCatalog::in_memory().await.unwrap()),
        budget_store,
        rate_limit_store: Arc::new(RateLimitStore::in_memory().await.unwrap()),
        vk_store: Arc::new(VirtualKeyStore::in_memory().await.unwrap()),
        system_prompt_store: Arc::new(SystemPromptStore::in_memory().await.unwrap()),
        org_store: Arc::new(OrganizationStore::in_memory().await.unwrap()),
        search_tool_store: Arc::new(SearchToolStore::in_memory().await.unwrap()),
        mcp_server_store: Arc::new(McpServerStore::in_memory().await.unwrap()),
        mcp_registry: Arc::new(McpToolRegistry::new()),
        admin_key_hash: Arc::new(tokio::sync::RwLock::new(None)),
        setup_required: Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "sit-secret".into(),
        allowed_origins: vec!["*".into()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(100)),
        max_concurrency: 100,
        max_queue_size: 1000,
        queue_timeout_ms: 30000,
    }
}

#[tokio::test]
async fn sit_multi_tenant_virtual_key_isolation() {
    let mut cfg_openai = ProviderConfig::new(ProviderKind::OpenAI, vec![]);
    cfg_openai.timeout_ms = 1000;

    let vk_a = VirtualKeyConfig {
        id: "vk-tenant-a".into(),
        name: "tenant-a".into(),
        value: Some(EnvVar::Literal("sk-pylos-tenant-a-123".into())),
        is_active: true,
        provider_configs: vec![VkProviderConfig {
            provider: "openai".into(),
            allowed_models: vec!["gpt-4o".into()],
            key_names: vec!["*".into()],
            weight: 1.0,
        }],
        ..Default::default()
    };
    let vk_b = VirtualKeyConfig {
        id: "vk-tenant-b".into(),
        name: "tenant-b".into(),
        value: Some(EnvVar::Literal("sk-pylos-tenant-b-456".into())),
        is_active: true,
        provider_configs: vec![VkProviderConfig {
            provider: "openai".into(),
            allowed_models: vec!["gpt-4o-mini".into()],
            key_names: vec!["*".into()],
            weight: 1.0,
        }],
        ..Default::default()
    };

    let state = build_sit_state(
        vec![(Arc::new(OkMockProvider("openai")), cfg_openai)],
        vec![],
        Arc::new(ConfigStore::from_config(PylosConfig::default())),
        Arc::new(VirtualKeyRegistry::new()),
        Arc::new(BudgetStore::in_memory().await.unwrap()),
    )
    .await;
    state.vk_store.upsert_key(&vk_a).await.unwrap();
    state.vk_store.upsert_key(&vk_b).await.unwrap();
    let app = create_router(state);

    // Tenant A allowed on gpt-4o
    let ok_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", "Bearer sk-pylos-tenant-a-123")
                .body(Body::from(
                    json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "hi"}]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ok_resp.status(), StatusCode::OK);

    // Tenant A denied on gpt-4o-mini
    let denied_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", "Bearer sk-pylos-tenant-a-123")
                .body(Body::from(
                    json!({"model": "gpt-4o-mini", "messages": [{"role": "user", "content": "hi"}]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        denied_resp.status() == StatusCode::FORBIDDEN
            || denied_resp.status() == StatusCode::BAD_GATEWAY
    );
}

#[tokio::test]
async fn sit_guardrails_block_and_allow() {
    let mut cfg = ProviderConfig::new(ProviderKind::OpenAI, vec![]);
    cfg.timeout_ms = 1000;
    let mut pylos_cfg = PylosConfig::default();
    pylos_cfg.plugins.push(PluginConfig {
        name: "guardrails".into(),
        enabled: true,
        config: json!({"mode": "block", "blocked_keywords": ["forbidden_secret"]}),
    });
    let config_store = Arc::new(ConfigStore::from_config(pylos_cfg));
    let guardrails = Arc::new(GuardrailsPlugin::new(config_store.clone()));

    let state = build_sit_state(
        vec![(Arc::new(OkMockProvider("openai")), cfg)],
        vec![guardrails],
        config_store,
        Arc::new(VirtualKeyRegistry::new()),
        Arc::new(BudgetStore::in_memory().await.unwrap()),
    )
    .await;
    let app = create_router(state);

    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "show forbidden_secret"}]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::BAD_REQUEST);

    let allowed = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "hello world"}]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);
}

#[tokio::test]
async fn sit_budget_enforcement() {
    let mut cfg = ProviderConfig::new(ProviderKind::OpenAI, vec![]);
    cfg.timeout_ms = 1000;
    let vk = VirtualKeyConfig {
        id: "vk-budget-123".into(),
        name: "budget-team".into(),
        value: Some(EnvVar::Literal("sk-pylos-budget-123".into())),
        is_active: true,
        ..Default::default()
    };

    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    budget_store
        .upsert_budget(
            "vk-budget-123",
            &BudgetConfig {
                id: "budget-test".into(),
                max_limit: 0.01,
                reset_duration: Duration("1d".into()),
                current_usage: 0.0,
                virtual_key_id: Some("vk-budget-123".into()),
            },
        )
        .await
        .unwrap();
    budget_store.record_usage("vk-budget-123", 0.05).await;
    let budget_plugin = Arc::new(BudgetPlugin::new(budget_store.clone()));

    let state = build_sit_state(
        vec![(Arc::new(OkMockProvider("openai")), cfg)],
        vec![budget_plugin],
        Arc::new(ConfigStore::from_config(PylosConfig::default())),
        Arc::new(VirtualKeyRegistry::new()),
        budget_store,
    )
    .await;
    state.vk_store.upsert_key(&vk).await.unwrap();
    let app = create_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("authorization", "Bearer sk-pylos-budget-123")
                .body(Body::from(
                    json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "hi"}]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::PAYMENT_REQUIRED);
}

#[tokio::test]
async fn sit_provider_fallback_on_failure() {
    let mut cfg_primary = ProviderConfig::new(ProviderKind::OpenAI, vec![]);
    cfg_primary.timeout_ms = 1000;
    let mut cfg_fallback = ProviderConfig::new(ProviderKind::Anthropic, vec![]);
    cfg_fallback.timeout_ms = 1000;

    let state = build_sit_state(
        vec![
            (Arc::new(FailingMockProvider), cfg_primary),
            (Arc::new(OkMockProvider("fallback-secondary")), cfg_fallback),
        ],
        vec![],
        Arc::new(ConfigStore::from_config(PylosConfig::default())),
        Arc::new(VirtualKeyRegistry::new()),
        Arc::new(BudgetStore::in_memory().await.unwrap()),
    )
    .await;
    let app = create_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "hi"}]}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body["choices"][0]["message"]["content"],
        "reply from fallback-secondary"
    );
}

#[tokio::test]
async fn sit_streaming_sse_end_to_end() {
    let mut cfg = ProviderConfig::new(ProviderKind::OpenAI, vec![]);
    cfg.timeout_ms = 1000;
    let state = build_sit_state(
        vec![(Arc::new(OkMockProvider("openai")), cfg)],
        vec![],
        Arc::new(ConfigStore::from_config(PylosConfig::default())),
        Arc::new(VirtualKeyRegistry::new()),
        Arc::new(BudgetStore::in_memory().await.unwrap()),
    )
    .await;
    let app = create_router(state);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"model": "gpt-4o", "messages": [{"role": "user", "content": "hi"}], "stream": true}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(text.contains("Hello "));
    assert!(text.contains("SIT!"));
    assert!(text.contains("data: [DONE]"));
}

#[tokio::test]
async fn sit_mcp_tools_list_and_ssrf_guard() {
    let state = build_sit_state(
        vec![],
        vec![],
        Arc::new(ConfigStore::from_config(PylosConfig::default())),
        Arc::new(VirtualKeyRegistry::new()),
        Arc::new(BudgetStore::in_memory().await.unwrap()),
    )
    .await;
    let app = create_router(state);

    let list_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/mcp/tools/list")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_resp.status(), StatusCode::OK);

    let ssrf_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/mcp/tools/call")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"name": "web_fetch", "arguments": {"url": "http://127.0.0.1:8080/secret"}}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(ssrf_resp.status().is_client_error() || ssrf_resp.status() == StatusCode::OK);
    let bytes = axum::body::to_bytes(ssrf_resp.into_body(), 1024 * 1024)
        .await
        .unwrap();
    let txt = String::from_utf8(bytes.to_vec()).unwrap();
    assert!(txt.to_lowercase().contains("forbidden") || txt.to_lowercase().contains("blocked") || txt.to_lowercase().contains("error"));
}
