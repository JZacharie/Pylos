#![cfg(feature = "memgraph")]

use async_trait::async_trait;
use neo4rs::{query, Graph};
use std::sync::Arc;
use tracing::{error, info};

use pylos_core::domain::openai::{ChatCompletionMessage, MessageRole};
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

pub struct MemoryPlugin {
    graph: Arc<Graph>,
}

impl MemoryPlugin {
    pub async fn new(memgraph_url: String) -> anyhow::Result<Self> {
        // Connect to Memgraph using neo4rs
        // Note: neo4rs usually takes "127.0.0.1:7687"
        let graph = Graph::new(&memgraph_url, "", "").await?;
        Ok(Self {
            graph: Arc::new(graph),
        })
    }
}

#[async_trait]
impl LlmPlugin for MemoryPlugin {
    fn name(&self) -> &str {
        "memory"
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        let chat_req = match request {
            PylosRequest::ChatCompletion(ref mut req) => req,
            _ => return Ok(None),
        };

        let virtual_key = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(None),
        };

        // Query Memgraph for context
        let q = query("MATCH (vk:VirtualKey {id: $vk_id})-[:HAS_MEMORY]->(e1)-[r:RELATES_TO]->(e2) RETURN e1.name AS e1_name, r.type AS rel_type, e2.name AS e2_name LIMIT 20")
            .param("vk_id", virtual_key.clone());

        let mut memories = Vec::new();
        if let Ok(mut result) = self.graph.execute(q).await {
            while let Ok(Some(row)) = result.next().await {
                let e1: String = row.get("e1_name").unwrap_or_default();
                let rel: String = row.get("rel_type").unwrap_or_default();
                let e2: String = row.get("e2_name").unwrap_or_default();
                if !e1.is_empty() && !e2.is_empty() {
                    memories.push(format!("{} {} {}", e1, rel, e2));
                }
            }
        }

        {
            let mut system_content = String::new();
            if !memories.is_empty() {
                system_content.push_str("Here is the Knowledge Graph context from your previous conversations with this user:\n");
                for mem in memories.iter() {
                    system_content.push_str(&format!("- {}\n", mem));
                }
                system_content.push('\n');
            }

            if !chat_req.stream.unwrap_or(false) {
                system_content.push_str("IMPORTANT CRITICAL INSTRUCTION: If the user shares any new facts, preferences, or important project context in this turn, you MUST output a Knowledge Graph representing that fact in <memory></memory> tags at the very end of your response. Use the exact format: `EntityA|RELATION|EntityB`. Example: `<memory>User|PREFERS|Rust</memory>`.");
            } else {
                system_content.push_str("IMPORTANT CRITICAL INSTRUCTION: After streaming completes, if the user shares any new facts, preferences, or important project context in this turn, you MUST output a Knowledge Graph representing that fact in <memory></memory> tags at the very end of your response. Use the exact format: `EntityA|RELATION|EntityB`. Example: `<memory>User|PREFERS|Rust</memory>`.");
            }

            if !system_content.is_empty() {
                chat_req.messages.insert(
                    0,
                    ChatCompletionMessage {
                        role: MessageRole::System,
                        content: Some(system_content),
                        ..Default::default()
                    },
                );
                info!(
                    "MemoryPlugin: Injected {} KG edges for vk {}",
                    memories.len(),
                    virtual_key
                );
            }
        }

        Ok(None)
    }

    async fn post_hook(
        &self,
        _request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        let chat_resp = match response {
            PylosResponse::ChatCompletion(ref mut resp) => resp,
            _ => return Ok(()),
        };

        let virtual_key = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(()),
        };

        let content = match chat_resp.choices.first_mut() {
            Some(choice) => match choice.message.content.as_mut() {
                Some(c) => c,
                None => return Ok(()),
            },
            None => return Ok(()),
        };

        // Search for <memory>...</memory> tags
        if let Some(start_idx) = content.find("<memory>") {
            if let Some(end_idx) = content.find("</memory>") {
                if end_idx > start_idx + 8 {
                    let memory_content = content[start_idx + 8..end_idx].trim().to_string();

                    // Strip the memory block
                    let mut new_content = content[..start_idx].to_string();
                    new_content.push_str(&content[end_idx + 9..]);
                    *content = new_content.trim_end().to_string();

                    info!("MemoryPlugin: Extracted KG edge: {}", memory_content);

                    let parts: Vec<&str> = memory_content.split('|').collect();
                    if parts.len() == 3 {
                        let e1 = parts[0].trim();
                        let rel = parts[1].trim();
                        let e2 = parts[2].trim();

                        // Upsert to Memgraph
                        let q = query(
                            "MERGE (vk:VirtualKey {id: $vk_id}) \
                             MERGE (n1:Entity {name: $e1}) \
                             MERGE (n2:Entity {name: $e2}) \
                             MERGE (n1)-[r:RELATES_TO {type: $rel}]->(n2) \
                             MERGE (vk)-[:HAS_MEMORY]->(n1) \
                             MERGE (vk)-[:HAS_MEMORY]->(n2)",
                        )
                        .param("vk_id", virtual_key.clone())
                        .param("e1", e1.to_string())
                        .param("e2", e2.to_string())
                        .param("rel", rel.to_string());

                        if let Err(e) = self.graph.run(q).await {
                            error!("MemoryPlugin: Failed to save to Memgraph: {:?}", e);
                        } else {
                            info!("MemoryPlugin: Saved KG edge to Memgraph");
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
