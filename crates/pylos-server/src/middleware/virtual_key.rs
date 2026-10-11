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

/// Horodatage courant en millisecondes (les `expires_at` sont stockes en ms epoch).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Une Virtual Key est expiree si son `expires_at` (ms epoch) est dans le passe.
///
/// `None`, `Some(0)` et les valeurs negatives sont traites comme "pas d'expiration" :
/// la couche de persistance relit une colonne NULL comme `0`, ce qui ne doit pas
/// invalider toutes les cles existantes.
fn is_expired(expires_at: Option<i64>) -> bool {
    matches!(expires_at, Some(ts) if ts > 0 && now_ms() > ts)
}

fn expired_response() -> axum::response::Response {
    (
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": {
                "message": "Virtual key is expired",
                "type": "governance_error",
                "code": 401
            }
        })),
    )
        .into_response()
}

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
    // Token possede : evite de garder un emprunt immuable de `req` pendant qu'on
    // enrichit ses extensions.
    let auth_token = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|value| value.to_string());

    match auth_token.as_deref() {
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
                    if is_expired(vk_cfg.expires_at) {
                        // Cle expiree -> retiree du registre memoire et refusee
                        state.vk_registry.deregister(token).await;
                        return expired_response();
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
            let (vk_id, vk_name, provider_configs) = if let Some(ref vk_cfg) = db_vk {
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
                (
                    vk_cfg.id.clone(),
                    vk_cfg.name.clone(),
                    vk_cfg.provider_configs.clone(),
                )
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

                    if is_expired(vk_cfg.expires_at) {
                        state.vk_registry.deregister(token).await;
                        return expired_response();
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
                    (
                        vk_cfg.id.clone(),
                        vk_cfg.name.clone(),
                        vk_cfg.provider_configs.clone(),
                    )
                } else {
                    (String::new(), String::new(), Vec::new())
                }
            };

            // 3. La cle doit exister (base de donnees ou configuration statique)
            if vk_id.is_empty() {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "error": {
                            "message": "Virtual key not found",
                            "type": "governance_error",
                            "code": 401
                        }
                    })),
                )
                    .into_response();
            }

            // Le quota de requetes (RPM/TPM) est applique par le RateLimitPlugin sur le
            // store persistant, partage entre repliques et conserve au redemarrage.
            // L'ancien compteur en memoire (par processus) est volontairement abandonne.
            req.extensions_mut().insert(Some(VirtualKeyInfo {
                id: vk_id,
                name: vk_name,
                key: token.to_string(),
                provider_configs,
            }));
            next.run(req).await
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
    /// Identifiant de la Virtual Key (clé de gouvernance : budgets, rate limits).
    pub id: String,
    pub name: String,
    pub key: String,
    pub provider_configs: Vec<pylos_core::domain::config::VkProviderConfig>,
}
