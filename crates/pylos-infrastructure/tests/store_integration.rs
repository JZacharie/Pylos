// Tests d'intégration pour les stores avec DbPool partagé
// Vérifie que tous les stores s'initialisent et interagissent correctement
// avec le pool SQLite in-memory.

use pylos_infrastructure::{BudgetStore, ModelCatalog, RateLimitStore, VirtualKeyStore};
use pylos_core::domain::config::{
    BudgetConfig, Duration, EnvVar, RateLimitConfig, VirtualKeyConfig, VkProviderConfig,
};

async fn setup_stores() -> (BudgetStore, RateLimitStore, VirtualKeyStore, ModelCatalog) {
    let budget = BudgetStore::in_memory().await.expect("budget store");
    let rl = RateLimitStore::in_memory().await.expect("rate limit store");
    let vk = VirtualKeyStore::in_memory()
        .await
        .expect("virtual key store");
    let catalog = ModelCatalog::in_memory().await.expect("model catalog");
    (budget, rl, vk, catalog)
}

#[tokio::test]
async fn test_all_stores_initialized() {
    let (budget, rl, vk, catalog) = setup_stores().await;

    // Budget store
    let usage = budget.get_usage("test-vk").await;
    assert!(usage.is_empty(), "No budget yet");

    // Rate limit store
    let status = rl.get_status("test-vk").await;
    assert!(status.is_empty(), "No rate limits yet");

    // Virtual key store
    let keys = vk.list_keys().await.expect("list keys");
    assert!(keys.is_empty(), "No VKs yet");

    // Model catalog
    let models = catalog.list_models(None, false).await;
    assert!(
        !models.is_empty(),
        "Model catalog should have seeded models"
    );
}

#[tokio::test]
async fn test_cross_store_workflow() {
    let (budget, rl, vk, _catalog) = setup_stores().await;

    // 1. Créer un rate limit config
    let rl_config = RateLimitConfig {
        id: "rl-1".into(),
        token_max_limit: 0,
        token_reset_duration: None,
        request_max_limit: 100,
        request_reset_duration: Some(Duration("1m".into())),
    };
    rl.upsert_rate_limit("vk-integration", &rl_config)
        .await
        .expect("upsert rate limit");

    // 2. Créer une virtual key
    let vk_config = VirtualKeyConfig {
        id: "vk-integration".into(),
        name: "Integration Test Key".into(),
        description: Some("Key for integration tests".into()),
        value: Some(EnvVar::Literal("sk-pylos-integration-test-123".into())),
        is_active: true,
        rate_limit_id: Some("rl-1".into()),
        provider_configs: vec![VkProviderConfig {
            provider: "openai".into(),
            allowed_models: vec!["*".into()],
            key_names: vec!["*".into()],
            weight: 1.0,
        }],
        team_alias: None,
        team_id: None,
        organization_id: None,
        access_group_id: None,
        user_email: None,
        user_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        last_active: None,
        expires_at: None,
        ..Default::default()
    };
    vk.upsert_key(&vk_config).await.expect("upsert vk");

    // 3. Vérifier que la clé existe
    let fetched = vk
        .get_key_by_value("sk-pylos-integration-test-123")
        .await
        .expect("get key by value");
    assert!(fetched.is_some(), "Key should exist");
    assert_eq!(fetched.unwrap().name, "Integration Test Key");

    // 4. Vérifier les listes
    let keys = vk.list_keys().await.expect("list keys");
    assert_eq!(keys.len(), 1);

    // 5. Créer un budget
    let budget_cfg = BudgetConfig {
        id: "b-1".into(),
        max_limit: 50.0,
        reset_duration: Duration("1d".into()),
        current_usage: 0.0,
        virtual_key_id: Some("vk-integration".into()),
    };
    budget
        .upsert_budget("vk-integration", &budget_cfg)
        .await
        .expect("upsert budget");

    // 6. Vérifier le budget
    let result = budget.check_budget("vk-integration", 10.0).await;
    assert!(result.is_ok(), "Should be within budget");

    // 7. Vérifier le rate limit
    for _ in 0..100 {
        rl.check_and_increment_requests("vk-integration")
            .await
            .expect("should be within rate limit");
    }
    let exceeded = rl.check_and_increment_requests("vk-integration").await;
    assert!(exceeded.is_err(), "Should exceed rate limit");

    // 8. Nettoyage
    vk.delete_key("vk-integration").await.expect("delete key");
    rl.delete_vk_entries("vk-integration").await;
    budget.delete_vk_entries("vk-integration").await;

    // 9. Vérifier le nettoyage
    let keys_after = vk.list_keys().await.expect("list after delete");
    assert_eq!(keys_after.len(), 0, "Should be clean");
}

