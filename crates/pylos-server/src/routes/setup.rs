use crate::state::hash_sha256;
use crate::state::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

#[derive(Serialize)]
pub struct SetupStatus {
    pub setup_required: bool,
}

#[derive(Deserialize)]
pub struct SetupRequest {
    pub password: String,
    pub confirm_password: String,
}

#[derive(Serialize)]
pub struct SetupResponse {
    pub status: String,
    pub message: String,
}

pub fn setup_router() -> Router<AppState> {
    Router::new()
        .route("/api/setup/status", get(get_setup_status))
        .route("/api/setup", post(run_setup))
}

async fn get_setup_status(State(state): State<AppState>) -> impl IntoResponse {
    let setup_required = *state.setup_required.read().await;
    Json(SetupStatus { setup_required })
}

async fn run_setup(
    State(state): State<AppState>,
    Json(payload): Json<SetupRequest>,
) -> impl IntoResponse {
    let setup_needed = *state.setup_required.read().await;
    if !setup_needed {
        return (
            StatusCode::BAD_REQUEST,
            Json(SetupResponse {
                status: "error".into(),
                message: "Setup is already completed".into(),
            }),
        )
            .into_response();
    }

    if payload.password != payload.confirm_password {
        return (
            StatusCode::BAD_REQUEST,
            Json(SetupResponse {
                status: "error".into(),
                message: "Passwords do not match".into(),
            }),
        )
            .into_response();
    }

    if payload.password.len() < 10 {
        return (
            StatusCode::BAD_REQUEST,
            Json(SetupResponse {
                status: "error".into(),
                message: "Password must be at least 10 characters long".into(),
            }),
        )
            .into_response();
    }

    info!("Setting up Pylos admin password...");
    let hash = hash_sha256(&payload.password);

    // Save to SQLite via config_store
    if let Err(e) = state.config_store.save_admin_key_hash(&hash).await {
        error!("Failed to save admin key hash to database: {:?}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(SetupResponse {
                status: "error".into(),
                message: "Failed to persist admin password".into(),
            }),
        )
            .into_response();
    }

    // Update state
    *state.admin_key_hash.write().await = Some(hash);
    *state.setup_required.write().await = false;

    info!("Pylos admin password setup successfully completed!");

    (
        StatusCode::OK,
        Json(SetupResponse {
            status: "success".into(),
            message: "Setup completed successfully".into(),
        }),
    )
        .into_response()
}
