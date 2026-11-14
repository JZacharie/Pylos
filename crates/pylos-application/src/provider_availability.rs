use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::traits::Provider;
use pylos_core::error::PylosError;

const MAX_CONSECUTIVE_FAILURES: u32 = 3;
const DEFAULT_COOLDOWN_SECS: u64 = 30;
const AUTH_COOLDOWN_SECS: u64 = 300;
const HEALTH_COOLDOWN_SECS: u64 = 120;
const DEFAULT_SATURATION_COOLDOWN_SECS: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvailabilityStatus {
    Available,
    Degraded,
    Unavailable,
    Saturated,
}

impl AvailabilityStatus {
    pub fn is_selectable(self) -> bool {
        matches!(self, Self::Available | Self::Degraded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BillingTier {
    Free,
    QuotaSubscription,
    Paid,
}

impl BillingTier {
    pub fn classify_provider(provider: &str) -> Self {
        if provider == "antigravity" {
            Self::QuotaSubscription
        } else if provider.starts_with("ollama") || provider == "mock" {
            Self::Free
        } else {
            Self::Paid
        }
    }

    pub fn is_priority_tier(self) -> bool {
        matches!(self, Self::Free | Self::QuotaSubscription)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatusSnapshot {
    pub provider: String,
    pub status: AvailabilityStatus,
    pub selectable: bool,
    pub reason: Option<String>,
    pub billing_tier: BillingTier,
    pub retry_after_secs: Option<u64>,
    pub consecutive_failures: u32,
    pub last_checked_ms: Option<i64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderState {
    status: AvailabilityStatus,
    reason: Option<String>,
    billing_tier: BillingTier,
    consecutive_failures: u32,
    cooldown_until: Option<Instant>,
    config_invalid: bool,
    last_checked_ms: Option<i64>,
    last_error: Option<String>,
}

impl ProviderState {
    fn new(provider: &str) -> Self {
        Self {
            status: AvailabilityStatus::Available,
            reason: None,
            billing_tier: BillingTier::classify_provider(provider),
            consecutive_failures: 0,
            cooldown_until: None,
            config_invalid: false,
            last_checked_ms: None,
            last_error: None,
        }
    }

    fn refresh_cooldown(&mut self) {
        if self.config_invalid {
            return;
        }
        if let Some(until) = self.cooldown_until {
            if Instant::now() >= until {
                self.status = AvailabilityStatus::Available;
                self.reason = None;
                self.cooldown_until = None;
                self.consecutive_failures = 0;
            }
        }
    }

    fn to_snapshot(&self, provider: &str) -> ProviderStatusSnapshot {
        let retry_after_secs = self.cooldown_until.and_then(|until| {
            let now = Instant::now();
            if until > now {
                Some((until - now).as_secs().max(1))
            } else {
                None
            }
        });
        ProviderStatusSnapshot {
            provider: provider.to_string(),
            status: self.status,
            selectable: self.status.is_selectable(),
            reason: self.reason.clone(),
            billing_tier: self.billing_tier,
            retry_after_secs,
            consecutive_failures: self.consecutive_failures,
            last_checked_ms: self.last_checked_ms,
            last_error: self.last_error.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FinOpsRoutingStats {
    pub total_priority_routes: u64,
    pub free_to_paid_fallbacks: u64,
}

/// Service centralisé de disponibilité, santé et saturation des providers LLM (#207, #208).
#[derive(Default)]
pub struct ProviderAvailabilityService {
    states: Mutex<HashMap<String, ProviderState>>,
    total_priority_routes: AtomicU64,
    free_to_paid_fallbacks: AtomicU64,
}

impl ProviderAvailabilityService {
    pub fn new() -> Self {
        Self::default()
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    fn provider_requires_api_key(name: &str) -> bool {
        !matches!(
            name,
            "ollama" | "ollama-jo3" | "bedrock" | "antigravity" | "mock"
        ) && !name.starts_with("ollama")
    }

    /// Synchronise l'état de configuration (présence de clés valides) depuis la liste des providers.
    pub fn sync_from_providers(&self, providers: &[(Arc<dyn Provider>, ProviderConfig)]) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        for (provider, cfg) in providers {
            let name = provider.name();
            let entry = guard
                .entry(name.to_string())
                .or_insert_with(|| ProviderState::new(name));
            entry.billing_tier = BillingTier::classify_provider(name);

            let has_valid_key = if Self::provider_requires_api_key(name) && !cfg.keys.is_empty() {
                cfg.keys.iter().any(|k| !k.value.trim().is_empty())
            } else {
                true
            };

            if !has_valid_key {
                entry.config_invalid = true;
                entry.status = AvailabilityStatus::Unavailable;
                entry.reason = Some("No valid API key configured".to_string());
                entry.cooldown_until = None;
            } else if entry.config_invalid {
                entry.config_invalid = false;
                entry.status = AvailabilityStatus::Available;
                entry.reason = None;
            }
        }
    }

    /// Met à jour la disponibilité d'après un health check (ModelHealth).
    pub fn sync_from_model_health(
        &self,
        provider: &str,
        health_status: &str,
        error_message: Option<&str>,
    ) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        entry.last_checked_ms = Some(Self::now_ms());

        if entry.config_invalid {
            return;
        }

        match health_status {
            "unhealthy" => {
                entry.status = AvailabilityStatus::Unavailable;
                let msg = error_message
                    .unwrap_or("Model health check failed")
                    .to_string();
                entry.reason = Some(msg.clone());
                entry.last_error = Some(msg);
                entry.cooldown_until =
                    Some(Instant::now() + Duration::from_secs(HEALTH_COOLDOWN_SECS));
            }
            "degraded" => {
                entry.status = AvailabilityStatus::Degraded;
                entry.reason = error_message.map(|s| s.to_string());
                entry.cooldown_until = None;
            }
            "healthy" => {
                entry.status = AvailabilityStatus::Available;
                entry.reason = None;
                entry.last_error = None;
                entry.consecutive_failures = 0;
                entry.cooldown_until = None;
            }
            _ => {}
        }
    }

    pub fn mark_available(&self, provider: &str) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        entry.config_invalid = false;
        entry.status = AvailabilityStatus::Available;
        entry.reason = None;
        entry.last_error = None;
        entry.consecutive_failures = 0;
        entry.cooldown_until = None;
    }

    pub fn mark_unavailable(&self, provider: &str, reason: impl Into<String>, cooldown_secs: Option<u64>) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        let msg = reason.into();
        entry.status = AvailabilityStatus::Unavailable;
        entry.reason = Some(msg.clone());
        entry.last_error = Some(msg);
        entry.last_checked_ms = Some(Self::now_ms());
        entry.cooldown_until = cooldown_secs.map(|s| Instant::now() + Duration::from_secs(s));
    }

    pub fn mark_saturated(&self, provider: &str, cooldown_secs: u64, reason: impl Into<String>) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        let msg = reason.into();
        entry.status = AvailabilityStatus::Saturated;
        entry.reason = Some(msg.clone());
        entry.last_error = Some(msg);
        entry.last_checked_ms = Some(Self::now_ms());
        entry.cooldown_until = Some(Instant::now() + Duration::from_secs(cooldown_secs.max(1)));
    }

    pub fn set_billing_tier(&self, provider: &str, tier: BillingTier) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        entry.billing_tier = tier;
    }

    pub fn billing_tier(&self, provider: &str) -> BillingTier {
        let guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        guard
            .get(provider)
            .map(|e| e.billing_tier)
            .unwrap_or_else(|| BillingTier::classify_provider(provider))
    }

    pub fn sync_billing_tiers_from_pricing(&self, provider_prices: &[(String, f64, f64)]) {
        let mut per_provider: HashMap<String, bool> = HashMap::new();
        for (prov, in_p, out_p) in provider_prices {
            let is_free = *in_p <= 0.0 && *out_p <= 0.0;
            per_provider
                .entry(prov.clone())
                .and_modify(|all_free| *all_free = *all_free && is_free)
                .or_insert(is_free);
        }
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        for (prov, all_free) in per_provider {
            let tier = if prov == "antigravity" {
                BillingTier::QuotaSubscription
            } else if all_free || prov.starts_with("ollama") {
                BillingTier::Free
            } else {
                BillingTier::Paid
            };
            let entry = guard
                .entry(prov.clone())
                .or_insert_with(|| ProviderState::new(&prov));
            entry.billing_tier = tier;
        }
    }

    pub fn record_priority_route(&self) {
        self.total_priority_routes.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_free_to_paid_fallback(&self) {
        self.free_to_paid_fallbacks.fetch_add(1, Ordering::Relaxed);
    }

    pub fn finops_stats(&self) -> FinOpsRoutingStats {
        FinOpsRoutingStats {
            total_priority_routes: self.total_priority_routes.load(Ordering::Relaxed),
            free_to_paid_fallbacks: self.free_to_paid_fallbacks.load(Ordering::Relaxed),
        }
    }

    pub fn record_success(&self, provider: &str) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        if !entry.config_invalid {
            entry.status = AvailabilityStatus::Available;
            entry.reason = None;
            entry.last_error = None;
        }
        entry.consecutive_failures = 0;
        entry.cooldown_until = None;
        entry.last_checked_ms = Some(Self::now_ms());
    }

    pub fn record_error(&self, provider: &str, err: &PylosError) {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        let msg = err.to_string();
        let lower = msg.to_lowercase();
        entry.last_error = Some(msg.clone());
        entry.last_checked_ms = Some(Self::now_ms());

        let is_auth_error = matches!(err, PylosError::Unauthorized(_))
            || lower.contains("401")
            || lower.contains("403")
            || lower.contains("invalid api key")
            || lower.contains("unauthorized");

        if is_auth_error {
            entry.status = AvailabilityStatus::Unavailable;
            entry.reason = Some(format!("Authentication failure: {}", msg));
            entry.cooldown_until = Some(Instant::now() + Duration::from_secs(AUTH_COOLDOWN_SECS));
            return;
        }

        let is_rate_limit = matches!(err, PylosError::RateLimitExceeded(_))
            || lower.contains("429")
            || lower.contains("resource_exhausted")
            || lower.contains("rate limit");

        if is_rate_limit {
            let cooldown = Self::extract_retry_after_secs(&lower)
                .unwrap_or(DEFAULT_SATURATION_COOLDOWN_SECS);
            entry.status = AvailabilityStatus::Saturated;
            entry.reason = Some(format!("Rate limit / quota saturated: {}", msg));
            entry.cooldown_until = Some(Instant::now() + Duration::from_secs(cooldown));
            return;
        }

        entry.consecutive_failures += 1;
        if entry.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
            entry.status = AvailabilityStatus::Unavailable;
            entry.reason = Some(format!(
                "Circuit breaker open after {} consecutive failures: {}",
                entry.consecutive_failures, msg
            ));
            entry.cooldown_until =
                Some(Instant::now() + Duration::from_secs(DEFAULT_COOLDOWN_SECS));
        }
    }

    fn extract_retry_after_secs(lower_msg: &str) -> Option<u64> {
        for marker in ["retry-after:", "retry after ", "retry_after="] {
            if let Some(idx) = lower_msg.find(marker) {
                let rest = lower_msg[idx + marker.len()..].trim_start();
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if let Ok(secs) = digits.parse::<u64>() {
                    if secs > 0 {
                        return Some(secs.min(3600));
                    }
                }
            }
        }
        None
    }

    pub fn is_selectable(&self, provider: &str) -> bool {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let Some(entry) = guard.get_mut(provider) else {
            return true;
        };
        entry.refresh_cooldown();
        entry.status.is_selectable()
    }

    pub fn is_circuit_breaker_open(&self, provider: &str) -> bool {
        !self.is_selectable(provider)
    }

    pub fn snapshot(&self, provider: &str) -> ProviderStatusSnapshot {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let entry = guard
            .entry(provider.to_string())
            .or_insert_with(|| ProviderState::new(provider));
        entry.refresh_cooldown();
        entry.to_snapshot(provider)
    }

    pub fn list_snapshots(&self) -> Vec<ProviderStatusSnapshot> {
        let mut guard = self.states.lock().unwrap_or_else(|e| e.into_inner());
        let mut out = Vec::with_capacity(guard.len());
        for (name, entry) in guard.iter_mut() {
            entry.refresh_cooldown();
            out.push(entry.to_snapshot(name));
        }
        out.sort_by(|a, b| a.provider.cmp(&b.provider));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_error_marks_unavailable_immediately() {
        let svc = ProviderAvailabilityService::new();
        svc.record_error(
            "openai",
            &PylosError::ProviderError {
                provider: "openai".into(),
                message: "401 Unauthorized: invalid api key".into(),
            },
        );
        assert!(!svc.is_selectable("openai"));
        let snap = svc.snapshot("openai");
        assert_eq!(snap.status, AvailabilityStatus::Unavailable);
    }

    #[test]
    fn test_consecutive_failures_open_circuit_and_success_recovers() {
        let svc = ProviderAvailabilityService::new();
        let err = PylosError::Timeout("upstream timeout".into());
        svc.record_error("anthropic", &err);
        assert!(svc.is_selectable("anthropic"));
        svc.record_error("anthropic", &err);
        assert!(svc.is_selectable("anthropic"));
        svc.record_error("anthropic", &err);
        assert!(!svc.is_selectable("anthropic"));

        svc.record_success("anthropic");
        assert!(svc.is_selectable("anthropic"));
        assert_eq!(svc.snapshot("anthropic").status, AvailabilityStatus::Available);
    }

    #[test]
    fn test_rate_limit_marks_saturated() {
        let svc = ProviderAvailabilityService::new();
        svc.record_error("gemini", &PylosError::RateLimitExceeded("429 RESOURCE_EXHAUSTED".into()));
        assert!(!svc.is_selectable("gemini"));
        assert_eq!(svc.snapshot("gemini").status, AvailabilityStatus::Saturated);
    }
}
