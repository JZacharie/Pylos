use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::domain::config::{BudgetConfig, RateLimitConfig, VirtualKeyConfig};
use crate::domain::logging::{
    GuardrailsBreakdown, GuardrailsTimeline, HistogramBucket, LogEntry, LogFilter, LogStats,
    LogStatus, TokenBucket,
};
use crate::domain::organization::Organization;
use crate::error::PylosError;

/// Port de persistance pour les budgets par Virtual Key.
#[async_trait]
pub trait BudgetRepository: Send + Sync {
    async fn check_budget(&self, vk_id: &str, estimated_cost: f64) -> Result<(), PylosError>;
    async fn record_usage(&self, vk_id: &str, cost_usd: f64);
    async fn upsert_budget(&self, vk_id: &str, budget: &BudgetConfig) -> Result<(), PylosError>;
    async fn delete_budget(&self, vk_id: &str) -> Result<(), PylosError>;
}

/// Port de persistance pour le rate limiting distribué par Virtual Key.
#[async_trait]
pub trait RateLimitRepository: Send + Sync {
    async fn check_and_increment_requests(&self, vk_id: &str) -> Result<(), PylosError>;
    async fn record_tokens(&self, vk_id: &str, tokens: i64);
    async fn upsert_rate_limit(
        &self,
        vk_id: &str,
        rl: &RateLimitConfig,
    ) -> Result<(), PylosError>;
    async fn delete_rate_limit(&self, vk_id: &str) -> Result<(), PylosError>;
}

/// Port de persistance pour la configuration gateway versionnée et les secrets serveur.
#[async_trait]
pub trait ConfigRepository: Send + Sync {
    async fn load_config_json(&self) -> Result<Option<(String, i64)>, PylosError>;
    async fn save_config_json(&self, json: &str, version: i64) -> Result<(), PylosError>;
    async fn backup_config_json(
        &self,
        json: &str,
        source_version: i64,
        target_version: i64,
    ) -> Result<(), PylosError>;
    async fn get_admin_key_hash(&self) -> Result<Option<String>, PylosError>;
    async fn save_admin_key_hash(&self, hash: &str) -> Result<(), PylosError>;
}

/// Port de persistance pour les Virtual Keys.
#[async_trait]
pub trait VirtualKeyRepository: Send + Sync {
    async fn list_keys(&self) -> Result<Vec<VirtualKeyConfig>, PylosError>;
    async fn get_key(&self, id: &str) -> Result<Option<VirtualKeyConfig>, PylosError>;
    async fn find_by_raw_value(
        &self,
        raw_value: &str,
    ) -> Result<Option<VirtualKeyConfig>, PylosError>;
    async fn upsert_key(&self, vk: &VirtualKeyConfig) -> Result<(), PylosError>;
    async fn delete_key(&self, id: &str) -> Result<bool, PylosError>;
}

/// Port de persistance pour les logs d'inférence et guardrails.
#[async_trait]
pub trait LogRepository: Send + Sync {
    async fn push(&self, entry: LogEntry);
    async fn list(&self, limit: usize, offset: usize, filter: &LogFilter) -> (Vec<LogEntry>, u64);
    async fn stats(&self, filter: &LogFilter) -> LogStats;
    async fn histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<HistogramBucket>;
    async fn token_histogram(&self, filter: &LogFilter, bucket_secs: i64) -> Vec<TokenBucket>;
    async fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64);
    async fn guardrails_breakdown(&self, filter: &LogFilter) -> GuardrailsBreakdown;
    async fn guardrails_timeline(
        &self,
        filter: &LogFilter,
        bucket_secs: i64,
    ) -> Vec<GuardrailsTimeline>;
}

/// Port de persistance pour les organisations.
#[async_trait]
pub trait OrganizationRepository: Send + Sync {
    async fn list_organizations(&self) -> Result<Vec<Organization>, PylosError>;
    async fn get_organization(&self, id: &str) -> Result<Option<Organization>, PylosError>;
    async fn upsert_organization(&self, org: &Organization) -> Result<(), PylosError>;
    async fn delete_organization(&self, id: &str) -> Result<bool, PylosError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Implémentations In-Memory (Fakes pour tests unitaires sans base de données)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default, Clone)]
pub struct InMemoryBudgetRepository {
    budgets: Arc<RwLock<HashMap<String, (f64, f64)>>>,
}

