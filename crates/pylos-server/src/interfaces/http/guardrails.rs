use axum::{
    extract::{Query, State},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use pylos_core::domain::logging::{now_ms, LogFilter};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct GuardrailsQuery {
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub offset: usize,
    pub guardrail_type: Option<String>,
    pub period: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
}

fn default_limit() -> usize {
    50
}

impl GuardrailsQuery {
    fn to_filter(&self) -> LogFilter {
        let since_ms = self.since_ms.or_else(|| {
            self.period.as_deref().map(|p| {
                let now = now_ms();
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
            provider: None,
            model: None,
            status: None,
            since_ms,
            until_ms: self.until_ms,
            virtual_key: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct GuardrailsStatsQuery {
    pub period: Option<String>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
}

pub async fn get_guardrails_events(
    State(state): State<AppState>,
    Query(query): Query<GuardrailsQuery>,
) -> impl IntoResponse {
    let filter = query.to_filter();
    let limit = query.limit.min(1000);

    let (logs, total) = state
        .log_store
        .list_guardrails(limit, query.offset, &filter)
        .await;
    let stats = state.log_store.guardrails_stats(&filter).await;

    Json(json!({
        "events": logs,
        "pagination": {
            "limit": query.limit,
            "offset": query.offset,
            "total_count": total
        },
        "stats": stats,
        "has_events": total > 0
    }))
}

pub async fn get_guardrails_stats(
    State(state): State<AppState>,
    Query(query): Query<GuardrailsStatsQuery>,
) -> impl IntoResponse {
    let since_ms = query.since_ms.or_else(|| {
        query.period.as_deref().map(|p| {
            let now = now_ms();
            let duration_ms = match p {
                "1h" => 3_600_000,
                "6h" => 21_600_000,
                "24h" => 86_400_000,
                "7d" => 604_800_000,
                "30d" => 2_592_000_000,
                _ => 86_400_000,
            };
            now - duration_ms
        })
    });

    let filter = LogFilter {
        provider: None,
        model: None,
        status: None,
        since_ms,
        until_ms: query.until_ms,
        virtual_key: None,
    };

    let breakdown = state.log_store.guardrails_breakdown(&filter).await;
    let timeline = state.log_store.guardrails_timeline(&filter, 3600).await;

    Json(json!({
        "breakdown": breakdown,
        "timeline": timeline,
    }))
}
