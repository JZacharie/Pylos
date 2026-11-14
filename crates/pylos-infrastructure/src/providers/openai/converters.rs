use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionResponse, MessageRole, Usage,
};
use pylos_core::domain::request::{PylosResponse, StreamChoice, StreamChunk, StreamDelta};
use pylos_core::error::PylosError;
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────────────────────
// Structures de requête OpenAI (format natif de l'API)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct OpenAIChatRequest<'a> {
    pub model: &'a str,
    pub messages: Vec<OpenAIMessage<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_completion_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<&'a [pylos_core::domain::openai::Tool]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<&'a pylos_core::domain::openai::ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<&'a str>,
}

#[derive(Debug, Serialize)]
pub(crate) struct StreamOptions {
    pub include_usage: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAITextRequest<'a> {
    pub model: &'a str,
    pub prompt: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<i32>,
    pub stream: bool,
    pub echo: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OpenAIMessage<'a> {
    pub role: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<&'a [pylos_core::domain::openai::ToolCall]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<&'a str>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Structures de réponse OpenAI
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIChatResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIChoice {
    pub index: i32,
    pub message: OpenAIResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAITextResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAITextChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAITextChoice {
    pub text: String,
    pub index: i32,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<pylos_core::domain::openai::ToolCall>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIUsage {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
    pub reasoning_tokens: Option<i32>,
    pub prompt_cache_hit_tokens: Option<i32>,
    pub prompt_cache_miss_tokens: Option<i32>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Structures de streaming OpenAI (SSE)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIStreamChunk {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIStreamChoice>,
    pub usage: Option<OpenAIUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIStreamChoice {
    pub index: i32,
    pub delta: OpenAIStreamDelta,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIStreamDelta {
    pub role: Option<String>,
    pub content: Option<String>,
    pub reasoning_content: Option<String>,
    pub tool_calls: Option<Vec<pylos_core::domain::request::StreamToolCallChunk>>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Conversions : OpenAI natif → Pylos domain
// ──────────────────────────────────────────────────────────────────────────────

pub(crate) fn role_to_str(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::System => "system",
        MessageRole::User => "user",
        MessageRole::Assistant => "assistant",
        MessageRole::Tool => "tool",
        MessageRole::Function => "function",
    }
}

pub(crate) fn str_to_role(s: &str) -> MessageRole {
    match s {
        "system" => MessageRole::System,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        "function" => MessageRole::Function,
        _ => MessageRole::User,
    }
}

pub(crate) fn to_openai_request<'a>(
    req: &'a pylos_core::domain::openai::ChatCompletionRequest,
    stream: bool,
) -> OpenAIChatRequest<'a> {
    OpenAIChatRequest {
        model: &req.model,
        messages: req
            .messages
            .iter()
            .map(|m| OpenAIMessage {
                role: role_to_str(&m.role),
                content: m.content.as_deref(),
                name: m.name.as_deref(),
                tool_calls: m.tool_calls.as_deref(),
                tool_call_id: m.tool_call_id.as_deref(),
            })
            .collect(),
        temperature: req.temperature,
        top_p: req.top_p,
        n: req.n,
        stream,
        max_tokens: req.max_tokens,
        presence_penalty: req.presence_penalty,
        frequency_penalty: req.frequency_penalty,
        user: req.user.as_deref(),
        max_completion_tokens: req.max_completion_tokens,
        stream_options: if stream {
            Some(StreamOptions {
                include_usage: true,
            })
        } else {
            None
        },
        tools: req.tools.as_deref(),
        tool_choice: req.tool_choice.as_ref(),
        reasoning_effort: req.reasoning_effort.as_deref(),
    }
}

pub(crate) fn to_openai_text_request<'a>(
    req: &'a pylos_core::domain::openai::TextCompletionRequest,
    stream: bool,
) -> OpenAITextRequest<'a> {
    OpenAITextRequest {
        model: &req.model,
        prompt: req.prompt.as_strings(),
        suffix: req.suffix.as_deref(),
        max_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        n: req.n,
        stream,
        echo: req.echo,
        stop: match &req.stop {
            Some(pylos_core::domain::openai::StopCondition::Single(s)) => Some(vec![s.clone()]),
            Some(pylos_core::domain::openai::StopCondition::Multiple(v)) => Some(v.clone()),
            None => None,
        },
        stream_options: if stream {
            Some(StreamOptions {
                include_usage: true,
            })
        } else {
            None
        },
    }
}

pub(crate) fn from_openai_text_response(resp: OpenAITextResponse) -> PylosResponse {
    PylosResponse::ChatCompletion(ChatCompletionResponse {
        id: resp.id,
        object: resp.object,
        created: resp.created,
        model: resp.model,
        choices: resp
            .choices
            .into_iter()
            .map(|c| ChatCompletionChoice {
                index: c.index,
                message: ChatCompletionMessage {
                    role: MessageRole::Assistant,
                    content: Some(c.text),
                    reasoning_content: None,
                    name: None,
                    tool_calls: None,
                    tool_call_id: None,
                },
                finish_reason: c.finish_reason,
            })
            .collect(),
        usage: resp.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            reasoning_tokens: u.reasoning_tokens,
            prompt_cache_hit_tokens: u.prompt_cache_hit_tokens,
            prompt_cache_miss_tokens: u.prompt_cache_miss_tokens,
        }),
    })
}

