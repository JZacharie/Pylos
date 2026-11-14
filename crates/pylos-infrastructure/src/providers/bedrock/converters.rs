use aws_sdk_bedrockruntime::types::{
    ContentBlock, ConversationRole, InferenceConfiguration, Message, SystemContentBlock, Tool,
    ToolConfiguration, ToolInputSchema, ToolResultBlock, ToolResultContentBlock, ToolSpecification,
    ToolUseBlock,
};
use aws_smithy_types::Document;

use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionResponse, MessageRole, ToolCall,
    ToolCallFunction, Usage,
};
use pylos_core::domain::request::{
    PylosResponse, StreamChoice, StreamChunk, StreamDelta, StreamToolCallChunk,
};
use pylos_core::error::PylosError;

// ─────────────────────────────────────────────────────────────────────────────
// Conversion PylosRequest → types SDK Bedrock Converse
// ─────────────────────────────────────────────────────────────────────────────

/// Convertit les messages OpenAI en messages Bedrock Converse
/// Identique à convertMessages() dans bifrost/core/providers/bedrock/utils.go
pub(crate) fn convert_messages(
    messages: &[pylos_core::domain::openai::ChatCompletionMessage],
) -> Result<(Vec<Message>, Vec<SystemContentBlock>), PylosError> {
    let mut bedrock_messages: Vec<Message> = Vec::new();
    let mut system_blocks: Vec<SystemContentBlock> = Vec::new();

    for msg in messages {
        match msg.role {
            // Les messages system sont extraits séparément (format Bedrock)
            MessageRole::System => {
                system_blocks.push(SystemContentBlock::Text(
                    msg.content.clone().unwrap_or_default(),
                ));
            }

            MessageRole::User => {
                let content = ContentBlock::Text(msg.content.clone().unwrap_or_default());
                bedrock_messages.push(
                    Message::builder()
                        .role(ConversationRole::User)
                        .content(content)
                        .build()
                        .map_err(|e| PylosError::Internal(e.to_string()))?,
                );
            }

            MessageRole::Assistant => {
                // Si l'assistant a émis des tool_calls, on les encode comme ToolUse blocks
                if let Some(tool_calls) = &msg.tool_calls {
                    let mut builder = Message::builder().role(ConversationRole::Assistant);
                    // Texte éventuel
                    if let Some(text) = &msg.content {
                        if !text.is_empty() {
                            builder = builder.content(ContentBlock::Text(text.clone()));
                        }
                    }
                    for tc in tool_calls {
                        let input: serde_json::Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Object(Default::default()));
                        let doc = json_to_bedrock_doc(&input);
                        let tool_use = ToolUseBlock::builder()
                            .tool_use_id(tc.id.clone())
                            .name(tc.function.name.clone())
                            .input(doc)
                            .build()
                            .map_err(|e| PylosError::Internal(e.to_string()))?;
                        builder = builder.content(ContentBlock::ToolUse(tool_use));
                    }
                    bedrock_messages.push(
                        builder
                            .build()
                            .map_err(|e| PylosError::Internal(e.to_string()))?,
                    );
                } else {
                    let content = ContentBlock::Text(msg.content.clone().unwrap_or_default());
                    bedrock_messages.push(
                        Message::builder()
                            .role(ConversationRole::Assistant)
                            .content(content)
                            .build()
                            .map_err(|e| PylosError::Internal(e.to_string()))?,
                    );
                }
            }

            // Tool / Function → traité comme user avec ToolResult
            MessageRole::Tool | MessageRole::Function => {
                let tool_use_id = msg
                    .tool_call_id
                    .clone()
                    .or_else(|| msg.name.clone())
                    .unwrap_or_default();
                let tool_result = ToolResultBlock::builder()
                    .tool_use_id(tool_use_id)
                    .content(ToolResultContentBlock::Text(
                        msg.content.clone().unwrap_or_default(),
                    ))
                    .build()
                    .map_err(|e| PylosError::Internal(e.to_string()))?;

                let content = ContentBlock::ToolResult(tool_result);
                bedrock_messages.push(
                    Message::builder()
                        .role(ConversationRole::User)
                        .content(content)
                        .build()
                        .map_err(|e| PylosError::Internal(e.to_string()))?,
                );
            }
        }
    }

    Ok((bedrock_messages, system_blocks))
}

/// Convertit les tools OpenAI en ToolConfiguration Bedrock
pub(crate) fn build_tool_config(
    tools: &[pylos_core::domain::openai::Tool],
) -> Result<ToolConfiguration, PylosError> {
    let mut bedrock_tools: Vec<Tool> = Vec::new();
    for t in tools {
        let schema_json = t
            .function
            .parameters
            .clone()
            .unwrap_or_else(|| serde_json::json!({ "type": "object", "properties": {} }));
        let schema_doc = json_to_bedrock_doc(&schema_json);
        let spec = ToolSpecification::builder()
            .name(t.function.name.clone())
            .set_description(t.function.description.clone())
            .input_schema(ToolInputSchema::Json(schema_doc))
            .build()
            .map_err(|e| PylosError::Internal(e.to_string()))?;
        bedrock_tools.push(Tool::ToolSpec(spec));
    }

    ToolConfiguration::builder()
        .set_tools(Some(bedrock_tools))
        .build()
        .map_err(|e| PylosError::Internal(e.to_string()))
}

