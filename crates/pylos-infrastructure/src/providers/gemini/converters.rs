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
// Structures de requête Gemini
// Bifrost source: core/providers/gemini/types.go
// Doc: https://ai.google.dev/api/generate-content
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct GeminiRequest {
    pub contents: Vec<GeminiContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_instruction: Option<GeminiSystemInstruction>,
    #[serde(rename = "generationConfig", skip_serializing_if = "Option::is_none")]
    pub generation_config: Option<GeminiGenerationConfig>,
    /// Outils exposés au modèle
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<GeminiTool>>,
    #[serde(rename = "toolConfig", skip_serializing_if = "Option::is_none")]
    pub tool_config: Option<GeminiToolConfig>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct GeminiContent {
    pub role: String, // "user" | "model"
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct GeminiPart {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Appel d'outil émis par le modèle
    #[serde(rename = "functionCall", skip_serializing_if = "Option::is_none")]
    pub function_call: Option<GeminiFunctionCall>,
    /// Résultat d'appel d'outil fourni par l'utilisateur
    #[serde(rename = "functionResponse", skip_serializing_if = "Option::is_none")]
    pub function_response: Option<GeminiFunctionResponse>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct GeminiFunctionCall {
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub(crate) struct GeminiFunctionResponse {
    pub name: String,
    pub response: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiSystemInstruction {
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiGenerationConfig {
    #[serde(rename = "maxOutputTokens", skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(rename = "topP", skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(rename = "stopSequences", skip_serializing_if = "Option::is_none")]
    pub stop_sequences: Option<Vec<String>>,
    #[serde(rename = "thinkingConfig", skip_serializing_if = "Option::is_none")]
    pub thinking_config: Option<GeminiThinkingConfig>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiThinkingConfig {
    #[serde(rename = "thinkingBudget")]
    pub thinking_budget: i32,
}

// ── Définitions d'outils Gemini ───────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct GeminiTool {
    #[serde(rename = "functionDeclarations")]
    pub function_declarations: Vec<GeminiFunctionDeclaration>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiFunctionDeclaration {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiToolConfig {
    #[serde(rename = "functionCallingConfig")]
    pub function_calling_config: GeminiFunctionCallingConfig,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiFunctionCallingConfig {
    pub mode: String, // "AUTO" | "ANY" | "NONE"
    #[serde(
        rename = "allowedFunctionNames",
        skip_serializing_if = "Option::is_none"
    )]
    pub allowed_function_names: Option<Vec<String>>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Structures de réponse Gemini
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiResponse {
    pub candidates: Option<Vec<GeminiCandidate>>,
    #[serde(rename = "usageMetadata")]
    pub usage_metadata: Option<GeminiUsageMetadata>,
    #[serde(rename = "modelVersion")]
    pub model_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiCandidate {
    pub content: Option<GeminiContent>,
    #[serde(rename = "finishReason")]
    pub finish_reason: Option<String>,
    #[allow(dead_code)]
    pub index: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiUsageMetadata {
    #[serde(rename = "promptTokenCount")]
    pub prompt_token_count: Option<i32>,
    #[serde(rename = "candidatesTokenCount")]
    pub candidates_token_count: Option<i32>,
    #[serde(rename = "totalTokenCount")]
    pub total_token_count: Option<i32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Structures embeddings Gemini
// POST /models/{model}:batchEmbedContents
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub(crate) struct GeminiBatchEmbedRequest {
    pub requests: Vec<GeminiEmbedRequest>,
}

#[derive(Debug, Serialize)]
pub(crate) struct GeminiEmbedRequest {
    pub model: String,
    pub content: GeminiContent,
    #[serde(
        rename = "outputDimensionality",
        skip_serializing_if = "Option::is_none"
    )]
    pub output_dimensionality: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiBatchEmbedResponse {
    pub embeddings: Vec<GeminiEmbedding>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiEmbedding {
    pub values: Vec<f32>,
    pub statistics: Option<GeminiEmbedStats>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct GeminiEmbedStats {
    #[serde(rename = "tokenCount")]
    pub token_count: Option<i32>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversions
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn to_gemini_request(
    req: &pylos_core::domain::openai::ChatCompletionRequest,
) -> GeminiRequest {
    let mut contents: Vec<GeminiContent> = Vec::new();
    let mut system_parts: Vec<GeminiPart> = Vec::new();

    for msg in &req.messages {
        match msg.role {
            MessageRole::System => {
                system_parts.push(GeminiPart {
                    text: msg.content.clone(),
                    function_call: None,
                    function_response: None,
                });
            }
            MessageRole::User => {
                contents.push(GeminiContent {
                    role: "user".to_string(),
                    parts: vec![GeminiPart {
                        text: msg.content.clone(),
                        function_call: None,
                        function_response: None,
                    }],
                });
            }
            MessageRole::Assistant => {
                // Si l'assistant a émis des tool_calls, on les encode comme functionCall parts
                if let Some(tool_calls) = &msg.tool_calls {
                    let parts: Vec<GeminiPart> = tool_calls
                        .iter()
                        .map(|tc| {
                            let args: serde_json::Value =
                                serde_json::from_str(&tc.function.arguments)
                                    .unwrap_or(serde_json::Value::Object(Default::default()));
                            GeminiPart {
                                text: None,
                                function_call: Some(GeminiFunctionCall {
                                    name: tc.function.name.clone(),
                                    args,
                                }),
                                function_response: None,
                            }
                        })
                        .collect();
                    contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts,
                    });
                } else {
                    contents.push(GeminiContent {
                        role: "model".to_string(),
                        parts: vec![GeminiPart {
                            text: msg.content.clone(),
                            function_call: None,
                            function_response: None,
                        }],
                    });
                }
            }
            MessageRole::Tool | MessageRole::Function => {
                // Résultat d'outil → functionResponse part côté user
                let name = msg.name.clone().unwrap_or_default();
                let response_val: serde_json::Value = msg
                    .content
                    .as_deref()
                    .and_then(|c| serde_json::from_str(c).ok())
                    .unwrap_or_else(
                        || serde_json::json!({ "output": msg.content.clone().unwrap_or_default() }),
                    );
                contents.push(GeminiContent {
                    role: "user".to_string(),
                    parts: vec![GeminiPart {
                        text: None,
                        function_call: None,
                        function_response: Some(GeminiFunctionResponse {
                            name,
                            response: response_val,
                        }),
                    }],
                });
            }
        }
    }

    let system_instruction = if system_parts.is_empty() {
        None
    } else {
        Some(GeminiSystemInstruction {
            parts: system_parts,
        })
    };

    let thinking_config = req.reasoning_effort.as_deref().map(|effort| {
        let thinking_budget = match effort {
            "high" => 16384,
            "medium" => 4096,
            _ => 512,
        };
        GeminiThinkingConfig { thinking_budget }
    });

    let generation_config = Some(GeminiGenerationConfig {
        max_output_tokens: req.max_tokens,
        temperature: req.temperature,
        top_p: req.top_p,
        stop_sequences: match &req.stop {
            Some(pylos_core::domain::openai::StopCondition::Single(s)) => Some(vec![s.clone()]),
            Some(pylos_core::domain::openai::StopCondition::Multiple(v)) => Some(v.clone()),
            None => None,
        },
        thinking_config,
    });

    // Conversion des tools OpenAI → Gemini functionDeclarations
    let tools = req.tools.as_ref().map(|ts| {
        vec![GeminiTool {
            function_declarations: ts
                .iter()
                .map(|t| GeminiFunctionDeclaration {
                    name: t.function.name.clone(),
                    description: t.function.description.clone(),
                    parameters: t.function.parameters.clone(),
                })
                .collect(),
        }]
    });

    // Conversion du tool_choice OpenAI → Gemini toolConfig
    let tool_config = req.tool_choice.as_ref().map(|tc| {
        use pylos_core::domain::openai::ToolChoice;
        let (mode, allowed) = match tc {
            ToolChoice::Mode(m) => match m.as_str() {
                "none" => ("NONE", None),
                "required" => ("ANY", None),
                _ => ("AUTO", None),
            },
            ToolChoice::Specific { function, .. } => ("ANY", Some(vec![function.name.clone()])),
        };
        GeminiToolConfig {
            function_calling_config: GeminiFunctionCallingConfig {
                mode: mode.to_string(),
                allowed_function_names: allowed,
            },
        }
    });

    GeminiRequest {
        contents,
        system_instruction,
        generation_config,
        tools,
        tool_config,
    }
}

pub(crate) fn from_gemini_response(resp: GeminiResponse, model: &str) -> PylosResponse {
    let candidate = resp.candidates.and_then(|mut v| {
        if v.is_empty() {
            None
        } else {
            Some(v.remove(0))
        }
    });

    let finish_reason = candidate
        .as_ref()
        .and_then(|c| c.finish_reason.as_deref())
        .map(map_gemini_finish_reason)
        .unwrap_or_else(|| "stop".to_string());

    // Extraire texte et tool calls depuis les parts
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    if let Some(c) = &candidate {
        if let Some(content) = &c.content {
            for (idx, part) in content.parts.iter().enumerate() {
                if let Some(text) = &part.text {
                    text_parts.push(text.clone());
                }
                if let Some(fc) = &part.function_call {
                    tool_calls.push(ToolCall {
                        id: format!("call_{}", fastrand::u64(..)),
                        call_type: "function".into(),
                        function: ToolCallFunction {
                            name: fc.name.clone(),
                            arguments: fc.args.to_string(),
                        },
                        index: Some(idx as i32),
                    });
                }
            }
        }
    }

    let content_text = text_parts.join("");
    let content = if content_text.is_empty() && !tool_calls.is_empty() {
        None
    } else {
        Some(content_text)
    };

    let usage = resp.usage_metadata.map(|u| Usage {
        prompt_tokens: u.prompt_token_count.unwrap_or(0),
        completion_tokens: u.candidates_token_count.unwrap_or(0),
        total_tokens: u.total_token_count.unwrap_or(0),
        ..Default::default()
    });

    let id = format!("gemini-{}", fastrand::u64(..));
    let model_used = resp.model_version.unwrap_or_else(|| model.to_string());

    PylosResponse::ChatCompletion(ChatCompletionResponse {
        id,
        object: "chat.completion".to_string(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        model: model_used,
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
            finish_reason: Some(finish_reason),
        }],
        usage,
    })
}

pub(crate) fn from_gemini_stream_chunk(resp: GeminiResponse, model: &str) -> Option<StreamChunk> {
    let candidate = resp.candidates.and_then(|mut v| {
        if v.is_empty() {
            None
        } else {
            Some(v.remove(0))
        }
    })?;

    let finish_reason = candidate
        .finish_reason
        .as_deref()
        .map(map_gemini_finish_reason);

    // Extraire texte et function_calls des parts
    let mut text_parts: Vec<String> = Vec::new();
    let mut tool_call_chunks: Vec<StreamToolCallChunk> = Vec::new();

    if let Some(content) = &candidate.content {
        for (idx, part) in content.parts.iter().enumerate() {
            if let Some(text) = &part.text {
                text_parts.push(text.clone());
            }
            if let Some(fc) = &part.function_call {
                tool_call_chunks.push(StreamToolCallChunk {
                    index: idx as i32,
                    id: Some(format!("call_{}", fastrand::u64(..))),
                    call_type: Some("function".into()),
                    function: Some(StreamToolCallFunction {
                        name: Some(fc.name.clone()),
                        arguments: Some(fc.args.to_string()),
                    }),
                });
            }
        }
    }

    let content_text = text_parts.join("");
    let content = if content_text.is_empty() {
        None
    } else {
        Some(content_text)
    };

    Some(StreamChunk {
        id: format!("gemini-{}", fastrand::u64(..)),
        object: "chat.completion.chunk".to_string(),
        created: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64,
        model: model.to_string(),
        choices: vec![StreamChoice {
            index: 0,
            delta: StreamDelta {
                content,
                tool_calls: if tool_call_chunks.is_empty() {
                    None
                } else {
                    Some(tool_call_chunks)
                },
                ..Default::default()
            },
            finish_reason,
        }],
        usage: None,
    })
}

fn map_gemini_finish_reason(reason: &str) -> String {
    match reason {
        "STOP" => "stop",
        "MAX_TOKENS" => "length",
        "SAFETY" | "RECITATION" | "BLOCKLIST" | "PROHIBITED_CONTENT" | "SPII" => "content_filter",
        _ => "stop",
    }
    .to_string()
}

pub(crate) fn to_gemini_embed_request(
    model: &str,
    texts: Vec<&str>,
    dimensions: Option<u32>,
) -> GeminiBatchEmbedRequest {
    let model_path = if model.starts_with("models/") {
        model.to_string()
    } else {
        format!("models/{}", model)
    };

    GeminiBatchEmbedRequest {
        requests: texts
            .into_iter()
            .map(|t| GeminiEmbedRequest {
                model: model_path.clone(),
                content: GeminiContent {
                    role: "user".to_string(),
                    parts: vec![GeminiPart {
                        text: Some(t.to_string()),
                        function_call: None,
                        function_response: None,
                    }],
                },
                output_dimensionality: dimensions,
            })
            .collect(),
    }
}

pub(crate) fn from_gemini_embed_response(
    resp: GeminiBatchEmbedResponse,
    model: &str,
) -> EmbeddingResponse {
    let total_tokens: i32 = resp
        .embeddings
        .iter()
        .filter_map(|e| e.statistics.as_ref()?.token_count)
        .sum();

    EmbeddingResponse {
        object: "list".to_string(),
        data: resp
            .embeddings
            .into_iter()
            .enumerate()
            .map(|(i, e)| EmbeddingData {
                index: i,
                object: "embedding".to_string(),
                embedding: e.values,
            })
            .collect(),
        model: model.to_string(),
        usage: EmbeddingUsage {
            prompt_tokens: total_tokens,
            total_tokens,
        },
    }
}

pub(crate) fn map_gemini_error(status: u16, body: &str) -> pylos_core::error::PylosError {
    use pylos_core::error::PylosError;
    #[derive(serde::Deserialize)]
    struct GeminiErrorEnvelope {
        error: GeminiErrorDetail,
    }
    #[derive(serde::Deserialize)]
    struct GeminiErrorDetail {
        message: String,
    }

    let message = serde_json::from_str::<GeminiErrorEnvelope>(body)
        .map(|e| e.error.message)
        .unwrap_or_else(|_| body.to_string());

    match status {
        401 | 403 => PylosError::Unauthorized(message),
        429 => PylosError::RateLimitExceeded(message),
        408 | 504 => PylosError::Timeout(message),
        _ => PylosError::ProviderError {
            provider: "gemini".into(),
            message,
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};

    fn make_chat_req(msgs: Vec<(&str, &str)>) -> ChatCompletionRequest {
        ChatCompletionRequest {
            model: "gemini-2.0-flash".to_string(),
            messages: msgs
                .into_iter()
                .map(|(role, content)| {
                    let r = match role {
                        "system" => MessageRole::System,
                        "assistant" => MessageRole::Assistant,
                        _ => MessageRole::User,
                    };
                    ChatCompletionMessage {
                        role: r,
                        content: Some(content.to_string()),
                        ..Default::default()
                    }
                })
                .collect(),
            stream: None,
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
            reasoning_effort: None,
        }
    }

    #[test]
    fn test_system_extracted() {
        let req = make_chat_req(vec![("system", "Be helpful"), ("user", "hello")]);
        let gemini = to_gemini_request(&req);
        assert!(gemini.system_instruction.is_some());
        assert_eq!(gemini.contents.len(), 1);
        assert_eq!(gemini.contents[0].role, "user");
    }

    #[test]
    fn test_assistant_role_becomes_model() {
        let req = make_chat_req(vec![("user", "hi"), ("assistant", "hello")]);
        let gemini = to_gemini_request(&req);
        assert_eq!(gemini.contents[1].role, "model");
    }
    #[test]
    fn test_finish_reason_mapping() {
        assert_eq!(map_gemini_finish_reason("STOP"), "stop");
        assert_eq!(map_gemini_finish_reason("MAX_TOKENS"), "length");
        assert_eq!(map_gemini_finish_reason("SAFETY"), "content_filter");
    }
}
