use std::sync::Arc;

use prometheus::{
    register_histogram_vec_with_registry, register_int_counter_vec_with_registry,
    register_int_gauge_with_registry, Encoder, HistogramVec, IntCounterVec, IntGauge, Registry,
    TextEncoder,
};

/// Métriques Prometheus exposées par Pylos
/// Équivalent du plugin telemetry de bifrost
#[derive(Clone)]
pub struct Metrics {
    pub registry: Arc<Registry>,

    /// Nombre total de requêtes d'inférence
    pub inference_requests_total: IntCounterVec,

    /// Nombre de requêtes réussies
    pub inference_success_total: IntCounterVec,

    /// Nombre de requêtes échouées
    pub inference_errors_total: IntCounterVec,

    /// Latence des requêtes d'inférence en secondes
    pub inference_duration_seconds: HistogramVec,

    /// Nombre de tokens utilisés (prompt)
    pub prompt_tokens_total: IntCounterVec,

    /// Nombre de tokens générés (completion)
    pub completion_tokens_total: IntCounterVec,

    /// Requêtes en cours
    pub inference_in_flight: IntGauge,

    /// Time-To-First-Token pour le streaming
    pub inference_ttft_seconds: HistogramVec,

    /// Débit de tokens par seconde (TPS)
    pub inference_tps: HistogramVec,

    /// Nombre d'octets économisés grâce à la compression / Caveman
    pub compression_saved_bytes_total: IntCounterVec,

    /// Requêtes par tenant / Virtual Key à cardinalité maîtrisée (#139)
    pub tenant_requests_total: IntCounterVec,

    /// Histogramme des coûts d'inférence en USD par provider et tenant (#139)
    pub inference_cost_usd: HistogramVec,

    /// Journal d'audit des actions d'administration (#139)
    pub admin_audit_events_total: IntCounterVec,
}

impl Metrics {
    #[allow(clippy::expect_used)] // Static compile-time Prometheus metric registration at startup
    pub fn new() -> Self {
        let registry = Registry::new();

        let inference_requests_total = register_int_counter_vec_with_registry!(
            "pylos_inference_requests_total",
            "Total number of inference requests",
            &["provider", "model", "request_type"],
            registry
        )
        .expect("Failed to register inference_requests_total");

        let inference_success_total = register_int_counter_vec_with_registry!(
            "pylos_inference_success_total",
            "Total number of successful inference requests",
            &["provider", "model"],
            registry
        )
        .expect("Failed to register inference_success_total");

        let inference_errors_total = register_int_counter_vec_with_registry!(
            "pylos_inference_errors_total",
            "Total number of failed inference requests",
            &["provider", "error_type"],
            registry
        )
        .expect("Failed to register inference_errors_total");

        let inference_duration_seconds = register_histogram_vec_with_registry!(
            "pylos_inference_duration_seconds",
            "Inference request duration in seconds",
            &["provider", "model"],
            vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0],
            registry
        )
        .expect("Failed to register inference_duration_seconds");

        let prompt_tokens_total = register_int_counter_vec_with_registry!(
            "pylos_prompt_tokens_total",
            "Total number of prompt tokens used",
            &["provider", "model"],
            registry
        )
        .expect("Failed to register prompt_tokens_total");

        let completion_tokens_total = register_int_counter_vec_with_registry!(
            "pylos_completion_tokens_total",
            "Total number of completion tokens generated",
            &["provider", "model"],
            registry
        )
        .expect("Failed to register completion_tokens_total");

        let inference_in_flight = register_int_gauge_with_registry!(
            "pylos_inference_in_flight",
            "Number of inference requests currently in flight",
            registry
        )
        .expect("Failed to register inference_in_flight");

        let inference_ttft_seconds = register_histogram_vec_with_registry!(
            "pylos_inference_ttft_seconds",
            "Time to first token in seconds",
            &["provider", "model"],
            vec![0.01, 0.05, 0.1, 0.25, 0.5, 1.0, 2.0, 5.0, 10.0],
            registry
        )
        .expect("Failed to register inference_ttft_seconds");

        let inference_tps = register_histogram_vec_with_registry!(
            "pylos_inference_tps",
            "Tokens per second generated",
            &["provider", "model"],
            vec![1.0, 5.0, 10.0, 20.0, 40.0, 60.0, 80.0, 100.0, 150.0],
            registry
        )
        .expect("Failed to register inference_tps");

        let compression_saved_bytes_total = register_int_counter_vec_with_registry!(
            "pylos_compression_saved_bytes_total",
            "Total number of bytes saved by request optimization and Caveman compression",
            &["provider", "model"],
            registry
        )
        .expect("Failed to register compression_saved_bytes_total");