/// Convertit une serde_json::Value en aws_smithy_types::Document
fn json_to_bedrock_doc(val: &serde_json::Value) -> Document {
    match val {
        serde_json::Value::Null => Document::Null,
        serde_json::Value::Bool(b) => Document::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Document::Number(aws_smithy_types::Number::NegInt(i))
            } else if let Some(u) = n.as_u64() {
                Document::Number(aws_smithy_types::Number::PosInt(u))
            } else {
                Document::Number(aws_smithy_types::Number::Float(n.as_f64().unwrap_or(0.0)))
            }
        }
        serde_json::Value::String(s) => Document::String(s.clone()),
        serde_json::Value::Array(arr) => {
            Document::Array(arr.iter().map(json_to_bedrock_doc).collect())
        }
        serde_json::Value::Object(obj) => Document::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), json_to_bedrock_doc(v)))
                .collect(),
        ),
    }
}

/// Convertit un aws_smithy_types::Document en serde_json::Value
fn bedrock_doc_to_json(doc: &Document) -> serde_json::Value {
    match doc {
        Document::Null => serde_json::Value::Null,
        Document::Bool(b) => serde_json::Value::Bool(*b),
        Document::Number(n) => match n {
            aws_smithy_types::Number::NegInt(i) => serde_json::json!(i),
            aws_smithy_types::Number::PosInt(u) => serde_json::json!(u),
            aws_smithy_types::Number::Float(f) => serde_json::json!(f),
        },
        Document::String(s) => serde_json::Value::String(s.clone()),
        Document::Array(arr) => {
            serde_json::Value::Array(arr.iter().map(bedrock_doc_to_json).collect())
        }
        Document::Object(obj) => serde_json::Value::Object(
            obj.iter()
                .map(|(k, v)| (k.clone(), bedrock_doc_to_json(v)))
                .collect(),
        ),
    }
}

/// Construit l'InferenceConfiguration depuis les paramètres de la requête
pub(crate) fn build_inference_config(
    req: &pylos_core::domain::openai::ChatCompletionRequest,
) -> InferenceConfiguration {
    let mut builder = InferenceConfiguration::builder();

    if let Some(max_tokens) = req.max_tokens {
        builder = builder.max_tokens(max_tokens);
    } else {
        // Bedrock exige max_tokens — on met un défaut raisonnable
        builder = builder.max_tokens(4096);
    }

    if let Some(temp) = req.temperature {
        builder = builder.temperature(temp);
    }

    if let Some(top_p) = req.top_p {
        builder = builder.top_p(top_p);
    }

    if let Some(stop) = &req.stop {
        match stop {
            pylos_core::domain::openai::StopCondition::Single(s) => {
                builder = builder.stop_sequences(s.clone());
            }
            pylos_core::domain::openai::StopCondition::Multiple(arr) => {
                for s in arr {
                    builder = builder.stop_sequences(s.clone());
                }
            }
        }
    }

    builder.build()
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion réponse Bedrock → PylosResponse
// ─────────────────────────────────────────────────────────────────────────────

/// Convertit une réponse Bedrock Converse en PylosResponse
pub(crate) fn from_bedrock_response(
    output_message: &Message,
    stop_reason: &str,
    usage: Option<&aws_sdk_bedrockruntime::types::TokenUsage>,
    model: &str,
    id: String,
) -> PylosResponse {
    let mut text_content = String::new();
    let mut tool_calls: Vec<ToolCall> = Vec::new();

    for (idx, block) in output_message.content().iter().enumerate() {
        match block {
            ContentBlock::Text(t) => {
                text_content.push_str(t);
            }
            ContentBlock::ToolUse(tu) => {
                let args = bedrock_doc_to_json(tu.input()).to_string();
                tool_calls.push(ToolCall {
                    id: tu.tool_use_id().to_string(),
                    call_type: "function".into(),
                    function: ToolCallFunction {
                        name: tu.name().to_string(),
                        arguments: args,
                    },
                    index: Some(idx as i32),
                });
            }
            _ => {}
        }
    }

    let finish_reason = map_stop_reason(stop_reason);
    let content = if text_content.is_empty() && !tool_calls.is_empty() {
        None
    } else {
        Some(text_content)
    };

    let usage_data = usage.map(|u| Usage {
        prompt_tokens: u.input_tokens(),
        completion_tokens: u.output_tokens(),
        total_tokens: u.total_tokens(),
        ..Default::default()
    });

    PylosResponse::ChatCompletion(ChatCompletionResponse {
        id,
        object: "chat.completion".into(),
        created: now_unix(),
        model: model.to_string(),
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
            finish_reason: Some(finish_reason.to_string()),
        }],
        usage: usage_data,
    })
}

