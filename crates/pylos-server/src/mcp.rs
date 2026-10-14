use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::state::AppState;
use pylos_core::domain::mcp_server::{
    McpServer, McpServerStatus, McpServerType, McpTransportConfig,
};

#[derive(Debug, Deserialize)]
pub struct CreateMcpServerRequest {
    pub name: String,
    #[serde(rename = "server_type")]
    pub server_type: String,
    pub target_url: Option<String>,
    pub container_image: Option<String>,
    pub env_vars: Option<Value>,
    pub virtual_key_id: Option<String>,
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_config: Option<McpTransportConfig>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMcpServerRequest {
    pub name: Option<String>,
    pub server_type: Option<String>,
    pub status: Option<String>,
    pub target_url: Option<String>,
    pub container_image: Option<String>,
    pub env_vars: Option<Value>,
    pub virtual_key_id: Option<String>,
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_config: Option<McpTransportConfig>,
}

#[derive(Debug, Serialize)]
pub struct McpServerResponse {
    pub id: String,
    pub name: String,
    pub server_type: String,
    pub status: String,
    pub target_url: Option<String>,
    pub container_image: Option<String>,
    pub env_vars: Option<Value>,
    pub virtual_key_id: Option<String>,
    pub team_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transport_config: Option<McpTransportConfig>,
    pub created_at: i64,
    pub updated_at: i64,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn server_to_response(s: McpServer) -> McpServerResponse {
    McpServerResponse {
        id: s.id,
        name: s.name,
        server_type: s.server_type.to_string(),
        status: s.status.to_string(),
        target_url: s.target_url,
        container_image: s.container_image,
        env_vars: s.env_vars,
        virtual_key_id: s.virtual_key_id,
        team_id: s.team_id,
        transport_config: s.transport_config,
        created_at: s.created_at,
        updated_at: s.updated_at,
    }
}

fn generate_id() -> String {
    use std::fmt::Write;
    let mut id = String::with_capacity(36);
    for i in 0..36 {
        if i == 8 || i == 13 || i == 18 || i == 23 {
            id.push('-');
        } else {
            write!(id, "{:x}", fastrand::u8(..)).ok();
        }
    }
    id
}

fn internal_err(msg: String) -> (StatusCode, Json<Value>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({"error": msg})),
    )
}

fn not_found(msg: String) -> (StatusCode, Json<Value>) {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg})))
}

pub async fn list_mcp_servers(State(state): State<AppState>) -> impl IntoResponse {
    match state.mcp_server_store.list().await {
        Ok(servers) => {
            let res: Vec<McpServerResponse> = servers.into_iter().map(server_to_response).collect();
            (StatusCode::OK, Json(json!(res))).into_response()
        }
        Err(e) => internal_err(e.to_string()).into_response(),
    }
}

pub async fn get_mcp_server(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.mcp_server_store.get(&id).await {
        Ok(server) => (StatusCode::OK, Json(json!(server_to_response(server)))).into_response(),
        Err(e) => match &e {
            pylos_core::error::PylosError::NotFound(_) => not_found(e.to_string()).into_response(),
            _ => internal_err(e.to_string()).into_response(),
        },
    }
}

pub async fn create_mcp_server(
    State(state): State<AppState>,
    Json(req): Json<CreateMcpServerRequest>,
) -> impl IntoResponse {
    let now = now_ms();

    let server_type = match req.server_type.as_str() {
        "python" => McpServerType::Python,
        "node" => McpServerType::Node,
        other => McpServerType::Custom(other.to_string()),
    };

    let server = McpServer {
        id: generate_id(),
        name: req.name,
        server_type,
        status: McpServerStatus::Inactive,
        target_url: req.target_url,
        container_image: req.container_image,
        env_vars: req.env_vars,
        virtual_key_id: req.virtual_key_id,
        team_id: req.team_id,
        transport_config: req.transport_config,
        created_at: now,
        updated_at: now,
    };

    match state.mcp_server_store.create(&server).await {
        Ok(created) => (
            StatusCode::CREATED,
            Json(json!(server_to_response(created))),
        )
            .into_response(),
        Err(e) => internal_err(e.to_string()).into_response(),
    }
}

pub async fn update_mcp_server(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(req): Json<UpdateMcpServerRequest>,
) -> impl IntoResponse {
    let existing = match state.mcp_server_store.get(&id).await {
        Ok(s) => s,
        Err(e) => {
            return match &e {
                pylos_core::error::PylosError::NotFound(_) => {
                    not_found(e.to_string()).into_response()
                }
                _ => internal_err(e.to_string()).into_response(),
            }
        }
    };

    let now = now_ms();

    let updated = McpServer {
        id: existing.id.clone(),
        name: req.name.unwrap_or(existing.name.clone()),
        server_type: req
            .server_type
            .as_deref()
            .map(|s| match s {
                "python" => McpServerType::Python,
                "node" => McpServerType::Node,
                other => McpServerType::Custom(other.to_string()),
            })
            .unwrap_or(existing.server_type.clone()),
        status: req
            .status
            .as_deref()
            .map(|s| match s {
                "active" => McpServerStatus::Active,
                "inactive" => McpServerStatus::Inactive,
                "error" => McpServerStatus::Error,
                _ => existing.status.clone(),
            })
            .unwrap_or(existing.status.clone()),
        target_url: req.target_url.or(existing.target_url.clone()),
        container_image: req.container_image.or(existing.container_image.clone()),
        env_vars: req.env_vars.or(existing.env_vars.clone()),
        virtual_key_id: req.virtual_key_id.or(existing.virtual_key_id.clone()),
        team_id: req.team_id.or(existing.team_id.clone()),
        transport_config: req.transport_config.or(existing.transport_config.clone()),
        created_at: existing.created_at,
        updated_at: now,
    };

    match state.mcp_server_store.update(&updated).await {
        Ok(saved) => (StatusCode::OK, Json(json!(server_to_response(saved)))).into_response(),
        Err(e) => internal_err(e.to_string()).into_response(),
    }
}

pub async fn delete_mcp_server(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.mcp_server_store.delete(&id).await {
        Ok(_) => (StatusCode::OK, Json(json!({"deleted": true}))).into_response(),
        Err(e) => match &e {
            pylos_core::error::PylosError::NotFound(_) => not_found(e.to_string()).into_response(),
            _ => internal_err(e.to_string()).into_response(),
        },
    }
}

pub async fn activate_mcp_server(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state
        .mcp_server_store
        .set_status(&id, &McpServerStatus::Active)
        .await
    {
        Ok(server) => (StatusCode::OK, Json(json!(server_to_response(server)))).into_response(),
        Err(e) => match &e {
            pylos_core::error::PylosError::NotFound(_) => not_found(e.to_string()).into_response(),
            _ => internal_err(e.to_string()).into_response(),
        },
    }
}

pub async fn deactivate_mcp_server(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state
        .mcp_server_store
        .set_status(&id, &McpServerStatus::Inactive)
        .await
    {
        Ok(server) => (StatusCode::OK, Json(json!(server_to_response(server)))).into_response(),
        Err(e) => match &e {
            pylos_core::error::PylosError::NotFound(_) => not_found(e.to_string()).into_response(),
            _ => internal_err(e.to_string()).into_response(),
        },
    }
}
