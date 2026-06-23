use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;

use pylos_application::{ModelHealth, ModelInfo};
use pylos_core::domain::embedding::EmbeddingRequest;
use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};
use pylos_core::domain::request::PylosRequest;

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ModelsQuery {
    /// Filtre par provider (optionnel)
    pub provider: Option<String>,
    #[serde(default)]
    pub include_deprecated: bool,
}

/// GET /v1/models — liste tous les modèles disponibles
/// Si ?provider=X est donné, retourne tous les modèles du catalog pour ce provider.
/// Sinon, retourne uniquement les modèles des providers configurés.
fn matches_pattern(pattern: &str, provider: &str, model_id: &str) -> bool {
    if pattern == "*" || pattern == "*::*" || pattern == "*/*" {
        return true;
    }
    let parts = if let Some((p, m)) = pattern.split_once("::") {
        Some((p, m))
    } else {
        pattern.split_once('/')
    };
    if let Some((pat_prov, pat_mod)) = parts {
        let prov_match = pat_prov == "*" || pat_prov == provider;
        let mod_match = pat_mod == "*"
            || pat_mod == model_id
            || (pat_mod.starts_with('*') && pat_mod.ends_with('*') && {
                let infix = &pat_mod[1..pat_mod.len() - 1];
                model_id.contains(infix)
            })
            || (pat_mod.starts_with('*') && model_id.ends_with(&pat_mod[1..]))
            || (pat_mod.ends_with('*') && model_id.starts_with(pat_mod.trim_end_matches('*')));
        prov_match && mod_match
    } else {
        pattern == model_id
            || (pattern.starts_with('*') && model_id.ends_with(&pattern[1..]))
            || (pattern.ends_with('*') && model_id.starts_with(pattern.trim_end_matches('*')))
    }
}

