use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::Mutex;
use tracing::debug;

use pylos_core::domain::logging::estimate_cost;
use pylos_core::domain::provider::ProviderKind;
use pylos_core::domain::repositories::BudgetRepository;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

pub struct BudgetPlugin {
    repo: Arc<dyn BudgetRepository>,
    pending_cost: Arc<Mutex<HashMap<String, f64>>>,
}

impl BudgetPlugin {
    pub fn new(repo: Arc<dyn BudgetRepository>) -> Self {
        Self {
            repo,
            pending_cost: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

fn estimate_request_cost(model: &str) -> f64 {
    let provider = ProviderKind::guess_from_model(model).to_string();
    estimate_cost(&provider, model, 1000, 1000)
}

#[async_trait::async_trait]
impl LlmPlugin for BudgetPlugin {
    fn name(&self) -> &str {
        "budget"
    }

    fn priority(&self) -> i32 {
        -70
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        let vk_id = match &ctx.virtual_key {
            Some(vk) => vk.clone(),
            None => return Ok(None),
        };

        let estimated = estimate_request_cost(request.model());
        self.repo.check_budget(&vk_id, estimated).await?;

        let mut pending = self.pending_cost.lock().await;
        pending.insert(vk_id, estimated);

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

        let actual_cost = match response {
            PylosResponse::ChatCompletion(r) => r
                .usage
                .as_ref()
                .map(|u| {
                    let provider = ProviderKind::guess_from_model(&r.model).to_string();
                    estimate_cost(&provider, &r.model, u.prompt_tokens, u.completion_tokens)
                })
                .unwrap_or(0.0),
            _ => 0.0,
        };

        let mut pending = self.pending_cost.lock().await;
        pending.remove(&vk_id);
        drop(pending);

        if actual_cost > 0.0 {
            self.repo.record_usage(&vk_id, actual_cost).await;
            debug!(vk_id = %vk_id, cost_usd = actual_cost, "Budget usage recorded");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::config::{BudgetConfig, Duration};
    use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};
    use pylos_core::domain::repositories::InMemoryBudgetRepository;

    #[tokio::test]
    async fn test_budget_plugin_with_in_memory_fake() {
        let repo = Arc::new(InMemoryBudgetRepository::new());
        repo.upsert_budget(
            "vk-mem",
            &BudgetConfig {
                id: "b1".into(),
                max_limit: 0.01,
                reset_duration: Duration("1d".into()),
                current_usage: 0.05,
                virtual_key_id: Some("vk-mem".into()),
            },
        )
        .await
        .unwrap();

        let plugin = BudgetPlugin::new(repo);
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
            virtual_key: Some("vk-mem".into()),
            ..Default::default()
        };

        let err = plugin.pre_hook(&mut req, &mut ctx).await.unwrap_err();
        assert!(matches!(err, PylosError::BudgetExceeded(_)));
    }
}
