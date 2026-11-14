use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionResponse, MessageRole, ToolCall,
    ToolCallFunction, Usage,
};
use pylos_core::domain::request::{PylosResponse, StreamChoice, StreamChunk, StreamDelta};
use pylos_core::error::PylosError;
use serde::{Deserialize, Serialize};

// ──────────────────────────────────────────────────────────────────────────────
// Format natif de l'API Anthropic Messages
// Ref : https://docs.anthropic.com/en/api/messages
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct AnthropicRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessage>,
    pub max_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    /// Liste d'outils exposés au modèle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<AnthropicTool>>,
    /// Contrôle quel outil est appelé : "auto" | "any" | {"type":"tool","name":"..."}
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    /// Extended thinking traduit depuis `reasoning_effort` (#210, #211)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<AnthropicThinkingConfig>,
}

#[derive(Debug, Serialize)]
pub(crate) struct AnthropicThinkingConfig {
    #[serde(rename = "type")]
    pub thinking_type: &'static str,
    pub budget_tokens: i32,
}

/// Message Anthropic — le contenu peut être texte ou une liste de blocs
#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct AnthropicMessage {
    pub role: String,
    pub content: AnthropicMessageContent,
}

/// Contenu d'un message Anthropic : texte simple ou liste de blocs
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum AnthropicMessageContent {
    Text(String),
    Blocks(Vec<AnthropicContentBlock>),
}

/// Bloc de contenu dans un message Anthropic
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum AnthropicContentBlock {
    Text {
        text: String,
    },
    /// Résultat d'appel d'outil (role=user)
    ToolResult {
        tool_use_id: String,
        content: String,
    },
    /// Appel d'outil dans un message assistant (émis par le modèle)
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
}

// ── Définition d'outil Anthropic ──────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct AnthropicTool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
}

// ──────────────────────────────────────────────────────────────────────────────
// Structures de réponse Anthropic
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicResponse {
    pub id: String,
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub response_type: String,
    #[allow(dead_code)]
    pub role: String,
    pub content: Vec<AnthropicResponseContent>,
    #[allow(dead_code)]
    pub model: String,
    pub stop_reason: Option<String>,
    pub usage: Option<AnthropicUsage>,
}

