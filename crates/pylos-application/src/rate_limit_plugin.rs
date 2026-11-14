use std::sync::Arc;

use pylos_core::domain::repositories::RateLimitRepository;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

pub struct RateLimitPlugin {
    repo: Arc<dyn RateLimitRepository>,
}

impl RateLimitPlugin {
    pub fn new(repo: Arc<dyn RateLimitRepository>) -> Self {
        Self { repo }
    }
}

#[async_trait::async_trait]
impl LlmPlugin for RateLimitPlugin {
    fn name(&self) -> &str {
        "rate_limit"
    }

    fn priority(&self) -> i32 {
        -80
    }

    async fn pre_hook(
        &self,
        _request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        let vk_id = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(None),
        };

        self.repo.check_and_increment_requests(&vk_id).await?;
        Ok(None)
    }

    async fn post_hook(
        &self,
        _request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        let vk_id = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(()),
        };

        let tokens = match response {
            PylosResponse::ChatCompletion(r) => {
                r.usage.as_ref().map(|u| i64::from(u.total_tokens)).unwrap_or(0)
            }
            PylosResponse::Embedding(r) => i64::from(r.usage.prompt_tokens),
            _ => 0,
        };

        if tokens > 0 {
            self.repo.record_tokens(&vk_id, tokens).await;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::config::RateLimitConfig;
    use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};
    use pylos_core::domain::repositories::InMemoryRateLimitRepository;

    #[tokio::test]
    async fn test_rate_limit_plugin_with_in_memory_fake() {
        let repo = Arc::new(InMemoryRateLimitRepository::new());
        repo.upsert_rate_limit(
            "vk-rl",
            &RateLimitConfig {
                id: "rl1".into(),
                token_max_limit: 0,
                token_reset_duration: None,
                request_max_limit: 1,
                request_reset_duration: None,
            },
        )
        .await
        .unwrap();

        let plugin = RateLimitPlugin::new(repo);
        let mut req = PylosRequest::ChatCompletion(ChatCompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("hi".into()),
                ..Default::default()
            }],
            ..Default::default()
        });
        let mut ctx = RequestContext {
            virtual_key: Some("vk-rl".into()),
            ..Default::default()
        };

        assert!(plugin.pre_hook(&mut req, &mut ctx).await.is_ok());
        let err = plugin.pre_hook(&mut req, &mut ctx).await.unwrap_err();
        assert!(matches!(err, PylosError::RateLimitExceeded(_)));
    }
}
