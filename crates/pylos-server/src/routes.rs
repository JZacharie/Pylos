use crate::interfaces::http::{
    access_control, auth, completions, config, embeddings, health, images, inference, logs,
    metrics, models, system_prompts, vector_stores,
};
use crate::mcp;
use crate::mcp_proxy;
use crate::middleware::{
    admin_guard_middleware, management_auth_middleware, playgroup_check_middleware,
    queuing_middleware, request_id_middleware, virtual_key_middleware,
};
use crate::state::AppState;
use axum::{
    extract::DefaultBodyLimit,
    http::Method,
    middleware,
    routing::{delete, get, post, put},
    Router,
};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

pub fn create_router(state: AppState) -> Router {
    // Routes d'inférence — protégées par le middleware Virtual Key + Playgroup
    let inference_routes = Router::new()
        .route("/v1/chat/completions", post(inference::chat_completions))
        .route("/v1/completions", post(completions::text_completions))
        .route("/v1/embeddings", post(embeddings::create_embeddings))
        .route("/v1/images/generations", post(images::generate_image))
        .layer(DefaultBodyLimit::max(10 * 1024 * 1024)) // 10 MB
        .layer(middleware::from_fn_with_state(
            state.clone(),
            queuing_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            playgroup_check_middleware,
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            virtual_key_middleware,
        ));

    let management_routes = Router::new()
        // Logs API (protégées par l'auth management)
        .route("/api/logs", get(logs::get_logs))
        .route("/api/logs/stats", get(logs::get_logs_stats))
        .route("/api/logs/histogram", get(logs::get_logs_histogram))
        .route("/api/logs/histogram/tokens", get(logs::get_token_histogram))
        .route("/api/logs/filterdata", get(logs::get_filter_data))
        // Models read-only (protégé — liste les providers configurés)
        .route("/v1/models", get(models::list_models))
        // Model management routes
        .route(
            "/v1/models/pull/:provider",
            post(models::pull_provider_models),
        )
        .route("/v1/models/catalog", post(models::upsert_catalog_model))
        .route(
            "/v1/models/catalog/:provider/:model_id",
            delete(models::delete_catalog_model),
        )
        .route("/v1/models/pricing/status", get(models::get_pricing_status))
        .route(
            "/v1/models/pricing/reload",
            post(models::reload_pricing_data),
        )
        .route(
            "/v1/models/pricing/schedule",
            post(models::schedule_pricing_reload),
        )
        .route("/v1/models/health", get(models::list_models_health))
        .route(
            "/v1/models/health/check",
            post(models::run_model_health_check),
        )
        .route(
            "/v1/models/health/check_all",
            post(models::run_all_models_health_check),
        )
        // Provider management routes (protected)
        .route("/providers", get(config::list_providers))
        .route("/providers", post(config::create_provider))
        .route("/providers/:name", put(config::upsert_provider))
        .route("/providers/:name", delete(config::delete_provider))
        .route("/providers/:name/test", post(config::test_provider))
        // Virtual Key management routes
        .route("/virtual-keys", get(config::list_virtual_keys))
        .route("/virtual-keys", post(config::create_virtual_key))
        .route("/virtual-keys/:id", put(config::update_virtual_key))
        .route("/virtual-keys/:id", delete(config::delete_virtual_key))
        .route(
            "/virtual-keys/:id/budget",
            get(config::get_virtual_key_budget),
        )
        .route(
            "/virtual-keys/:id/value",
            get(config::reveal_virtual_key_value),
        )
        // Config management routes
        .route("/config", get(config::get_config))
        .route("/config/reload", post(config::reload_config))
        .route("/api/github/promote", post(config::promote_to_prod_handler))
        // Access Control routes
        .route(
            "/api/organizations",
            get(access_control::list_organizations),
        )
        .route(
            "/api/organizations",
            post(access_control::create_organization),
        )
        .route(
            "/api/organizations/:id",
            get(access_control::get_organization),
        )
        .route(
            "/api/organizations/:id",
            put(access_control::update_organization),
        )
        .route(
            "/api/organizations/:id",
            delete(access_control::delete_organization),
        )
        .route("/api/teams", get(access_control::list_teams))
        .route("/api/teams", post(access_control::create_team))
        .route("/api/teams/:id", get(access_control::get_team))
        .route("/api/teams/:id", put(access_control::update_team))
        .route("/api/teams/:id", delete(access_control::delete_team))
        .route("/api/users", get(access_control::list_users))
        .route("/api/users", post(access_control::create_user))
        .route("/api/users/:id", get(access_control::get_user))
        .route("/api/users/:id", put(access_control::update_user))
        .route("/api/users/:id", delete(access_control::delete_user))
        .route(
            "/api/access-groups",
            get(access_control::list_access_groups),
        )
        .route(
            "/api/access-groups",
            post(access_control::create_access_group),
        )
        .route(
            "/api/access-groups/:id",
            get(access_control::get_access_group),
        )
        .route(
            "/api/access-groups/:id",
            put(access_control::update_access_group),
        )
        .route(
            "/api/access-groups/:id",
            delete(access_control::delete_access_group),
        )
        .route("/api/policies", get(access_control::list_policies))
        .route("/api/policies", post(access_control::create_policy))
        .route("/api/policies/:id", put(access_control::update_policy))
        .route("/api/policies/:id", delete(access_control::delete_policy))
        .route(
            "/api/tool-policies",
            get(access_control::list_tool_policies),
        )
        .route(
            "/api/tool-policies",
            post(access_control::create_tool_policy),
        )
        .route(
            "/api/tool-policies/:id",
            put(access_control::update_tool_policy),
        )
        .route(
            "/api/tool-policies/:id",
            delete(access_control::delete_tool_policy),
        )
        // Search Tools routes
        .route("/api/search-tools", get(access_control::list_search_tools))
        .route(
            "/api/search-tools",
            post(access_control::create_search_tool),
        )
        .route(
            "/api/search-tools/:id",
            put(access_control::update_search_tool),
        )
        .route(
            "/api/search-tools/:id",
            delete(access_control::delete_search_tool),
        )
        // Vector Stores routes
        .route(
            "/api/vector-stores/collections",
            get(vector_stores::list_collections),
        )
        .route(
            "/api/vector-stores/collections",
            post(vector_stores::create_collection),
        )
        .route(
            "/api/vector-stores/collections/:name",
            delete(vector_stores::delete_collection),
        )
        .route(
            "/api/vector-stores/collections/:name/points",
            post(vector_stores::add_document),
        )
        .route(
            "/api/vector-stores/collections/:name/search",
            post(vector_stores::search_collection),
        )
        // System Prompt management routes
        .route(
            "/api/system-prompts",
            get(system_prompts::list_system_prompts),
        )
        .route(
            "/api/system-prompts",
            post(system_prompts::create_system_prompt),
        )
        .route(
            "/api/system-prompts/:id",
            get(system_prompts::get_system_prompt),
        )
        .route(
            "/api/system-prompts/:id",
            put(system_prompts::update_system_prompt),
        )
        .route(
            "/api/system-prompts/:id",
            delete(system_prompts::delete_system_prompt),
        )
        // MCP Server management routes
        .route("/api/mcp-servers", get(mcp::list_mcp_servers))
        .route("/api/mcp-servers", post(mcp::create_mcp_server))
        .route("/api/mcp-servers/:id", get(mcp::get_mcp_server))
        .route("/api/mcp-servers/:id", put(mcp::update_mcp_server))
        .route("/api/mcp-servers/:id", delete(mcp::delete_mcp_server))
        .route(
            "/api/mcp-servers/:id/activate",
            post(mcp::activate_mcp_server),
        )
        .route(
            "/api/mcp-servers/:id/deactivate",
            post(mcp::deactivate_mcp_server),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            management_auth_middleware,
        ));

    let guardrails_admin_routes = Router::new()
        .route("/config/guardrails", put(config::update_guardrails))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            admin_guard_middleware,
        ));

    let auth_routes = Router::new()
        .route("/api/auth/config", get(auth::get_auth_config))
        .route("/api/auth/google/callback", post(auth::google_callback))
        .route("/api/auth/logout", post(auth::logout));

    // MCP proxy routes — accessibles via Virtual Key (Authorization) ou Team (X-Team-Id)
    let mcp_proxy_routes = Router::new()
        .route("/mcp/:server_name", get(mcp_proxy::mcp_server_status))
        .route(
            "/mcp/:server_name/*path",
            axum::routing::any(mcp_proxy::mcp_proxy_handler),
        );

    Router::new()
        .route("/", get(health::root))
        .route("/health", get(health::health_check))
        .route("/metrics", get(metrics::metrics))
        .merge(auth_routes)
        // Inférence
        .merge(inference_routes)
        .merge(management_routes)
        .merge(guardrails_admin_routes)
        .merge(mcp_proxy_routes)
        // Middleware global
        .layer(TraceLayer::new_for_http())
        .layer(middleware::from_fn(request_id_middleware))
        .layer(build_cors(&state))
        .with_state(state)
}

fn build_cors(state: &AppState) -> CorsLayer {
    if state.allowed_origins.iter().any(|o| o == "*") {
        return CorsLayer::permissive();
    }
    let allowed_origins: Vec<axum::http::HeaderValue> = state
        .allowed_origins
        .iter()
        .filter_map(|o| match o.parse::<axum::http::HeaderValue>() {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::warn!(origin = %o, error = %e, "Invalid allowed_origin in config, skipping");
                None
            }
        })
        .collect();
    if allowed_origins.is_empty() {
        return CorsLayer::permissive();
    }
    CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
            axum::http::header::HeaderName::from_static("x-admin-key"),
        ])
}
