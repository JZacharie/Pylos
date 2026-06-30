use axum::{
    body::Body,
    http::{header, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
};
use rust_embed::RustEmbed;
use std::sync::LazyLock;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../ui/dist"]
struct Assets;

static INDEX_HTML: LazyLock<Option<rust_embed::EmbeddedFile>> =
    LazyLock::new(|| Assets::get("index.html"));

pub async fn serve_ui(req: Request<Body>) -> Response {
    let path = req.uri().path().trim_start_matches('/').to_string();
    let accepts_html = req
        .headers()
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.contains("text/html"))
        .unwrap_or(false);

    // For root or index.html, check if client wants HTML (browser) or JSON (API)
    if path.is_empty() || path == "index.html" {
        return if accepts_html {
            serve_index()
        } else {
            (
                StatusCode::NOT_FOUND,
                "{\"error\":\"Not found. API routes: /health, /v1/chat/completions, /v1/models, ...\"}",
            )
                .into_response()
        };
    }

    if let Some(content) = Assets::get(&path) {
        let mime = mime_guess::from_path(&path).first_or_octet_stream();
        Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_str(mime.as_ref())
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            )
            .header(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            )
            .body(Body::from(content.data))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    } else if accepts_html {
        serve_index()
    } else {
        (StatusCode::NOT_FOUND, "{\"error\":\"Not found\"}").into_response()
    }
}

fn serve_index() -> Response {
    match INDEX_HTML.as_ref() {
        Some(content) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            )
            .body(Body::from(content.data.to_vec()))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response()),
        None => (
            StatusCode::NOT_FOUND,
            "UI not built. Run `cd ui && npm run build` first.",
        )
            .into_response(),
    }
}

pub fn is_ui_available() -> bool {
    INDEX_HTML.is_some()
}
