pub mod converters;
pub mod provider;
pub mod session;

pub use provider::AntigravityProvider;
pub use session::SessionManager;

#[cfg(test)]
mod tests {
    use super::converters::*;
    use super::session::*;
    use super::*;
    use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};
    use pylos_core::domain::provider::{ProviderConfig, ProviderKey, ProviderKind};
    use pylos_core::domain::request::{PylosRequest, PylosResponse};
    use pylos_core::domain::traits::Provider;

    #[test]
    fn test_normalize_antigravity_model() {
        assert_eq!(
            normalize_antigravity_model("antigravity/gemini-3.8-flash"),
            "gemini-3.8-flash-medium"
        );
        assert_eq!(
            normalize_antigravity_model("gemini-3.8-flash"),
            "gemini-3.8-flash-medium"
        );
        assert_eq!(
            normalize_antigravity_model("gemini-4-argon"),
            "gemini-4-argon-medium"
        );
        assert_eq!(
            normalize_antigravity_model("antigravity:gemini-3.8-flash-low"),
            "gemini-3.8-flash-low"
        );
        assert_eq!(
            normalize_antigravity_model("claude-opus-5-5"),
            "claude-opus-5-5-medium"
        );
        assert_eq!(
            normalize_antigravity_model("gpt-oss-120b"),
            "gpt-oss-120b-medium"
        );
    }

    #[tokio::test]
    async fn test_session_manager_binding() {
        let mgr = SessionManager::new();
        assert_eq!(mgr.get_agy_conversation("client_session_1").await, None);

        mgr.bind_session(
            "client_session_1",
            "12345678-1234-1234-1234-123456789abc",
            Some("joseph.zacharie@gmail.com"),
        )
        .await;

        assert_eq!(
            mgr.get_agy_conversation("client_session_1").await,
            Some("12345678-1234-1234-1234-123456789abc".to_string())
        );

        let (conv_id, account) = mgr
            .get_session("client_session_1")
            .await
            .expect("session must exist");
        assert_eq!(conv_id, "12345678-1234-1234-1234-123456789abc");
        assert_eq!(account, Some("joseph.zacharie@gmail.com".to_string()));

        let removed = mgr.remove_session("client_session_1").await;
        assert_eq!(
            removed,
            Some("12345678-1234-1234-1234-123456789abc".to_string())
        );
        assert_eq!(mgr.get_agy_conversation("client_session_1").await, None);
    }

    #[tokio::test]
    async fn test_antigravity_multi_account_resolution() {
        let provider = AntigravityProvider::new();
        let config = ProviderConfig::new(
            ProviderKind::Antigravity,
            vec![
                ProviderKey::new("joseph.zacharie@gmail.com").with_weight(1.0),
                ProviderKey::new("joseph.zacharie.ai3@gmail.com").with_weight(1.0),
            ],
        );

        // 1. Explicit account specified in request takes priority
        let acc1 = provider.resolve_account(
            &config,
            Some("joseph.zacharie.ai3@gmail.com"),
            None,
            None,
        );
        assert_eq!(acc1, Some("joseph.zacharie.ai3@gmail.com".to_string()));

        // 2. Existing session account takes priority (sticky session)
        let acc_sticky = provider.resolve_account(
            &config,
            None,
            Some("joseph.zacharie@gmail.com"),
            None,
        );
        assert_eq!(
            acc_sticky,
            Some("joseph.zacharie@gmail.com".to_string())
        );

        // 3. User email detection
        let acc_user = provider.resolve_account(
            &config,
            None,
            None,
            Some("joseph.zacharie@gmail.com"),
        );
        assert_eq!(acc_user, Some("joseph.zacharie@gmail.com".to_string()));

        // 4. Round-robin alternates between both configured accounts
        let r1 = provider.resolve_account(&config, None, None, None);
        let r2 = provider.resolve_account(&config, None, None, None);
        assert_ne!(r1, r2);
        assert!(r1 == Some("joseph.zacharie@gmail.com".into()) || r1 == Some("joseph.zacharie.ai3@gmail.com".into()));
        assert!(r2 == Some("joseph.zacharie@gmail.com".into()) || r2 == Some("joseph.zacharie.ai3@gmail.com".into()));
    }

    #[test]
    fn test_to_chat_completion_response_usage() {
        let agy_resp = AgyResponse {
            conversation_id: Some("conv-abc-123".into()),
            status: "SUCCESS".into(),
            response: "Hello Antigravity!".into(),
            error: None,
            duration_seconds: Some(1.23),
            num_turns: Some(1),
            usage: Some(AgyUsage {
                input_tokens: 150,
                output_tokens: 25,
                thinking_tokens: 10,
                cache_read_tokens: 50,
                total_tokens: 175,
            }),
        };

        let chat_resp = to_chat_completion_response(agy_resp, "antigravity/gemini-3.8-flash");
        assert_eq!(chat_resp.id, "chatcmpl-agy-conv-abc-123");
        assert_eq!(
            chat_resp.choices[0].message.content,
            Some("Hello Antigravity!".to_string())
        );

        let usage = chat_resp.usage.expect("usage must be present");
        assert_eq!(usage.prompt_tokens, 150);
        assert_eq!(usage.completion_tokens, 25);
        assert_eq!(usage.total_tokens, 175);
        assert_eq!(usage.reasoning_tokens, Some(10));
        assert_eq!(usage.prompt_cache_hit_tokens, Some(50));
    }

    #[tokio::test]
    async fn test_antigravity_live_execution_and_session() {
        let provider = AntigravityProvider::new();
        let config = ProviderConfig::new(
            ProviderKind::Antigravity,
            vec![ProviderKey::new("antigravity-local")],
        );

        // Si agy n'est pas accessible, on ignore le test pour ne pas bloquer les CI hors-machine
        if provider.resolve_bin(&config).is_err() {
            return;
        }

        // 1. Première requête créant une session
        let req1 = PylosRequest::ChatCompletion(ChatCompletionRequest {
            model: "antigravity/gemini-3.8-flash-low".into(),
            messages: vec![ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("Say 'PING_ANTIGRAVITY_1'".into()),
                ..Default::default()
            }],
            user: Some("test_user_session_42".into()),
            ..Default::default()
        });

        let resp1 = provider.complete(&req1, &config).await;
        if let Ok(PylosResponse::ChatCompletion(c_resp)) = resp1 {
            assert!(c_resp.id.starts_with("chatcmpl-agy-"));
            let usage = c_resp.usage.expect("Usage must be reported");
            assert!(usage.total_tokens > 0);

            // Vérifie que la session a été liée
            let conv_id = provider
                .session_manager()
                .get_agy_conversation("test_user_session_42")
                .await;
            assert!(conv_id.is_some());
        }
    }
}