/// GET /v1/models — liste tous les modèles disponibles
/// Si ?provider=X est donné, retourne tous les modèles du catalog pour ce provider.
/// Sinon, retourne uniquement les modèles des providers configurés.
pub async fn list_models(
    State(state): State<AppState>,
    Extension(vk_info): Extension<Option<crate::middleware::virtual_key::VirtualKeyInfo>>,
    Extension(user): Extension<Option<pylos_core::domain::organization::InternalUser>>,
    Query(query): Query<ModelsQuery>,
) -> impl IntoResponse {
    // Extrait les patterns de modèles autorisés
    let mut allowed_model_patterns = None;

    if let Some(vk) = &vk_info {
        let mut patterns = Vec::new();
        for pc in &vk.provider_configs {
            for m in &pc.allowed_models {
                patterns.push(format!("{}::{}", pc.provider, m));
            }
        }
        if !patterns.is_empty() {
            allowed_model_patterns = Some(patterns);
        }
    } else if let Some(u) = &user {
        if u.role != "admin" {
            let mut patterns = Vec::new();
            if let Ok(groups) = state.org_store.list_access_groups(None).await {
                for ag in groups {
                    if ag.is_active {
                        let user_match = ag.user_ids.contains(&u.id);
                        let team_match = ag.team_ids.iter().any(|t_id| u.team_ids.contains(t_id));
                        if user_match || team_match {
                            for m in &ag.model_ids {
                                patterns.push(m.clone());
                            }
                        }
                    }
                }
            }
            if !patterns.is_empty() {
                allowed_model_patterns = Some(patterns);
            }
        }
    }

    // Filtre par provider direct depuis le catalog
    if let Some(ref prov) = query.provider {
        let catalog_models = state
            .model_catalog
            .list_models(Some(prov), query.include_deprecated)
            .await;
        let mut data: Vec<_> = catalog_models.iter().map(model_info_to_entry).collect();
        if let Some(patterns) = &allowed_model_patterns {
            data.retain(|m| {
                let provider = m["provider"].as_str().unwrap_or("");
                let model_id = m["id"].as_str().unwrap_or("");
                patterns
                    .iter()
                    .any(|pat| matches_pattern(pat, provider, model_id))
            });
        }
        return Json(json!({ "object": "list", "data": data }));
    }

    let cfg = state.config_store.get().await;
    let mut models = Vec::new();

    for (provider_name, provider_cfg) in &cfg.providers {
        // ── Ollama : interrogation en direct ─────────────────────────────────
        if provider_name == "ollama-jo3" {
            if let Some(base_url) = &provider_cfg.network.base_url {
                let tags_url = base_url.trim_end_matches("/v1").to_string() + "/api/tags";
                if let Ok(resp) = reqwest::get(&tags_url).await {
                    if let Ok(body) = resp.json::<serde_json::Value>().await {
                        if let Some(ollama_models) = body["models"].as_array() {
                            for m in ollama_models {
                                let name = m["name"].as_str().unwrap_or("");
                                let family = m["details"]["family"].as_str().unwrap_or("unknown");
                                let size = m["details"]["parameter_size"].as_str().unwrap_or("");
                                let info = state.model_catalog.get_model("ollama-jo3", name).await;
                                let pylos_field =
                                    info.as_ref().map(model_info_pylos_field).unwrap_or_else(
                                        || make_minimal_pylos("ollama-jo3", name, None),
                                    );
                                models.push(json!({
                                    "id": name,
                                    "provider": "ollama-jo3",
                                    "object": "model",
                                    "owned_by": "ollama-jo3",
                                    "details": { "family": family, "parameter_size": size },
                                    "pylos": pylos_field,
                                }));
                            }
                            continue;
                        }
                    }
                }
            }
            // Fallback catalog
            let catalog_models = state
                .model_catalog
                .list_models(Some("ollama-jo3"), false)
                .await;
            for info in catalog_models {
                models.push(model_info_to_entry(&info));
            }
            continue;
        }

        // ── Autres providers ──────────────────────────────────────────────────
        let mut provider_model_ids: Vec<String> = Vec::new();
        let mut wildcard = false;

        for key in &provider_cfg.keys {
            for model in &key.models {
                if model == "*" {
                    wildcard = true;
                    break;
                }
                if !provider_model_ids.contains(model) {
                    provider_model_ids.push(model.clone());
                }
            }
            if wildcard {
                break;
            }
        }

        if wildcard {
            let mut fetched_dynamic = false;
            if let Some(base_url) = &provider_cfg.network.base_url {
                let is_custom_or_lemonade = provider_name == "lemonade"
                    || ![
                        "openai",
                        "anthropic",
                        "gemini",
                        "google",
                        "cohere",
                        "groq",
                        "mistral",
                        "cerebras",
                        "perplexity",
                        "fireworks",
                        "xai",
                        "x-ai",
                        "nebius",
                        "deepseek",
                        "bedrock",
                        "azure",
                        "ollama-jo3",
                    ]
                    .contains(&provider_name.as_str());

                if is_custom_or_lemonade {
                    let client = reqwest::Client::builder()
                        .timeout(std::time::Duration::from_secs(3))
                        .build()
                        .unwrap_or_default();
                    let url = format!("{}/models", base_url.trim_end_matches('/'));
                    let mut req = client.get(&url);
                    if let Some(api_key) = provider_cfg.keys.first().and_then(|k| k.value.resolve())
                    {
                        if !api_key.is_empty() {
                            req = req.bearer_auth(api_key);
                        }
                    }
                    if let Ok(resp) = req.send().await {
                        if resp.status().is_success() {
                            if let Ok(body) = resp.json::<serde_json::Value>().await {
                                if let Some(data_array) = body["data"].as_array() {
                                    for m in data_array {
                                        if let Some(id) = m["id"].as_str() {
                                            let info = state
                                                .model_catalog
                                                .get_model(provider_name, id)
                                                .await;
                                            let pylos_field = info
                                                .as_ref()
                                                .map(model_info_pylos_field)
                                                .unwrap_or_else(|| {
                                                    let mut min_p =
                                                        make_minimal_pylos(provider_name, id, None);
                                                    if id.to_lowercase().contains("embed") {
                                                        min_p["supports_embeddings"] =
                                                            serde_json::json!(true);
                                                    }
                                                    min_p
                                                });
                                            models.push(json!({
                                                "id": id,
                                                "provider": provider_name,
                                                "object": "model",
                                                "owned_by": provider_name,
                                                "pylos": pylos_field,
                                            }));
                                        }
                                    }
                                    fetched_dynamic = true;
                                }
                            }
                        }
                    }
                }
            }

            if !fetched_dynamic {
                let catalog_models = state
                    .model_catalog
                    .list_models(Some(provider_name), false)
                    .await;
                for info in catalog_models {
                    models.push(model_info_to_entry(&info));
                }
            }
        } else {
            for model_id in &provider_model_ids {
                let info = state.model_catalog.get_model(provider_name, model_id).await;
                let entry = if let Some(ref info) = info {
                    model_info_to_entry(info)
                } else {
                    json!({
                        "id": model_id,
                        "provider": provider_name,
                        "object": "model",
                        "owned_by": provider_name,
                        "pylos": make_minimal_pylos(provider_name, model_id, None),
                    })
                };
                models.push(entry);
            }
        }
    }

    if let Some(patterns) = &allowed_model_patterns {
        models.retain(|m| {
            let provider = m["provider"].as_str().unwrap_or("");
            let model_id = m["id"].as_str().unwrap_or("");
            patterns
                .iter()
                .any(|pat| matches_pattern(pat, provider, model_id))
        });
    }

    models.sort_by(|a, b| {
        let pa = a["provider"].as_str().unwrap_or("");
        let pb = b["provider"].as_str().unwrap_or("");
        pa.cmp(pb).then(
            a["id"]
                .as_str()
                .unwrap_or("")
                .cmp(b["id"].as_str().unwrap_or("")),
        )
    });
    models.dedup_by(|a, b| a["provider"] == b["provider"] && a["id"] == b["id"]);

    Json(json!({ "object": "list", "data": models }))
}

