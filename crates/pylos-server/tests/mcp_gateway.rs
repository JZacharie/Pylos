use std::collections::HashMap;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use pylos_application::use_cases::inference::InferenceOrchestrator;
use pylos_application::{
    BudgetStore, ConfigStore, LogStore, McpServerStore, McpToolRegistry, ModelCatalog,
    OrganizationStore, RateLimitStore, SearchToolStore, SystemPromptStore, VirtualKeyStore,
};
use pylos_core::domain::mcp_server::{
    McpServer, McpServerStatus, McpServerType, McpTransportConfig,
};
use pylos_core::domain::openai::Tool;
use pylos_core::domain::virtual_key::VirtualKeyRegistry;
use pylos_server::metrics::Metrics;
use pylos_server::routes::create_router;
use pylos_server::state::{AppState, LogStoreVariant};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn ax_body_to_json(body: Body) -> Value {
    let bytes = axum::body::to_bytes(body, 1024 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

async fn test_app_state() -> AppState {
    let config_store = Arc::new(ConfigStore::load(None).await.unwrap());

    let log_store = Arc::new(LogStore::new(None, 1, 100));
    let vk_store = Arc::new(VirtualKeyStore::in_memory().await.unwrap());
    let rate_limit_store = Arc::new(RateLimitStore::in_memory().await.unwrap());
    let budget_store = Arc::new(BudgetStore::in_memory().await.unwrap());
    let model_catalog = Arc::new(ModelCatalog::in_memory().await.unwrap());
    let system_prompt_store = Arc::new(SystemPromptStore::in_memory().await.unwrap());
    let org_store = Arc::new(OrganizationStore::in_memory().await.unwrap());
    let search_tool_store = Arc::new(SearchToolStore::in_memory().await.unwrap());
    let mcp_server_store = Arc::new(McpServerStore::in_memory().await.unwrap());
    let mcp_registry = Arc::new(McpToolRegistry::new());

    let orchestrator = Arc::new(InferenceOrchestrator::new(vec![], vec![]));
    let metrics = Arc::new(Metrics::new());
    let vk_registry = Arc::new(VirtualKeyRegistry::new());

    AppState {
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
        org_store,
        search_tool_store,
        mcp_server_store,
        mcp_registry,
        admin_key_hash: Arc::new(tokio::sync::RwLock::new(Some(
            pylos_server::state::hash_admin_secret("test-admin-key"),
        ))),
        setup_required: Arc::new(tokio::sync::RwLock::new(false)),
        google_client_id: None,
        google_client_secret: None,
        google_redirect_uri: None,
        jwt_secret: "test_jwt_secret".to_string(),
        allowed_origins: vec!["*".to_string()],
        inference_semaphore: Arc::new(tokio::sync::Semaphore::new(10)),
        max_concurrency: 10,
        max_queue_size: 100,
        queue_timeout_ms: 30000,
    }
}

#[tokio::test]
async fn test_mcp_tools_list_endpoint() {
    let state = test_app_state().await;
    let app = create_router(state);

    let req = Request::builder()
        .uri("/mcp/tools/list")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = ax_body_to_json(resp.into_body()).await;

    let tools = json.get("tools").and_then(|v| v.as_array()).unwrap();
    // Au moins l'outil web_fetch built-in doit être présent
    assert!(tools.iter().any(|t| t["function"]["name"] == "web_fetch"));
}

#[tokio::test]
async fn test_mcp_tools_call_ssrf_blocked() {
    let state = test_app_state().await;
    let app = create_router(state);

    // Tentative d'accès à une IP privée / loopback via web_fetch
    let payload = json!({
        "name": "web_fetch",
        "arguments": {
            "url": "http://127.0.0.1:8080/secrets"
        }
    });

    let req = Request::builder()
        .uri("/mcp/tools/call")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    // Doit être rejeté (BAD_REQUEST à cause de la protection SSRF)
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

    let json = ax_body_to_json(resp.into_body()).await;
    let err_msg = json["error"].as_str().unwrap();
    assert!(err_msg.contains("SSRF protection"));
}

#[tokio::test]
async fn test_mcp_server_crud_with_transport_config() {
    let state = test_app_state().await;
    let app = create_router(state);

    // Création d'un serveur MCP avec transport HTTP
    let payload = json!({
        "name": "test-github-mcp",
        "server_type": "node",
        "transport_config": {
            "type": "http",
            "url": "https://mcp.example.com/api",
            "headers": {
                "Authorization": "Bearer gh_token"
            },
            "timeout_secs": 15
        }
    });

    let req = Request::builder()
        .uri("/api/mcp-servers")
        .method("POST")
        .header("Authorization", "Bearer test-admin-key")
        .header("Content-Type", "application/json")
        .body(Body::from(serde_json::to_vec(&payload).unwrap()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    let json_resp = ax_body_to_json(resp.into_body()).await;
    let id = json_resp["id"].as_str().unwrap();

    assert_eq!(json_resp["name"], "test-github-mcp");
    assert_eq!(json_resp["transport_config"]["type"], "http");
    assert_eq!(
        json_resp["transport_config"]["url"],
        "https://mcp.example.com/api"
    );
    assert_eq!(json_resp["transport_config"]["timeout_secs"], 15);

    // Récupération par ID directement depuis le store
    let state2 = test_app_state().await;
    let server = McpServer {
        id: id.to_string(),
        name: "test-github-mcp".to_string(),
        server_type: McpServerType::Node,
        status: McpServerStatus::Active,
        target_url: None,
        container_image: None,
        env_vars: None,
        virtual_key_id: None,
        team_id: None,
        transport_config: Some(McpTransportConfig::Http {
            url: "https://mcp.example.com/api".to_string(),
            headers: Some(HashMap::from([("X-Test".to_string(), "true".to_string())])),
            timeout_secs: Some(20),
        }),
        created_at: 1000,
        updated_at: 1000,
    };
    state2.mcp_server_store.create(&server).await.unwrap();

    let fetched = state2.mcp_server_store.get(id).await.unwrap();
    assert_eq!(fetched.id, id);
    assert_eq!(
        fetched.effective_transport(),
        Some(McpTransportConfig::Http {
            url: "https://mcp.example.com/api".to_string(),
            headers: Some(HashMap::from([("X-Test".to_string(), "true".to_string())])),
            timeout_secs: Some(20),
        })
    );
}

#[test]
fn test_mcp_tool_to_openai_tool_conversion() {
    let mcp_tool = pylos_core::domain::mcp_server::McpTool {
        name: "query_database".to_string(),
        description: Some("Executes a read-only SQL query".to_string()),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": { "type": "string" }
            },
            "required": ["query"]
        }),
        server_id: Some("postgres-mcp".to_string()),
    };

    let openai_tool: Tool = mcp_tool.into();
    assert_eq!(openai_tool.tool_type, "function");
    assert_eq!(openai_tool.function.name, "query_database");
    assert_eq!(
        openai_tool.function.description.as_deref(),
        Some("Executes a read-only SQL query")
    );
    assert!(openai_tool.function.parameters.is_some());
}

#[tokio::test]
async fn test_mcp_governance_vk_filtering() {
    let state = test_app_state().await;

    // 1. Clé avec denied_tools = ["web_fetch"]
    let vk = pylos_core::domain::config::VirtualKeyConfig {
        id: "vk-restricted".into(),
        name: "Restricted Key".into(),
        value: Some(pylos_core::domain::config::EnvVar::Literal(
            "sk-pylos-restricted".into(),
        )),
        is_active: true,
        denied_tools: Some(vec!["web_fetch".into()]),
        ..Default::default()
    };
    state.vk_store.upsert_key(&vk).await.unwrap();

    let app = create_router(state);

    // Requête GET /mcp/tools/list avec la clé restreinte
    let req = Request::builder()
        .uri("/mcp/tools/list")
        .method("GET")
        .header("Authorization", "Bearer sk-pylos-restricted")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    let json = ax_body_to_json(resp.into_body()).await;
    let tools = json.get("tools").and_then(|v| v.as_array()).unwrap();
    assert!(
        !tools.iter().any(|t| t["function"]["name"] == "web_fetch"),
        "web_fetch doit être masqué"
    );

    // Tentative d'exécution POST /mcp/tools/call avec la clé restreinte
    let call_req = Request::builder()
        .uri("/mcp/tools/call")
        .method("POST")
        .header("Authorization", "Bearer sk-pylos-restricted")
        .header("Content-Type", "application/json")
        .body(Body::from(
            json!({
                "name": "web_fetch",
                "arguments": { "url": "https://example.com" }
            })
            .to_string(),
        ))
        .unwrap();

    let call_resp = app.oneshot(call_req).await.unwrap();
    assert_eq!(
        call_resp.status(),
        StatusCode::FORBIDDEN,
        "web_fetch doit être refusé"
    );
}

struct MockAgentProvider;

#[async_trait::async_trait]
impl pylos_core::domain::traits::Provider for MockAgentProvider {
    fn name(&self) -> &str {
        "mock-agent-provider"
    }

    async fn complete(
        &self,
        request: &pylos_core::domain::request::PylosRequest,
        _config: &pylos_core::domain::provider::ProviderConfig,
    ) -> Result<pylos_core::domain::request::PylosResponse, pylos_core::error::PylosError> {
        let chat_req = match request {
            pylos_core::domain::request::PylosRequest::ChatCompletion(r) => r,
            _ => {
                return Err(pylos_core::error::PylosError::Unsupported(
                    "chat only".into(),
                ))
            }
        };

        // Si le dernier message est un rôle "tool", répondre avec la réponse finale
        let has_tool_response = chat_req
            .messages
            .iter()
            .any(|m| matches!(m.role, pylos_core::domain::openai::MessageRole::Tool));

        if has_tool_response {
            // Tour 2 : réponse finale
            Ok(pylos_core::domain::request::PylosResponse::ChatCompletion(
                pylos_core::domain::openai::ChatCompletionResponse {
                    id: "chatcmpl-agent-done".into(),
                    object: "chat.completion".into(),
                    created: 123456789,
                    model: chat_req.model.clone(),
                    choices: vec![pylos_core::domain::openai::ChatCompletionChoice {
                        index: 0,
                        message: pylos_core::domain::openai::ChatCompletionMessage {
                            role: pylos_core::domain::openai::MessageRole::Assistant,
                            content: Some("Here is the fetched data summary.".into()),
                            ..Default::default()
                        },
                        finish_reason: Some("stop".into()),
                    }],
                    usage: Some(pylos_core::domain::openai::Usage {
                        prompt_tokens: 20,
                        completion_tokens: 10,
                        total_tokens: 30,
                        ..Default::default()
                    }),
                },
            ))
        } else {
            // Tour 1 : appel de tool
            Ok(pylos_core::domain::request::PylosResponse::ChatCompletion(
                pylos_core::domain::openai::ChatCompletionResponse {
                    id: "chatcmpl-agent-step1".into(),
                    object: "chat.completion".into(),
                    created: 123456789,
                    model: chat_req.model.clone(),
                    choices: vec![pylos_core::domain::openai::ChatCompletionChoice {
                        index: 0,
                        message: pylos_core::domain::openai::ChatCompletionMessage {
                            role: pylos_core::domain::openai::MessageRole::Assistant,
                            content: None,
                            tool_calls: Some(vec![pylos_core::domain::openai::ToolCall {
                                id: "call_mock_123".into(),
                                call_type: "function".into(),
                                function: pylos_core::domain::openai::ToolCallFunction {
                                    name: "mock_calculate".into(),
                                    arguments: "{\"a\": 2, \"b\": 3}".into(),
                                },
                                index: Some(0),
                            }]),
                            ..Default::default()
                        },
                        finish_reason: Some("tool_calls".into()),
                    }],
                    usage: Some(pylos_core::domain::openai::Usage {
                        prompt_tokens: 15,
                        completion_tokens: 5,
                        total_tokens: 20,
                        ..Default::default()
                    }),
                },
            ))
        }
    }

    async fn stream(
        &self,
        _request: &pylos_core::domain::request::PylosRequest,
        _config: &pylos_core::domain::provider::ProviderConfig,
    ) -> Result<pylos_core::domain::traits::ChunkStream, pylos_core::error::PylosError> {
        Err(pylos_core::error::PylosError::Unsupported(
            "Not implemented".into(),
        ))
    }
}

struct MockMcpClient;

#[async_trait::async_trait]
impl pylos_application::mcp::client::McpClient for MockMcpClient {
    async fn initialize(&self) -> Result<(), pylos_core::error::PylosError> {
        Ok(())
    }

    async fn list_tools(
        &self,
    ) -> Result<Vec<pylos_core::domain::mcp_server::McpTool>, pylos_core::error::PylosError> {
        Ok(vec![pylos_core::domain::mcp_server::McpTool {
            name: "mock_calculate".into(),
            description: Some("Performs calculation".into()),
            input_schema: json!({ "type": "object" }),
            server_id: Some("calc-mcp".into()),
        }])
    }

    async fn call_tool(
        &self,
        name: &str,
        _arguments: Value,
    ) -> Result<pylos_core::domain::mcp_server::McpToolCallResult, pylos_core::error::PylosError>
    {
        Ok(pylos_core::domain::mcp_server::McpToolCallResult {
            content: vec![pylos_core::domain::mcp_server::McpContentItem::Text {
                text: format!("Result from {name}: 5"),
            }],
            is_error: false,
        })
    }

    fn transport_name(&self) -> &str {
        "mock"
    }
}

#[tokio::test]
async fn test_agent_loop_multi_turn_e2e() {
    let mut state = test_app_state().await;

    // 1. Enregistrer le MockMcpClient dans le registry
    let mock_server = McpServer {
        id: "calc-mcp".into(),
        name: "Calculation Server".into(),
        server_type: McpServerType::Custom("mock".into()),
        status: McpServerStatus::Active,
        target_url: None,
        container_image: None,
        env_vars: None,
        virtual_key_id: None,
        team_id: None,
        transport_config: None,
        created_at: 1000,
        updated_at: 1000,
    };
    let mock_tools = vec![pylos_core::domain::mcp_server::McpTool {
        name: "mock_calculate".into(),
        description: Some("Performs calculation".into()),
        input_schema: json!({ "type": "object" }),
        server_id: Some("calc-mcp".into()),
    }];
    state
        .mcp_registry
        .register_client(mock_server, Arc::new(MockMcpClient), mock_tools)
        .await;

    // 2. Configurer le Provider MockAgentProvider dans l'orchestrator
    let mut prov_config = pylos_core::domain::provider::ProviderConfig::new(
        pylos_core::domain::provider::ProviderKind::Custom("mock-agent-provider".into()),
        vec![],
    );
    prov_config.allowed_models = vec!["gpt-4o".into()];
    let orchestrator = Arc::new(InferenceOrchestrator::new(
        vec![(Arc::new(MockAgentProvider), prov_config)],
        vec![],
    ));
    state.orchestrator = orchestrator;

    let app = create_router(state);

    // 3. Envoyer une requête POST /v1/chat/completions avec tools
    let payload = json!({
        "model": "gpt-4o",
        "messages": [
            { "role": "user", "content": "How much is 2 + 3?" }
        ],
        "tools": [
            {
                "type": "function",
                "function": {
                    "name": "mock_calculate",
                    "description": "Performs calculation",
                    "parameters": { "type": "object" }
                }
            }
        ]
    });

    let req = Request::builder()
        .uri("/v1/chat/completions")
        .method("POST")
        .header("Content-Type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let json = ax_body_to_json(resp.into_body()).await;
    let content = json["choices"][0]["message"]["content"].as_str().unwrap();
    assert_eq!(content, "Here is the fetched data summary.");

    // Vérifier l'accumulation correcte des tokens (20 du tour 1 + 30 du tour 2 = 50 au total)
    let usage = &json["usage"];
    assert_eq!(usage["total_tokens"], 50);
}
