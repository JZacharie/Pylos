use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::interfaces::http::auth::PylosSessionClaims;
use crate::middleware::virtual_key::VirtualKeyInfo;
use crate::state::AppState;
use pylos_core::domain::organization::InternalUser;

// ─────────────────────────────────────────────────────────────────────────────
// Management API auth middleware
//
// Protège les endpoints /providers, /virtual-keys, /config, /api/*, /v1/models/catalog.
//
// Identités acceptées :
//   1. JWT de session dont l'utilisateur est ACTIF et a le rôle "admin"
//      (le rôle est relu depuis le store, pas depuis le token).
//   2. Clé d'administration statique : Authorization: Bearer <PYLOS_ADMIN_KEY>
//      ou X-Admin-Key: <PYLOS_ADMIN_KEY>.
//
// Les Virtual Keys (sk-pylos-*) sont des identifiants de DATA-PLANE : elles ne
// donnent JAMAIS accès au plan de contrôle.
// ─────────────────────────────────────────────────────────────────────────────

pub async fn management_auth_middleware(
    State(state): State<AppState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    // Block access if setup is required
    if *state.setup_required.read().await {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({
                "error": {
                    "message": "Pylos setup is required. Please set up the administrator password.",
                    "type": "setup_required",
                    "code": 403
                }
            })),
        )
            .into_response();
    }

    // Insert default extensions to avoid missing extension errors in downstream handlers
    request
        .extensions_mut()
        .insert(None::<pylos_core::domain::organization::InternalUser>);
    request
        .extensions_mut()
        .insert(None::<VirtualKeyInfo>);

    let provided_key = match extract_admin_key(request.headers()) {
        Some(key) => key.to_string(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "error": {
                        "message": "Management API requires Authorization: Bearer <token> or X-Admin-Key",
                        "type": "unauthorized",
                        "code": 401
                    }
                })),
            )
                .into_response();
        }
    };

    // 1. JWT de session — rôle admin exigé et relu depuis le store
    if let Some(user) = session_admin(&state, &provided_key).await {
        request.extensions_mut().insert(Some(user));
        return next.run(request).await;
    }

    // 2. Clé d'administration statique globale
    let admin_key_hash_opt = state.admin_key_hash.read().await.clone();
    if let Some(expected_hash) = &admin_key_hash_opt {
        if crate::state::verify_admin_secret(&provided_key, expected_hash) {
            return next.run(request).await;
        }
    }

    // 3. Tout le reste est refusé (Virtual Keys incluses)
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": {
                "message": "Invalid token or admin key",
                "type": "forbidden",
                "code": 403
            }
        })),
    )
        .into_response()
}

/// Valide un JWT de session et retourne l'utilisateur uniquement s'il est actif ET administrateur.
async fn session_admin(state: &AppState, token: &str) -> Option<InternalUser> {
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(state.jwt_secret.as_bytes());
    let token_data = jsonwebtoken::decode::<PylosSessionClaims>(
        token,
        &decoding_key,
        &jsonwebtoken::Validation::default(),
    )
    .ok()?;

    let email = token_data.claims.sub.to_lowercase();
    let users = state.org_store.list_users().await.ok()?;
    users.into_iter().find(|u| {
        u.email.to_lowercase() == email && u.is_active && u.role.eq_ignore_ascii_case("admin")
    })
}

fn extract_admin_key(headers: &axum::http::HeaderMap) -> Option<&str> {
    // Authorization: Bearer <key>
    if let Some(auth) = headers.get("authorization") {
        if let Ok(auth_str) = auth.to_str() {
            if let Some(key) = auth_str.strip_prefix("Bearer ") {
                return Some(key);
            }
        }
    }
    // X-Admin-Key: <key>
    if let Some(key) = headers.get("x-admin-key") {
        return key.to_str().ok();
    }
    None
}
