use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;

pub async fn root() -> impl IntoResponse {
    Json(json!({
        "name": "Pylos AI Gateway",
        "version": env!("CARGO_PKG_VERSION"),
        "status": "running",
        "endpoints": {
            "inference": "POST /v1/chat/completions",
            "health":    "GET  /health",
            "metrics":   "GET  /metrics"
        },
        "docs": "https://github.com/your-org/pylos"
    }))
}

pub async fn health_check() -> impl IntoResponse {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
        "build_date": env!("PYLOS_BUILD_DATE"),
        "git_sha": option_env!("PYLOS_GIT_SHA")
    }))
}
