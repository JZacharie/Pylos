use std::time::Instant;

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use tracing::error;

use pylos_application::log_store::{build_log_entry, LogStatus};
use pylos_core::domain::image::ImageRequest;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::error::PylosError;

use crate::middleware::virtual_key::VirtualKeyInfo;
use crate::provider_utils::guess_provider;
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// POST /v1/images/generations — compatible OpenAI Image Generation API
// ─────────────────────────────────────────────────────────────────────────────

pub async fn generate_image(
    State(state): State<AppState>,
    Extension(vk_info): Extension<Option<VirtualKeyInfo>>,
    Json(payload): Json<ImageRequest>,
) -> impl IntoResponse {
    let model = payload.model.clone();
    let start = Instant::now();
    let vk_name = vk_info.clone().map(|v| v.name);

    let request = PylosRequest::Image(payload);
    let mut request_ctx = RequestContext::default();
    if let Some(vk) = &vk_info {
        request_ctx.virtual_key = Some(vk.name.clone());
        request_ctx.provider_configs = vk.provider_configs.clone();
    }

    match state.orchestrator.complete(request, request_ctx).await {
        Ok((PylosResponse::Image(resp), _)) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let provider = guess_provider(&model);

            // Log entry creation
            let entry = build_log_entry(
                &provider,
                &model,
                false,
                LogStatus::Success,
                latency,
                None, // No usage info for images in standard OpenAI format
                None,
                None,
                None,
                None,
                vk_name,
                0,
            );
            let state_clone = state.clone();
            tokio::spawn(async move {
                state_clone.log_store.push(entry).await;
            });

            Json(resp).into_response()
        }
        Ok((other, _)) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let provider = guess_provider(&model);
            let err_msg = format!("Unexpected response type: {:?}", other);
            error!(model = %model, error = %err_msg, "Image generation returned unexpected response");

            let entry = build_log_entry(
                &provider,
                &model,
                false,
                LogStatus::Error,
                latency,
                None,
                None,
                Some(err_msg.clone()),
                None,
                None,
                vk_name,
                0,
            );
            let state_clone = state.clone();
            tokio::spawn(async move {
                state_clone.log_store.push(entry).await;
            });

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "message": err_msg,
                        "type": "internal_error",
                        "code": 500
                    }
                })),
            )
                .into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let provider = guess_provider(&model);
            error!(model = %model, error = %e, "Image generation request failed");

            let entry = build_log_entry(
                &provider,
                &model,
                false,
                LogStatus::Error,
                latency,
                None,
                None,
                Some(e.to_string()),
                None,
                None,
                vk_name,
                0,
            );
            tokio::spawn(async move {
                state.log_store.push(entry).await;
            });

            image_error_response(&e)
        }
    }
}

fn image_error_response(error: &PylosError) -> axum::response::Response {
    let (status, code) = match error {
        PylosError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
        PylosError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
        PylosError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
        PylosError::RateLimitExceeded(_) => (StatusCode::TOO_MANY_REQUESTS, "rate_limit_error"),
        PylosError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "timeout"),
        PylosError::Unsupported(_) => (StatusCode::NOT_IMPLEMENTED, "not_implemented"),
        PylosError::BudgetExceeded(_) => (StatusCode::PAYMENT_REQUIRED, "budget_exceeded"),
        PylosError::AllProvidersFailed(_) | PylosError::ProviderError { .. } => {
            (StatusCode::BAD_GATEWAY, "provider_error")
        }
        PylosError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    };

    let body = json!({
        "error": {
            "message": error.to_string(),
            "type": code,
            "code": status.as_u16()
        }
    });

    (status, Json(body)).into_response()
}