/// Construit l'objet `pylos` avec la structure complète ModelInfo attendue par le TS
fn model_info_pylos_field(info: &pylos_application::ModelInfo) -> serde_json::Value {
    json!({
        "id": info.id,
        "provider": info.provider,
        "model_id": info.model_id,
        "display_name": info.display_name,
        "context_window": info.context_window,
        "max_output_tokens": info.max_output_tokens,
        "input_price_per_1m_usd": info.input_price_per_1m_usd,
        "output_price_per_1m_usd": info.output_price_per_1m_usd,
        "supports_vision": info.supports_vision,
        "supports_tools": info.supports_tools,
        "supports_streaming": info.supports_streaming,
        "supports_embeddings": info.supports_embeddings,
        "is_deprecated": info.is_deprecated,
        "enabled": info.enabled,
        "api_base": info.api_base,
        "tpm": info.tpm,
        "rpm": info.rpm,
        "max_retries": info.max_retries,
        "timeout_secs": info.timeout_secs,
        "stream_timeout_secs": info.stream_timeout_secs,
        "model_access_groups": info.model_access_groups,
        "guardrails": info.guardrails,
        "tags": info.tags,
        "provider_params": info.provider_params,
        "organization_id": info.organization_id,
    })
}

/// Construit un `pylos` minimal pour les modèles sans entrée dans le catalog
fn make_minimal_pylos(
    provider: &str,
    model_id: &str,
    display_name: Option<&str>,
) -> serde_json::Value {
    json!({
        "id": format!("{}/{}", provider, model_id),
        "provider": provider,
        "model_id": model_id,
        "display_name": display_name,
        "context_window": 0,
        "max_output_tokens": 0,
        "input_price_per_1m_usd": 0.0,
        "output_price_per_1m_usd": 0.0,
        "supports_vision": false,
        "supports_tools": true,
        "supports_streaming": true,
        "supports_embeddings": false,
        "is_deprecated": false,
        "enabled": true,
        "api_base": null,
        "tpm": null,
        "rpm": null,
        "max_retries": null,
        "timeout_secs": null,
        "stream_timeout_secs": null,
        "model_access_groups": null,
        "guardrails": null,
        "tags": null,
        "provider_params": null,
        "organization_id": null,
    })
}

