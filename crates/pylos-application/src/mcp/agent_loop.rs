use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::future::join_all;
use tracing::{debug, info, warn};

use pylos_core::domain::mcp_server::McpContentItem;
use pylos_core::domain::openai::{
    ChatCompletionMessage, ChatCompletionRequest, MessageRole, ToolCall, ToolCallFunction,
};
use pylos_core::domain::request::{
    PylosRequest, PylosResponse, RequestContext, StreamToolCallChunk,
};
use pylos_core::error::PylosError;

use super::registry::McpToolRegistry;
use crate::use_cases::inference::InferenceOrchestrator;

/// Configuration de l'agent loop Pylos
#[derive(Debug, Clone)]
pub struct AgentLoopConfig {
    /// Nombre maximal d'itérations tool_call -> LLM (défaut : 5)
    pub max_iterations: usize,
    /// Timeout d'exécution d'un tool individuel (défaut : 30 secondes)
    pub tool_timeout: Duration,
    /// Exécution automatique des tool_calls côté serveur (défaut : true)
    pub auto_execute_tools: bool,
}

impl Default for AgentLoopConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            tool_timeout: Duration::from_secs(30),
            auto_execute_tools: true,
        }
    }
}

/// Accumulateur pour reconstituer les ToolCalls fragmentés en streaming SSE
#[derive(Debug, Default)]
pub struct StreamToolCallAccumulator {
    calls: HashMap<i32, (Option<String>, Option<String>, String)>,
}

impl StreamToolCallAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Intègre un chunk de streaming
    pub fn ingest(&mut self, chunk: &StreamToolCallChunk) {
        let entry = self
            .calls
            .entry(chunk.index)
            .or_insert_with(|| (None, None, String::new()));

        if let Some(ref id) = chunk.id {
            entry.0 = Some(id.clone());
        }
        if let Some(ref f) = chunk.function {
            if let Some(ref name) = f.name {
                entry.1 = Some(name.clone());
            }
            if let Some(ref args) = f.arguments {
                entry.2.push_str(args);
            }
        }
    }

    /// Reconstitue la liste ordonnée des appels d'outils
    pub fn to_tool_calls(&self) -> Vec<ToolCall> {
        let mut sorted_keys: Vec<_> = self.calls.keys().copied().collect();
        sorted_keys.sort();

        let mut res = Vec::new();
        for k in sorted_keys {
            if let Some((id, name, args)) = self.calls.get(&k) {
                res.push(ToolCall {
                    id: id.clone().unwrap_or_else(|| format!("call_{k}")),
                    call_type: "function".to_string(),
                    function: ToolCallFunction {
                        name: name.clone().unwrap_or_default(),
                        arguments: args.clone(),
                    },
                    index: Some(k),
                });
            }
        }
        res
    }

    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}

