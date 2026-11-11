use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionResponse, MessageRole, Usage,
};
use pylos_core::domain::request::{StreamChoice, StreamChunk, StreamDelta};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Deserialize)]
pub struct AgyUsage {
    #[serde(default)]
    pub input_tokens: i32,
    #[serde(default)]
    pub output_tokens: i32,
    #[serde(default)]
    pub thinking_tokens: i32,
    #[serde(default)]
    pub cache_read_tokens: i32,
    #[serde(default)]
    pub total_tokens: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgyResponse {
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub response: String,
    pub error: Option<String>,
    pub duration_seconds: Option<f64>,
    pub num_turns: Option<i32>,
    pub usage: Option<AgyUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgyStreamEvent {
    pub event: String,
    pub conversation_id: Option<String>,
    pub step_update: Option<AgyStepUpdate>,
    pub result: Option<AgyResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct AgyStepUpdate {
    pub conversation_id: Option<String>,
    pub step_index: Option<i32>,
    pub state: Option<String>,
    pub step_type: Option<String>,
    pub text_delta: Option<String>,
    pub duration_seconds: Option<f64>,
    pub usage: Option<AgyUsage>,
}

/// Normalise le nom du modèle pour Antigravity CLI (agy)
pub fn normalize_antigravity_model(model: &str) -> String {
    let clean = model
        .trim_start_matches("antigravity/")
        .trim_start_matches("antigravity:")
        .trim_start_matches("agy/")
        .trim_start_matches("agy:");

    if clean.ends_with("-low") || clean.ends_with("-medium") || clean.ends_with("-high") {
        return clean.to_string();
    }

    match clean {
        "gemini-3.8-flash" | "gemini-flash" | "flash" => "gemini-3.8-flash-medium".into(),
        "gemini-4-argon" | "argon" | "gemini-argon" => "gemini-4-argon-medium".into(),
        "gemini-2.5-flash" | "gemini-1.5-flash" => "gemini-3.8-flash-medium".into(),
        "gemini-2.5-pro" | "gemini-1.5-pro" | "pro" => "gemini-4-argon-medium".into(),
        "claude-opus-5-5" | "claude-opus" | "opus" => "claude-opus-5-5-medium".into(),
        "claude-sonnet-5-5" | "claude-sonnet" | "sonnet" => "claude-sonnet-5-5-medium".into(),
        "gpt-oss-120b" | "gpt-oss" => "gpt-oss-120b-medium".into(),
        other if other.is_empty() => "gemini-3.8-flash-medium".into(),
        other => {
            if other.contains("argon") {
                "gemini-4-argon-medium".into()
            } else if other.contains("claude") && other.contains("opus") {
                "claude-opus-5-5-medium".into()
            } else if other.contains("claude") {
                "claude-sonnet-5-5-medium".into()
            } else {
                format!("{}-medium", other)
            }
        }
    }
}

/// Convertit la réponse Antigravity en ChatCompletionResponse standard
pub fn to_chat_completion_response(
    agy_resp: AgyResponse,
    requested_model: &str,
) -> ChatCompletionResponse {
    let conv_id = agy_resp
        .conversation_id
        .clone()
        .unwrap_or_else(|| fastrand::u64(..).to_string());

    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let usage = agy_resp.usage.map(|u| Usage {
        prompt_tokens: u.input_tokens,
        completion_tokens: u.output_tokens,
        total_tokens: if u.total_tokens > 0 {
            u.total_tokens
        } else {
            u.input_tokens + u.output_tokens
        },
        reasoning_tokens: if u.thinking_tokens > 0 {
            Some(u.thinking_tokens)
        } else {
            None
        },
        prompt_cache_hit_tokens: if u.cache_read_tokens > 0 {
            Some(u.cache_read_tokens)
        } else {
            None
        },
        prompt_cache_miss_tokens: None,
    });

    ChatCompletionResponse {
        id: format!("chatcmpl-agy-{}", conv_id),
        object: "chat.completion".to_string(),
        created,
        model: requested_model.to_string(),
        choices: vec![ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: MessageRole::Assistant,
                content: Some(agy_resp.response),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            },
            finish_reason: Some("stop".to_string()),
        }],
        usage,
    }
}

/// Crée un StreamChunk de delta textuel pour SSE
pub fn to_stream_chunk_delta(
    delta: &str,
    conv_id: &str,
    requested_model: &str,
) -> StreamChunk {
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    StreamChunk {
        id: format!("chatcmpl-agy-{}", conv_id),
        object: "chat.completion.chunk".to_string(),
        created,
        model: requested_model.to_string(),
        choices: vec![StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: Some("assistant".to_string()),
                content: Some(delta.to_string()),
                reasoning_content: None,
                tool_calls: None,
            },
            finish_reason: None,
        }],
        usage: None,
    }
}

/// Crée le dernier StreamChunk marquant la fin du stream et incluant l'usage exact des quotas
pub fn to_stream_chunk_final(
    conv_id: &str,
    requested_model: &str,
    usage: Option<AgyUsage>,
) -> StreamChunk {
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let openai_usage = usage.map(|u| Usage {
        prompt_tokens: u.input_tokens,
        completion_tokens: u.output_tokens,
        total_tokens: if u.total_tokens > 0 {
            u.total_tokens
        } else {
            u.input_tokens + u.output_tokens
        },
        reasoning_tokens: if u.thinking_tokens > 0 {
            Some(u.thinking_tokens)
        } else {
            None
        },
        prompt_cache_hit_tokens: if u.cache_read_tokens > 0 {
            Some(u.cache_read_tokens)
        } else {
            None
        },
        prompt_cache_miss_tokens: None,
    });

    StreamChunk {
        id: format!("chatcmpl-agy-{}", conv_id),
        object: "chat.completion.chunk".to_string(),
        created,
        model: requested_model.to_string(),
        choices: vec![StreamChoice {
            index: 0,
            delta: StreamDelta::default(),
            finish_reason: Some("stop".to_string()),
        }],
        usage: openai_usage,
    }
}
