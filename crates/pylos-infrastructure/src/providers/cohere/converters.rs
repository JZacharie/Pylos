use serde::{Deserialize, Serialize};

use pylos_core::domain::embedding::{EmbeddingData, EmbeddingResponse, EmbeddingUsage};
use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionResponse, MessageRole, ToolCall,
    ToolCallFunction, Usage,
};
use pylos_core::domain::request::{
    PylosResponse, StreamChoice, StreamChunk, StreamDelta, StreamToolCallChunk,
    StreamToolCallFunction,
};

// ─────────────────────────────────────────────────────────────────────────────
// Structures de requête Cohere v2
// Bifrost source: core/providers/cohere/types.go
// Doc: https://docs.cohere.com/reference/chat
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct CohereChatRequest {
    pub model: String,
    pub messages: Vec<CohereMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub p: Option<f32>, // top_p
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    pub stream: bool,
    /// Outils exposés au modèle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<CohereTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<String>, // "REQUIRED" | "NONE"
}

#[derive(Debug, Serialize)]
pub(crate) struct CohereMessage {
    pub role: String, // "system" | "user" | "assistant" | "tool"
    pub content: serde_json::Value,
}

// ── Définitions d'outils Cohere ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct CohereTool {
    #[serde(rename = "type")]
    pub tool_type: String, // "function"
    pub function: CohereToolFunction,
}

