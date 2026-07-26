use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use pylos_core::domain::config::{EnvVar, ProviderConfig, VirtualKeyConfig, VkProviderConfig};

use crate::state::AppState;

// ─────────────────────────────────────────────────────────────────────────────
// GET /config
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config_store.get().await;
    let mut val = redact_config(&cfg);
    let env = std::env::var("PYLOS_ENV").unwrap_or_else(|_| "production".to_string());
    if let Some(obj) = val.as_object_mut() {
        obj.insert("environment".to_string(), serde_json::Value::String(env));
    }
    Json(val)
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /config/reload
// ─────────────────────────────────────────────────────────────────────────────

pub async fn reload_config(State(state): State<AppState>) -> impl IntoResponse {
    match state.config_store.reload().await {
        Ok(summary) => {
            // Propager les nouveaux providers à l'orchestrateur avec les données atomiques de ReloadSummary
            if summary.changed {
                state
                    .orchestrator
                    .update_providers(summary.runtime_providers)
                    .await;
            }

            Json(json!({
                "changed": summary.changed,
                "providers_reloaded": summary.providers_reloaded,
                "message": if summary.changed { "Config reloaded" } else { "Config unchanged" }
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /providers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn list_providers(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config_store.get().await;
    let providers: Vec<_> = cfg
        .providers
        .iter()
        .map(|(name, p)| {
            json!({
                "name": name,
                "keys_count": p.keys.len(),
                "keys": p.keys.iter().map(|k| json!({
                    "name": k.name,
                    "value": k.value.redacted(),
                    "models": k.models,
                    "weight": k.weight
                })).collect::<Vec<_>>(),
                "network": {
                    "base_url": p.network.base_url,
                    "timeout_secs": p.network.timeout_secs,
                    "max_retries": p.network.max_retries
                }
            })
        })
        .collect();

    Json(json!({ "providers": providers, "total": providers.len() }))
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /providers — crée un provider
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateProviderRequest {
    pub name: String,
    #[serde(flatten)]
    pub config: ProviderConfig,
}

pub async fn create_provider(
    State(state): State<AppState>,
    Json(req): Json<CreateProviderRequest>,
) -> impl IntoResponse {
    let name = req.name.clone();
    match state
        .config_store
        .upsert_provider(name.clone(), req.config)
        .await
    {
        Ok(()) => {
            let providers = state.config_store.runtime_providers().await;
            state.orchestrator.update_providers(providers).await;
            (
                StatusCode::CREATED,
                Json(
                    json!({ "message": format!("Provider '{}' created", name), "provider": name }),
                ),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PUT /providers/:name — met à jour un provider
// ─────────────────────────────────────────────────────────────────────────────

pub async fn upsert_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(provider): Json<ProviderConfig>,
) -> impl IntoResponse {
    match state
        .config_store
        .upsert_provider(name.clone(), provider)
        .await
    {
        Ok(()) => {
            let providers = state.config_store.runtime_providers().await;
            state.orchestrator.update_providers(providers).await;
            Json(json!({ "message": format!("Provider '{}' updated", name), "provider": name }))
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DELETE /providers/:name — supprime un provider
// ─────────────────────────────────────────────────────────────────────────────

pub async fn delete_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.config_store.remove_provider(&name).await {
        Ok(true) => {
            let providers = state.config_store.runtime_providers().await;
            state.orchestrator.update_providers(providers).await;
            Json(json!({ "message": format!("Provider '{}' deleted", name) })).into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Provider '{}' not found", name) })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /providers/:name/test — teste la connectivité d'un provider
// ─────────────────────────────────────────────────────────────────────────────

pub async fn test_provider(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    match state.orchestrator.test_provider(&name).await {
        Ok(()) => Json(
            json!({ "status": "success", "message": format!("Provider '{}' is available", name) }),
        )
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "status": "error", "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /virtual-keys
// ─────────────────────────────────────────────────────────────────────────────

pub async fn list_virtual_keys(State(state): State<AppState>) -> impl IntoResponse {
    let mut all_vks = vec![];

    let redact = |v: &Option<EnvVar>| v.as_ref().map(|v| v.redacted()).unwrap_or_default();

    let make_entry = |vk: &VirtualKeyConfig| {
        json!({
            "id": vk.id,
            "name": vk.name,
            "description": vk.description,
            "is_active": vk.is_active,
            "value": redact(&vk.value),
            "rate_limit_id": vk.rate_limit_id,
            "team_alias": vk.team_alias,
            "team_id": vk.team_id,
            "organization_id": vk.organization_id,
            "access_group_id": vk.access_group_id,
            "user_email": vk.user_email,
            "user_id": vk.user_id,
            "created_at": vk.created_at,
            "created_by": vk.created_by,
            "updated_at": vk.updated_at,
            "last_active": vk.last_active,
            "expires_at": vk.expires_at,
            "provider_configs": vk.provider_configs.iter().map(|p| json!({
                "provider": p.provider,
                "allowed_models": p.allowed_models,
                "weight": p.weight
            })).collect::<Vec<_>>()
        })
    };

    // 1. Clés statiques de la config pylos.json
    let cfg = state.config_store.get().await;
    for vk in &cfg.governance.virtual_keys {
        all_vks.push(make_entry(vk));
    }

    // 2. Clés dynamiques de la base de données
    if let Ok(db_vks) = state.vk_store.list_keys().await {
        for vk in &db_vks {
            if !all_vks
                .iter()
                .any(|v| v.get("id").and_then(|i| i.as_str()) == Some(&vk.id))
            {
                all_vks.push(make_entry(vk));
            }
        }
    }

    Json(json!({ "virtual_keys": all_vks, "total": all_vks.len() }))
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /virtual-keys — crée une virtual key
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct CreateVirtualKeyRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: Option<String>,
    pub value: Option<String>,
    #[serde(default = "default_true")]
    pub is_active: bool,
    #[serde(default)]
    pub provider_configs: Vec<VkProviderConfig>,
    pub rate_limit_id: Option<String>,
    #[serde(default)]
    pub team_alias: Option<String>,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub access_group_id: Option<String>,
    #[serde(default)]
    pub user_email: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

fn default_true() -> bool {
    true
}

pub async fn create_virtual_key(
    State(state): State<AppState>,
    Json(req): Json<CreateVirtualKeyRequest>,
) -> impl IntoResponse {
    let id = req
        .id
        .unwrap_or_else(|| format!("vk-{}", fastrand::u32(..)));
    let key_value = req
        .value
        .unwrap_or_else(|| format!("sk-pylos-{}", fastrand::u64(..)));

    let vk_cfg = VirtualKeyConfig {
        id: id.clone(),
        name: req.name.clone(),
        description: req.description,
        value: Some(EnvVar::Literal(key_value.clone())),
        is_active: req.is_active,
        rate_limit_id: req.rate_limit_id.clone(),
        provider_configs: req.provider_configs,
        team_alias: req.team_alias,
        team_id: req.team_id,
        organization_id: req.organization_id,
        access_group_id: req.access_group_id,
        user_email: req.user_email,
        user_id: req.user_id,
        created_at: None,
        created_by: req.created_by,
        updated_at: None,
        last_active: None,
        expires_at: req.expires_at,
    };

    match state.vk_store.upsert_key(&vk_cfg).await {
        Ok(()) => {
            // Résout le RPM depuis la config pour l'enregistrer correctement dans le registry
            let cfg = state.config_store.get().await;
            let rpm = req
                .rate_limit_id
                .as_ref()
                .and_then(|rl_id| cfg.governance.rate_limits.iter().find(|r| &r.id == rl_id))
                .map(|rl| rl.request_max_limit)
                .unwrap_or(0);

            let vk = pylos_core::domain::virtual_key::VirtualKey::new(key_value.clone(), &req.name)
                .with_rpm(rpm);
            state.vk_registry.register(vk).await;

            // Propage le rate limit au store persistant
            if let Some(rl_id) = &req.rate_limit_id {
                if let Some(rl_cfg) = cfg.governance.rate_limits.iter().find(|r| &r.id == rl_id) {
                    if let Err(e) = state.rate_limit_store.upsert_rate_limit(&id, rl_cfg).await {
                        tracing::warn!(vk_id = %id, error = %e, "Failed to sync rate limit store on VK creation");
                    }
                }
            }

            (
                StatusCode::CREATED,
                Json(json!({
                    "id": id,
                    "name": req.name,
                    "value": key_value,
                    "message": "Virtual key created"
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PUT /virtual-keys/:id — met à jour une virtual key
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateVirtualKeyRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub provider_configs: Option<Vec<VkProviderConfig>>,
    pub rate_limit_id: Option<String>,
    #[serde(default)]
    pub team_alias: Option<String>,
    #[serde(default)]
    pub team_id: Option<String>,
    #[serde(default)]
    pub organization_id: Option<String>,
    #[serde(default)]
    pub access_group_id: Option<String>,
    #[serde(default)]
    pub user_email: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub created_by: Option<String>,
    #[serde(default)]
    pub expires_at: Option<i64>,
}

pub async fn update_virtual_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateVirtualKeyRequest>,
) -> impl IntoResponse {
    let mut vk = match state.vk_store.get_key_by_id(&id).await {
        Ok(Some(v)) => v,
        _ => {
            let cfg = state.config_store.get().await;
            if let Some(static_vk) = cfg.governance.virtual_keys.iter().find(|v| v.id == id) {
                static_vk.clone()
            } else {
                return (
                    StatusCode::NOT_FOUND,
                    Json(json!({ "error": format!("Virtual key '{}' not found", id) })),
                )
                    .into_response();
            }
        }
    };

    let old_key_value = vk.value.as_ref().and_then(|v| v.resolve());

    if let Some(name) = req.name {
        vk.name = name;
    }
    if let Some(desc) = req.description {
        vk.description = Some(desc);
    }
    if let Some(active) = req.is_active {
        vk.is_active = active;
    }
    if let Some(pcs) = req.provider_configs {
        vk.provider_configs = pcs;
    }
    if req.rate_limit_id.is_some() {
        vk.rate_limit_id = req.rate_limit_id.clone();
    }
    if req.team_alias.is_some() {
        vk.team_alias = req.team_alias;
    }
    if req.team_id.is_some() {
        vk.team_id = req.team_id;
    }
    if req.organization_id.is_some() {
        vk.organization_id = req.organization_id;
    }
    if req.access_group_id.is_some() {
        vk.access_group_id = req.access_group_id;
    }
    if req.user_email.is_some() {
        vk.user_email = req.user_email;
    }
    if req.user_id.is_some() {
        vk.user_id = req.user_id;
    }
    if req.created_by.is_some() {
        vk.created_by = req.created_by;
    }
    if req.expires_at.is_some() {
        vk.expires_at = req.expires_at;
    }

    match state.vk_store.upsert_key(&vk).await {
        Ok(()) => {
            // Retire l'ancienne clé en mémoire si elle a changé ou est inactive
            if let Some(ref old_val) = old_key_value {
                state.vk_registry.deregister(old_val).await;
            }

            // Met à jour la clé en mémoire si elle est active
            if vk.is_active {
                let cfg = state.config_store.get().await;
                let rpm = vk
                    .rate_limit_id
                    .as_ref()
                    .and_then(|rl_id| cfg.governance.rate_limits.iter().find(|r| &r.id == rl_id))
                    .map(|rl| rl.request_max_limit)
                    .unwrap_or(0);

                if let Some(ref key_str) = vk.value.as_ref().and_then(|v| v.resolve()) {
                    let v_key =
                        pylos_core::domain::virtual_key::VirtualKey::new(key_str.clone(), &vk.name)
                            .with_rpm(rpm);
                    state.vk_registry.register(v_key).await;
                }
            }

            // Propage le nouveau rate_limit_id au store SQLite si modifié
            if let Some(rl_id) = &req.rate_limit_id {
                let cfg = state.config_store.get().await;
                if let Some(rl_cfg) = cfg.governance.rate_limits.iter().find(|r| &r.id == rl_id) {
                    if let Err(e) = state.rate_limit_store.upsert_rate_limit(&id, rl_cfg).await {
                        tracing::warn!(vk_id = %id, error = %e, "Failed to sync rate limit store on VK update");
                    }
                }
            }
            Json(json!({ "id": id, "message": "Virtual key updated" })).into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DELETE /virtual-keys/:id
// ─────────────────────────────────────────────────────────────────────────────

pub async fn delete_virtual_key(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    // Récupère la clé avant de la supprimer pour pouvoir la deregister de la mémoire
    let key_value = match state.vk_store.get_key_by_id(&id).await {
        Ok(Some(vk)) => vk.value.as_ref().and_then(|v| v.resolve()),
        _ => {
            let cfg = state.config_store.get().await;
            cfg.governance
                .virtual_keys
                .iter()
                .find(|v| v.id == id)
                .and_then(|vk| vk.value.as_ref().and_then(|v| v.resolve()))
        }
    };

    match state.vk_store.delete_key(&id).await {
        Ok(true) => {
            if let Some(ref val) = key_value {
                state.vk_registry.deregister(val).await;
            }

            // Nettoie les entrées orphelines dans les stores
            state.budget_store.delete_vk_entries(&id).await;
            state.rate_limit_store.delete_vk_entries(&id).await;

            Json(json!({ "message": format!("Virtual key '{}' deleted", id) })).into_response()
        }
        Ok(false) => {
            // Si présente uniquement en config statique (et qu'on ne l'a pas en DB), on la deregister quand même de la mémoire
            if let Some(ref val) = key_value {
                state.vk_registry.deregister(val).await;
            }
            Json(json!({ "message": format!("Virtual key '{}' deleted from memory", id) }))
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /virtual-keys/:id/budget — statut du budget et rate limits d'une VK
// ─────────────────────────────────────────────────────────────────────────────

pub async fn get_virtual_key_budget(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let budget = state.budget_store.get_usage(&id).await;
    let rate_limits = state.rate_limit_store.get_status(&id).await;

    Json(json!({
        "virtual_key_id": id,
        "budget": budget,
        "rate_limits": rate_limits,
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// GET /virtual-keys/:id/value — révèle la valeur claire d'une clé (RBAC)
// Protégé par management_auth_middleware (admin key ou JWT admin)
// ─────────────────────────────────────────────────────────────────────────────

pub async fn reveal_virtual_key_value(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.vk_store.reveal_key_value(&id).await {
        Ok(Some(plaintext)) => Json(json!({
            "id": id,
            "value": plaintext,
        }))
        .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Virtual key not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
// ─────────────────────────────────────────────────────────────────────────────

fn redact_config(cfg: &pylos_core::domain::config::PylosConfig) -> serde_json::Value {
    let mut v = serde_json::to_value(cfg).unwrap_or_default();

    if let Some(providers) = v.get_mut("providers").and_then(|p| p.as_object_mut()) {
        for provider in providers.values_mut() {
            if let Some(keys) = provider.get_mut("keys").and_then(|k| k.as_array_mut()) {
                for key in keys.iter_mut() {
                    if let Some(val) = key.get_mut("value") {
                        let raw = val.as_str().unwrap_or("").to_string();
                        let env_var = EnvVar::from(raw.as_str());
                        *val = serde_json::Value::String(env_var.redacted());
                    }
                }
            }
        }
    }

    v
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /api/github/promote
// ─────────────────────────────────────────────────────────────────────────────

pub async fn promote_to_prod_handler() -> impl IntoResponse {
    let gh_pat = std::env::var("GH_PAT");
    if gh_pat.is_err() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "GitHub promotion is not configured (GH_PAT not set)" })),
        )
            .into_response();
    }
    match promote_to_production().await {
        Ok(res) => Json(res).into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Failed to trigger promotion workflow" })),
        )
            .into_response(),
    }
}

pub async fn promote_to_production() -> Result<serde_json::Value, String> {
    let pat = std::env::var("GH_PAT").map_err(|_| "GH_PAT not set".to_string())?;
    let client = reqwest::Client::builder()
        .user_agent("Pylos-Dashboard/0.1.0")
        .build()
        .map_err(|e| e.to_string())?;

    let repo = "JZacharie/Pylos";
    let workflow_id = "promote.yml";

    let url = format!(
        "https://api.github.com/repos/{}/actions/workflows/{}/dispatches",
        repo, workflow_id
    );

    let payload = json!({
        "ref": "main"
    });

    let resp = client
        .post(&url)
        .bearer_auth(&pat)
        .header("Accept", "application/vnd.github.v3+json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Failed to trigger promotion workflow: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_body = resp.text().await.unwrap_or_default();
        return Err(format!("GitHub API error ({}): {}", status, error_body));
    }

    Ok(json!({
        "success": true,
        "message": "Promotion workflow (promote.yml) triggered successfully on branch main."
    }))
}

// ─────────────────────────────────────────────────────────────────────────────
// PUT /config/guardrails
// ─────────────────────────────────────────────────────────────────────────────
// ─────────────────────────────────────────────────────────────────────────────
// GET /providers/defaults — presets de configuration pour les providers courants
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct ProviderPreset {
    pub name: &'static str,
    pub label: &'static str,
    pub base_url: Option<&'static str>,
    pub env_var: Option<&'static str>,
    pub docs_url: Option<&'static str>,
    pub default_models: Vec<&'static str>,
}

pub async fn list_provider_defaults() -> impl IntoResponse {
    let presets = vec![
        ProviderPreset {
            name: "openai",
            label: "OpenAI",
            base_url: None,
            env_var: Some("OPENAI_API_KEY"),
            docs_url: Some("https://platform.openai.com/api-keys"),
            default_models: vec!["gpt-4o", "gpt-4o-mini"],
        },
        ProviderPreset {
            name: "anthropic",
            label: "Anthropic",
            base_url: None,
            env_var: Some("ANTHROPIC_API_KEY"),
            docs_url: Some("https://console.anthropic.com/settings/keys"),
            default_models: vec!["claude-sonnet-4-5", "claude-haiku-3-5"],
        },
        ProviderPreset {
            name: "ollama",
            label: "Ollama (Local)",
            base_url: Some("http://localhost:11434"),
            env_var: None,
            docs_url: Some("https://ollama.ai/download"),
            default_models: vec!["llama3.2:3b", "llama3.1:8b", "mistral:7b"],
        },
        ProviderPreset {
            name: "lemonade",
            label: "Lemonade",
            base_url: None,
            env_var: Some("LEMONADE_API_KEY"),
            docs_url: None,
            default_models: vec!["lemonade-optimus"],
        },
        ProviderPreset {
            name: "deepseek",
            label: "DeepSeek",
            base_url: Some("https://api.deepseek.com"),
            env_var: Some("DEEPSEEK_API_KEY"),
            docs_url: Some("https://platform.deepseek.com/api_keys"),
            default_models: vec!["deepseek-v4-pro", "deepseek-v4-flash"],
        },
        ProviderPreset {
            name: "gemini",
            label: "Google Gemini",
            base_url: Some("https://generativelanguage.googleapis.com/v1beta"),
            env_var: Some("GEMINI_API_KEY"),
            docs_url: Some("https://aistudio.google.com/apikey"),
            default_models: vec!["gemini-3.5-pro", "gemini-3.5-flash", "gemini-2.5-pro", "gemini-2.5-flash"],
        },
        ProviderPreset {
            name: "groq",
            label: "Groq",
            base_url: Some("https://api.groq.com/openai/v1"),
            env_var: Some("GROQ_API_KEY"),
            docs_url: Some("https://console.groq.com/keys"),
            default_models: vec!["llama-3.3-70b-versatile", "llama-3.1-8b-instant"],
        },
        ProviderPreset {
            name: "mistral",
            label: "Mistral AI",
            base_url: Some("https://api.mistral.ai/v1"),
            env_var: Some("MISTRAL_API_KEY"),
            docs_url: Some("https://console.mistral.ai/api-keys/"),
            default_models: vec!["mistral-large-latest", "mistral-small-latest"],
        },
        ProviderPreset {
            name: "openrouter",
            label: "OpenRouter",
            base_url: Some("https://openrouter.ai/api/v1"),
            env_var: Some("OPENROUTER_API_KEY"),
            docs_url: Some("https://openrouter.ai/keys"),
            default_models: vec![],
        },
    ];
    Json(json!({ "defaults": presets, "total": presets.len() }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateGuardrailsRequest {
    pub enabled: bool,
    pub config: serde_json::Value,
}

pub async fn update_guardrails(
    State(state): State<AppState>,
    Json(req): Json<UpdateGuardrailsRequest>,
) -> impl IntoResponse {
    match state
        .config_store
        .upsert_guardrails_config(req.enabled, req.config)
        .await
    {
        Ok(()) => {
            // Recharger la configuration pour propager les changements.
            let _ = state.config_store.reload().await;
            (
                StatusCode::OK,
                Json(json!({ "message": "Guardrails configuration updated successfully" })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