/// Convertit un ModelInfo en entrée complète pour l'API /v1/models
fn model_info_to_entry(info: &pylos_application::ModelInfo) -> serde_json::Value {
    json!({
        "id": info.model_id,
        "provider": info.provider,
        "object": "model",
        "owned_by": info.provider,
        "pylos": model_info_pylos_field(info),
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// POST /v1/models/catalog — upsert un modèle custom dans le catalog
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct UpsertModelRequest {
    pub provider: String,
    pub model_id: String,
    pub display_name: Option<String>,
    #[serde(default)]
    pub context_window: u32,
    #[serde(default)]
    pub max_output_tokens: u32,
    #[serde(default)]
    pub input_price_per_1m_usd: f64,
    #[serde(default)]
    pub output_price_per_1m_usd: f64,
    #[serde(default)]
    pub supports_vision: bool,
    #[serde(default = "default_true")]
    pub supports_tools: bool,
    #[serde(default = "default_true")]
    pub supports_streaming: bool,
    #[serde(default)]
    pub supports_embeddings: bool,
    #[serde(default)]
    pub is_deprecated: bool,
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub api_base: Option<String>,
    #[serde(default)]
    pub tpm: Option<u32>,
    #[serde(default)]
    pub rpm: Option<u32>,
    #[serde(default)]
    pub max_retries: Option<u32>,
    #[serde(default)]
    pub timeout_secs: Option<u32>,
    #[serde(default)]
    pub stream_timeout_secs: Option<u32>,
    #[serde(default)]
    pub model_access_groups: Option<Vec<String>>,
    #[serde(default)]
    pub guardrails: Option<Vec<String>>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub provider_params: Option<serde_json::Value>,
    #[serde(default)]
    pub organization_id: Option<String>,
}

fn default_true() -> bool {
    true
}

pub async fn upsert_catalog_model(
    State(state): State<AppState>,
    Json(req): Json<UpsertModelRequest>,
) -> impl IntoResponse {
    let info = ModelInfo {
        id: format!("{}/{}", req.provider, req.model_id),
        provider: req.provider.clone(),
        model_id: req.model_id.clone(),
        display_name: req.display_name,
        context_window: req.context_window,
        max_output_tokens: req.max_output_tokens,
        input_price_per_1m_usd: req.input_price_per_1m_usd,
        output_price_per_1m_usd: req.output_price_per_1m_usd,
        supports_vision: req.supports_vision,
        supports_tools: req.supports_tools,
        supports_streaming: req.supports_streaming,
        supports_embeddings: req.supports_embeddings,
        is_deprecated: req.is_deprecated,
        enabled: req.enabled,
        api_base: req.api_base,
        tpm: req.tpm,
        rpm: req.rpm,
        max_retries: req.max_retries,
        timeout_secs: req.timeout_secs,
        stream_timeout_secs: req.stream_timeout_secs,
        model_access_groups: req.model_access_groups,
        guardrails: req.guardrails,
        tags: req.tags,
        provider_params: req.provider_params,
        organization_id: req.organization_id,
    };

    match state.model_catalog.upsert_model(&info).await {
        Ok(()) => (
            StatusCode::OK,
            Json(json!({
                "message": format!("Model '{}/{}' upserted in catalog", req.provider, req.model_id),
                "model": model_info_pylos_field(&info),
            })),
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
// DELETE /v1/models/catalog/:provider/:model_id — supprime un modèle du catalog
// ─────────────────────────────────────────────────────────────────────────────

pub async fn delete_catalog_model(
    State(state): State<AppState>,
    Path((provider, model_id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.model_catalog.delete_model(&provider, &model_id).await {
        Ok(true) => Json(json!({
            "message": format!("Model '{}/{}' removed from catalog", provider, model_id)
        }))
        .into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": format!("Model '{}/{}' not found in catalog", provider, model_id) })),
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
// POST /v1/models/pull/:provider — synchronise la liste des modèles du provider
// ─────────────────────────────────────────────────────────────────────────────

pub async fn pull_provider_models(
    State(state): State<AppState>,
    Path(provider_name): Path<String>,
) -> impl IntoResponse {
    let cfg = state.config_store.get().await;
    let provider_cfg = match cfg.providers.get(&provider_name) {
        Some(c) => c,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(
                    json!({ "error": format!("Provider '{}' not found in config", provider_name) }),
                ),
            )
                .into_response();
        }
    };

    let mut model_ids = Vec::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    if provider_name == "ollama-jo3" || provider_name.contains("ollama") {
        if let Some(base_url) = &provider_cfg.network.base_url {
            let tags_url = base_url.trim_end_matches("/v1").to_string() + "/api/tags";
            if let Ok(resp) = client.get(&tags_url).send().await {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(arr) = body["models"].as_array() {
                        for m in arr {
                            if let Some(name) = m["name"].as_str() {
                                model_ids.push(name.to_string());
                            }
                        }
                    }
                }
            }
        }
    } else if provider_name == "gemini" || provider_name == "google" {
        if let Some(api_key) = provider_cfg.keys.first().and_then(|k| k.value.resolve()) {
            let url = format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                api_key
            );
            if let Ok(resp) = client.get(&url).send().await {
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(arr) = body["models"].as_array() {
                        for m in arr {
                            if let Some(name) = m["name"].as_str() {
                                let id = name.strip_prefix("models/").unwrap_or(name);
                                model_ids.push(id.to_string());
                            }
                        }
                    }
                }
            }
        }
    } else if provider_name == "anthropic" {
        let base_url = provider_cfg
            .network
            .base_url
            .clone()
            .unwrap_or_else(|| "https://api.anthropic.com/v1".to_string());
        let url = format!("{}/models", base_url.trim_end_matches('/'));
        let mut req = client.get(&url);
        if let Some(api_key) = provider_cfg.keys.first().and_then(|k| k.value.resolve()) {
            if !api_key.is_empty() {
                req = req
                    .header("x-api-key", api_key)
                    .header("anthropic-version", "2023-06-01");
            }
        }
        if let Ok(resp) = req.send().await {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = body["data"].as_array() {
                    for m in arr {
                        if let Some(id) = m["id"].as_str() {
                            model_ids.push(id.to_string());
                        }
                    }
                }
            }
        }
    } else {
        let base_url = provider_cfg
            .network
            .base_url
            .clone()
            .unwrap_or_else(|| match provider_name.as_str() {
                "openai" => "https://api.openai.com/v1".to_string(),
                "groq" => "https://api.groq.com/openai/v1".to_string(),
                "mistral" => "https://api.mistral.ai/v1".to_string(),
                "cohere" => "https://api.cohere.com/v1".to_string(),
                "deepseek" => "https://api.deepseek.com/v1".to_string(),
                _ => "https://api.openai.com/v1".to_string(),
            });
        let url = format!("{}/models", base_url.trim_end_matches('/'));
        let mut req = client.get(&url);
        if let Some(api_key) = provider_cfg.keys.first().and_then(|k| k.value.resolve()) {
            if !api_key.is_empty() {
                req = req.bearer_auth(api_key);
            }
        }
        if let Ok(resp) = req.send().await {
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = body["data"].as_array() {
                    for m in arr {
                        if let Some(id) = m["id"].as_str() {
                            model_ids.push(id.to_string());
                        }
                    }
                } else if let Some(arr) = body["models"].as_array() {
                    for m in arr {
                        if let Some(id) = m["name"].as_str().or_else(|| m["id"].as_str()) {
                            model_ids.push(id.to_string());
                        }
                    }
                }
            }
        }
    }

    if model_ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "No models could be retrieved from the provider. Check configuration/API keys." })),
        )
            .into_response();
    }

    let mut upserted_count = 0;
    for model_id in &model_ids {
        let exists = state
            .model_catalog
            .get_model(&provider_name, model_id)
            .await;
        if exists.is_none() {
            let is_embed = model_id.to_lowercase().contains("embed");
            let is_vision = model_id.to_lowercase().contains("vision")
                || model_id.to_lowercase().contains("vl");
            let info = ModelInfo {
                id: format!("{}/{}", provider_name, model_id),
                provider: provider_name.clone(),
                model_id: model_id.clone(),
                display_name: Some(model_id.clone()),
                context_window: if is_embed { 8192 } else { 128_000 },
                max_output_tokens: if is_embed { 0 } else { 4096 },
                input_price_per_1m_usd: 0.0,
                output_price_per_1m_usd: 0.0,
                supports_vision: is_vision,
                supports_tools: !is_embed,
                supports_streaming: !is_embed,
                supports_embeddings: is_embed,
                is_deprecated: false,
                enabled: true,
                api_base: None,
                tpm: None,
                rpm: None,
                max_retries: None,
                timeout_secs: None,
                stream_timeout_secs: None,
                model_access_groups: None,
                guardrails: None,
                tags: None,
                provider_params: None,
                organization_id: None,
            };
            if state.model_catalog.upsert_model(&info).await.is_ok() {
                upserted_count += 1;
            }
        }
    }

    Json(json!({
        "success": true,
        "message": format!("Successfully pulled {} models, added {} new ones to the catalog.", model_ids.len(), upserted_count),
        "total_fetched": model_ids.len(),
        "new_added": upserted_count,
    }))
    .into_response()
}

