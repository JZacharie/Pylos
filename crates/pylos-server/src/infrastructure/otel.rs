//! OpenTelemetry initialization — inspired by Kusanagi's infrastructure style.
//!
//! Configuration via environment variables:
//!   OTEL_ENDPOINT        — OTLP HTTP endpoint (ex: http://jaeger:4318/v1/traces)
//!   OTEL_SERVICE_NAME    — nom du service (défaut: "pylos")
//!
//! Si OTEL_ENDPOINT n'est pas défini, l'export OTLP est désactivé (no-op).

use std::collections::HashMap;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_otlp::WithHttpConfig;
use opentelemetry_sdk::trace::SdkTracerProvider;

/// Initialise le SDK OpenTelemetry avec export OTLP via HTTP.
///
/// Retourne un `SdkTracerProvider` à conserver pour la durée de vie du processus.
/// Si `OTEL_ENDPOINT` ou `OTEL_EXPORTER_OTLP_ENDPOINT` n'est pas configuré,
/// retourne `None` et l'OTel reste inactif.
pub fn setup_otel() -> Option<SdkTracerProvider> {
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_TRACES_ENDPOINT")
        .or_else(|_| std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT"))
        .or_else(|_| std::env::var("OTEL_ENDPOINT"))
        .ok()
        .filter(|s| !s.is_empty())?;
    let service_name = std::env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| "pylos".to_string());

    eprintln!(
        "[pylos] OpenTelemetry — initialisation de l'export OTLP (endpoint: {}, service: {})",
        endpoint, service_name
    );

    let mut http_builder = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint);

    if let Ok(headers_str) = std::env::var("OTEL_EXPORTER_OTLP_HEADERS") {
        let headers: HashMap<String, String> = headers_str
            .split(',')
            .filter_map(|kv| {
                let mut parts = kv.splitn(2, '=');
                let k = parts.next()?.trim().to_string();
                let v = parts.next()?.trim().to_string();
                Some((k, v))
            })
            .collect();
        if !headers.is_empty() {
            http_builder = http_builder.with_headers(headers);
        }
    }

    let exporter = http_builder
        .build()
        .expect("Échec de la construction de l'exporteur OTLP SpanExporter");

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .build();

    let _tracer = provider.tracer(service_name);
    opentelemetry::global::set_tracer_provider(provider.clone());

    eprintln!("[pylos] OpenTelemetry — export OTLP activé");
    Some(provider)
}
