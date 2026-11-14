use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

use pylos_core::domain::openai::MessageRole;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

#[derive(Clone)]
pub struct PromptTemplate {
    pub name: String,
    pub version: String,
    pub template: String,
}

#[derive(Clone)]
pub struct PromptRegistryPlugin {
    // Registre interne clé -> template
    templates: Arc<RwLock<HashMap<String, PromptTemplate>>>,
}

impl PromptRegistryPlugin {
    pub fn new() -> Self {
        Self {
            templates: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn register(&self, name: &str, version: &str, template: &str) {
        let key = format!("{}:{}", name, version);
        self.templates.write().await.insert(
            key,
            PromptTemplate {
                name: name.to_string(),
                version: version.to_string(),
                template: template.to_string(),
            },
        );
        info!("PromptRegistry: Registered template '{}:{}'", name, version);
    }
}

impl Default for PromptRegistryPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmPlugin for PromptRegistryPlugin {
    fn name(&self) -> &str {
        "prompt_registry"
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

        // Regarde si un template est demandé via un en-tête ou si le premier message est un appel de template pylos://
        let mut template_call = None;
        if let Some(msg) = chat_req.messages.first() {
            if let Some(ref content) = msg.content {
                if let Some(stripped) = content.strip_prefix("pylos://") {
                    template_call = Some(stripped.to_string());
                }
            }
        }

        let template_key = match template_call {
            Some(key) => key,
            None => return Ok(None),
        };

        let templates_map = self.templates.read().await;
        let template_obj = match templates_map.get(&template_key) {
            Some(t) => t,
            None => {
                warn!(
                    "PromptRegistry: Template '{}' not found in registry",
                    template_key
                );
                return Err(PylosError::NotFound(format!(
                    "Template {} not found",
                    template_key
                )));
            }
        };

        // Rendu basique du template par remplacement de variables issues du contexte de requête
        let mut rendered = template_obj.template.clone();
        for (k, v) in &ctx.headers {
            let placeholder = format!("{{{{{}}}}}", k);
            rendered = rendered.replace(&placeholder, v);
        }

        info!(
            "PromptRegistry: Successfully rendered template '{}'",
            template_key
        );
        // Remplace le message appelant par le prompt rendu
        if let Some(msg) = chat_req.messages.first_mut() {
            msg.content = Some(rendered);
            msg.role = MessageRole::System;
        }

        Ok(None)
    }

    async fn post_hook(
        &self,
        _request: &PylosRequest,
        _response: &mut PylosResponse,
        _ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest};

    #[tokio::test]
    async fn test_prompt_registry_interpolation() {
        let plugin = PromptRegistryPlugin::new();
        plugin
            .register(
                "summary-bot",
                "v1",
                "You are a helpful assistant that summarizes text. User name is {{user_name}}.",
            )
            .await;

        let mut request = PylosRequest::ChatCompletion(ChatCompletionRequest {
            model: "gpt-4".to_string(),
            messages: vec![ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("pylos://summary-bot:v1".to_string()),
                ..Default::default()
            }],
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
            session_id: None,
            tools: None,
            tool_choice: None,
            seed: None,
            top_k: None,
            min_p: None,
            repetition_penalty: None,
            max_completion_tokens: None,
            account: None,
        });

        let mut ctx = RequestContext::default();
        ctx.headers
            .insert("user_name".to_string(), "Joseph".to_string());

        let pre_result = plugin.pre_hook(&mut request, &mut ctx).await.unwrap();
        assert!(pre_result.is_none());

        // Check that template was rendered and injected
        let chat_req = match request {
            PylosRequest::ChatCompletion(req) => req,
            _ => panic!("Expected ChatCompletion request"),
        };

        let sys_message = chat_req.messages.first().unwrap();
        assert_eq!(sys_message.role, MessageRole::System);
        assert_eq!(
            sys_message.content.as_ref().unwrap(),
            "You are a helpful assistant that summarizes text. User name is Joseph."
        );
    }
}
