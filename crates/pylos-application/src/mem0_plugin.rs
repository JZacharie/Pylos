use async_trait::async_trait;
use tracing::{info, warn};

use pylos_core::domain::openai::{ChatCompletionMessage, MessageRole};
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

pub struct Mem0Plugin {
    sidecar_url: String,
    client: reqwest::Client,
}

impl Mem0Plugin {
    pub fn new(sidecar_url: String) -> Self {
        Self {
            sidecar_url,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
        }
    }

    async fn fetch_context(
        &self,
        query: &str,
        user_id: &str,
        session_id: Option<&str>,
        max_tokens: Option<u32>,
    ) -> Option<String> {
        let mut url = format!(
            "{}/api/memory/context?query={}&user_id={}",
            self.sidecar_url,
            urlencoding(query),
            urlencoding(user_id),
        );
        if let Some(sid) = session_id {
            url.push_str(&format!("&session_id={}", urlencoding(sid)));
        }
        if let Some(tokens) = max_tokens {
            url.push_str(&format!("&max_tokens={}", tokens));
        }

        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(body) => {
                        let context = body.get("context").and_then(|c| c.as_str()).unwrap_or("");
                        if context.is_empty() {
                            return None;
                        }
                        Some(context.to_string())
                    }
                    Err(e) => {
                        warn!("Mem0Plugin: failed to parse context response: {}", e);
                        None
                    }
                }
            }
            Ok(resp) => {
                warn!("Mem0Plugin: context fetch returned {}", resp.status());
                None
            }
            Err(e) => {
                warn!("Mem0Plugin: context fetch error: {}", e);
                None
            }
        }
    }

    async fn store_interaction(
        &self,
        user_id: &str,
        session_id: Option<&str>,
        data: &str,
        role: &str,
    ) {
        let mut body = serde_json::json!({
            "user_id": user_id,
            "data": data,
            "role": role,
            "metadata": {
                "source": "interaction",
                "type": "session_memory",
            }
        });
        if let Some(sid) = session_id {
            body["session_id"] = serde_json::json!(sid);
        }

        let url = format!("{}/api/memory/session", self.sidecar_url);
        match self.client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Mem0Plugin: stored interaction for user {}", user_id);
            }
            Ok(resp) => {
                warn!("Mem0Plugin: store returned {}", resp.status());
            }
            Err(e) => {
                warn!("Mem0Plugin: store error: {}", e);
            }
        }
    }
}

#[async_trait]
impl LlmPlugin for Mem0Plugin {
    fn name(&self) -> &str {
        "mem0"
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

        let last_user_msg = chat_req
            .messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .and_then(|m| m.content.as_deref())
            .unwrap_or("")
            .to_string();

        if let Some(context) = self
            .fetch_context(&last_user_msg, &virtual_key, None, None)
            .await
        {
            chat_req.messages.insert(
                0,
                ChatCompletionMessage {
                    role: MessageRole::System,
                    content: Some(context),
                    ..Default::default()
                },
            );
            info!(
                "Mem0Plugin: injected context into request for vk {}",
                virtual_key
            );
        }

        Ok(None)
    }

    async fn post_hook(
        &self,
        request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        let virtual_key = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(()),
        };

        let user_msg = match request {
            PylosRequest::ChatCompletion(req) => req
                .messages
                .iter()
                .rev()
                .find(|m| m.role == MessageRole::User)
                .and_then(|m| m.content.as_deref())
                .unwrap_or(""),
            _ => return Ok(()),
        };

        let assistant_reply = match response {
            PylosResponse::ChatCompletion(resp) => resp
                .choices
                .first()
                .and_then(|c| c.message.content.as_deref())
                .unwrap_or(""),
            _ => return Ok(()),
        };

        if !user_msg.is_empty() {
            let payload = format!("User: {}\nAssistant: {}", user_msg, assistant_reply);
            self.store_interaction(&virtual_key, None, &payload, "conversation")
                .await;
        }

        Ok(())
    }
}

fn urlencoding(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            b' ' => encoded.push_str("%20"),
            _ => encoded.push_str(&format!("%{:02X}", byte)),
        }
    }
    encoded
}
