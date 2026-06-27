use std::path::PathBuf;

use opentelemetry::trace::TracerProvider as _;
use pylos_server::infrastructure::otel;
use pylos_server::routes::create_router;
use pylos_server::state::AppState;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    // Résolution du chemin de config pour deux usages :
    //   1. Niveau de log (avant init subscriber)
    //   2. Chargement complet de l'état (AppState::from_config)
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

    // Niveau de log : RUST_LOG env var > pylos.json server.log_level > "info"
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

    // OpenTelemetry (OTLP via OTEL_ENDPOINT)
    // Appelé AVANT l'init du subscriber — ses logs utilisent eprintln! pour être visibles
    let _otel_provider = otel::setup_otel();
    let otel_tracer = _otel_provider.as_ref().map(|p| p.tracer("pylos"));

    let env_filter = tracing_subscriber::EnvFilter::new(&log_level);

    let is_json = std::env::var("LOG_FORMAT")
        .map(|s| s.to_lowercase())
        .unwrap_or_default()
        == "json";

    // Initialisation unique du subscriber avec TOUS les layers (fmt + OTel)
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

    // Construction de l'état depuis la config
    let state = AppState::from_config(config_path).await?;

    // Warning si l'API management n'est pas protégée
    if state.admin_key.is_none() {
        tracing::warn!(
            "PYLOS_ADMIN_KEY is not set — management API (/providers, /virtual-keys, /config) is unprotected"
        );
    } else {
        tracing::info!("Management API protected with PYLOS_ADMIN_KEY");
    }

    // Port depuis la config ou PORT env var (env var prioritaire pour docker/k8s)
    let port = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .unwrap_or(state.config_store.get_port().await);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    // Création du router Axum
    let app = create_router(state);

    tracing::info!("Pylos listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