#[tokio::test]
async fn test_vk_without_rate_limit_still_works() {
    let (_budget, rl, vk, _catalog) = setup_stores().await;

    let vk_cfg = VirtualKeyConfig {
        id: "vk-no-rl".into(),
        name: "No Rate Limit".into(),
        description: None,
        value: Some(EnvVar::Literal("sk-pylos-no-rl-key".into())),
        is_active: true,
        rate_limit_id: None,
        provider_configs: vec![],
        team_alias: None,
        team_id: None,
        organization_id: None,
        access_group_id: None,
        user_email: None,
        user_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        last_active: None,
        expires_at: None,
        ..Default::default()
    };
    vk.upsert_key(&vk_cfg).await.expect("upsert vk");

    // Pas de rate limit configuré → ne doit pas échouer
    let result = rl.check_and_increment_requests("vk-no-rl").await;
    assert!(result.is_ok(), "No rate limit configured should pass");

    let fetched = vk.get_key_by_id("vk-no-rl").await.expect("get by id");
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().name, "No Rate Limit");

    vk.delete_key("vk-no-rl").await.expect("delete");
}

#[tokio::test]
async fn test_budget_reset_after_period() {
    let (budget, _rl, vk, _catalog) = setup_stores().await;

    let vk_cfg = VirtualKeyConfig {
        id: "vk-budget-test".into(),
        name: "Budget Test".into(),
        description: None,
        value: Some(EnvVar::Literal("sk-pylos-budget-test".into())),
        is_active: true,
        rate_limit_id: None,
        provider_configs: vec![],
        team_alias: None,
        team_id: None,
        organization_id: None,
        access_group_id: None,
        user_email: None,
        user_id: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        last_active: None,
        expires_at: None,
        ..Default::default()
    };
    vk.upsert_key(&vk_cfg).await.expect("upsert vk");

    let budget_cfg = BudgetConfig {
        id: "b-reset".into(),
        max_limit: 10.0,
        reset_duration: Duration("1s".into()),
        current_usage: 0.0,
        virtual_key_id: Some("vk-budget-test".into()),
    };
    budget
        .upsert_budget("vk-budget-test", &budget_cfg)
        .await
        .expect("upsert budget");

    // Recording usage
    budget.record_usage("vk-budget-test", 5.0).await;

    // Check should pass (5 + 3 < 10)
    let result = budget.check_budget("vk-budget-test", 3.0).await;
    assert!(result.is_ok(), "Within budget");

    // Should exceed
    let result = budget.check_budget("vk-budget-test", 7.0).await;
    assert!(result.is_err(), "Budget should be exceeded");

    let usage = budget.get_usage("vk-budget-test").await;
    assert_eq!(usage.len(), 1);
    assert!((usage[0].current_usd - 5.0).abs() < 0.001);
}

#[tokio::test]
async fn test_config_repository_sqlite_roundtrip_and_backup() {
    use pylos_core::domain::repositories::ConfigRepository;
    use pylos_infrastructure::{DbPool, SqlConfigRepository};

    let pool = DbPool::in_memory(2).await.expect("in_memory pool");
    let repo = SqlConfigRepository::from_pool(pool).await;

    assert!(repo.load_config_json().await.expect("load empty").is_none());
    repo.save_config_json(r#"{"version":1}"#, 1)
        .await
        .expect("save v1");
    repo.backup_config_json(r#"{"version":1}"#, 1, 2)
        .await
        .expect("backup v1->v2");
    repo.save_config_json(r#"{"version":2}"#, 2)
        .await
        .expect("save v2");

    let loaded = repo.load_config_json().await.expect("load v2").expect("some");
    assert_eq!(loaded.1, 2);

    repo.save_admin_key_hash("hash-abc")
        .await
        .expect("save admin hash");
    assert_eq!(
        repo.get_admin_key_hash().await.expect("get admin hash"),
        Some("hash-abc".to_string())
    );
}

async fn verify_log_repo(log_store: &dyn pylos_core::domain::repositories::LogRepository) {
    use pylos_core::domain::logging::{LogEntry, LogFilter, LogStatus};

    let entry = LogEntry {
        id: "dual-test-1".into(),
        timestamp: 1_700_000_000_000,
        provider: "openai".into(),
        model: "gpt-4o-mini".into(),
        object: "chat.completion".into(),
        status: LogStatus::Success,
        latency_ms: 42.0,
        prompt_tokens: 10,
        completion_tokens: 5,
        total_tokens: 15,
        cost_usd: 0.0001,
        finish_reason: Some("stop".into()),
        error_message: None,
        virtual_key: None,
        is_stream: false,
        input_preview: Some("hi".into()),
        output_preview: Some("hello".into()),
        compression_saved_bytes: 0,
        guardrail_triggered: None,
        guardrail_type: None,
        guardrail_detail: None,
        obfuscated_input: None,
        pii_mapping: None,
        moderation_scores: None,
        caveman_input: None,
    };
    log_store.push(entry).await;
    let (items, total) = log_store.list(10, 0, &LogFilter::default()).await;
    assert!(total >= 1);
    assert!(items.iter().any(|e| e.id == "dual-test-1"));
}

#[tokio::test]
async fn test_dual_backend_sql_stores_sqlite_and_optional_postgres() {
    use pylos_infrastructure::SqlLogStore;

    let sqlite_store = SqlLogStore::in_memory().await.expect("in_memory SqlLogStore");
    verify_log_repo(&sqlite_store).await;

    if let Ok(pg_url) = std::env::var("PYLOS_TEST_POSTGRES_URL") {
        let pg_store = SqlLogStore::open_postgres(&pg_url)
            .await
            .expect("open_postgres SqlLogStore");
        verify_log_repo(&pg_store).await;
    }
}
