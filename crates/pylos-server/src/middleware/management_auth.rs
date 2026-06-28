use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// Management API auth middleware (H-2 fix)
//
// Protège les endpoints /providers, /virtual-keys, /config, /v1/models/catalog.
// Auth via header : Authorization: Bearer <PYLOS_ADMIN_KEY>
// ou                X-Admin-Key: <PYLOS_ADMIN_KEY>
//
// Si PYLOS_ADMIN_KEY n'est pas défini → les endpoints management sont ouverts
// (comportement legacy, avec warning au démarrage).
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
        .insert(None::<crate::middleware::virtual_key::VirtualKeyInfo>);

    // Extrait la clé depuis Authorization: Bearer ou X-Admin-Key
    let provided = extract_admin_key(request.headers()).map(|s| s.to_string());

    let admin_key_hash_opt = state.admin_key_hash.read().await.clone();

    let Some(provided_key) = provided else {
        // Si pas de clé admin configurée globale → laisse passer (compatibilité)
        if admin_key_hash_opt.is_none() {
            return next.run(request).await;
        }
        return (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "error": {
                    "message": "Management API requires Authorization: Bearer <token>",
                    "type": "unauthorized",
                    "code": 401
                }
            })),
        )
            .into_response();
    };

    // 1. Essayer de valider le token comme un JWT de session
    let validation = jsonwebtoken::Validation::default();
    let decoding_key = jsonwebtoken::DecodingKey::from_secret(state.jwt_secret.as_bytes());
    if let Ok(token_data) = jsonwebtoken::decode::<crate::interfaces::http::auth::PylosSessionClaims>(
        &provided_key,
        &decoding_key,
        &validation,
    ) {
        let email = token_data.claims.sub.to_lowercase();
        // Vérifie si l'utilisateur est toujours actif dans le store
        if let Ok(users) = state.org_store.list_users().await {
            if let Some(user) = users
                .iter()
                .find(|u| u.email.to_lowercase() == email && u.is_active)
            {
                request.extensions_mut().insert(Some(user.clone()));
                return next.run(request).await;
            }
        }
    }

    // 2. Fallback sur la clé d'administration statique globale
    if let Some(expected_hash) = &admin_key_hash_opt {
        let provided_hash = crate::state::hash_sha256(&provided_key);
        if constant_time_eq(provided_hash.as_bytes(), expected_hash.as_bytes()) {
            return next.run(request).await;
        }
    }

    // 3. Fallback sur la validation de la Virtual Key si c'est un sk-pylos-*
    if provided_key.starts_with(pylos_core::domain::virtual_key::VIRTUAL_KEY_PREFIX) {
        if let Ok(Some(vk_cfg)) = state.vk_store.get_key_by_value(&provided_key).await {
            if vk_cfg.is_active {
                // Register/update in the in-memory registry with rate limit
                let cfg = state.config_store.get().await;
                let rate_limit = cfg
                    .governance
                    .rate_limits
                    .iter()
                    .find(|rl| Some(&rl.id) == vk_cfg.rate_limit_id.as_ref())
                    .map(|rl| rl.request_max_limit)
                    .unwrap_or(0);
                let v_key = pylos_core::domain::virtual_key::VirtualKey::new(
                    provided_key.to_string(),
                    &vk_cfg.name,
                )
                .with_rpm(rate_limit);
                state.vk_registry.register(v_key).await;

                // Enforce rate limiting on management routes too
                if let Err(reason) = state.vk_registry.check_and_increment(&provided_key).await {
                    return (
                        StatusCode::TOO_MANY_REQUESTS,
                        Json(json!({
                            "error": {
                                "message": reason,
                                "type": "governance_error",
                                "code": 429
                            }
                        })),
                    )
                        .into_response();
                }
                request.extensions_mut().insert(Some(
                    crate::middleware::virtual_key::VirtualKeyInfo {
                        name: vk_cfg.name.clone(),
                        key: provided_key.to_string(),
                        provider_configs: vk_cfg.provider_configs.clone(),
                    },
                ));
                return next.run(request).await;
            }
        }
    }

    // 4. Mode legacy : si PYLOS_ADMIN_KEY n'est pas défini, endpoints ouverts
    if admin_key_hash_opt.is_none() {
        return next.run(request).await;
    }

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

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
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