        let tenant_requests_total = register_int_counter_vec_with_registry!(
            "pylos_tenant_requests_total",
            "Total inference requests per tenant (bounded cardinality)",
            &["tenant"],
            registry
        )
        .expect("Failed to register tenant_requests_total");

        let inference_cost_usd = register_histogram_vec_with_registry!(
            "pylos_inference_cost_usd",
            "Estimated inference cost in USD per provider and tenant",
            &["provider", "tenant"],
            vec![0.0001, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0],
            registry
        )
        .expect("Failed to register inference_cost_usd");

        let admin_audit_events_total = register_int_counter_vec_with_registry!(
            "pylos_admin_audit_events_total",
            "Total audited admin actions by action and outcome",
            &["action", "outcome"],
            registry
        )
        .expect("Failed to register admin_audit_events_total");

        Self {
            registry: Arc::new(registry),
            inference_requests_total,
            inference_success_total,
            inference_errors_total,
            inference_duration_seconds,
            prompt_tokens_total,
            completion_tokens_total,
            inference_in_flight,
            inference_ttft_seconds,
            inference_tps,
            compression_saved_bytes_total,
            tenant_requests_total,
            inference_cost_usd,
            admin_audit_events_total,
        }
    }

    /// Incrémente le compteur de requêtes
    pub fn inc_requests(&self, provider: &str, model: &str, request_type: &str) {
        self.inference_requests_total
            .with_label_values(&[provider, model, request_type])
            .inc();
    }

    /// Incrémente le compteur de succès
    pub fn inc_success(&self, provider: &str, model: &str) {
        self.inference_success_total
            .with_label_values(&[provider, model])
            .inc();
    }

    /// Incrémente le compteur d'erreurs
    pub fn inc_error(&self, provider: &str, error_type: &str) {
        self.inference_errors_total
            .with_label_values(&[provider, error_type])
            .inc();
    }

    /// Observe la latence d'une requête
    pub fn observe_duration(&self, provider: &str, model: &str, duration_secs: f64) {
        self.inference_duration_seconds
            .with_label_values(&[provider, model])
            .observe(duration_secs);
    }

    /// Ajoute des tokens prompt
    pub fn add_prompt_tokens(&self, provider: &str, model: &str, count: u64) {
        self.prompt_tokens_total
            .with_label_values(&[provider, model])
            .inc_by(count);
    }

    /// Ajoute des tokens completion
    pub fn add_completion_tokens(&self, provider: &str, model: &str, count: u64) {
        self.completion_tokens_total
            .with_label_values(&[provider, model])
            .inc_by(count);
    }

    /// Ajoute le nombre d'octets économisés grâce à la compression / Caveman
    pub fn add_saved_bytes(&self, provider: &str, model: &str, count: u64) {
        self.compression_saved_bytes_total
            .with_label_values(&[provider, model])
            .inc_by(count);
    }

    /// Enregistre une requête et son coût estimé pour un tenant (cardinalité bornée) (#139)
    pub fn record_tenant_usage(&self, provider: &str, tenant: Option<&str>, cost_usd: f64) {
        let label = tenant
            .filter(|s| !s.is_empty() && s.len() <= 64)
            .unwrap_or("anonymous");
        self.tenant_requests_total.with_label_values(&[label]).inc();
        if cost_usd >= 0.0 {
            self.inference_cost_usd
                .with_label_values(&[provider, label])
                .observe(cost_usd);
        }
    }

    /// Journalise et comptabilise une action d'administration (#139)
    pub fn record_admin_audit(&self, actor: &str, action: &str, outcome: &str, detail: &str) {
        self.admin_audit_events_total
            .with_label_values(&[action, outcome])
            .inc();
        tracing::info!(
            target: "pylos_admin_audit",
            actor = actor,
            action = action,
            outcome = outcome,
            detail = detail,
            "[AdminAudit] action={} outcome={}",
            action,
            outcome
        );
    }

    /// Incrémente le gauge des requêtes en cours
    pub fn inc_in_flight(&self) {
        self.inference_in_flight.inc();
    }

    /// Décrémente le gauge des requêtes en cours
    pub fn dec_in_flight(&self) {
        self.inference_in_flight.dec();
    }

    /// Sérialise les métriques au format texte Prometheus
    pub fn export(&self) -> String {
        let encoder = TextEncoder::new();
        let mut metric_families = self.registry.gather();
        metric_families.extend(prometheus::gather());
        let mut buffer = Vec::new();
        encoder
            .encode(&metric_families, &mut buffer)
            .unwrap_or_default();
        String::from_utf8(buffer).unwrap_or_default()
    }
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}