/// Exécute une liste de tool calls en parallèle avec timeout individuel (S-MCP-07)
pub async fn execute_tool_calls_parallel(
    registry: &McpToolRegistry,
    tool_calls: &[ToolCall],
    timeout: Duration,
    allowed_tools: Option<&[String]>,
    denied_tools: Option<&[String]>,
) -> Vec<(String, String, String)> {
    let mut tasks = Vec::with_capacity(tool_calls.len());

    for call in tool_calls {
        let tool_name = call.function.name.clone();
        let call_id = call.id.clone();
        let args_val = serde_json::from_str::<serde_json::Value>(&call.function.arguments)
            .unwrap_or(serde_json::Value::Null);

        let registry = registry.clone();
        let allowed = allowed_tools.map(|s| s.to_vec());
        let denied = denied_tools.map(|s| s.to_vec());

        tasks.push(async move {
            let exec_future = registry.execute_tool(
                &tool_name,
                args_val,
                allowed.as_deref(),
                denied.as_deref(),
            );

            let res = tokio::time::timeout(timeout, exec_future).await;

            match res {
                Ok(Ok(call_result)) => {
                    let text = call_result
                        .content
                        .iter()
                        .filter_map(|c| match c {
                            McpContentItem::Text { text } => Some(text.as_str()),
                            McpContentItem::Resource {
                                text: Some(ref t), ..
                            } => Some(t.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");

                    let text = if text.is_empty() {
                        serde_json::to_string(&call_result).unwrap_or_else(|_| "{}".to_string())
                    } else {
                        text
                    };
                    (call_id, tool_name, text)
                }
                Ok(Err(e)) => {
                    warn!(tool = %tool_name, error = %e, "Tool execution failed in agent loop");
                    (
                        call_id,
                        tool_name,
                        format!("{{\"error\": \"Tool execution failed: {}\"}}", e),
                    )
                }
                Err(_) => {
                    warn!(tool = %tool_name, timeout_secs = timeout.as_secs(), "Tool execution timed out");
                    (
                        call_id,
                        tool_name,
                        format!(
                            "{{\"error\": \"Tool execution timed out after {} seconds\"}}",
                            timeout.as_secs()
                        ),
                    )
                }
            }
        });
    }

    join_all(tasks).await
}

/// Orchestre la boucle agent complète non-streaming (S-MCP-06 / S-MCP-09)
pub async fn run_agent_loop(
    orchestrator: Arc<InferenceOrchestrator>,
    registry: Arc<McpToolRegistry>,
    mut chat_req: ChatCompletionRequest,
    mut ctx: RequestContext,
    config: &AgentLoopConfig,
    allowed_tools: Option<&[String]>,
    denied_tools: Option<&[String]>,
) -> Result<(PylosResponse, RequestContext), PylosError> {
    let mut iteration = 0;
    let mut total_prompt_tokens = 0;
    let mut total_completion_tokens = 0;
    let mut total_tokens = 0;

    loop {
        // Pour chaque nouveau tour d'inférence de l'agent loop, réinitialiser la chaîne de fallback
        ctx.tried_providers.clear();

        debug!(
            iteration = iteration,
            messages_count = chat_req.messages.len(),
            "Agent loop: starting iteration"
        );

        let (resp, updated_ctx) = orchestrator
            .complete(PylosRequest::ChatCompletion(chat_req.clone()), ctx.clone())
            .await?;
        ctx = updated_ctx;

        match resp {
            PylosResponse::ChatCompletion(mut chat_resp) => {
                let choice = chat_resp.choices.first().cloned();
                let tool_calls = choice.as_ref().and_then(|c| c.message.tool_calls.clone());

                if let Some(calls) = tool_calls {
                    if !calls.is_empty() && config.auto_execute_tools {
                        // Cumuler les tokens
                        if let Some(ref usage) = chat_resp.usage {
                            total_prompt_tokens += usage.prompt_tokens;
                            total_completion_tokens += usage.completion_tokens;
                            total_tokens += usage.total_tokens;
                        }

                        iteration += 1;
                        if iteration > config.max_iterations {
                            warn!(
                                max_iterations = config.max_iterations,
                                "Max agent loop iterations reached; returning current response"
                            );
                            if let Some(usage) = &mut chat_resp.usage {
                                usage.prompt_tokens = total_prompt_tokens;
                                usage.completion_tokens = total_completion_tokens;
                                usage.total_tokens = total_tokens;
                            }
                            return Ok((PylosResponse::ChatCompletion(chat_resp), ctx));
                        }

                        info!(
                            iteration = iteration,
                            tool_count = calls.len(),
                            "Agent loop: executing tools requested by model"
                        );

                        // 1. Ajouter le message de l'assistant (avec les tool_calls) à l'historique
                        let Some(c) = choice else {
                            continue;
                        };
                        chat_req.messages.push(c.message);

                        // 2. Exécuter tous les tool_calls en parallèle (S-MCP-07)
                        let results = execute_tool_calls_parallel(
                            &registry,
                            &calls,
                            config.tool_timeout,
                            allowed_tools,
                            denied_tools,
                        )
                        .await;

                        // 3. Injecter les réponses des outils (role = "tool")
                        for (call_id, tool_name, result_text) in results {
                            chat_req.messages.push(ChatCompletionMessage {
                                role: MessageRole::Tool,
                                content: Some(result_text),
                                tool_call_id: Some(call_id),
                                name: Some(tool_name),
                                ..Default::default()
                            });
                        }

                        // Boucler vers le LLM avec le contexte mis à jour
                        continue;
                    }
                }

                // Pas de tool call ou auto_execute_tools désactivé : fin du cycle
                if let Some(usage) = &mut chat_resp.usage {
                    usage.prompt_tokens += total_prompt_tokens;
                    usage.completion_tokens += total_completion_tokens;
                    usage.total_tokens += total_tokens;
                }
                return Ok((PylosResponse::ChatCompletion(chat_resp), ctx));
            }
            other => return Ok((other, ctx)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_tool_call_accumulator() {
        let mut acc = StreamToolCallAccumulator::new();
        assert!(acc.is_empty());

        acc.ingest(&StreamToolCallChunk {
            index: 0,
            id: Some("call_abc123".to_string()),
            call_type: Some("function".to_string()),
            function: Some(pylos_core::domain::request::StreamToolCallFunction {
                name: Some("web_fetch".to_string()),
                arguments: Some("{\"url\":".to_string()),
            }),
        });

        acc.ingest(&StreamToolCallChunk {
            index: 0,
            id: None,
            call_type: None,
            function: Some(pylos_core::domain::request::StreamToolCallFunction {
                name: None,
                arguments: Some(" \"https://example.com\"}".to_string()),
            }),
        });

        let calls = acc.to_tool_calls();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_abc123");
        assert_eq!(calls[0].function.name, "web_fetch");
        assert_eq!(
            calls[0].function.arguments,
            "{\"url\": \"https://example.com\"}"
        );
    }
}