impl InMemoryBudgetRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl BudgetRepository for InMemoryBudgetRepository {
    async fn check_budget(&self, vk_id: &str, estimated_cost: f64) -> Result<(), PylosError> {
        let map = self.budgets.read().await;
        if let Some((max_usd, current_usd)) = map.get(vk_id) {
            if current_usd + estimated_cost > *max_usd {
                return Err(PylosError::BudgetExceeded(format!(
                    "Virtual key '{}' exceeded budget: ${:.4} + ${:.4} > ${:.2}",
                    vk_id, current_usd, estimated_cost, max_usd
                )));
            }
        }
        Ok(())
    }

    async fn record_usage(&self, vk_id: &str, cost_usd: f64) {
        if cost_usd <= 0.0 {
            return;
        }
        let mut map = self.budgets.write().await;
        if let Some((_, current_usd)) = map.get_mut(vk_id) {
            *current_usd += cost_usd;
        }
    }

    async fn upsert_budget(&self, vk_id: &str, budget: &BudgetConfig) -> Result<(), PylosError> {
        let mut map = self.budgets.write().await;
        map.insert(vk_id.to_string(), (budget.max_limit, budget.current_usage));
        Ok(())
    }

    async fn delete_budget(&self, vk_id: &str) -> Result<(), PylosError> {
        self.budgets.write().await.remove(vk_id);
        Ok(())
    }
}

type RateLimitRules = HashMap<String, (RateLimitConfig, u32, u64)>;

#[derive(Default, Clone)]
pub struct InMemoryRateLimitRepository {
    rules: Arc<RwLock<RateLimitRules>>,
}

impl InMemoryRateLimitRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RateLimitRepository for InMemoryRateLimitRepository {
    async fn check_and_increment_requests(&self, vk_id: &str) -> Result<(), PylosError> {
        let mut map = self.rules.write().await;
        if let Some((cfg, reqs, _)) = map.get_mut(vk_id) {
            if cfg.request_max_limit > 0 && *reqs >= cfg.request_max_limit {
                return Err(PylosError::RateLimitExceeded(format!(
                    "Request limit exceeded for '{}'",
                    vk_id
                )));
            }
            *reqs += 1;
        }
        Ok(())
    }

    async fn record_tokens(&self, vk_id: &str, tokens: i64) {
        if tokens <= 0 {
            return;
        }
        let mut map = self.rules.write().await;
        if let Some((_, _, used_tokens)) = map.get_mut(vk_id) {
            *used_tokens += tokens as u64;
        }
    }

    async fn upsert_rate_limit(
        &self,
        vk_id: &str,
        rl: &RateLimitConfig,
    ) -> Result<(), PylosError> {
        self.rules
            .write()
            .await
            .insert(vk_id.to_string(), (rl.clone(), 0, 0));
        Ok(())
    }

