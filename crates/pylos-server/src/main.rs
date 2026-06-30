use std::path::PathBuf;

use axum::Router;
use opentelemetry::trace::TracerProvider as _;
use pylos_server::infrastructure::otel;
use pylos_server::routes::create_router;
use pylos_server::state::AppState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = jsonwebtoken::crypto::aws_lc::DEFAULT_PROVIDER.install_default();
    dotenvy::dotenv().ok();

    let config_path = std::env::var("PYLOS_CONFIG")
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let p = PathBuf::from("pylos.json");
            if p.exists() {
                Some(p)
            } else {
                None
            }
        });

    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| {
        config_path
            .as_ref()
            .and_then(|p| {
                let content = std::fs::read_to_string(p).ok()?;
                let json: serde_json::Value = serde_json::from_str(&content).ok()?;
                json.pointer("/server/log_level")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| "info".into())
    });

    let _otel_provider = otel::setup_otel();
    let otel_tracer = _otel_provider.as_ref().map(|p| p.tracer("pylos"));

    let env_filter = tracing_subscriber::EnvFilter::new(&log_level);

    let is_json = std::env::var("LOG_FORMAT")
        .map(|s| s.to_lowercase())
        .unwrap_or_default()
        == "json";

    if is_json {
        let otel_layer = otel_tracer
            .as_ref()
            .map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer.clone()));
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer().json())
            .with(otel_layer)
            .init();
    } else {
        let otel_layer = otel_tracer
            .as_ref()
            .map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer.clone()));
        tracing_subscriber::registry()
            .with(env_filter)
            .with(tracing_subscriber::fmt::layer())
            .with(otel_layer)
            .init();
    }

    tracing::info!(
        log_level = %log_level,
        version = env!("CARGO_PKG_VERSION"),
        "Starting Pylos AI Gateway"
    );

    if let Some(ref p) = config_path {
        tracing::info!(path = %p.display(), "Using config file");
    }

    let state = AppState::from_config(config_path).await?;

    let admin_key_set = state.admin_key_hash.read().await.is_some();
    let setup_needed = *state.setup_required.read().await;

    if setup_needed {
        tracing::warn!("🔒 Setup Required: Go to http://localhost:8080/setup to configure your admin password.");
    } else if !admin_key_set {
        tracing::warn!(
            "PYLOS_ADMIN_KEY is not set — management API (/providers, /virtual-keys, /config) is unprotected"
        );
    } else {
        tracing::info!("Management API protected with admin key hash");
    }

    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(state.config_store.get_port().await);

    let api_addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    let app = create_router(state);

    if pylos_server::ui::is_ui_available() {
        tracing::info!("Admin UI available at http://localhost:{}", port);
    } else {
        tracing::warn!("Admin UI not embedded — run `cd ui && npm run build` first");
    }

    // Serve UI on a separate port if PYLOS_UI_PORT is set (e.g. 8080)
    let ui_port = std::env::var("PYLOS_UI_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok());

    let api_listener = tokio::net::TcpListener::bind(&api_addr).await?;

    if let Some(ui_port) = ui_port {
        let ui_addr = std::net::SocketAddr::from(([0, 0, 0, 0], ui_port));
        let ui_app = Router::new().fallback_service(tower::service_fn(
            |req: axum::http::Request<axum::body::Body>| async move {
                Ok::<_, std::convert::Infallible>(pylos_server::ui::serve_ui(req).await)
            },
        ));
        let ui_listener = tokio::net::TcpListener::bind(&ui_addr).await?;
        tracing::info!("Admin UI also serving on http://localhost:{}", ui_port);

        tokio::try_join!(
            axum::serve(api_listener, app),
            axum::serve(ui_listener, ui_app),
        )?;
    } else {
        axum::serve(api_listener, app).await?;
    }

    Ok(())
}
