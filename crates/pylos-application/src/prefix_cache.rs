use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use moka::future::Cache;
use tracing::{debug, info};

use pylos_core::domain::openai::ChatCompletionResponse;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

pub struct PrefixCachePlugin {
    cache: Cache<String, Arc<ChatCompletionResponse>>,
    min_prefix_len: usize,
}

impl PrefixCachePlugin {
    pub fn new(ttl_secs: u64, max_capacity: u64, min_prefix_len: usize) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(Duration::from_secs(ttl_secs))
            .build();
        Self {
            cache,
            min_prefix_len,
        }
    }

    fn compute_prefix_keys(&self, request: &PylosRequest) -> Vec<(usize, String)> {
        let chat_req = match request {
            PylosRequest::ChatCompletion(ref req) => req,
            _ => return vec![],
        };

        let model = request.model();
        let mut keys: Vec<(usize, String)> = Vec::with_capacity(chat_req.messages.len());
        let mut rolling = String::with_capacity(256);
        rolling.push_str(model);
        rolling.push('|');

        for (i, msg) in chat_req.messages.iter().enumerate() {
            rolling.push_str(&format!("{:?}:{:?}|", msg.role, msg.content));
            let prefix_len = i + 1;
            if prefix_len >= self.min_prefix_len {
                keys.push((prefix_len, rolling.clone()));
            }
        }

        keys
    }
}

#[async_trait]
impl LlmPlugin for PrefixCachePlugin {
    fn name(&self) -> &str {
        "prefix_cache"
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        if let PylosRequest::ChatCompletion(ref req) = request {
            if req.stream.unwrap_or(false) {
                return Ok(None);
            }
        }

        let keys = self.compute_prefix_keys(request);
        if keys.is_empty() {
            return Ok(None);
        }

        let total_msgs = keys.len();

        for (prefix_len, key) in keys.iter().rev() {
            if let Some(cached) = self.cache.get(key).await {
                let is_exact = *prefix_len == total_msgs;
                info!(
                    prefix_len = *prefix_len,
                    total_msgs = total_msgs,
                    exact = is_exact,
                    match_pct = format!("{:.1}%", (*prefix_len as f64 / total_msgs as f64) * 100.0),
                    "PrefixCache: {} hit (prefix {}/{})",
                    if is_exact { "exact" } else { "prefix" },
                    prefix_len,
                    total_msgs,
                );

                ctx.headers.insert(
                    "x-prefix-cache-match".to_string(),
                    format!("{}/{}", prefix_len, total_msgs),
                );

                if is_exact {
                    ctx.headers
                        .insert("x-prefix-cache-hit".to_string(), "true".to_string());
                    return Ok(Some(PylosResponse::ChatCompletion(cached.as_ref().clone())));
                }

                ctx.headers
                    .insert("x-prefix-cache-prefix".to_string(), "true".to_string());
                break;
            }
        }

        Ok(None)
    }