    async fn delete_rate_limit(&self, vk_id: &str) -> Result<(), PylosError> {
        self.rules.write().await.remove(vk_id);
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct InMemoryConfigRepository {
    inner: Arc<RwLock<Option<(String, i64)>>>,
    admin_hash: Arc<RwLock<Option<String>>>,
}

impl InMemoryConfigRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ConfigRepository for InMemoryConfigRepository {
    async fn load_config_json(&self) -> Result<Option<(String, i64)>, PylosError> {
        Ok(self.inner.read().await.clone())
    }

    async fn save_config_json(&self, json: &str, version: i64) -> Result<(), PylosError> {
        *self.inner.write().await = Some((json.to_string(), version));
        Ok(())
    }

    async fn backup_config_json(
        &self,
        _json: &str,
        _source_version: i64,
        _target_version: i64,
    ) -> Result<(), PylosError> {
        Ok(())
    }

    async fn get_admin_key_hash(&self) -> Result<Option<String>, PylosError> {
        Ok(self.admin_hash.read().await.clone())
    }

    async fn save_admin_key_hash(&self, hash: &str) -> Result<(), PylosError> {
        *self.admin_hash.write().await = Some(hash.to_string());
        Ok(())
    }
}

#[derive(Default, Clone)]
pub struct InMemoryVirtualKeyRepository {
    keys: Arc<RwLock<HashMap<String, VirtualKeyConfig>>>,
}

impl InMemoryVirtualKeyRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl VirtualKeyRepository for InMemoryVirtualKeyRepository {
    async fn list_keys(&self) -> Result<Vec<VirtualKeyConfig>, PylosError> {
        Ok(self.keys.read().await.values().cloned().collect())
    }

    async fn get_key(&self, id: &str) -> Result<Option<VirtualKeyConfig>, PylosError> {
        Ok(self.keys.read().await.get(id).cloned())
    }

    async fn find_by_raw_value(
        &self,
        raw_value: &str,
    ) -> Result<Option<VirtualKeyConfig>, PylosError> {
        let map = self.keys.read().await;
        Ok(map
            .values()
            .find(|vk| {
                vk.value
                    .as_ref()
                    .and_then(|v| v.resolve())
                    .as_deref()
                    == Some(raw_value)
            })
            .cloned())
    }

    async fn upsert_key(&self, vk: &VirtualKeyConfig) -> Result<(), PylosError> {
        self.keys.write().await.insert(vk.id.clone(), vk.clone());
        Ok(())
    }

    async fn delete_key(&self, id: &str) -> Result<bool, PylosError> {
        Ok(self.keys.write().await.remove(id).is_some())
    }
}

#[derive(Default, Clone)]
pub struct InMemoryLogRepository {
    entries: Arc<RwLock<Vec<LogEntry>>>,
}

impl InMemoryLogRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl LogRepository for InMemoryLogRepository {
    async fn push(&self, entry: LogEntry) {
        self.entries.write().await.push(entry);
    }

    async fn list(&self, limit: usize, offset: usize, filter: &LogFilter) -> (Vec<LogEntry>, u64) {
        let guard = self.entries.read().await;
        let filtered: Vec<LogEntry> = guard
            .iter()
            .rev()
            .filter(|e| filter.matches(e))
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let page = filtered.into_iter().skip(offset).take(limit).collect();
        (page, total)
    }

    async fn stats(&self, filter: &LogFilter) -> LogStats {
        let guard = self.entries.read().await;
        let filtered: Vec<&LogEntry> = guard.iter().filter(|e| filter.matches(e)).collect();
        let total_requests = filtered.len() as u64;
        if total_requests == 0 {
            return LogStats::default();
        }
        let successes = filtered
            .iter()
            .filter(|e| e.status == LogStatus::Success)
            .count() as f64;
        let sum_latency: f64 = filtered.iter().map(|e| e.latency_ms).sum();
        LogStats {
            total_requests,
            success_rate: successes / total_requests as f64,
            average_latency_ms: sum_latency / total_requests as f64,
            total_tokens: filtered.iter().map(|e| i64::from(e.total_tokens)).sum(),
            total_cost_usd: filtered.iter().map(|e| e.cost_usd).sum(),
            total_prompt_tokens: filtered.iter().map(|e| i64::from(e.prompt_tokens)).sum(),
            total_completion_tokens: filtered
                .iter()
                .map(|e| i64::from(e.completion_tokens))
                .sum(),
            total_compression_saved_bytes: filtered
                .iter()
                .map(|e| e.compression_saved_bytes as i64)
                .sum(),
        }
    }

    async fn histogram(&self, _filter: &LogFilter, _bucket_secs: i64) -> Vec<HistogramBucket> {
        vec![]
    }

    async fn token_histogram(&self, _filter: &LogFilter, _bucket_secs: i64) -> Vec<TokenBucket> {
        vec![]
    }

    async fn list_guardrails(
        &self,
        limit: usize,
        offset: usize,
        filter: &LogFilter,
    ) -> (Vec<LogEntry>, u64) {
        let guard = self.entries.read().await;
        let filtered: Vec<LogEntry> = guard
            .iter()
            .rev()
            .filter(|e| e.guardrail_triggered == Some(true) && filter.matches(e))
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let page = filtered.into_iter().skip(offset).take(limit).collect();
        (page, total)
    }

    async fn guardrails_breakdown(&self, _filter: &LogFilter) -> GuardrailsBreakdown {
        GuardrailsBreakdown::default()
    }

    async fn guardrails_timeline(
        &self,
        _filter: &LogFilter,
        _bucket_secs: i64,
    ) -> Vec<GuardrailsTimeline> {
        vec![]
    }
}

#[derive(Default, Clone)]
pub struct InMemoryOrganizationRepository {
    orgs: Arc<RwLock<HashMap<String, Organization>>>,
}

impl InMemoryOrganizationRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OrganizationRepository for InMemoryOrganizationRepository {
    async fn list_organizations(&self) -> Result<Vec<Organization>, PylosError> {
        Ok(self.orgs.read().await.values().cloned().collect())
    }

    async fn get_organization(&self, id: &str) -> Result<Option<Organization>, PylosError> {
        Ok(self.orgs.read().await.get(id).cloned())
    }

    async fn upsert_organization(&self, org: &Organization) -> Result<(), PylosError> {
        self.orgs.write().await.insert(org.id.clone(), org.clone());
        Ok(())
    }

    async fn delete_organization(&self, id: &str) -> Result<bool, PylosError> {
        Ok(self.orgs.write().await.remove(id).is_some())
    }
}
