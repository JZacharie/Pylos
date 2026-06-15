use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use pylos_application::log_store::{LogFilter, LogStatus};

use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Query params
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<String>,
    /// Période prédéfinie : "1h", "6h", "24h", "7d", "30d"
    pub period: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub virtual_key: Option<String>,
    pub finish_reason: Option<String>,
}

fn default_limit() -> usize {
    50
}

impl LogsQuery {
    fn to_filter(&self) -> LogFilter {
        let status = self.status.as_deref().and_then(|s| match s {
            "success" => Some(LogStatus::Success),
            "error" => Some(LogStatus::Error),
            _ => None,
        });

        let since_ms = self.since_ms.or_else(|| {
            self.period.as_deref().map(|p| {
                let now = pylos_application::log_store::now_ms();
                let duration_ms = match p {
                    "1h" => 3_600_000,
                    "6h" => 21_600_000,
                    "24h" => 86_400_000,
                    "7d" => 604_800_000,
                    "30d" => 2_592_000_000,
                    _ => 3_600_000,
                };
                now - duration_ms
            })
        });

        LogFilter {
            provider: self.provider.clone(),
            model: self.model.clone(),
            status,
            since_ms,
            until_ms: self.until_ms,
            virtual_key: self.virtual_key.clone(),
            finish_reason: self.finish_reason.clone(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/logs
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_logs(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    let filter = query.to_filter();
    let limit = query.limit.min(1000);
    let (logs, total) = state.log_store.list(limit, query.offset, &filter).await;

    let stats = state.log_store.stats(&filter).await;

    Json(json!({
        "logs": logs,
        "pagination": {
            "limit": query.limit,
            "offset": query.offset,
            "total_count": total
        },
        "stats": stats,
        "has_logs": total > 0
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/logs/stats
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_logs_stats(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    let filter = query.to_filter();
    let stats = state.log_store.stats(&filter).await;
    Json(stats)
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/logs/histogram
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_logs_histogram(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    let filter = query.to_filter();
    let bucket_secs = filter.bucket_size_secs();
    let buckets = state.log_store.histogram(&filter, bucket_secs).await;

    Json(json!({
        "buckets": buckets,
        "bucket_size_seconds": bucket_secs
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/logs/histogram/tokens
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_token_histogram(
    State(state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> impl IntoResponse {
    let filter = query.to_filter();
    let bucket_secs = filter.bucket_size_secs();
    let buckets = state.log_store.token_histogram(&filter, bucket_secs).await;

    Json(json!({
        "buckets": buckets,
        "bucket_size_seconds": bucket_secs
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /api/logs/filterdata — valeurs disponibles pour les filtres UI
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_filter_data(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config_store.get().await;

    let providers: Vec<String> = cfg.providers.keys().cloned().collect();
    let mut virtual_keys: Vec<_> = cfg
        .governance
        .virtual_keys
        .iter()
        .map(|vk| json!({"id": vk.id, "name": vk.name}))
        .collect();

    if let Ok(db_vks) = state.vk_store.list_keys().await {
        for vk in db_vks {
            if !virtual_keys
                .iter()
                .any(|v| v.get("id").and_then(|i| i.as_str()) == Some(&vk.id))
            {
                virtual_keys.push(json!({"id": vk.id, "name": vk.name}));
            }
        }
    }

    Json(json!({
        "providers": providers,
        "virtual_keys": virtual_keys,
        "status": ["success", "error"],
        "objects": ["chat.completion", "chat.completion.stream"]
    }))
}