pub async fn get_pricing_status(State(state): State<AppState>) -> impl IntoResponse {
    match state.model_catalog.get_pricing_status().await {
        Ok(status) => Json(status).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SchedulePricingRequest {
    pub schedule: Option<String>,
}

pub async fn schedule_pricing_reload(
    State(state): State<AppState>,
    Json(req): Json<SchedulePricingRequest>,
) -> impl IntoResponse {
    let mut current_status = match state.model_catalog.get_pricing_status().await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };
    current_status.periodic_schedule = req.schedule.clone();
    match state
        .model_catalog
        .update_pricing_status(
            &current_status.source_url,
            current_status.last_reload_ms,
            current_status.models_count,
            current_status.periodic_schedule.as_deref(),
        )
        .await
    {
        Ok(()) => Json(json!({ "success": true, "status": current_status })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn reload_pricing_data(State(state): State<AppState>) -> impl IntoResponse {
    let mut current_status = match state.model_catalog.get_pricing_status().await {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .unwrap_or_default();

    let resp = match client.get(&current_status.source_url).send().await {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": format!("Failed to fetch pricing data: {}", e) })),
            )
                .into_response();
        }
    };

    let body = match resp.json::<serde_json::Value>().await {
        Ok(b) => b,
        Err(e) => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": format!("Failed to parse JSON body: {}", e) })),
            )
                .into_response();
        }
    };

    let obj = match body.as_object() {
        Some(o) => o,
        None => {
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": "JSON root is not an object" })),
            )
                .into_response();
        }
    };

    let mut upserted_count = 0;

    for (key, val) in obj {
        if key == "sample_spec" {
            continue;
        }
        let litellm_provider = val["litellm_provider"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let model_id = if key.starts_with(&format!("{}/", litellm_provider)) {
            key.strip_prefix(&format!("{}/", litellm_provider))
                .unwrap()
                .to_string()
        } else {
            key.to_string()
        };

        let id = format!("{}/{}", litellm_provider, model_id);

        let mode = val["mode"].as_str().unwrap_or("chat");
        let is_embed = mode == "embedding";

        let input_cost = val["input_cost_per_token"].as_f64().unwrap_or(0.0) * 1_000_000.0;
        let output_cost = val["output_cost_per_token"].as_f64().unwrap_or(0.0) * 1_000_000.0;

        let max_input_tokens = val["max_input_tokens"]
            .as_u64()
            .or_else(|| val["max_tokens"].as_u64())
            .unwrap_or(0) as u32;

        let max_output_tokens = val["max_output_tokens"]
            .as_u64()
            .or_else(|| val["max_tokens"].as_u64())
            .unwrap_or(0) as u32;

        let supports_vision = val["supports_vision"].as_bool().unwrap_or(false);
        let supports_tools = val["supports_function_calling"]
            .as_bool()
            .or_else(|| val["supports_parallel_function_calling"].as_bool())
            .unwrap_or(!is_embed);

        let info = ModelInfo {
            id,
            provider: litellm_provider,
            model_id: model_id.clone(),
            display_name: Some(model_id),
            context_window: max_input_tokens,
            max_output_tokens,
            input_price_per_1m_usd: input_cost,
            output_price_per_1m_usd: output_cost,
            supports_vision,
            supports_tools,
            supports_streaming: !is_embed,
            supports_embeddings: is_embed,
            is_deprecated: false,
            enabled: true,
            api_base: None,
            tpm: None,
            rpm: None,
            max_retries: None,
            timeout_secs: None,
            stream_timeout_secs: None,
            model_access_groups: None,
            guardrails: None,
            tags: None,
            provider_params: None,
            organization_id: None,
        };

        if state.model_catalog.upsert_model(&info).await.is_ok() {
            upserted_count += 1;
        }
    }

    let now = pylos_application::log_store::now_ms();
    current_status.last_reload_ms = Some(now);
    current_status.models_count = upserted_count;

    let _ = state
        .model_catalog
        .update_pricing_status(
            &current_status.source_url,
            current_status.last_reload_ms,
            current_status.models_count,
            current_status.periodic_schedule.as_deref(),
        )
        .await;

    Json(json!({
        "success": true,
        "message": format!("Successfully loaded {} models.", upserted_count),
        "status": current_status
    }))
    .into_response()
}

pub async fn list_models_health(State(state): State<AppState>) -> impl IntoResponse {
    match state.model_catalog.list_model_health().await {
        Ok(health) => Json(health).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ModelHealthCheckRequest {
    pub provider: String,
    pub model_id: String,
}

pub async fn run_model_health_check(
    State(state): State<AppState>,
    Json(req): Json<ModelHealthCheckRequest>,
) -> impl IntoResponse {
    let health = execute_single_health_check(&state, &req.provider, &req.model_id).await;
    match state.model_catalog.update_model_health(&health).await {
        Ok(()) => Json(health).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn run_all_models_health_check(State(state): State<AppState>) -> impl IntoResponse {
    let catalog_models = state.model_catalog.list_models(None, false).await;
    let handles: Vec<_> = catalog_models
        .into_iter()
        .map(|model| {
            let state = state.clone();
            tokio::spawn(async move {
                tokio::time::timeout(
                    std::time::Duration::from_secs(15),
                    execute_single_health_check(&state, &model.provider, &model.model_id),
                )
                .await
                .unwrap_or_else(|_| ModelHealth {
                    id: format!("{}/{}", model.provider, model.model_id),
                    provider: model.provider,
                    model_id: model.model_id,
                    health_status: "unhealthy".to_string(),
                    error_details: Some("Health check timed out after 15s".to_string()),
                    last_check_ms: Some(pylos_application::log_store::now_ms()),
                    last_success_ms: None,
                })
            })
        })
        .collect();

    let mut results = Vec::new();
    for handle in handles {
        if let Ok(health) = handle.await {
            let _ = state.model_catalog.update_model_health(&health).await;
            results.push(health);
        }
    }
    Json(results).into_response()
}

async fn execute_single_health_check(
    state: &AppState,
    provider_name: &str,
    model_id: &str,
) -> ModelHealth {
    let now = pylos_application::log_store::now_ms();
    let id = format!("{}/{}", provider_name, model_id);

    let providers_arc = state.orchestrator.providers();
    let providers = providers_arc.read().await;
    let found = providers
        .iter()
        .find(|(provider, _)| provider.name() == provider_name);

    let (provider, config) = match found {
        Some((p, c)) => (p, c),
        None => {
            return ModelHealth {
                id,
                provider: provider_name.to_string(),
                model_id: model_id.to_string(),
                health_status: "unhealthy".to_string(),
                error_details: Some(format!(
                    "Provider '{}' not configured or active",
                    provider_name
                )),
                last_check_ms: Some(now),
                last_success_ms: None,
            };
        }
    };

    let is_embed = model_id.to_lowercase().contains("embed");
    let res = if is_embed {
        let req = EmbeddingRequest {
            model: model_id.to_string(),
            input: pylos_core::domain::embedding::EmbeddingInput::Single("ping".to_string()),
            dimensions: None,
            user: None,
            encoding_format: None,
        };
        provider.embed(&req, config).await.map(|_| ())
    } else {
        let req = ChatCompletionRequest {
            model: model_id.to_string(),
            messages: vec![ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("ping".to_string()),
                name: None,
                tool_calls: None,
                tool_call_id: None,
                reasoning_content: None,
            }],
            max_tokens: Some(5),
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            top_k: None,
            min_p: None,
            repetition_penalty: None,
            max_completion_tokens: None,
        };
        let request = PylosRequest::ChatCompletion(req);
        provider.complete(&request, config).await.map(|_| ())
    };

    match res {
        Ok(()) => ModelHealth {
            id,
            provider: provider_name.to_string(),
            model_id: model_id.to_string(),
            health_status: "healthy".to_string(),
            error_details: None,
            last_check_ms: Some(now),
            last_success_ms: Some(now),
        },
        Err(e) => {
            let mut last_success = None;
            if let Ok(health_list) = state.model_catalog.list_model_health().await {
                if let Some(h) = health_list.iter().find(|x| x.id == id) {
                    last_success = h.last_success_ms;
                }
            }

            ModelHealth {
                id,
                provider: provider_name.to_string(),
                model_id: model_id.to_string(),
                health_status: "unhealthy".to_string(),
                error_details: Some(e.to_string()),
                last_check_ms: Some(now),
                last_success_ms: last_success,
            }
        }
    }
}
