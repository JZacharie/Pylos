use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use pylos_core::domain::config::{EnvVar, VirtualKeyConfig};
use pylos_core::domain::organization::InternalUser;
use pylos_server::interfaces::http::auth::PylosSessionClaims;
use pylos_server::routes::create_router;
use pylos_server::state::{hash_admin_secret, AppState};
use serde_json::json;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::tempdir;
use tower::ServiceExt;

const ADMIN_KEY: &str = "test-admin-key";

async fn test_state(dir: &std::path::Path) -> AppState {
    let state = AppState::from_config_with_dir(None, Some(dir.to_path_buf()))
        .await
        .unwrap();
    let mut s = state.clone();
    s.admin_key_hash = Arc::new(tokio::sync::RwLock::new(Some(hash_admin_secret(ADMIN_KEY))));
    s.setup_required = Arc::new(tokio::sync::RwLock::new(false));
    s
}

fn session_token(state: &AppState, email: &str, role: &str) -> String {
    let exp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as usize
        + 3600;
    let claims = PylosSessionClaims {
        sub: email.to_string(),
        exp,
        role: role.to_string(),
        name: email.to_string(),
        group: "default".to_string(),
    };
    jsonwebtoken::encode(
        &jsonwebtoken::Header::default(),
        &claims,
        &jsonwebtoken::EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .unwrap()
}

fn internal_user(id: &str, email: &str, role: &str, is_active: bool) -> InternalUser {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    InternalUser {
        id: id.to_string(),
        email: email.to_string(),
        name: email.to_string(),
        role: role.to_string(),
        group: "default".to_string(),
        organization_id: None,
        team_ids: vec![],
        is_active,
        created_at: now,
        updated_at: now,
    }
}

async fn get_status(app: &axum::Router, uri: &str, credential: &str) -> StatusCode {
    let req = Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {credential}"))
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

#[tokio::test]
async fn test_management_api_auth() {
    let tdir = tempdir().unwrap();
    let state = test_state(tdir.path()).await;
    let app = create_router(state);

    // 1. Sans clé -> 401
    let req = Request::builder()
        .uri("/providers")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // 2. Mauvaise clé -> 403
    let req = Request::builder()
        .uri("/providers")
        .header(header::AUTHORIZATION, "Bearer wrong-key")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);

    // 3. Bonne clé (Bearer) -> 200
    let req = Request::builder()
        .uri("/providers")
        .header(header::AUTHORIZATION, format!("Bearer {ADMIN_KEY}"))
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

/// H-01 : une Virtual Key de data-plane ne doit JAMAIS ouvrir le plan de contrôle.
#[tokio::test]
async fn test_virtual_key_cannot_access_management_api() {
    let tdir = tempdir().unwrap();
    let state = test_state(tdir.path()).await;

    let vk = VirtualKeyConfig {
        id: "vk-attacker".into(),
        name: "attacker".into(),
        description: None,
        value: Some(EnvVar::Literal("sk-pylos-attacker".into())),
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
    };
    state.vk_store.upsert_key(&vk).await.unwrap();

    let app = create_router(state);
    for uri in [
        "/providers",
        "/virtual-keys",
        "/virtual-keys/vk-attacker/value",
        "/config",
        "/api/users",
        "/api/organizations",
        "/api/logs",
    ] {
        let status = get_status(&app, uri, "sk-pylos-attacker").await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "une Virtual Key ne doit pas accéder à {uri}"
        );
    }

    // Elle reste utilisable sur le data-plane (elle n'y est simplement pas enregistrée ici)
    assert_eq!(
        get_status(&app, "/health", "sk-pylos-attacker").await,
        StatusCode::OK
    );
}

/// H-02 : le rôle admin est exigé et relu depuis le store (pas depuis le token).
#[tokio::test]
async fn test_management_api_requires_admin_role() {
    let tdir = tempdir().unwrap();
    let state = test_state(tdir.path()).await;

    state
        .org_store
        .upsert_user(&internal_user("u-member", "member@example.com", "member", true))
        .await
        .unwrap();
    state
        .org_store
        .upsert_user(&internal_user("u-admin", "admin@example.com", "admin", true))
        .await
        .unwrap();
    state
        .org_store
        .upsert_user(&internal_user(
            "u-inactive",
            "inactive@example.com",
            "admin",
            false,
        ))
        .await
        .unwrap();

    let member = session_token(&state, "member@example.com", "member");
    let admin = session_token(&state, "admin@example.com", "admin");
    let forged = session_token(&state, "member@example.com", "admin");
    let inactive = session_token(&state, "inactive@example.com", "admin");
    let app = create_router(state);

    assert_eq!(
        get_status(&app, "/providers", &member).await,
        StatusCode::FORBIDDEN,
        "un membre ne doit pas accéder au management"
    );
    assert_eq!(
        get_status(&app, "/providers", &forged).await,
        StatusCode::FORBIDDEN,
        "le rôle doit être relu depuis le store, pas depuis le token"
    );
    assert_eq!(
        get_status(&app, "/providers", &inactive).await,
        StatusCode::FORBIDDEN,
        "un administrateur désactivé ne doit pas accéder au management"
    );
    assert_eq!(
        get_status(&app, "/providers", &admin).await,
        StatusCode::OK,
        "un administrateur actif doit accéder au management"
    );
}

#[tokio::test]
async fn test_provider_management() {
    let tdir = tempdir().unwrap();
    let state = test_state(tdir.path()).await;
    let app = create_router(state);

    // 1. Liste initiale
    let req = Request::builder()
        .uri("/providers")
        .header("X-Admin-Key", ADMIN_KEY)
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
        .header("X-Admin-Key", ADMIN_KEY)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(provider_json.to_string()))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);

    // 3. Test de connectivité
    let req = Request::builder()
        .method("POST")
        .uri("/providers/custom-test/test")
        .header("X-Admin-Key", ADMIN_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert!(resp.status() == StatusCode::BAD_REQUEST || resp.status() == StatusCode::OK);
}
/// #124 : le statut d'un serveur MCP est reserve aux administrateurs.
#[tokio::test]
async fn test_mcp_status_requires_admin() {
    let tdir = tempdir().unwrap();
    let state = test_state(tdir.path()).await;
    let app = create_router(state);

    // Sans identite admin -> 401
    let req = Request::builder()
        .uri("/mcp/whatever")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    // Avec la cle admin -> l'authentification passe (404 : serveur inconnu)
    let req = Request::builder()
        .uri("/mcp/whatever")
        .header("X-Admin-Key", ADMIN_KEY)
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}
/// #119 : le secret d'administration est haché en Argon2id (format PHC) avec sel
/// aléatoire, et l'ancien hash SHA-256 non salé reste accepté (migration transparente).
#[tokio::test]
async fn test_admin_secret_hashing() {
    use pylos_server::state::{hash_admin_secret, hash_sha256, verify_admin_secret};

    let hashed = hash_admin_secret("s3cret-admin");
    assert!(
        hashed.starts_with("$argon2"),
        "format PHC Argon2id attendu, obtenu: {hashed}"
    );
    assert!(verify_admin_secret("s3cret-admin", &hashed));
    assert!(!verify_admin_secret("mauvais-secret", &hashed));

    // Sel aléatoire : deux hachages du même secret diffèrent
    assert_ne!(hash_admin_secret("s3cret-admin"), hash_admin_secret("s3cret-admin"));

    // Compatibilité legacy SHA-256
    let legacy = hash_sha256("legacy-admin");
    assert!(verify_admin_secret("legacy-admin", &legacy));
    assert!(!verify_admin_secret("nope", &legacy));
}