pub(crate) fn from_openai_text_stream_chunk(resp: OpenAITextResponse) -> StreamChunk {
    StreamChunk {
        id: resp.id,
        object: resp.object,
        created: resp.created,
        model: resp.model,
        choices: resp
            .choices
            .into_iter()
            .map(|c| StreamChoice {
                index: c.index,
                delta: StreamDelta {
                    role: Some("assistant".into()),
                    content: Some(c.text),
                    ..Default::default()
                },
                finish_reason: c.finish_reason,
            })
            .collect(),
        usage: resp.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            reasoning_tokens: u.reasoning_tokens,
            prompt_cache_hit_tokens: u.prompt_cache_hit_tokens,
            prompt_cache_miss_tokens: u.prompt_cache_miss_tokens,
        }),
    }
}

pub(crate) fn from_openai_response(resp: OpenAIChatResponse) -> PylosResponse {
    PylosResponse::ChatCompletion(ChatCompletionResponse {
        id: resp.id,
        object: resp.object,
        created: resp.created,
        model: resp.model,
        choices: resp
            .choices
            .into_iter()
            .map(|c| ChatCompletionChoice {
                index: c.index,
                message: ChatCompletionMessage {
                    role: str_to_role(&c.message.role),
                    content: Some(c.message.content.unwrap_or_default()),
                    reasoning_content: c.message.reasoning_content,
                    tool_calls: c.message.tool_calls,
                    ..Default::default()
                },
                finish_reason: c.finish_reason,
            })
            .collect(),
        usage: resp.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            reasoning_tokens: u.reasoning_tokens,
            prompt_cache_hit_tokens: u.prompt_cache_hit_tokens,
            prompt_cache_miss_tokens: u.prompt_cache_miss_tokens,
        }),
    })
}

pub(crate) fn from_openai_stream_chunk(chunk: OpenAIStreamChunk) -> StreamChunk {
    StreamChunk {
        id: chunk.id,
        object: chunk.object,
        created: chunk.created,
        model: chunk.model,
        choices: chunk
            .choices
            .into_iter()
            .map(|c| StreamChoice {
                index: c.index,
                delta: StreamDelta {
                    role: c.delta.role,
                    content: c.delta.content,
                    reasoning_content: c.delta.reasoning_content,
                    tool_calls: c.delta.tool_calls,
                },
                finish_reason: c.finish_reason,
            })
            .collect(),
        usage: chunk.usage.map(|u| Usage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            total_tokens: u.total_tokens,
            reasoning_tokens: u.reasoning_tokens,
            prompt_cache_hit_tokens: u.prompt_cache_hit_tokens,
            prompt_cache_miss_tokens: u.prompt_cache_miss_tokens,
        }),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Conversion d'erreurs HTTP OpenAI
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIErrorBody {
    pub error: OpenAIErrorDetail,
}

#[derive(Debug, Deserialize)]
pub(crate) struct OpenAIErrorDetail {
    pub message: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub error_type: Option<String>,
    #[allow(dead_code)]
    pub code: Option<String>,
}

pub(crate) fn map_openai_error(status: u16, body: &str) -> PylosError {
    let message = serde_json::from_str::<OpenAIErrorBody>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| body.to_string());

    match status {
        401 => PylosError::Unauthorized(message),
        429 => PylosError::RateLimitExceeded(message),
        408 | 504 => PylosError::Timeout(message),
        _ => PylosError::ProviderError {
            provider: "openai".into(),
            message,
        },
    }
}