/// Construit un StreamChunk texte/rôle/finish depuis les événements Bedrock
pub(crate) fn make_stream_chunk(
    id: &str,
    model: &str,
    content: Option<String>,
    role: Option<String>,
    finish_reason: Option<String>,
) -> StreamChunk {
    StreamChunk {
        id: id.to_string(),
        object: "chat.completion.chunk".into(),
        created: now_unix(),
        model: model.to_string(),
        choices: vec![StreamChoice {
            index: 0,
            delta: StreamDelta {
                role,
                content,
                ..Default::default()
            },
            finish_reason,
        }],
        usage: None,
    }
}

/// Construit un StreamChunk pour un outil (streaming Bedrock ToolUse)
pub(crate) fn make_tool_stream_chunk(
    id: &str,
    model: &str,
    tool_chunk: StreamToolCallChunk,
) -> StreamChunk {
    StreamChunk {
        id: id.to_string(),
        object: "chat.completion.chunk".into(),
        created: now_unix(),
        model: model.to_string(),
        choices: vec![StreamChoice {
            index: 0,
            delta: StreamDelta {
                tool_calls: Some(vec![tool_chunk]),
                ..Default::default()
            },
            finish_reason: None,
        }],
        usage: None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Mapping stop reasons  (Bedrock → OpenAI)
// ─────────────────────────────────────────────────────────────────────────────

/// Identique à bedrockFinishReasonToBifrost dans bifrost/core/providers/bedrock/chat.go
pub(crate) fn map_stop_reason(reason: &str) -> &'static str {
    match reason {
        "end_turn" => "stop",
        "max_tokens" => "length",
        "stop_sequence" => "stop",
        "tool_use" => "tool_calls",
        "guardrail_intervened" | "content_filtered" => "content_filter",
        _ => "stop",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Génère un ID unique pour une complétion Bedrock
pub(crate) fn generate_completion_id() -> String {
    format!("bedrock-{}", fastrand::u64(..))
}

/// Extrait le nom du modèle pour l'URL Bedrock Converse
/// Bedrock accepte soit l'ID natif soit le cross-region inference profile
/// Exemples :
///   "anthropic.claude-3-5-sonnet-20241022-v2:0"
///   "us.anthropic.claude-3-5-sonnet-20241022-v2:0"  (cross-region)
///   "amazon.nova-pro-v1:0"
pub(crate) fn normalize_model_id(model: &str) -> &str {
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::openai::{ChatCompletionMessage, MessageRole};

    #[test]
    fn test_stop_reason_mapping() {
        assert_eq!(map_stop_reason("end_turn"), "stop");
        assert_eq!(map_stop_reason("max_tokens"), "length");
        assert_eq!(map_stop_reason("stop_sequence"), "stop");
        assert_eq!(map_stop_reason("tool_use"), "tool_calls");
        assert_eq!(map_stop_reason("guardrail_intervened"), "content_filter");
        assert_eq!(map_stop_reason("unknown"), "stop");
    }

    #[test]
    fn test_convert_messages_system_extracted() {
        let messages = vec![
            ChatCompletionMessage {
                role: MessageRole::System,
                content: Some("You are helpful.".into()),
                ..Default::default()
            },
            ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("Hello".into()),
                ..Default::default()
            },
        ];

        let (bedrock_msgs, system_blocks) = convert_messages(&messages).unwrap();
        assert_eq!(system_blocks.len(), 1, "System message should be extracted");
        assert_eq!(bedrock_msgs.len(), 1, "Only user message should remain");
        assert_eq!(bedrock_msgs[0].role(), &ConversationRole::User);
    }

    #[test]
    fn test_convert_messages_roles() {
        let messages = vec![
            ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("Hi".into()),
                ..Default::default()
            },
            ChatCompletionMessage {
                role: MessageRole::Assistant,
                content: Some("Hello!".into()),
                ..Default::default()
            },
        ];

        let (bedrock_msgs, system_blocks) = convert_messages(&messages).unwrap();
        assert!(system_blocks.is_empty());
        assert_eq!(bedrock_msgs.len(), 2);
        assert_eq!(bedrock_msgs[0].role(), &ConversationRole::User);
        assert_eq!(bedrock_msgs[1].role(), &ConversationRole::Assistant);
    }

    #[test]
    fn test_inference_config_defaults() {
        use pylos_core::domain::openai::ChatCompletionRequest;

        let req = ChatCompletionRequest {
            model: "anthropic.claude-3-5-sonnet-20241022-v2:0".into(),
            messages: vec![],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            session_id: None,
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
        };

        let config = build_inference_config(&req);
        assert_eq!(config.max_tokens(), Some(4096));
    }
}
