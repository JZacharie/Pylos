use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::domain::openai::Usage;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: i64,
    pub provider: String,
    pub model: String,
    pub object: String,
    pub status: LogStatus,
    pub latency_ms: f64,
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub cost_usd: f64,
    pub finish_reason: Option<String>,
    pub error_message: Option<String>,
    pub virtual_key: Option<String>,
    pub is_stream: bool,
    pub input_preview: Option<String>,
    pub output_preview: Option<String>,
    pub compression_saved_bytes: usize,
    pub guardrail_triggered: Option<bool>,
    pub guardrail_type: Option<String>,
    pub guardrail_detail: Option<String>,
    pub obfuscated_input: Option<String>,
    pub pii_mapping: Option<String>,
    pub moderation_scores: Option<String>,
    pub caveman_input: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogStatus {
    Success,
    Error,
}

impl LogStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            LogStatus::Success => "success",
            LogStatus::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "success" {
            LogStatus::Success
        } else {
            LogStatus::Error
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LogStats {
    pub total_requests: u64,
    pub success_rate: f64,
    pub average_latency_ms: f64,
    pub total_tokens: i64,
    pub total_cost_usd: f64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_compression_saved_bytes: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistogramBucket {
    pub timestamp: i64,
    pub count: u64,
    pub success: u64,
    pub error: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenBucket {
    pub timestamp: i64,
    pub prompt_tokens: i64,
    pub completion_tokens: i64,
    pub total_tokens: i64,
}

#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub provider: Option<String>,
    pub model: Option<String>,
    pub status: Option<LogStatus>,
    pub since_ms: Option<i64>,
    pub until_ms: Option<i64>,
    pub virtual_key: Option<String>,
}

impl LogFilter {
    pub fn bucket_size_secs(&self) -> i64 {
        let range_ms = match (self.since_ms, self.until_ms) {
            (Some(s), Some(u)) => u - s,
            (Some(s), None) => now_ms() - s,
            _ => 3_600_000,
        };
        match range_ms / 1000 {
            0..=7200 => 60,
            7201..=86400 => 600,
            86401..=259200 => 3600,
            _ => 86400,
        }
    }

    pub fn matches(&self, entry: &LogEntry) -> bool {
        if let Some(ref p) = self.provider {
            if &entry.provider != p {
                return false;
            }
        }
        if let Some(ref m) = self.model {
            if !entry.model.contains(m.as_str()) {
                return false;
            }
        }
        if let Some(ref s) = self.status {
            if &entry.status != s {
                return false;
            }
        }
        if let Some(since) = self.since_ms {
            if entry.timestamp < since {
                return false;
            }
        }
        if let Some(until) = self.until_ms {
            if entry.timestamp > until {
                return false;
            }
        }
        if let Some(ref vk) = self.virtual_key {
            if entry.virtual_key.as_deref() != Some(vk.as_str()) {
                return false;
            }
        }
        true
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GuardrailsBreakdown {
    pub total_blocks: u64,
    pub keyword_blocks: u64,
    pub prompt_injection_blocks: u64,
    pub content_filter_blocks: u64,
    pub top_keywords: Vec<KeywordCount>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordCount {
    pub keyword: String,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuardrailsTimeline {
    pub timestamp: i64,
    pub total: u64,
    pub keyword_blocks: u64,
    pub prompt_injection: u64,
    pub content_filter: u64,
}

pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn generate_log_id() -> String {
    format!("log_{}", rand::random::<u64>())
}

#[allow(clippy::too_many_arguments)]
pub fn build_log_entry(
    provider: &str,
    model: &str,
    is_stream: bool,
    status: LogStatus,
    latency_ms: f64,
    usage: Option<&Usage>,
    finish_reason: Option<String>,
    error_message: Option<String>,
    input_preview: Option<String>,
    output_preview: Option<String>,
    virtual_key: Option<String>,
    compression_saved_bytes: usize,
) -> LogEntry {
    build_log_entry_full(
        provider,
        model,
        is_stream,
        status,
        latency_ms,
        usage,
        finish_reason,
        error_message,
        input_preview,
        output_preview,
        virtual_key,
        compression_saved_bytes,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn build_log_entry_full(
    provider: &str,
    model: &str,
    is_stream: bool,
    status: LogStatus,
    latency_ms: f64,
    usage: Option<&Usage>,
    finish_reason: Option<String>,
    error_message: Option<String>,
    input_preview: Option<String>,
    output_preview: Option<String>,
    virtual_key: Option<String>,
    compression_saved_bytes: usize,
    guardrail_triggered: Option<bool>,
    guardrail_type: Option<String>,
    guardrail_detail: Option<String>,
    obfuscated_input: Option<String>,
    pii_mapping: Option<String>,
    moderation_scores: Option<String>,
    caveman_input: Option<String>,
) -> LogEntry {
    let (prompt_tokens, completion_tokens, total_tokens) = usage
        .map(|u| (u.prompt_tokens, u.completion_tokens, u.total_tokens))
        .unwrap_or((0, 0, 0));

    LogEntry {
        id: generate_log_id(),
        timestamp: now_ms(),
        provider: provider.to_string(),
        model: model.to_string(),
        object: if is_stream {
            "chat.completion.stream".into()
        } else {
            "chat.completion".into()
        },
        status,
        latency_ms,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        cost_usd: estimate_cost(provider, model, prompt_tokens, completion_tokens),
        finish_reason,
        error_message,
        virtual_key,
        is_stream,
        input_preview,
        output_preview,
        compression_saved_bytes,
        guardrail_triggered,
        guardrail_type,
        guardrail_detail,
        obfuscated_input,
        pii_mapping,
        moderation_scores,
        caveman_input,
    }
}

pub fn estimate_cost(provider: &str, model: &str, prompt: i32, completion: i32) -> f64 {
    if provider.starts_with("ollama") {
        return 0.0;
    }
    let (in_m, out_m): (f64, f64) = match provider {
        "ollama-jo3" => (0.0, 0.0),
        "openai" | "openrouter" => {
            if model.contains("gpt-4o-mini") {
                (0.15, 0.60)
            } else if model.contains("gpt-4o") {
                (5.0, 15.0)
            } else if model.contains("o4-mini") || model.contains("o3-mini") {
                (1.1, 4.4)
            } else if model.contains("o3") {
                (10.0, 40.0)
            } else if model.contains("o1-mini") {
                (3.0, 12.0)
            } else if model.contains("o1") {
                (15.0, 60.0)
            } else if model.starts_with("text-embedding") {
                (0.1, 0.0)
            } else {
                (1.0, 3.0)
            }
        }
        "anthropic" => {
            if model.contains("haiku-3-5") || model.contains("haiku-4") {
                (0.8, 4.0)
            } else if model.contains("haiku") {
                (0.25, 1.25)
            } else if model.contains("sonnet-4")
                || model.contains("sonnet-3-7")
                || model.contains("sonnet")
            {
                (3.0, 15.0)
            } else {
                (15.0, 75.0)
            }
        }
        "gemini" => {
            if model.contains("2.5-pro") {
                (7.0, 21.0)
            } else if model.contains("2.5-flash") {
                (0.3, 2.5)
            } else if model.contains("2.0-flash") {
                (0.1, 0.4)
            } else if model.contains("1.5-pro") {
                (3.5, 10.5)
            } else if model.contains("1.5-flash") {
                (0.075, 0.3)
            } else if model.contains("embedding") {
                (0.025, 0.0)
            } else {
                (0.5, 1.5)
            }
        }
        "cohere" => {
            if model.contains("command-a") {
                (2.5, 10.0)
            } else if model.contains("command-r-plus") {
                (3.0, 15.0)
            } else if model.contains("command-r") {
                (0.15, 0.60)
            } else if model.contains("embed") {
                (0.1, 0.0)
            } else {
                (1.0, 3.0)
            }
        }
        "groq" => {
            if model.contains("llama-3.3-70b") || model.contains("llama-3.1-70b") {
                (0.59, 0.79)
            } else if model.contains("llama-3.1-8b") {
                (0.05, 0.08)
            } else if model.contains("mixtral") {
                (0.24, 0.24)
            } else {
                (0.2, 0.2)
            }
        }
        "mistral" => {
            if model.contains("large") {
                (3.0, 9.0)
            } else if model.contains("codestral") {
                (0.3, 0.9)
            } else {
                (0.2, 0.6)
            }
        }
        "xai" => {
            if model.contains("grok-3-mini") {
                (0.3, 0.5)
            } else {
                (5.0, 15.0)
            }
        }
        "deepseek" => {
            if model.contains("reasoner") || model.contains("r1") {
                (0.55, 2.19)
            } else {
                (0.14, 0.28)
            }
        }
        "bedrock" => {
            if model.contains("nova-lite") {
                (0.06, 0.24)
            } else if model.contains("nova-pro") {
                (0.80, 3.20)
            } else if model.contains("nova-micro") {
                (0.035, 0.14)
            } else if model.contains("haiku") {
                (0.25, 1.25)
            } else if model.contains("sonnet") {
                (3.0, 15.0)
            } else if model.contains("claude") {
                (15.0, 75.0)
            } else {
                (0.50, 1.50)
            }
        }
        _ => (1.0, 3.0),
    };
    let cost = (prompt as f64 / 1_000_000.0) * in_m + (completion as f64 / 1_000_000.0) * out_m;
    (cost * 1_000_000.0).round() / 1_000_000.0
}
