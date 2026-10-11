use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::state::AppState;

/// Comparaison en temps constant (evite une fuite par timing sur le jeton).
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// GET /metrics — expose les metriques Prometheus.
///
/// Si `PYLOS_METRICS_TOKEN` est defini (non vide), un jeton est exige :
/// `Authorization: Bearer <token>` ou `X-Metrics-Token: <token>`.
/// Sans variable definie, le comportement historique (acces libre) est conserve.
pub async fn metrics(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Ok(expected) = std::env::var("PYLOS_METRICS_TOKEN") {
        let expected = expected.trim();
        if !expected.is_empty() {
            let provided = headers
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .or_else(|| headers.get("x-metrics-token").and_then(|v| v.to_str().ok()));

            let allowed = provided
                .map(|p| constant_time_eq(p.as_bytes(), expected.as_bytes()))
                .unwrap_or(false);

            if !allowed {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({ "error": "metrics token required" })),
                )
                    .into_response();
            }
        }
    }

    let body = state.metrics.export();
    (
        [(
            header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        body,
    )
        .into_response()
}