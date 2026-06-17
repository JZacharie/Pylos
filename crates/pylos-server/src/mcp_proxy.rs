use axum::{
    extract::{Path, State},
    http::{HeaderMap, Method, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::state::AppState;

/// Proxy handler: transfère la requête au serveur MCP cible
/// après validation des droits (Virtual Key ou Team).
pub async fn mcp_proxy_handler(
    State(state): State<AppState>,
    Path((server_name, path)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> impl IntoResponse {
    // 1. Extraire l'authentification
    let token = headers
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string());

    // Valider la Virtual Key (rate limit + existence)
    let virtual_key_id = if let Some(ref t) = token {
        // Check dans le registre mémoire (rate limit)
        if let Err(msg) = state.vk_registry.check_and_increment(t).await {
            tracing::warn!(key = %t, error = %msg, "MCP proxy rate limit exceeded");
            return (StatusCode::TOO_MANY_REQUESTS, Json(json!({"error": msg}))).into_response();
        }
        // Récupérer l'ID DB depuis le store
        match state.vk_store.get_key_by_value(t).await {
            Ok(Some(vk)) => Some(vk.id.clone()),
            _ => None,
        }
    } else {
        None
    };

    let team_id = headers
        .get("X-Team-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // 2. Chercher le serveur MCP actif correspondant
    let servers = state.mcp_server_store.list().await.unwrap_or_default();
    let server = servers.into_iter().find(|s| {
        s.name == server_name
            && s.status == pylos_core::domain::mcp_server::McpServerStatus::Active
            && (match (&s.virtual_key_id, &s.team_id) {
                (Some(vk_id), Some(t_id)) => {
                    virtual_key_id.as_ref() == Some(vk_id) && team_id.as_ref() == Some(t_id)
                }
                (Some(vk_id), None) => virtual_key_id.as_ref() == Some(vk_id),
                (None, Some(t_id)) => team_id.as_ref() == Some(t_id),
                (None, None) => false,
            })
    });

    let srv = match server {
        Some(s) => s,
        None => {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error": "No active MCP server found for this key/team"})),
            )
                .into_response();
        }
    };

    // 3. Forwarder la requête
    let target_url = srv.target_url.unwrap_or_else(|| {
        format!(
            "http://pylos-mcp-{}:8000",
            srv.name.replace(' ', "-").to_lowercase()
        )
    });

    let full_path = if path.starts_with('/') {
        path.clone()
    } else {
        format!("/{path}")
    };
    let full_url = format!("{}{}", target_url.trim_end_matches('/'), full_path);

    let client = reqwest::Client::new();

    // Convertir la méthode et l'URL
    let reqwest_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
    let mut proxy_req = client.request(reqwest_method, &full_url);

    // Copier les headers via conversion string (pour éviter le conflit http 0.2 vs 1.0)
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_string();
        if let Ok(val_str) = value.to_str() {
            if key_str != "host" {
                proxy_req = proxy_req.header(&key_str, val_str);
            }
        }
    }

    // Copier le body
    if !body.is_empty() {
        proxy_req = proxy_req.body(body.to_vec());
    }

    match proxy_req.send().await {
        Ok(resp) => {
            let status_code = resp.status().as_u16();
            let resp_headers = resp.headers().clone();
            let resp_body = resp.bytes().await.unwrap_or_default();

            let mut response = axum::response::Response::new(axum::body::Body::from(resp_body));
            *response.status_mut() =
                StatusCode::from_u16(status_code).unwrap_or(StatusCode::BAD_GATEWAY);

            for (key, value) in resp_headers.iter() {
                let key_lower = key.as_str().to_lowercase();
                if key_lower != "transfer-encoding" && key_lower != "content-length" {
                    if let Ok(val_str) = value.to_str() {
                        if let Ok(header_name) =
                            axum::http::HeaderName::from_bytes(key.as_str().as_bytes())
                        {
                            if let Ok(header_value) = axum::http::HeaderValue::from_str(val_str) {
                                response.headers_mut().insert(header_name, header_value);
                            }
                        }
                    }
                }
            }

            response
        }
        Err(e) => {
            tracing::warn!(server = %server_name, error = %e, "MCP proxy request failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({
                    "error": "MCP server unreachable",
                    "server": server_name
                })),
            )
                .into_response()
        }
    }
}

/// Endpoint de statut pour un serveur MCP (GET /mcp/:server_name)
pub async fn mcp_server_status(
    State(state): State<AppState>,
    Path(server_name): Path<String>,
) -> impl IntoResponse {
    let servers = state.mcp_server_store.list().await.unwrap_or_default();
    let server = servers.into_iter().find(|s| s.name == server_name);

    match server {
        Some(s) => (
            StatusCode::OK,
            Json(json!({
                "id": s.id,
                "name": s.name,
                "server_type": s.server_type.to_string(),
                "status": s.status.to_string(),
                "target_url": s.target_url,
                "active": s.status == pylos_core::domain::mcp_server::McpServerStatus::Active,
            })),
        )
            .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(json!({"error": format!("MCP server '{server_name}' not found")})),
        )
            .into_response(),
    }
}
