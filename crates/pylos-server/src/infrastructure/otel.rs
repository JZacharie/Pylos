//! OpenTelemetry initialization — inspired by Kusanagi's infrastructure style.
//!
//! Configuration via environment variables:
//!   OTEL_ENDPOINT        — OTLP HTTP endpoint (ex: http://jaeger:4318/v1/traces)
//!   OTEL_SERVICE_NAME    — nom du service (défaut: "pylos")
//!
//! Si OTEL_ENDPOINT n'est pas défini, l'export OTLP est désactivé (no-op).

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::TracerProvider;

/// Initialise le SDK OpenTelemetry avec export OTLP via HTTP.
///
/// Retourne un `TracerProvider` à conserver pour la durée de vie du processus.
/// Si `OTEL_ENDPOINT` n'est pas configuré, retourne `None` et l'OTel reste inactif.
pub fn setup_otel() -> Option<TracerProvider> {
    let endpoint = std::env::var("OTEL_ENDPOINT")
        .ok()
        .filter(|s| !s.is_empty())?;
    let service_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "pylos".to_string());

    eprintln!(
        "[pylos] OpenTelemetry — initialisation de l'export OTLP (endpoint: {}, service: {})",
        endpoint, service_name
    );

    let exporter = opentelemetry_otlp::new_exporter()
        .http()
        .with_endpoint(endpoint)
        .build_span_exporter()
        .expect("Échec de la construction de l'exporteur OTLP SpanExporter");

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build();

    let _tracer = provider.tracer(service_name);
    opentelemetry::global::set_tracer_provider(provider.clone());

    eprintln!("[pylos] OpenTelemetry — export OTLP activé");
    Some(provider)
}
