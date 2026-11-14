use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use pylos_core::error::PylosError;

const DEFAULT_WINDOW_MS: i64 = 5 * 3600 * 1000; // 5h sliding window
const DEFAULT_SOFT_CAP: u64 = 50;
const DEFAULT_HARD_CAP: u64 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestPriority {
    Interactive,
    Recurring,
}

impl RequestPriority {
    pub fn from_header(val: Option<&str>) -> Self {
        match val.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
            Some("recurring" | "background" | "cron" | "probe") => Self::Recurring,
            _ => Self::Interactive,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuotaStatus {
    Ok,
    Low,
    Saturated,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountQuotaSnapshot {
    pub provider: String,
    pub account: String,
    pub used_requests: u64,
    pub soft_cap: u64,
    pub hard_cap: u64,
    pub remaining_pct: f64,
    pub reset_at_ms: i64,
    pub status: QuotaStatus,
    pub cooldown_until_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuotaOverview {
    pub accounts: Vec<AccountQuotaSnapshot>,
    pub economy_mode: bool,
    pub low_threshold_pct: f64,
    pub shed_count: u64,
}

#[derive(Debug, Clone)]
struct AccountEntry {
    provider: String,
    account: String,
    used_requests: u64,
    soft_cap: u64,
    hard_cap: u64,
    reset_at_ms: i64,
    cooldown_until_ms: Option<i64>,
}

impl AccountEntry {
    fn new(provider: &str, account: &str, soft_cap: u64, hard_cap: u64, now_ms: i64) -> Self {
        Self {
            provider: provider.to_string(),
            account: account.to_string(),
            used_requests: 0,
            soft_cap: soft_cap.max(1),
            hard_cap: hard_cap.max(soft_cap).max(1),
            reset_at_ms: now_ms + DEFAULT_WINDOW_MS,
            cooldown_until_ms: None,
        }
    }

    fn refresh(&mut self, now_ms: i64) {
        if now_ms >= self.reset_at_ms {
            self.used_requests = 0;
            self.reset_at_ms = now_ms + DEFAULT_WINDOW_MS;
            self.cooldown_until_ms = None;
        }
        if let Some(until) = self.cooldown_until_ms {
            if now_ms >= until {
                self.cooldown_until_ms = None;
            }
        }
    }

    fn remaining_pct(&self) -> f64 {
        let cap = self.hard_cap.max(1) as f64;
        let rem = self.hard_cap.saturating_sub(self.used_requests) as f64;
        ((rem / cap) * 100.0).clamp(0.0, 100.0)
    }

    fn status(&self, low_threshold_pct: f64, now_ms: i64) -> QuotaStatus {
        if let Some(until) = self.cooldown_until_ms {
            if now_ms < until {
                return QuotaStatus::Saturated;
            }
        }
        if self.used_requests >= self.hard_cap {
            return QuotaStatus::Blocked;
        }
        if self.used_requests >= self.soft_cap {
            return QuotaStatus::Saturated;
        }
        if self.remaining_pct() < low_threshold_pct {
            return QuotaStatus::Low;
        }
        QuotaStatus::Ok
    }

    fn to_snapshot(&self, low_threshold_pct: f64, now_ms: i64) -> AccountQuotaSnapshot {
        AccountQuotaSnapshot {
            provider: self.provider.clone(),
            account: self.account.clone(),
            used_requests: self.used_requests,
            soft_cap: self.soft_cap,
            hard_cap: self.hard_cap,
            remaining_pct: (self.remaining_pct() * 10.0).round() / 10.0,
            reset_at_ms: self.reset_at_ms,
            status: self.status(low_threshold_pct, now_ms),
            cooldown_until_ms: self.cooldown_until_ms,
        }
    }
}

/// Garde de quota multi-comptes (Antigravity / Free tiers) et délestage à seuil bas (#209, #212).
pub struct QuotaGuard {
    entries: Mutex<HashMap<(String, String), AccountEntry>>,
    economy_mode: AtomicBool,
    low_threshold_bits: AtomicU64,
    shed_count: AtomicU64,
}

impl Default for QuotaGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl QuotaGuard {
    pub fn new() -> Self {
        let guard = Self {
            entries: Mutex::new(HashMap::new()),
            economy_mode: AtomicBool::new(true),
            low_threshold_bits: AtomicU64::new(20.0_f64.to_bits()),
            shed_count: AtomicU64::new(0),
        };
        guard.register_accounts("antigravity", vec!["default".to_string()], DEFAULT_SOFT_CAP, DEFAULT_HARD_CAP);
        guard
    }

    fn now_ms() -> i64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64
    }

    pub fn low_threshold_pct(&self) -> f64 {
        f64::from_bits(self.low_threshold_bits.load(Ordering::Relaxed))
    }

    pub fn set_low_threshold_pct(&self, pct: f64) {
        self.low_threshold_bits
            .store(pct.clamp(1.0, 95.0).to_bits(), Ordering::Relaxed);
    }

    pub fn economy_mode(&self) -> bool {
        self.economy_mode.load(Ordering::Relaxed)
    }

    pub fn set_economy_mode(&self, enabled: bool) {
        self.economy_mode.store(enabled, Ordering::Relaxed);
    }

    pub fn register_accounts(
        &self,
        provider: &str,
        accounts: Vec<String>,
        soft_cap: u64,
        hard_cap: u64,
    ) {
        let now = Self::now_ms();
        let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        if !accounts.is_empty() && !accounts.iter().any(|a| a == "default") {
            map.remove(&(provider.to_string(), "default".to_string()));
        }
        for acc in accounts {
            let key = (provider.to_string(), acc.clone());
            map.entry(key)
                .and_modify(|e| {
                    e.soft_cap = soft_cap.max(1);
                    e.hard_cap = hard_cap.max(soft_cap).max(1);
                })
                .or_insert_with(|| AccountEntry::new(provider, &acc, soft_cap, hard_cap, now));
        }
    }

    /// Sélectionne le meilleur compte disponible (plus fort % restant, hors cooldown/soft-cap)
    /// puis consomme 1 unité de quota.
    pub fn check_and_consume(
        &self,
        provider: &str,
        requested_account: Option<&str>,
        priority: RequestPriority,
    ) -> Result<String, PylosError> {
        let now = Self::now_ms();
        let low_pct = self.low_threshold_pct();
        let economy = self.economy_mode();
        let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());

        // Si le provider n'a pas encore de compte enregistré, en créer un par défaut.
        if !map.keys().any(|(p, _)| p == provider) {
            let acc = requested_account.unwrap_or("default").to_string();
            map.insert(
                (provider.to_string(), acc.clone()),
                AccountEntry::new(provider, &acc, DEFAULT_SOFT_CAP, DEFAULT_HARD_CAP, now),
            );
        }

        // Refresh window & cooldowns for this provider
        for ((p, _), entry) in map.iter_mut() {
            if p == provider {
                entry.refresh(now);
            }
        }

        let mut candidates: Vec<(String, f64, QuotaStatus)> = map
            .iter()
            .filter(|((p, acc), _)| {
                p == provider && requested_account.is_none_or(|req| req == "auto" || req == acc)
            })
            .map(|((_, acc), entry)| {
                (
                    acc.clone(),
                    entry.remaining_pct(),
                    entry.status(low_pct, now),
                )
            })
            .collect();

        // Sort highest remaining percentage first
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // 1. Check if all accounts are saturated/blocked
        let non_saturated: Vec<_> = candidates
            .iter()
            .filter(|(_, _, st)| !matches!(st, QuotaStatus::Saturated | QuotaStatus::Blocked))
            .collect();

        if non_saturated.is_empty() {
            return Err(PylosError::RateLimitExceeded(format!(
                "All quota accounts for '{}' reached soft/hard cap or cooldown",
                provider
            )));
        }

        // 2. Low-threshold shedding for recurring/background tasks (#212)
        if economy && priority == RequestPriority::Recurring {
            if let Some((_, rem_pct, QuotaStatus::Low)) = non_saturated.first() {
                self.shed_count.fetch_add(1, Ordering::Relaxed);
                return Err(PylosError::RateLimitExceeded(format!(
                    "quota_low_shedding: '{}' remaining quota {:.1}% < {:.1}% threshold — reserved for interactive requests",
                    provider, rem_pct, low_pct
                )));
            }
        }

        let chosen_acc = non_saturated[0].0.clone();
        if let Some(entry) = map.get_mut(&(provider.to_string(), chosen_acc.clone())) {
            entry.used_requests += 1;
        }
        Ok(chosen_acc)
    }

    pub fn mark_account_saturated(&self, provider: &str, account: &str, cooldown_secs: u64) {
        let now = Self::now_ms();
        let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let entry = map
            .entry((provider.to_string(), account.to_string()))
            .or_insert_with(|| {
                AccountEntry::new(provider, account, DEFAULT_SOFT_CAP, DEFAULT_HARD_CAP, now)
            });
        entry.cooldown_until_ms = Some(now + (cooldown_secs.max(1) as i64) * 1000);
    }

    pub fn overview(&self) -> QuotaOverview {
        let now = Self::now_ms();
        let low_pct = self.low_threshold_pct();
        let mut map = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let mut accounts: Vec<AccountQuotaSnapshot> = map
            .values_mut()
            .map(|e| {
                e.refresh(now);
                e.to_snapshot(low_pct, now)
            })
            .collect();
        accounts.sort_by(|a, b| a.provider.cmp(&b.provider).then(a.account.cmp(&b.account)));
        QuotaOverview {
            accounts,
            economy_mode: self.economy_mode(),
            low_threshold_pct: low_pct,
            shed_count: self.shed_count.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_account_rotation_avoids_soft_cap() {
        let guard = QuotaGuard::new();
        guard.register_accounts(
            "antigravity",
            vec!["acc1".into(), "acc2".into()],
            2,
            4,
        );

        let a1 = guard
            .check_and_consume("antigravity", Some("acc1"), RequestPriority::Interactive)
            .unwrap();
        assert_eq!(a1, "acc1");
        let _ = guard
            .check_and_consume("antigravity", Some("acc1"), RequestPriority::Interactive)
            .unwrap();

        // acc1 is now at soft_cap (2), auto-select must rotate to acc2
        let next = guard
            .check_and_consume("antigravity", None, RequestPriority::Interactive)
            .unwrap();
        assert_eq!(next, "acc2");
    }

    #[test]
    fn test_low_threshold_sheds_recurring_but_allows_interactive() {
        let guard = QuotaGuard::new();
        guard.register_accounts("antigravity", vec!["default".into()], 9, 10);
        guard.set_low_threshold_pct(20.0);
        guard.set_economy_mode(true);

        // Consume 9 -> remaining = 10% (< 20%, below hard_cap 10, wait soft_cap=9 makes it Saturated at 9!)
        // Use soft_cap=9, hard_cap=10 with 8 requests -> 20% left; 9th request leaves 10% -> wait soft_cap=10, hard_cap=10!
        guard.register_accounts("antigravity", vec!["default".into()], 10, 10);
        for _ in 0..9 {
            guard
                .check_and_consume("antigravity", Some("default"), RequestPriority::Interactive)
                .unwrap();
        }
        // Remaining = 10% (< 20% threshold)
        let recurring =
            guard.check_and_consume("antigravity", Some("default"), RequestPriority::Recurring);
        assert!(recurring.is_err());
        assert!(recurring.unwrap_err().to_string().contains("quota_low_shedding"));

        // Interactive still succeeds on the reserved 10%
        let interactive =
            guard.check_and_consume("antigravity", Some("default"), RequestPriority::Interactive);
        assert!(interactive.is_ok());
    }
}