    async fn post_hook(
        &self,
        request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        if ctx.headers.contains_key("x-prefix-cache-hit") {
            return Ok(());
        }

        let chat_resp = match response {
            PylosResponse::ChatCompletion(ref resp) => resp,
            _ => return Ok(()),
        };

        let keys = self.compute_prefix_keys(request);
        if keys.is_empty() {
            return Ok(());
        }

        let shared = Arc::new(chat_resp.clone());

        for (_prefix_len, key) in &keys {
            self.cache.insert(key.clone(), Arc::clone(&shared)).await;
        }

        debug!(
            n_keys = keys.len(),
            "PrefixCache: cached response at {} prefix levels",
            keys.len()
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::openai::{
        ChatCompletionChoice, ChatCompletionMessage, ChatCompletionRequest, ChatCompletionResponse,
        MessageRole,
    };

    fn make_request(messages: Vec<(&str, &str)>) -> PylosRequest {
        PylosRequest::ChatCompletion(ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: messages
                .into_iter()
                .map(|(role, content)| {
                    let r = if role == "user" {
                        MessageRole::User
                    } else if role == "assistant" {
                        MessageRole::Assistant
                    } else {
                        MessageRole::System
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
            max_tokens: None,
            response_format: None,
            top_p: None,
            n: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            seed: None,
            top_k: None,
            min_p: None,
            repetition_penalty: None,
            max_completion_tokens: None,
        })
    }

    fn make_response(content: &str) -> PylosResponse {
        PylosResponse::ChatCompletion(ChatCompletionResponse {
            id: "chat-test".to_string(),
            object: "chat.completion".to_string(),
            created: 1234567,
            model: "gpt-4".to_string(),
            choices: vec![ChatCompletionChoice {
                index: 0,
                message: ChatCompletionMessage {
                    role: MessageRole::Assistant,
                    content: Some(content.to_string()),
                    ..Default::default()
                },
                finish_reason: Some("stop".to_string()),
            }],
            usage: None,
        })
    }

    #[tokio::test]
    async fn test_exact_match_returns_cached() {
        let plugin = PrefixCachePlugin::new(60, 100, 1);
        let req = make_request(vec![("user", "Hello")]);
        let mut ctx = RequestContext::default();

        // First request -> Miss, then cache
        assert!(plugin
            .pre_hook(&mut req.clone(), &mut ctx)
            .await
            .unwrap()
            .is_none());
        let mut resp = make_response("World");
        plugin.post_hook(&req, &mut resp, &mut ctx).await.unwrap();

        // Second exact request -> Hit
        let mut ctx2 = RequestContext::default();
        let hit = plugin.pre_hook(&mut req.clone(), &mut ctx2).await.unwrap();
        assert!(hit.is_some());
        assert_eq!(ctx2.headers.get("x-prefix-cache-hit").unwrap(), "true");
    }

    #[tokio::test]
    async fn test_prefix_detected_when_suffix_differs() {
        let plugin = PrefixCachePlugin::new(60, 100, 1);
        let req_a = make_request(vec![
            ("system", "Be helpful"),
            ("user", "Hello"),
            ("assistant", "Hi!"),
            ("user", "Tell me a joke"),
        ]);
        let mut ctx = RequestContext::default();

        assert!(plugin
            .pre_hook(&mut req_a.clone(), &mut ctx)
            .await
            .unwrap()
            .is_none());
        let mut resp = make_response("Why did the chicken...");
        plugin.post_hook(&req_a, &mut resp, &mut ctx).await.unwrap();

        // Different last message but same prefix
        let req_b = make_request(vec![
            ("system", "Be helpful"),
            ("user", "Hello"),
            ("assistant", "Hi!"),
            ("user", "Tell me a poem"),
        ]);
        let mut ctx2 = RequestContext::default();
        let hit = plugin
            .pre_hook(&mut req_b.clone(), &mut ctx2)
            .await
            .unwrap();
        assert!(
            hit.is_none(),
            "should NOT return cached response for different suffix"
        );

        assert!(
            ctx2.headers.contains_key("x-prefix-cache-match"),
            "should report prefix match"
        );
        assert_eq!(ctx2.headers.get("x-prefix-cache-match").unwrap(), "3/4");
    }

    #[tokio::test]
    async fn test_single_message_different_is_miss() {
        let plugin = PrefixCachePlugin::new(60, 100, 1);
        let req_a = make_request(vec![("user", "Hello")]);
        let mut ctx = RequestContext::default();

        assert!(plugin
            .pre_hook(&mut req_a.clone(), &mut ctx)
            .await
            .unwrap()
            .is_none());
        let mut resp = make_response("World");
        plugin.post_hook(&req_a, &mut resp, &mut ctx).await.unwrap();

        // Different first message -> no prefix match
        let req_b = make_request(vec![("user", "Goodbye")]);
        let mut ctx2 = RequestContext::default();
        let hit = plugin
            .pre_hook(&mut req_b.clone(), &mut ctx2)
            .await
            .unwrap();
        assert!(hit.is_none());
        assert!(!ctx2.headers.contains_key("x-prefix-cache-match"));
    }

    #[tokio::test]
    async fn test_short_request_with_cached_longer_prefix() {
        let plugin = PrefixCachePlugin::new(60, 100, 1);
        let req_long = make_request(vec![
            ("user", "Hello"),
            ("assistant", "Hi!"),
            ("user", "How are you?"),
        ]);
        let mut ctx = RequestContext::default();

        assert!(plugin
            .pre_hook(&mut req_long.clone(), &mut ctx)
            .await
            .unwrap()
            .is_none());
        let mut resp = make_response("I'm great!");
        plugin
            .post_hook(&req_long, &mut resp, &mut ctx)
            .await
            .unwrap();

        // Shorter request that matches first part
        let req_short = make_request(vec![("user", "Hello")]);
        let mut ctx2 = RequestContext::default();
        let hit = plugin
            .pre_hook(&mut req_short.clone(), &mut ctx2)
            .await
            .unwrap();
        assert!(
            hit.is_some(),
            "should return cached for shorter matching prefix"
        );
        assert_eq!(ctx2.headers.get("x-prefix-cache-hit").unwrap(), "true");
    }

    #[tokio::test]
    async fn test_min_prefix_len_skips_short_messages() {
        let plugin = PrefixCachePlugin::new(60, 100, 3);
        let req = make_request(vec![("user", "Hi")]);

        let keys = plugin.compute_prefix_keys(&req);
        assert!(
            keys.is_empty(),
            "should not generate keys below min_prefix_len"
        );
    }
}
