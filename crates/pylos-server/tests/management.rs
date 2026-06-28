use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use pylos_server::state::AppState;
use serde_json::json;
use tempfile::tempdir;
use tower::ServiceExt;

#[tokio::test]
async fn test_management_api_auth() {
    let tdir = tempdir().unwrap();

    let state = AppState::from_config_with_dir(None, Some(tdir.path().to_path_buf()))
        .await
        .unwrap();
    // On force une clé admin pour le test
    let mut state_with_key = state.clone();
    state_with_key.admin_key_hash = std::sync::Arc::new(tokio::sync::RwLock::new(Some(
        pylos_server::state::hash_sha256("test-admin-key"),
    )));
    state_with_key.setup_required = std::sync::Arc::new(tokio::sync::RwLock::new(false));

    let app = pylos_server::routes::create_router(state_with_key);

    // 1. Test sans clé (devrait échouer)
    let req = Request::builder()
        .uri("/providers")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 2. Test avec mauvaise clé
    let req = Request::builder()
        .uri("/providers")
        .header(header::AUTHORIZATION, "Bearer wrong-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 3. Test avec bonne clé (Bearer)
    let req = Request::builder()
        .uri("/providers")
        .header(header::AUTHORIZATION, "Bearer test-admin-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_provider_management() {
    let tdir = tempdir().unwrap();

    let state = AppState::from_config_with_dir(None, Some(tdir.path().to_path_buf()))
        .await
        .unwrap();
    let mut state_with_key = state.clone();
    state_with_key.admin_key_hash = std::sync::Arc::new(tokio::sync::RwLock::new(Some(
        pylos_server::state::hash_sha256("test-admin-key"),
    )));
    state_with_key.setup_required = std::sync::Arc::new(tokio::sync::RwLock::new(false));
    let app = pylos_server::routes::create_router(state_with_key);

    // 1. Liste initiale
    let req = Request::builder()
        .uri("/providers")
        .header("X-Admin-Key", "test-admin-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    // 2. Création d'un provider
    let provider_json = json!({
        "name": "custom-test",
        "keys": [{
            "name": "key1",
            "value": "sk-test",
            "models": ["*"],
            "weight": 1.0
        }],
        "network": {
            "base_url": "https://api.example.com/v1"
        }
    });

    let req = Request::builder()
        .method("POST")
        .uri("/providers")
        .header("X-Admin-Key", "test-admin-key")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(provider_json.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3. Test de connectivité (devrait être appelé mais échouera / réussira selon le mock ou l'adresse)
    let req = Request::builder()
        .method("POST")
        .uri("/providers/custom-test/test")
        .header("X-Admin-Key", "test-admin-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    // On s'attend à une réponse car l'endpoint existe (probablement BAD_REQUEST ou autre en raison du réseau fictif)
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::OK);
}
