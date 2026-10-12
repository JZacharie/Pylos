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
use pylos_core::domain::embedding::EmbeddingRequest;
use pylos_core::domain::request::RequestContext;
use pylos_core::error::PylosError;

use crate::middleware::virtual_key::VirtualKeyInfo;
use crate::provider_utils::guess_provider;
use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// POST /v1/embeddings — compatible OpenAI Embeddings API
// ─────────────────────────────────────────────────────────────────────────────

pub async fn create_embeddings(
    State(state): State<AppState>,
    Extension(vk_info): Extension<Option<VirtualKeyInfo>>,
    Json(payload): Json<EmbeddingRequest>,
) -> impl IntoResponse {
    let model = payload.model.clone();
    let start = Instant::now();

    let mut ctx = RequestContext::default();
    if let Some(vk) = &vk_info {
        ctx.virtual_key = Some(vk.id.clone());
        ctx.provider_configs = vk.provider_configs.clone();
    }
    let vk_name = vk_info.map(|v| v.name);

    match state.orchestrator.embed(payload, ctx).await {
        Ok(resp) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let provider = guess_provider(&model);

            let usage = pylos_core::domain::openai::Usage {
                prompt_tokens: resp.usage.prompt_tokens,
                completion_tokens: 0,
                total_tokens: resp.usage.prompt_tokens,
                ..Default::default()
            };
            let entry = build_log_entry(
                &provider,
                &model,
                false,
                LogStatus::Success,
                latency,
                Some(&usage),
                Some("stop".into()),
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
        Err(e) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let provider = guess_provider(&model);
            error!(model = %model, error = %e, "Embedding request failed");

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

            embedding_error_response(&e)
        }
    }
}

fn embedding_error_response(error: &PylosError) -> axum::response::Response {
    let (status, code) = match error {
        PylosError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
        PylosError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
        PylosError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
        PylosError::RateLimitExceeded(_) => (StatusCode::TOO_MANY_REQUESTS, "rate_limit_error"),
        PylosError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "timeout"),
        PylosError::Unsupported(_) => (StatusCode::NOT_IMPLEMENTED, "not_implemented"),
        PylosError::BudgetExceeded(_) => (StatusCode::PAYMENT_REQUIRED, "budget_exceeded"),
        PylosError::GuardrailViolation(_) => (StatusCode::BAD_REQUEST, "content_policy_violation"),
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
