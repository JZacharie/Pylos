use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use pylos_core::domain::virtual_key::VIRTUAL_KEY_PREFIX;

use crate::state::AppState;

/// Middleware Axum de vérification des Virtual Keys
///
/// Si le header Authorization est présent avec un sk-pylos-* :
///   - Vérifie la clé dans le registre
///   - Vérifie le rate limit
///   - Enrichit le contexte de la requête
///
/// Si le header est absent ou est une clé provider directe (pas sk-pylos-*),
/// on laisse passer (mode non-gouverné)
pub async fn virtual_key_middleware(
    State(state): State<AppState>,
    mut req: Request<Body>,
    next: Next,
) -> Response {
    // Extraction du Bearer token depuis Authorization header
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));

    match auth_header {
        Some(token) if token.starts_with(VIRTUAL_KEY_PREFIX) => {
            // 1. Check in the database for absolute configuration freshness
            let db_vk = match state.vk_store.get_key_by_value(token).await {
                Ok(Some(vk_cfg)) => {
                    if !vk_cfg.is_active {
                        // Deactivated in DB -> ensure it is removed from memory
                        state.vk_registry.deregister(token).await;
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(json!({
                                "error": {
                                    "message": "Virtual key is inactive",
                                    "type": "governance_error",
                                    "code": 401
                                }
                            })),
                        )
                            .into_response();
                    }
                    Some(vk_cfg)
                }
                Ok(None) => {
                    // Deleted in DB -> ensure it is removed from memory only if not in static config
                    let cfg = state.config_store.get().await;
                    let is_in_static = cfg.governance.virtual_keys.iter().any(|v| {
                        v.value.as_ref().and_then(|val| val.resolve()).as_deref() == Some(token)
                    });
                    if !is_in_static {
                        state.vk_registry.deregister(token).await;
                    }
                    None
                }
                Err(_) => {
                    // Fallback to memory registry if DB query fails
                    None
                }
            };

            // 2. If active key is found in DB or config, register/update it in the in-memory registry
            let provider_configs = if let Some(ref vk_cfg) = db_vk {
                let cfg = state.config_store.get().await;
                let rate_limit = cfg
                    .governance
                    .rate_limits
                    .iter()
                    .find(|rl| Some(&rl.id) == vk_cfg.rate_limit_id.as_ref())
                    .map(|rl| rl.request_max_limit)
                    .unwrap_or(0);

                let v_key = pylos_core::domain::virtual_key::VirtualKey::new(
                    token.to_string(),
                    &vk_cfg.name,
                )
                .with_rpm(rate_limit);
                state.vk_registry.register(v_key).await;
                vk_cfg.provider_configs.clone()
            } else {
                let cfg = state.config_store.get().await;
                let static_vk = cfg.governance.virtual_keys.iter().find(|v| {
                    v.value.as_ref().and_then(|val| val.resolve()).as_deref() == Some(token)
                });

                if let Some(vk_cfg) = static_vk {
                    if !vk_cfg.is_active {
                        state.vk_registry.deregister(token).await;
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(json!({
                                "error": {
                                    "message": "Virtual key is inactive",
                                    "type": "governance_error",
                                    "code": 401
                                }
                            })),
                        )
                            .into_response();
                    }

                    let rate_limit = cfg
                        .governance
                        .rate_limits
                        .iter()
                        .find(|rl| Some(&rl.id) == vk_cfg.rate_limit_id.as_ref())
                        .map(|rl| rl.request_max_limit)
                        .unwrap_or(0);

                    let v_key = pylos_core::domain::virtual_key::VirtualKey::new(
                        token.to_string(),
                        &vk_cfg.name,
                    )
                    .with_rpm(rate_limit);
                    state.vk_registry.register(v_key).await;
                    vk_cfg.provider_configs.clone()
                } else {
                    Vec::new()
                }
            };

            // 3. Verify in memory registry (for RPM rate limiting check & increment)
            match state.vk_registry.check_and_increment(token).await {
                Ok(vk) => {
                    // Inject virtual key info into request extensions
                    req.extensions_mut().insert(Some(VirtualKeyInfo {
                        name: vk.name.clone(),
                        key: vk.key.clone(),
                        provider_configs,
                    }));
                    next.run(req).await
                }
                Err(reason) => {
                    let status = if reason.contains("Rate limit") {
                        StatusCode::TOO_MANY_REQUESTS
                    } else if reason.contains("not found") {
                        StatusCode::UNAUTHORIZED
                    } else {
                        StatusCode::FORBIDDEN
                    };

                    (
                        status,
                        Json(json!({
                            "error": {
                                        "message": reason,
                                        "type": "governance_error",
                                        "code": status.as_u16()
                            }
                        })),
                    )
                        .into_response()
                }
            }
        }
        _ => {
            // No virtual key or direct provider key -> insert None for extractor
            req.extensions_mut().insert(None::<VirtualKeyInfo>);
            next.run(req).await
        }
    }
}

/// Information sur la Virtual Key injectée dans les extensions de requête
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct VirtualKeyInfo {
    pub name: String,
    pub key: String,
    pub provider_configs: Vec<pylos_core::domain::config::VkProviderConfig>,
}