/// Bloc de contenu dans une réponse Anthropic
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum AnthropicResponseContent {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicUsage {
    pub input_tokens: i32,
    pub output_tokens: i32,
}

// ──────────────────────────────────────────────────────────────────────────────
// Structures de streaming Anthropic (SSE events)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicStreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: Option<i32>,
    pub delta: Option<AnthropicStreamDelta>,
    pub message: Option<AnthropicStreamMessage>,
    #[allow(dead_code)]
    pub usage: Option<AnthropicUsage>,
    /// Présent dans content_block_start pour les blocs tool_use
    pub content_block: Option<AnthropicStreamContentBlock>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicStreamDelta {
    #[serde(rename = "type")]
    pub delta_type: Option<String>,
    /// Texte (content_block_delta de type text_delta)
    pub text: Option<String>,
    /// JSON partiel pour tool_use (input_json_delta)
    pub partial_json: Option<String>,
    pub stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicStreamMessage {
    pub id: String,
    pub model: String,
}

/// Bloc annoncé dans content_block_start
#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicStreamContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    /// Présent si block_type = "tool_use"
    pub id: Option<String>,
    pub name: Option<String>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Contexte de streaming (pour accumuler les métadonnées du message)
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub(crate) struct StreamContext {
    pub message_id: String,
    pub model: String,
    pub created: i64,
    /// Accumulation JSON d'un outil en cours de streaming
    pub current_tool_id: Option<String>,
    pub current_tool_name: Option<String>,
    pub current_tool_json: String,
    /// Index du bloc courant
    pub current_block_index: i32,
}

// ──────────────────────────────────────────────────────────────────────────────
// Conversions PylosRequest → Anthropic natif
// ──────────────────────────────────────────────────────────────────────────────

pub(crate) fn to_anthropic_request(
    req: &pylos_core::domain::openai::ChatCompletionRequest,
    stream: bool,
) -> AnthropicRequest {
    // Anthropic sépare les messages system des messages user/assistant
    let mut system_parts: Vec<String> = Vec::new();
    let mut messages: Vec<AnthropicMessage> = Vec::new();

    for msg in &req.messages {
        match msg.role {
            MessageRole::System => {
                system_parts.push(msg.content.clone().unwrap_or_default());
            }
            MessageRole::User => {
                messages.push(AnthropicMessage {
                    role: "user".into(),
                    content: AnthropicMessageContent::Text(msg.content.clone().unwrap_or_default()),
                });
            }
            MessageRole::Assistant => {
                // Si l'assistant a émis des tool_calls, on les encode comme blocs
                if let Some(tool_calls) = &msg.tool_calls {
                    let mut blocks: Vec<AnthropicContentBlock> = Vec::new();
                    // Texte éventuel avant les tool calls
                    if let Some(text) = &msg.content {
                        if !text.is_empty() {
                            blocks.push(AnthropicContentBlock::Text { text: text.clone() });
                        }
                    }
                    for tc in tool_calls {
                        let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                        blocks.push(AnthropicContentBlock::ToolUse {
                            id: tc.id.clone(),
                            name: tc.function.name.clone(),
                            input,
                        });
                    }
                    messages.push(AnthropicMessage {
                        role: "assistant".into(),
                        content: AnthropicMessageContent::Blocks(blocks),
                    });
                } else {
                    messages.push(AnthropicMessage {
                        role: "assistant".into(),
                        content: AnthropicMessageContent::Text(
                            msg.content.clone().unwrap_or_default(),
                        ),
                    });
                }
            }
            MessageRole::Tool | MessageRole::Function => {
                // Résultat d'appel d'outil → bloc tool_result côté user
                let tool_use_id = msg.tool_call_id.clone().unwrap_or_default();
                let content_text = msg.content.clone().unwrap_or_default();
                messages.push(AnthropicMessage {
                    role: "user".into(),
                    content: AnthropicMessageContent::Blocks(vec![
                        AnthropicContentBlock::ToolResult {
                            tool_use_id,
                            content: content_text,
                        },
                    ]),
                });
            }
        }
    }

    // Anthropic exige au moins un message et que le premier soit "user"
    if messages.is_empty() {
        messages.push(AnthropicMessage {
            role: "user".into(),
            content: AnthropicMessageContent::Text(String::new()),
        });
    }

    let system = if system_parts.is_empty() {
        None
    } else {
        Some(system_parts.join("\n\n"))
    };

    let max_tokens = req.max_tokens.unwrap_or(4096);

    let stop_sequences = match &req.stop {
        Some(pylos_core::domain::openai::StopCondition::Single(s)) => Some(vec![s.clone()]),
        Some(pylos_core::domain::openai::StopCondition::Multiple(arr)) => Some(arr.clone()),
        None => None,
    };

    // Conversion des tools OpenAI → Anthropic
    let tools =
        req.tools.as_ref().map(|ts| {
            ts.iter()
                .map(|t| AnthropicTool {
                    name: t.function.name.clone(),
                    description: t.function.description.clone(),
                    input_schema: t.function.parameters.clone().unwrap_or_else(
                        || serde_json::json!({ "type": "object", "properties": {} }),
                    ),
                })
                .collect::<Vec<_>>()
        });

    // Conversion du tool_choice OpenAI → Anthropic
    let tool_choice = req.tool_choice.as_ref().map(|tc| {
        use pylos_core::domain::openai::ToolChoice;
        match tc {
            ToolChoice::Mode(mode) => match mode.as_str() {
                "none" => serde_json::json!({ "type": "none" }),
                "required" => serde_json::json!({ "type": "any" }),
                _ => serde_json::json!({ "type": "auto" }), // "auto"
            },
            ToolChoice::Specific { function, .. } => {
                serde_json::json!({ "type": "tool", "name": function.name })
            }
        }
    });

    let thinking = match req.reasoning_effort.as_deref() {
        Some("medium") => Some(AnthropicThinkingConfig {
            thinking_type: "enabled",
            budget_tokens: 4096,
        }),
        Some("high") => Some(AnthropicThinkingConfig {
            thinking_type: "enabled",
            budget_tokens: 16384,
        }),
        _ => None,
    };
    let max_tokens = match &thinking {
        Some(t) if max_tokens <= t.budget_tokens => t.budget_tokens + 2048,
        _ => max_tokens,
    };

    AnthropicRequest {
        model: req.model.clone(),
        messages,
        max_tokens,
        system,
        temperature: if thinking.is_some() { None } else { req.temperature },
        top_p: req.top_p,
        stream: if stream { Some(true) } else { None },
        stop_sequences,
        tools,
        tool_choice,
        thinking,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Conversions Anthropic natif → Pylos domain
// ──────────────────────────────────────────────────────────────────────────────

pub(crate) fn from_anthropic_response(
    resp: AnthropicResponse,
    requested_model: &str,
) -> PylosResponse {
    // Sépare les blocs texte des blocs tool_use
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for (idx, block) in resp.content.into_iter().enumerate() {
        match block {
            AnthropicResponseContent::Text { text } => {
                text_parts.push(text);
            }
            AnthropicResponseContent::ToolUse { id, name, input } => {
                tool_calls.push(ToolCall {
                    id,
                    call_type: "function".into(),
                    function: ToolCallFunction {
                        name,
                        arguments: input.to_string(),
                    },
                    index: Some(idx as i32),
                });
            }
            AnthropicResponseContent::Unknown => {}
        }
    }

    let text = text_parts.join("");
    let content = if text.is_empty() && !tool_calls.is_empty() {
        None
    } else {
        Some(text)
    };

    let finish_reason = resp
        .stop_reason
        .as_deref()
        .map(|r| match r {
            "end_turn" => "stop",
            "max_tokens" => "length",
            "stop_sequence" => "stop",
            "tool_use" => "tool_calls",
            other => other,
        })
        .map(String::from);

    PylosResponse::ChatCompletion(ChatCompletionResponse {
        id: resp.id,
        object: "chat.completion".into(),
        created: chrono_now(),
        model: requested_model.to_string(),
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
        usage: resp.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.input_tokens + u.output_tokens,
            ..Default::default()
        }),
    })
}

pub(crate) fn from_anthropic_stream_event(
    event: AnthropicStreamEvent,
    ctx: &mut StreamContext,
) -> Option<StreamChunk> {
    use pylos_core::domain::request::{StreamToolCallChunk, StreamToolCallFunction};

    match event.event_type.as_str() {
        // Annonce d'un nouveau bloc — on retient les métadonnées si tool_use
        "content_block_start" => {
            ctx.current_block_index = event.index.unwrap_or(0);
            if let Some(block) = &event.content_block {
                if block.block_type == "tool_use" {
                    ctx.current_tool_id = block.id.clone();
                    ctx.current_tool_name = block.name.clone();
                    ctx.current_tool_json.clear();
                    // Émet le premier chunk avec l'id + nom de l'outil
                    return Some(StreamChunk {
                        id: ctx.message_id.clone(),
                        object: "chat.completion.chunk".into(),
                        created: ctx.created,
                        model: ctx.model.clone(),
                        choices: vec![StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                tool_calls: Some(vec![StreamToolCallChunk {
                                    index: ctx.current_block_index,
                                    id: ctx.current_tool_id.clone(),
                                    call_type: Some("function".into()),
                                    function: Some(StreamToolCallFunction {
                                        name: ctx.current_tool_name.clone(),
                                        arguments: None,
                                    }),
                                }]),
                                ..Default::default()
                            },
                            finish_reason: None,
                        }],
                        usage: None,
                    });
                }
            }
            None
        }

        "content_block_delta" => {
            let delta = event.delta.as_ref()?;
            match delta.delta_type.as_deref() {
                Some("text_delta") => {
                    let content = delta.text.clone();
                    Some(StreamChunk {
                        id: ctx.message_id.clone(),
                        object: "chat.completion.chunk".into(),
                        created: ctx.created,
                        model: ctx.model.clone(),
                        choices: vec![StreamChoice {
                            index: event.index.unwrap_or(0),
                            delta: StreamDelta {
                                content,
                                ..Default::default()
                            },
                            finish_reason: None,
                        }],
                        usage: None,
                    })
                }
                Some("input_json_delta") => {
                    // Fragments JSON des arguments de l'outil
                    let partial = delta.partial_json.clone().unwrap_or_default();
                    ctx.current_tool_json.push_str(&partial);
                    Some(StreamChunk {
                        id: ctx.message_id.clone(),
                        object: "chat.completion.chunk".into(),
                        created: ctx.created,
                        model: ctx.model.clone(),
                        choices: vec![StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                tool_calls: Some(vec![StreamToolCallChunk {
                                    index: ctx.current_block_index,
                                    id: None,
                                    call_type: None,
                                    function: Some(StreamToolCallFunction {
                                        name: None,
                                        arguments: Some(partial),
                                    }),
                                }]),
                                ..Default::default()
                            },
                            finish_reason: None,
                        }],
                        usage: None,
                    })
                }
                _ => None,
            }
        }

        "message_delta" => {
            let finish_reason = event
                .delta
                .as_ref()
                .and_then(|d| d.stop_reason.as_deref())
                .map(|r| match r {
                    "end_turn" => "stop",
                    "max_tokens" => "length",
                    "tool_use" => "tool_calls",
                    other => other,
                })
                .map(String::from);

            Some(StreamChunk {
                id: ctx.message_id.clone(),
                object: "chat.completion.chunk".into(),
                created: ctx.created,
                model: ctx.model.clone(),
                choices: vec![StreamChoice {
                    index: 0,
                    delta: StreamDelta::default(),
                    finish_reason,
                }],
                usage: None,
            })
        }
        _ => None, // message_start, ping, content_block_stop, etc.
    }
}

/// Erreurs Anthropic
#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicErrorBody {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub error_type: String,
    pub error: AnthropicErrorDetail,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicErrorDetail {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    pub error_type: String,
    pub message: String,
}

pub(crate) fn map_anthropic_error(status: u16, body: &str) -> PylosError {
    let message = serde_json::from_str::<AnthropicErrorBody>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| body.to_string());

    match status {
        401 => PylosError::Unauthorized(message),
        429 => PylosError::RateLimitExceeded(message),
        408 | 504 => PylosError::Timeout(message),
        _ => PylosError::ProviderError {
            provider: "anthropic".into(),
            message,
        },
    }
}

fn chrono_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