#[derive(Debug, Serialize)]
pub(crate) struct CohereToolFunction {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Structures de réponse Cohere
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct CohereChatResponse {
    pub id: Option<String>,
    pub finish_reason: Option<String>,
    pub message: Option<CohereResponseMessage>,
    pub usage: Option<CohereUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereResponseMessage {
    #[allow(dead_code)]
    pub role: Option<String>,
    pub content: Option<Vec<CohereContentBlock>>,
    /// Tool calls émis par l'assistant
    pub tool_calls: Option<Vec<CohereToolCall>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereContentBlock {
    #[serde(rename = "type")]
    pub block_type: Option<String>,
    pub text: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereToolCall {
    pub id: Option<String>,
    #[serde(rename = "type")]
    pub call_type: Option<String>,
    pub function: Option<CohereToolCallFunction>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereToolCallFunction {
    pub name: Option<String>,
    pub arguments: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereUsage {
    pub billed_units: Option<CohereTokens>,
    pub tokens: Option<CohereTokens>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereTokens {
    pub input_tokens: Option<i32>,
    pub output_tokens: Option<i32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Streaming Cohere
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct CohereStreamEvent {
    #[serde(rename = "type")]
    pub event_type: Option<String>,
    pub delta: Option<CohereStreamDelta>,
    #[allow(dead_code)]
    pub index: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereStreamDelta {
    pub message: Option<CohereStreamMessage>,
    pub finish_reason: Option<String>,
    #[allow(dead_code)]
    pub usage: Option<CohereUsage>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereStreamMessage {
    pub content: Option<CohereStreamContent>,
    #[allow(dead_code)]
    pub role: Option<String>,
    /// Tool calls streamés
    pub tool_calls: Option<Vec<CohereToolCall>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereStreamContent {
    pub text: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub content_type: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Embeddings Cohere v2
// POST /v2/embed
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct CohereEmbedRequest {
    pub model: String,
    pub texts: Vec<String>,
    pub input_type: String, // "search_document" | "search_query" | ...
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_dimension: Option<u32>,
    pub embedding_types: Vec<String>, // ["float"]
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereEmbedResponse {
    #[allow(dead_code)]
    pub id: Option<String>,
    pub embeddings: Option<CohereEmbeddings>,
    pub meta: Option<CohereEmbedMeta>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereEmbeddings {
    pub float: Option<Vec<Vec<f32>>>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct CohereEmbedMeta {
    pub billed_units: Option<CohereTokens>,
    pub tokens: Option<CohereTokens>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversions
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn to_cohere_request(
    req: &pylos_core::domain::openai::ChatCompletionRequest,
    stream: bool,
) -> CohereChatRequest {
    let messages = req
        .messages
        .iter()
        .map(|m| {
            let role = match m.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool | MessageRole::Function => "tool",
            };

            // Les messages assistant avec tool_calls sont encodés différemment
            let content = if m.role == MessageRole::Assistant {
                if let Some(tool_calls) = &m.tool_calls {
                    // Cohere v2 attend un tableau de tool_call objects pour l'assistant
                    let tc_arr: Vec<serde_json::Value> = tool_calls
                        .iter()
                        .map(|tc| {
                            serde_json::json!({
                                "id": tc.id,
                                "type": "function",
                                "function": {
                                    "name": tc.function.name,
                                    "arguments": tc.function.arguments
                                }
                            })
                        })
                        .collect();
                    serde_json::json!(tc_arr)
                } else {
                    serde_json::json!(m.content.clone().unwrap_or_default())
                }
            } else if m.role == MessageRole::Tool || m.role == MessageRole::Function {
                // Messages tool_result : Cohere attend un array de tool_result
                serde_json::json!([{
                    "type": "tool_result",
                    "tool_use_id": m.tool_call_id.clone().unwrap_or_default(),
                    "content": m.content.clone().unwrap_or_default()
                }])
            } else {
                serde_json::json!(m.content.clone().unwrap_or_default())
            };

            CohereMessage {
                role: role.to_string(),
                content,
            }
        })
        .collect();

    let stop_sequences = match &req.stop {
        Some(pylos_core::domain::openai::StopCondition::Single(s)) => Some(vec![s.clone()]),
        Some(pylos_core::domain::openai::StopCondition::Multiple(v)) => Some(v.clone()),
        None => None,
    };

    // Conversion des tools OpenAI → Cohere (format identique function calling)
    let tools = req.tools.as_ref().map(|ts| {
        ts.iter()
            .map(|t| CohereTool {
                tool_type: "function".to_string(),
                function: CohereToolFunction {
                    name: t.function.name.clone(),
                    description: t.function.description.clone(),
                    parameters: t.function.parameters.clone(),
                },
            })
            .collect::<Vec<_>>()
    });

    // Conversion du tool_choice
    let tool_choice = req.tool_choice.as_ref().and_then(|tc| {
        use pylos_core::domain::openai::ToolChoice;
        match tc {
            ToolChoice::Mode(m) => match m.as_str() {
                "none" => Some("NONE".to_string()),
                "required" => Some("REQUIRED".to_string()),
                _ => None, // "auto" = défaut Cohere, on n'envoie pas le champ
            },
            ToolChoice::Specific { .. } => Some("REQUIRED".to_string()),
        }
    });

    CohereChatRequest {
        model: req.model.clone(),
        messages,
        temperature: req.temperature,
        max_tokens: req.max_tokens,
        p: req.top_p,
        stop_sequences,
        frequency_penalty: req.frequency_penalty,
        presence_penalty: req.presence_penalty,
        stream,
        tools,
        tool_choice,
    }
}

pub(crate) fn from_cohere_response(resp: CohereChatResponse) -> PylosResponse {
    let text = resp
        .message
        .as_ref()
        .and_then(|m| m.content.as_ref())
        .and_then(|blocks| {
            blocks
                .iter()
                .filter(|b| b.block_type.as_deref() == Some("text"))
                .filter_map(|b| b.text.as_ref())
                .cloned()
                .reduce(|a, b| a + &b)
        })
        .unwrap_or_default();

    // Extraction des tool_calls depuis la réponse
    let tool_calls: Vec<ToolCall> = resp
        .message
        .as_ref()
        .and_then(|m| m.tool_calls.as_ref())
        .map(|tcs| {
            tcs.iter()
                .enumerate()
                .map(|(idx, tc)| ToolCall {
                    id: tc.id.clone().unwrap_or_else(|| format!("call_{}", idx)),
                    call_type: "function".into(),
                    function: ToolCallFunction {
                        name: tc
                            .function
                            .as_ref()
                            .and_then(|f| f.name.clone())
                            .unwrap_or_default(),
                        arguments: tc
                            .function
                            .as_ref()
                            .and_then(|f| f.arguments.clone())
                            .unwrap_or_else(|| "{}".to_string()),
                    },
                    index: Some(idx as i32),
                })
                .collect()
        })
        .unwrap_or_default();

    let finish_reason = resp.finish_reason.as_deref().map(map_cohere_finish_reason);
    let usage = resp
        .usage
        .as_ref()
        .and_then(|u| u.billed_units.as_ref().or(u.tokens.as_ref()))
        .map(|t| Usage {
            prompt_tokens: t.input_tokens.unwrap_or(0),
            completion_tokens: t.output_tokens.unwrap_or(0),
            total_tokens: t.input_tokens.unwrap_or(0) + t.output_tokens.unwrap_or(0),
            ..Default::default()
        });

    let id = resp
        .id
        .unwrap_or_else(|| format!("cohere-{}", fastrand::u64(..)));

    let content = if text.is_empty() && !tool_calls.is_empty() {
        None
    } else {
        Some(text)
    };

    PylosResponse::ChatCompletion(ChatCompletionResponse {
        id,
        object: "chat.completion".to_string(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        model: String::new(),
        choices: vec![ChatCompletionChoice {
            index: 0,
            message: ChatCompletionMessage {
                role: MessageRole::Assistant,
                content,
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
                ..Default::default()
            },
            finish_reason,
        }],
        usage,
    })
}

/// Convertit un event SSE Cohere en StreamChunk (retourne None si à ignorer)
pub(crate) fn from_cohere_stream_event(
    event: CohereStreamEvent,
    model: &str,
) -> Option<StreamChunk> {
    let event_type = event.event_type.as_deref()?;

    match event_type {
        "content-delta" => {
            let text = event
                .delta
                .as_ref()
                .and_then(|d| d.message.as_ref())
                .and_then(|m| m.content.as_ref())
                .and_then(|c| c.text.clone());

            Some(StreamChunk {
                id: format!("cohere-{}", fastrand::u64(..)),
                object: "chat.completion.chunk".to_string(),
                created: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                model: model.to_string(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta {
                        content: text,
                        ..Default::default()
                    },
                    finish_reason: None,
                }],
                usage: None,
            })
        }
        "tool-call-start" | "tool-call-delta" => {
            // Cohere émet les tool calls en streaming via ces events
            let tool_calls = event
                .delta
                .as_ref()
                .and_then(|d| d.message.as_ref())
                .and_then(|m| m.tool_calls.as_ref())
                .map(|tcs| {
                    tcs.iter()
                        .enumerate()
                        .map(|(idx, tc)| StreamToolCallChunk {
                            index: idx as i32,
                            id: tc.id.clone(),
                            call_type: tc.call_type.clone(),
                            function: tc.function.as_ref().map(|f| StreamToolCallFunction {
                                name: f.name.clone(),
                                arguments: f.arguments.clone(),
                            }),
                        })
                        .collect::<Vec<_>>()
                });

            tool_calls.map(|tcs| StreamChunk {
                id: format!("cohere-{}", fastrand::u64(..)),
                object: "chat.completion.chunk".to_string(),
                created: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                model: model.to_string(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta {
                        tool_calls: Some(tcs),
                        ..Default::default()
                    },
                    finish_reason: None,
                }],
                usage: None,
            })
        }
        "message-end" => {
            let finish_reason = event
                .delta
                .as_ref()
                .and_then(|d| d.finish_reason.as_deref())
                .map(map_cohere_finish_reason);

            Some(StreamChunk {
                id: format!("cohere-{}", fastrand::u64(..)),
                object: "chat.completion.chunk".to_string(),
                created: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                model: model.to_string(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta::default(),
                    finish_reason,
                }],
                usage: None,
            })
        }
        _ => None,
    }
}

fn map_cohere_finish_reason(reason: &str) -> String {
    match reason {
        "COMPLETE" | "STOP_SEQUENCE" => "stop",
        "MAX_TOKENS" => "length",
        "TOOL_CALL" => "tool_calls",
        other => other,
    }
    .to_string()
}

pub(crate) fn to_cohere_embed_request(
    req: &pylos_core::domain::embedding::EmbeddingRequest,
) -> CohereEmbedRequest {
    let texts = req
        .input
        .as_strings()
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    CohereEmbedRequest {
        model: req.model.clone(),
        texts,
        input_type: "search_document".to_string(),
        output_dimension: req.dimensions,
        embedding_types: vec!["float".to_string()],
    }
}

pub(crate) fn from_cohere_embed_response(
    resp: CohereEmbedResponse,
    model: &str,
) -> EmbeddingResponse {
    let floats = resp.embeddings.and_then(|e| e.float).unwrap_or_default();

    let total_tokens = resp
        .meta
        .and_then(|m| m.billed_units.or(m.tokens))
        .and_then(|t| t.input_tokens)
        .unwrap_or(0);

    EmbeddingResponse {
        object: "list".to_string(),
        data: floats
            .into_iter()
            .enumerate()
            .map(|(i, vec)| EmbeddingData {
                index: i,
                object: "embedding".to_string(),
                embedding: vec,
            })
            .collect(),
        model: model.to_string(),
        usage: EmbeddingUsage {
            prompt_tokens: total_tokens,
            total_tokens,
        },
    }
}

pub(crate) fn map_cohere_error(status: u16, body: &str) -> pylos_core::error::PylosError {
    use pylos_core::error::PylosError;
    #[derive(serde::Deserialize)]
    struct CohereErr {
        message: Option<String>,
    }
    let message = serde_json::from_str::<CohereErr>(body)
        .ok()
        .and_then(|e| e.message)
        .unwrap_or_else(|| body.to_string());

    match status {
        401 | 403 => PylosError::Unauthorized(message),
        429 => PylosError::RateLimitExceeded(message),
        408 | 504 => PylosError::Timeout(message),
        _ => PylosError::ProviderError {
            provider: "cohere".into(),
            message,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finish_reason_mapping() {
        assert_eq!(map_cohere_finish_reason("COMPLETE"), "stop");
        assert_eq!(map_cohere_finish_reason("MAX_TOKENS"), "length");
        assert_eq!(map_cohere_finish_reason("TOOL_CALL"), "tool_calls");
    }

    #[test]
    fn test_to_cohere_request_roles() {
        use pylos_core::domain::openai::{
            ChatCompletionMessage, ChatCompletionRequest, MessageRole,
        };
        let req = ChatCompletionRequest {
            model: "command-a-03-2025".to_string(),
            messages: vec![
                ChatCompletionMessage {
                    role: MessageRole::System,
                    content: Some("Be helpful".to_string()),
                    ..Default::default()
                },
                ChatCompletionMessage {
                    role: MessageRole::User,
                    content: Some("Hello".to_string()),
                    ..Default::default()
                },
            ],
            stream: Some(false),
            temperature: None,
            top_p: None,
            n: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            user: None,
            session_id: None,
            stop: None,
            logit_bias: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            top_k: None,
            min_p: None,
            repetition_penalty: None,
            max_completion_tokens: None,
            account: None,
        };
        let cohere_req = to_cohere_request(&req, false);
        assert_eq!(cohere_req.messages[0].role, "system");
        assert_eq!(cohere_req.messages[1].role, "user");
        assert!(!cohere_req.stream);
    }
}
