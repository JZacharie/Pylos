use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use futures::StreamExt;
use pylos_core::domain::embedding::{EmbeddingRequest, EmbeddingResponse};
use pylos_core::domain::openai::{
    ChatCompletionChoice, ChatCompletionMessage, ChatCompletionRequest, ChatCompletionResponse,
    MessageRole, TextCompletionChoice, TextCompletionResponse,
};
use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext, StreamChunk};
use pylos_core::domain::traits::{ChunkStream, LlmPlugin, Provider};
use pylos_core::error::PylosError;

type ProviderList = Vec<(Arc<dyn Provider>, ProviderConfig)>;

const MAX_CONSECUTIVE_FAILURES: u32 = 5;
const COOLDOWN_SECS: u64 = 30;

#[derive(Debug, Clone, Default)]
struct CircuitBreakerState {
    consecutive_failures: u32,
    last_failure: Option<Instant>,
}

/// Compteur Round-Robin pour chaque modèle.
/// Permet de distribuer les requêtes séquentiellement entre les providers
/// qui supportent un modèle donné.
#[derive(Debug, Default)]
struct RoundRobinState {
    counters: Mutex<HashMap<String, usize>>,
}

impl RoundRobinState {
    fn select_and_advance(&self, model: &str, provider_count: usize) -> usize {
        let mut counters = self.counters.lock().unwrap_or_else(|e| e.into_inner());
        let entry = counters.entry(model.to_string()).or_insert(0);
        let current = *entry;
        *entry = (*entry + 1) % provider_count;
        current
    }
}

/// Trait permettant à weighted_shuffle d'extraire le poids d'un item.
trait HasWeight {
    fn weight(&self) -> f64;
}

impl HasWeight for (Arc<dyn Provider>, ProviderConfig, f64, bool) {
    fn weight(&self) -> f64 {
        self.2
    }
}

/// Orchestre une requête sur un ou plusieurs providers avec retry et fallback
/// Équivalent de core/inference.go (bifrost) — SendRequest / inferenceLoopHelper
pub struct InferenceOrchestrator {
    /// RwLock pour permettre le hot-reload des providers sans redémarrage
    /// Identique au mécanisme atomic.Pointer de bifrost pour les plugins
    providers: Arc<RwLock<ProviderList>>,
    plugins: Vec<Arc<dyn LlmPlugin>>,
    circuit_breakers: Arc<Mutex<HashMap<String, CircuitBreakerState>>>,
    round_robin: RoundRobinState,
}

impl InferenceOrchestrator {
    pub fn new(providers: ProviderList, mut plugins: Vec<Arc<dyn LlmPlugin>>) -> Self {
        // Tri déterministe par priorité explicite (plus petit = exécuté plus tôt en pre_hook,
        // plus tard en post_hook / post_stream_chunk).
        plugins.sort_by_key(|p| p.priority());
        Self {
            providers: Arc::new(RwLock::new(providers)),
            plugins,
            circuit_breakers: Arc::new(Mutex::new(HashMap::new())),
            round_robin: RoundRobinState::default(),
        }
    }

    pub fn providers(&self) -> Arc<RwLock<ProviderList>> {
        self.providers.clone()
    }

    /// Hot-reload des providers sans interruption de service
    pub async fn update_providers(&self, new_providers: ProviderList) {
        let mut guard = self.providers.write().await;
        *guard = new_providers;
        info!("Inference providers updated (hot-reload)");
    }

    /// Teste la connectivité avec un provider donné par son nom
    pub async fn test_provider(&self, name: &str) -> Result<(), PylosError> {
        let providers = self.providers.read().await;
        let found = providers
            .iter()
            .find(|(provider, _)| provider.name() == name);
        if let Some((provider, config)) = found {
            provider.health_check(config).await
        } else {
            Err(PylosError::NotFound(format!(
                "Provider '{}' not found",
                name
            )))
        }
    }

    fn is_circuit_breaker_open(&self, name: &str) -> bool {
        let breakers = self
            .circuit_breakers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(state) = breakers.get(name) {
            if state.consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                if let Some(last_failure) = state.last_failure {
                    if last_failure.elapsed().as_secs() < COOLDOWN_SECS {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn record_success(&self, name: &str) {
        let mut breakers = self
            .circuit_breakers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(state) = breakers.get_mut(name) {
            state.consecutive_failures = 0;
            state.last_failure = None;
        }
    }

    fn record_failure(&self, name: &str) {
        let mut breakers = self
            .circuit_breakers
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let state = breakers.entry(name.to_string()).or_default();
        state.consecutive_failures += 1;
        state.last_failure = Some(Instant::now());
    }

    /// Weighted random shuffle using the A-Res algorithm.
    /// Pre-computes random scores to ensure a deterministic total order.
    fn weighted_shuffle<T>(items: Vec<T>) -> Vec<T>
    where
        T: HasWeight,
    {
        let mut scored: Vec<_> = items
            .into_iter()
            .map(|item| {
                let weight = item.weight().max(0.0001);
                let score = fastrand::f64().powf(1.0 / weight);
                (item, score)
            })
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().map(|(item, _)| item).collect()
    }

    fn select_and_order_providers(
        &self,
        providers: &ProviderList,
        model: &str,
        ctx: &RequestContext,
    ) -> Vec<(Arc<dyn Provider>, ProviderConfig)> {
        // 1. Périmètre autorisé : les restrictions de la Virtual Key s'appliquent TOUJOURS.
        //    Les en-têtes x-pylos-force-* ne peuvent jamais élargir les droits d'une clé :
        //    le provider forcé est appliqué comme un filtre DANS ce périmètre.
        let mut allowed_providers: Vec<&(Arc<dyn Provider>, ProviderConfig)> =
            if ctx.virtual_key.is_some() {
                providers
                    .iter()
                    .filter(|(provider, _config)| {
                        ctx.provider_configs.iter().any(|allowed| {
                            let provider_matches =
                                allowed.provider == "*" || allowed.provider == provider.name();
                            let model_matches =
                                allowed.allowed_models.iter().any(|allowed_model| {
                                    allowed_model == "*" || allowed_model == model
                                });
                            provider_matches && model_matches
                        })
                    })
                    .collect()
            } else {
                providers.iter().collect()
            };

        if let Some(forced_prov) = &ctx.force_provider {
            allowed_providers.retain(|(provider, _)| provider.name() == forced_prov);
        }

        // 2. Separate into supporting and non-supporting providers
        let mut supporting = Vec::new();
        let mut non_supporting = Vec::new();

        let provider_names: Vec<String> = allowed_providers
            .iter()
            .map(|(p, _)| p.name().to_string())
            .collect();
        info!(
            model = %model,
            available_providers = format!("{:?}", provider_names),
            "[Routing] Available providers for model"
        );

        for &(provider, config) in &allowed_providers {
            let is_open = self.is_circuit_breaker_open(provider.name());

            let weight = if ctx.virtual_key.is_some() {
                ctx.provider_configs
                    .iter()
                    .find(|allowed| allowed.provider == "*" || allowed.provider == provider.name())
                    .map(|allowed| allowed.weight)
                    .unwrap_or(1.0)
            } else {
                1.0
            };

            let supports = model_supported_by(config, model);
            if supports {
                supporting.push((provider.clone(), config.clone(), weight, is_open));
            } else {
                non_supporting.push((provider.clone(), config.clone(), weight, is_open));
            }
        }

        let (mut active_supporting, broken_supporting): (Vec<_>, Vec<_>) = supporting
            .into_iter()
            .partition(|(_, _, _, is_open)| !*is_open);

        // 3. Round-Robin selection among ACTIVE supporting providers
        let n_active = active_supporting.len();
        if n_active > 0 {
            let rr_index = self.round_robin.select_and_advance(model, n_active);
            active_supporting.rotate_left(rr_index);

            let selected_provider = &active_supporting[0].0;
            info!(
                model = %model,
                selected_provider = %selected_provider.name(),
                round_robin_index = rr_index,
                total_active_supporting = n_active,
                "[Routing] Model: {} -> Selected Provider: {} (Round-Robin index: {})",
                model,
                selected_provider.name(),
                rr_index,
            );
        } else {
            info!(
                model = %model,
                "[Routing] No active supporting providers for model — will try fallback chain"
            );
        }

        // 4. Weighted shuffle for remaining ordering
        let broken_supporting = Self::weighted_shuffle(broken_supporting);
        let (active_non_supporting, broken_non_supporting): (Vec<_>, Vec<_>) = non_supporting
            .into_iter()
            .partition(|(_, _, _, is_open)| !*is_open);
        let active_non_supporting = Self::weighted_shuffle(active_non_supporting);
        let broken_non_supporting = Self::weighted_shuffle(broken_non_supporting);

        let mut ordered = Vec::new();
        for (prov, cfg, _, _) in active_supporting {
            ordered.push((prov, cfg));
        }
        for (prov, cfg, _, _) in broken_supporting {
            ordered.push((prov, cfg));
        }
        for (prov, cfg, _, _) in active_non_supporting {
            ordered.push((prov, cfg));
        }
        for (prov, cfg, _, _) in broken_non_supporting {
            ordered.push((prov, cfg));
        }

        let ordered_names: Vec<String> =
            ordered.iter().map(|(p, _)| p.name().to_string()).collect();
        info!(
            model = %model,
            ordered_providers = format!("{:?}", ordered_names),
            "[Routing] Final provider order for model"
        );

        ordered
    }

    /// Calcule des embeddings avec fallback entre providers
    pub async fn embed(
        &self,
        request: EmbeddingRequest,
        ctx: RequestContext,
    ) -> Result<EmbeddingResponse, PylosError> {
        let providers = self.providers.read().await;
        let mut last_error: Option<PylosError> = None;

        let model = if let Some(forced_m) = &ctx.force_model {
            forced_m.clone()
        } else {
            request.model.clone()
        };
        let ordered = self.select_and_order_providers(&providers, &model, &ctx);

        for (provider, config) in ordered {
            let mut req_to_send = request.clone();
            if let Some(forced_m) = &ctx.force_model {
                req_to_send.model = forced_m.clone();
            } else {
                let is_supported = model_supported_by(&config, &model);
                if !is_supported {
                    let mapped_model =
                        map_model_for_provider(provider.name(), &model, &config.allowed_models);
                    info!(
                        provider = provider.name(),
                        original_model = %model,
                        mapped_model = %mapped_model,
                        "[Path] Embedding model '{}' mapped to '{}' for provider '{}'",
                        model, mapped_model, provider.name()
                    );
                    req_to_send.model = mapped_model;
                }
            }

            match provider.embed(&req_to_send, &config).await {
                Ok(resp) => {
                    self.record_success(provider.name());
                    info!(provider = provider.name(), model = %req_to_send.model, "Embedding successful");
                    return Ok(resp);
                }
                Err(PylosError::Unsupported(_)) => {
                    debug!(
                        provider = provider.name(),
                        "Provider does not support embeddings, skipping"
                    );
                    continue;
                }
                Err(e) => {
                    self.record_failure(provider.name());
                    warn!(provider = provider.name(), error = %e, "Embedding failed");
                    // Ne pas retenter si l'erreur est liée au client (budget, guardrail, requête invalide)
                    if matches!(
                        e,
                        PylosError::BudgetExceeded(_)
                            | PylosError::GuardrailViolation(_)
                            | PylosError::InvalidRequest(_)
                    ) {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            PylosError::AllProvidersFailed("No provider supports embeddings".into())
        }))
    }

    /// Envoie une requête non-streaming avec pre/post hooks et fallback
    pub async fn complete(
        &self,
        mut request: PylosRequest,
        mut ctx: RequestContext,
    ) -> Result<(PylosResponse, RequestContext), PylosError> {
        // Convertit TextCompletion en ChatCompletion (compat)
        let text_completion_prompt = if let PylosRequest::TextCompletion(ref tc) = request {
            let prompt = tc.prompt.first().to_string();
            let chat_req = ChatCompletionRequest {
                model: tc.model.clone(),
                messages: vec![ChatCompletionMessage {
                    role: MessageRole::User,
                    content: Some(prompt.clone()),
                    ..Default::default()
                }],
                temperature: tc.temperature,
                top_p: tc.top_p,
                n: tc.n,
                stream: Some(false),
                stop: tc.stop.clone(),
                max_tokens: tc.max_tokens,
                presence_penalty: tc.presence_penalty,
                frequency_penalty: tc.frequency_penalty,
                logit_bias: None,
                user: tc.user.clone(),
                session_id: tc.session_id.clone(),
                account: tc.account.clone(),
                tools: None,
                tool_choice: None,
                response_format: None,
                seed: None,
                top_k: None,
                min_p: None,
                repetition_penalty: None,
                max_completion_tokens: None,
            };
            let original_model = tc.model.clone();
            request = PylosRequest::ChatCompletion(chat_req);
            Some((original_model, prompt))
        } else {
            None
        };

        // Pre-hooks
        for plugin in &self.plugins {
            match plugin.pre_hook(&mut request, &mut ctx).await {
                Ok(Some(short_circuit)) => {
                    debug!(plugin = plugin.name(), "Pre-hook short-circuited request");
                    return Ok((short_circuit, ctx));
                }
                Ok(None) => {}
                // Une violation de gouvernance (budget, quota, autorisation) doit bloquer la requête.
                Err(e) if e.is_governance_violation() => {
                    warn!(plugin = plugin.name(), error = %e, "Pre-hook blocked the request");
                    return Err(e);
                }
                Err(e) => {
                    warn!(plugin = plugin.name(), error = %e, "Pre-hook error (ignored)");
                }
            }
        }

        // Snapshot des providers (lecture lock-free pendant l'inférence)
        let providers = self.providers.read().await;
        let mut last_error: Option<PylosError> = None;

        let model = if let Some(forced_m) = &ctx.force_model {
            forced_m.clone()
        } else {
            request.model().to_string()
        };
        let ordered = self.select_and_order_providers(&providers, &model, &ctx);

        for (provider, config) in &ordered {
            if ctx.tried_providers.contains(&provider.name().to_string()) {
                debug!(
                    provider = provider.name(),
                    "Skipping already-tried provider"
                );
                continue;
            }

            debug!(
                provider = provider.name(),
                model = %model,
                "Starting inference with provider"
            );
            ctx.tried_providers.push(provider.name().to_string());

            let mut req_to_send = request.clone();
            if let Some(forced_m) = &ctx.force_model {
                req_to_send.set_model(forced_m.clone());
            } else {
                let is_supported = model_supported_by(config, &model);
                if !is_supported {
                    let mapped_model =
                        map_model_for_provider(provider.name(), &model, &config.allowed_models);
                    info!(
                        provider = provider.name(),
                        original_model = %model,
                        mapped_model = %mapped_model,
                        trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                        "[Path] Model '{}' not supported natively by '{}', mapped to '{}' for fallback",
                        model, provider.name(), mapped_model
                    );
                    req_to_send.set_model(mapped_model);
                }
            }

            info!(
                provider = provider.name(),
                model = %model,
                trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                "[Path] Trying provider '{}' for model '{}'",
                provider.name(), model
            );

            match self
                .try_complete_with_retry(provider.as_ref(), config, &req_to_send)
                .await
            {
                Ok(mut response) => {
                    self.record_success(provider.name());
                    // Post-hooks (ordre inverse — LIFO)
                    for plugin in self.plugins.iter().rev() {
                        if let Err(e) = plugin.post_hook(&request, &mut response, &mut ctx).await {
                            warn!(plugin = plugin.name(), error = %e, "Post-hook error");
                            return Err(e);
                        }
                    }
                    info!(
                        provider = provider.name(),
                        model = request.model(),
                        trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                        "Inference successful"
                    );

                    // Si c'était une TextCompletion, convertit la réponse ChatCompletion en TextCompletion
                    if let Some((model, _prompt)) = &text_completion_prompt {
                        if let PylosResponse::ChatCompletion(ref chat_resp) = response {
                            let text = chat_resp
                                .choices
                                .first()
                                .and_then(|c| c.message.content.clone())
                                .unwrap_or_default();
                            let finish = chat_resp
                                .choices
                                .first()
                                .and_then(|c| c.finish_reason.clone());
                            return Ok((
                                PylosResponse::TextCompletion(TextCompletionResponse {
                                    id: chat_resp.id.clone(),
                                    object: "text_completion".to_string(),
                                    created: chat_resp.created,
                                    model: model.clone(),
                                    choices: vec![TextCompletionChoice {
                                        text,
                                        index: 0,
                                        finish_reason: finish,
                                        logprobs: None,
                                    }],
                                    usage: chat_resp.usage.clone(),
                                }),
                                ctx,
                            ));
                        }
                    }

                    return Ok((response, ctx));
                }
                Err(e) => {
                    self.record_failure(provider.name());
                    warn!(
                        provider = provider.name(),
                        model = %model,
                        error = %e,
                        trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                        "[Path] Provider '{}' FAILED for model '{}' — error: {}",
                        provider.name(), model, e
                    );
                    // Ne pas retenter si l'erreur est liée au client (budget, guardrail, requête invalide)
                    if matches!(
                        e,
                        PylosError::BudgetExceeded(_)
                            | PylosError::GuardrailViolation(_)
                            | PylosError::InvalidRequest(_)
                    ) {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| PylosError::AllProvidersFailed("No providers configured".into())))
    }

    /// Envoie une requête streaming avec fallback
    /// Retourne le stream ET le nom du provider qui a effectivement pris la requête
    pub async fn stream(
        &self,
        mut request: PylosRequest,
        mut ctx: RequestContext,
    ) -> Result<(ChunkStream, String, RequestContext), PylosError> {
        // Pre-hooks
        for plugin in &self.plugins {
            match plugin.pre_hook(&mut request, &mut ctx).await {
                Ok(Some(short_circuit)) => {
                    // Convertit la réponse short-circuit en stream d'un seul chunk de fin
                    debug!(
                        plugin = plugin.name(),
                        "Pre-hook short-circuited stream request"
                    );
                    let content = match &short_circuit {
                        PylosResponse::ChatCompletion(r) => r
                            .choices
                            .first()
                            .and_then(|c| c.message.content.clone())
                            .unwrap_or_default(),
                        _ => String::new(),
                    };
                    let model = request.model().to_string();
                    let chunk = make_terminal_chunk(&model, &content);
                    let stream: ChunkStream =
                        Box::pin(futures::stream::once(async move { Ok(chunk) }));
                    return Ok((stream, "short-circuit".into(), ctx));
                }
                Ok(None) => {}
                // Une violation de gouvernance (budget, quota, autorisation) doit bloquer la requête.
                Err(e) if e.is_governance_violation() => {
                    warn!(plugin = plugin.name(), error = %e, "Pre-hook blocked the stream request");
                    return Err(e);
                }
                Err(e) => {
                    warn!(plugin = plugin.name(), error = %e, "Pre-hook stream error (ignored)");
                }
            }
        }

        let providers = self.providers.read().await;
        let mut last_error: Option<PylosError> = None;

        let stream_model = if let Some(forced_m) = &ctx.force_model {
            forced_m.clone()
        } else {
            request.model().to_string()
        };
        let ordered_stream = self.select_and_order_providers(&providers, &stream_model, &ctx);

        for (provider, config) in &ordered_stream {
            if ctx.tried_providers.contains(&provider.name().to_string()) {
                continue;
            }
            ctx.tried_providers.push(provider.name().to_string());

            let mut req_to_send = request.clone();
            if let Some(forced_m) = &ctx.force_model {
                req_to_send.set_model(forced_m.clone());
            } else {
                let is_supported = model_supported_by(config, &stream_model);
                if !is_supported {
                    let mapped_model = map_model_for_provider(
                        provider.name(),
                        &stream_model,
                        &config.allowed_models,
                    );
                    info!(
                        provider = provider.name(),
                        original_model = %stream_model,
                        mapped_model = %mapped_model,
                        trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                        "[Path] Stream: Model '{}' not supported natively by '{}', mapped to '{}'",
                        stream_model, provider.name(), mapped_model
                    );
                    req_to_send.set_model(mapped_model);
                }
            }

            info!(
                provider = provider.name(),
                model = %stream_model,
                trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                "[Path] Stream: Trying provider '{}' for model '{}'",
                provider.name(), stream_model
            );

            match provider.stream(&req_to_send, config).await {
                Ok(stream) => {
                    let actual_provider = provider.name().to_string();
                    self.record_success(&actual_provider);
                    info!(
                        provider = %actual_provider,
                        model = request.model(),
                        trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                        "Streaming inference started successfully"
                    );

                    // Wrap the stream to collect chunks and run post-hooks on completion
                    let wrapped = wrap_stream_with_post_hooks(
                        stream,
                        self.plugins.clone(),
                        request.clone(),
                        ctx.clone(),
                    );
                    return Ok((wrapped, actual_provider, ctx));
                }
                Err(e) => {
                    self.record_failure(provider.name());
                    warn!(
                        provider = provider.name(),
                        model = %stream_model,
                        error = %e,
                        trace_id = %ctx.trace_id.as_deref().unwrap_or(""),
                        "[Path] Stream: Provider '{}' FAILED for model '{}' — error: {}",
                        provider.name(), stream_model, e
                    );
                    // Ne pas retenter si l'erreur est liée au client (budget, guardrail, requête invalide)
                    if matches!(
                        e,
                        PylosError::BudgetExceeded(_)
                            | PylosError::GuardrailViolation(_)
                            | PylosError::InvalidRequest(_)
                    ) {
                        return Err(e);
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| PylosError::AllProvidersFailed("No providers configured".into())))
    }

    /// Retry avec backoff exponentiel sur un provider donné
    async fn try_complete_with_retry(
        &self,
        provider: &dyn Provider,
        config: &ProviderConfig,
        request: &PylosRequest,
    ) -> Result<PylosResponse, PylosError> {
        let mut attempt = 0u32;
        let max_retries = config.max_retries;

        loop {
            let res = match request {
                PylosRequest::Image(img_req) => provider
                    .generate_image(img_req, config)
                    .await
                    .map(PylosResponse::Image),
                _ => provider.complete(request, config).await,
            };

            match res {
                Ok(resp) => return Ok(resp),
                Err(e) if e.is_retriable() && attempt < max_retries => {
                    attempt += 1;
                    let backoff = exponential_backoff(
                        attempt,
                        config.retry_backoff_initial_ms,
                        config.retry_backoff_max_ms,
                    );
                    debug!(
                        provider = provider.name(),
                        attempt = attempt,
                        backoff_ms = backoff,
                        error = %e,
                        "Retrying after backoff"
                    );
                    tokio::time::sleep(Duration::from_millis(backoff)).await;
                }
                Err(e) => return Err(e),
            }
        }
    }
}

/// Calcule le délai de backoff exponentiel avec jitter
fn exponential_backoff(attempt: u32, initial_ms: u64, max_ms: u64) -> u64 {
    let shift = attempt.saturating_sub(1).min(62);
    let base = initial_ms.saturating_mul(1u64 << shift);
    let jitter = (base as f64 * 0.2 * (fastrand::f64() * 2.0 - 1.0)) as i64;
    let backoff = (base as i64 + jitter).max(0) as u64;
    backoff.min(max_ms)
}

/// Détermine si un provider supporte explicitement un modèle donné.
/// Délègue à ProviderKind::supports_model.
fn model_supported_by(config: &ProviderConfig, model: &str) -> bool {
    if !config.allowed_models.is_empty() {
        let matches = config.allowed_models.iter().any(|allowed| {
            if allowed == "*" {
                true
            } else if allowed.contains('*') {
                let prefix = allowed.trim_end_matches('*');
                model.starts_with(prefix)
            } else {
                allowed == model
            }
        });
        if !matches {
            return false;
        }
    }
    config.kind.supports_model(model)
}

/// Mappe/traduit un modèle pour un provider de destination en cas de fallback.
fn map_model_for_provider(provider_name: &str, model: &str, allowed_models: &[String]) -> String {
    // 1. Si le provider a des allowed_models configurés (et que ce n'est pas un wildcard), et que le modèle demandé n'y est pas :
    if !allowed_models.is_empty() && !allowed_models.iter().any(|m| m == "*" || m == model) {
        if let Some(first_allowed) = allowed_models.iter().find(|m| !m.contains('*')) {
            return first_allowed.clone();
        }
    }

    let is_pro = model.contains("pro")
        || model.contains("opus")
        || model.contains("large")
        || model.contains("sonnet")
        || model.contains("70b")
        || model.contains("gpt-4")
        || model.contains("o1")
        || model.contains("o3");

    // 2. Sinon, on associe les familles de modèles standards aux modèles équivalents supportés par le provider
    match provider_name {
        "openai" => {
            if model.contains("flash")
                || model.contains("mini")
                || model.contains("haiku")
                || model.contains("8b")
                || model.contains("lite")
            {
                "gpt-4o-mini".to_string()
            } else {
                "gpt-4o".to_string()
            }
        }
        "anthropic" => {
            if model.contains("flash")
                || model.contains("mini")
                || model.contains("haiku")
                || model.contains("8b")
                || model.contains("lite")
            {
                "claude-haiku-3-5".to_string()
            } else {
                "claude-3-5-sonnet-20241022".to_string()
            }
        }
        "gemini" => {
            if is_pro {
                "gemini-3.5-pro".to_string()
            } else {
                "gemini-3.5-flash".to_string()
            }
        }
        "groq" => {
            if is_pro {
                "llama-3.3-70b-versatile".to_string()
            } else {
                "llama-3.1-8b-instant".to_string()
            }
        }
        "deepseek" => {
            if is_pro {
                "deepseek-v4-pro".to_string()
            } else {
                "deepseek-v4-flash".to_string()
            }
        }
        "bedrock" => {
            if model.contains("claude") {
                "anthropic.claude-3-5-sonnet-20241022-v2:0".to_string()
            } else if is_pro {
                "amazon.nova-pro-v1:0".to_string()
            } else {
                "amazon.nova-lite-v1:0".to_string()
            }
        }
        "mistral" => {
            if is_pro {
                "mistral-large-latest".to_string()
            } else {
                "mistral-small-latest".to_string()
            }
        }
        "xai" => {
            if is_pro {
                "grok-3".to_string()
            } else {
                "grok-3-mini".to_string()
            }
        }
        "antigravity" => {
            if model.contains("opus") {
                "claude-opus-4-6-thinking".to_string()
            } else if model.contains("sonnet") || model.contains("claude") {
                "claude-sonnet-4-6".to_string()
            } else if model.contains("gpt-oss") || model.contains("120b") {
                "gpt-oss-120b-medium".to_string()
            } else if is_pro || model.contains("argon") {
                "gemini-4-argon-medium".to_string()
            } else if model.contains("high") {
                "gemini-3.8-flash-high".to_string()
            } else if model.contains("low") {
                "gemini-3.8-flash-low".to_string()
            } else {
                "gemini-3.8-flash-medium".to_string()
            }
        }
        "lemonade-optimus" => {
            if model == "nomic-embed-text" {
                "nomic-embed-text-v2-moe-GGUF".to_string()
            } else {
                model.to_string()
            }
        }
        "ollama-optimus" => {
            if model == "nomic-embed-text" {
                "nomic-embed-text:latest".to_string()
            } else {
                model.to_string()
            }
        }
        _ => model.to_string(),
    }
}

/// Split off a trailing incomplete `[...` placeholder prefix (up to 40 chars)
/// so PII placeholders split across SSE chunks can be reassembled before unmasking.
fn split_incomplete_placeholder_tail(text: &str) -> (&str, &str) {
    if let Some(open_idx) = text.rfind('[') {
        let after_open = &text[open_idx..];
        if !after_open.contains(']') && after_open.len() <= 40 {
            return (&text[..open_idx], after_open);
        }
    }
    (text, "")
}

/// Estimate token usage when a streaming provider omits the final `usage` field.
fn estimate_stream_usage(request: &PylosRequest, full_content: &str) -> pylos_core::domain::openai::Usage {
    let prompt_chars: usize = match request {
        PylosRequest::ChatCompletion(r) => r
            .messages
            .iter()
            .filter_map(|m| m.content.as_ref().map(|c| c.len()))
            .sum(),
        _ => 0,
    };
    let prompt_tokens = ((prompt_chars + 3) / 4) as i32;
    let completion_tokens = ((full_content.len() + 3) / 4) as i32;
    pylos_core::domain::openai::Usage {
        prompt_tokens,
        completion_tokens,
        total_tokens: prompt_tokens + completion_tokens,
        ..Default::default()
    }
}

/// Wrap a streaming response so that:
/// 1. Each chunk passes through `post_stream_chunk` (in reverse priority order) BEFORE being yielded
///    to the client — restoring PII placeholders in real time (even when split across chunks).
/// 2. Any trailing buffered fragment is flushed at stream end.
/// 3. Full `post_hook` runs at completion with actual (or estimated) `Usage` for budget/metrics/validation.
fn wrap_stream_with_post_hooks(
    stream: ChunkStream,
    plugins: Vec<Arc<dyn LlmPlugin>>,
    request: PylosRequest,
    ctx: RequestContext,
) -> ChunkStream {
    let accumulated = Arc::new(Mutex::new(Vec::<StreamChunk>::new()));
    let last_usage = Arc::new(Mutex::new(None::<pylos_core::domain::openai::Usage>));
    let pending_tail = Arc::new(Mutex::new(String::new()));
    let has_pii_mapping = ctx.headers.contains_key("x-pii-mapping");

    let acc = accumulated.clone();
    let usage_acc = last_usage.clone();
    let tail_acc = pending_tail.clone();
    let plugins_for_chunk = plugins.clone();
    let ctx_for_chunk = Arc::new(Mutex::new(ctx.clone()));
    let ctx_chunk_handle = ctx_for_chunk.clone();

    let collected = stream.map(move |result| {
        match result {
            Ok(mut chunk) => {
                if let Some(ref u) = chunk.usage {
                    if let Ok(mut u_guard) = usage_acc.lock() {
                        *u_guard = Some(u.clone());
                    }
                }

                if has_pii_mapping {
                    if let Ok(mut tail) = tail_acc.lock() {
                        if let Some(choice) = chunk.choices.first_mut() {
                            if let Some(ref mut content) = choice.delta.content {
                                if !tail.is_empty() {
                                    let combined = format!("{}{}", *tail, content);
                                    tail.clear();
                                    *content = combined;
                                }
                                let (head, rest) = split_incomplete_placeholder_tail(content);
                                if !rest.is_empty() {
                                    *tail = rest.to_string();
                                    *content = head.to_string();
                                }
                            }
                        }
                    }
                }

                if let Ok(mut c_guard) = ctx_chunk_handle.lock() {
                    for plugin in plugins_for_chunk.iter().rev() {
                        if let Err(e) = plugin.post_stream_chunk(&mut chunk, &mut c_guard) {
                            warn!(plugin = plugin.name(), error = %e, "Post-stream-chunk error");
                        }
                    }
                }

                if let Ok(mut guard) = acc.lock() {
                    guard.push(chunk.clone());
                }
                Ok(chunk)
            }
            Err(e) => Err(e),
        }
    });

    let plugins_for_done = plugins.clone();
    let request_for_done = request.clone();
    let accumulated_for_done = accumulated.clone();
    let usage_for_done = last_usage.clone();
    let tail_for_done = pending_tail.clone();

    let done_stream = futures::stream::once(async move {
        let remaining_tail = tail_for_done
            .lock()
            .map(|mut t| std::mem::take(&mut *t))
            .unwrap_or_default();

        let mut chunks = accumulated_for_done
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        let mut flushed_chunk: Option<StreamChunk> = None;
        if !remaining_tail.is_empty() {
            let mut tail_chunk = make_terminal_chunk(request_for_done.model(), &remaining_tail);
            if let Ok(mut c_guard) = ctx_for_chunk.lock() {
                for plugin in plugins_for_done.iter().rev() {
                    if let Err(e) = plugin.post_stream_chunk(&mut tail_chunk, &mut c_guard) {
                        warn!(plugin = plugin.name(), error = %e, "Post-stream-chunk tail flush error");
                    }
                }
            }
            chunks.push(tail_chunk.clone());
            flushed_chunk = Some(tail_chunk);
        }

        if !chunks.is_empty() {
            let full_content: String = chunks
                .iter()
                .flat_map(|c| c.choices.iter())
                .filter_map(|ch| ch.delta.content.as_ref())
                .cloned()
                .collect();

            let model = request_for_done.model().to_string();
            let last_chunk = chunks.last();
            let finish_reason = last_chunk
                .and_then(|c| c.choices.first())
                .and_then(|ch| ch.finish_reason.clone());

            let stream_usage = usage_for_done
                .lock()
                .ok()
                .and_then(|g| g.clone())
                .or_else(|| Some(estimate_stream_usage(&request_for_done, &full_content)));

            let mut response = PylosResponse::ChatCompletion(ChatCompletionResponse {
                id: chunks.first().map(|c| c.id.clone()).unwrap_or_default(),
                object: "chat.completion".to_string(),
                created: chunks.first().map(|c| c.created).unwrap_or(0),
                model,
                choices: vec![ChatCompletionChoice {
                    index: 0,
                    message: ChatCompletionMessage {
                        role: MessageRole::Assistant,
                        content: Some(full_content),
                        ..Default::default()
                    },
                    finish_reason,
                }],
                usage: stream_usage,
            });

            let mut final_ctx = ctx_for_chunk
                .lock()
                .map(|g| g.clone())
                .unwrap_or(ctx);
            for plugin in plugins_for_done.iter().rev() {
                if let Err(e) = plugin
                    .post_hook(&request_for_done, &mut response, &mut final_ctx)
                    .await
                {
                    warn!(plugin = plugin.name(), error = %e, "Post-hook stream error");
                }
            }
        }

        match flushed_chunk {
            Some(c) => Ok(c),
            None => Err::<StreamChunk, PylosError>(PylosError::Internal("stream-end".into())),
        }
    });

    Box::pin(collected.chain(done_stream).take_while(|item| {
        let should_continue =
            !matches!(item, Err(PylosError::Internal(ref msg)) if msg == "stream-end");
        futures::future::ready(should_continue)
    }))
}

/// Crée un chunk de streaming terminal (finish_reason = "stop") à partir d'un contenu texte.
/// Utilisé pour convertir une réponse short-circuit (plugin pre-hook) en stream.
pub(crate) fn make_terminal_chunk(
    model: &str,
    content: &str,
) -> pylos_core::domain::request::StreamChunk {
    use pylos_core::domain::request::{StreamChoice, StreamDelta};
    use std::time::{SystemTime, UNIX_EPOCH};
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    pylos_core::domain::request::StreamChunk {
        id: format!("shortcircuit-{}", fastrand::u32(..)),
        object: "chat.completion.chunk".into(),
        created,
        model: model.to_string(),
        choices: vec![StreamChoice {
            index: 0,
            delta: StreamDelta {
                role: Some("assistant".into()),
                content: if content.is_empty() {
                    None
                } else {
                    Some(content.to_string())
                },
                ..Default::default()
            },
            finish_reason: Some("stop".into()),
        }],
        usage: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff_bounds() {
        for attempt in 1..=10 {
            let b = exponential_backoff(attempt, 100, 5_000);
            assert!(
                b <= 5_000,
                "Backoff {} exceeded max at attempt {}",
                b,
                attempt
            );
        }
        for _ in 0..20 {
            let b = exponential_backoff(1, 100, 5_000);
            assert!(b <= 120, "First backoff too high: {}", b);
        }
    }

    #[test]
    fn test_map_model_for_provider_cases() {
        // Test exact model whitelist override
        assert_eq!(
            map_model_for_provider(
                "deepseek",
                "gemini-3.5-flash",
                &["deepseek-v4-pro".to_string()]
            ),
            "deepseek-v4-pro"
        );

        // Test fallback to deepseek-v4-flash for a flash model
        assert_eq!(
            map_model_for_provider("deepseek", "gemini-3.5-flash", &[]),
            "deepseek-v4-flash"
        );

        // Test fallback to deepseek-v4-pro for a pro model
        assert_eq!(
            map_model_for_provider("deepseek", "gemini-3.5-pro", &[]),
            "deepseek-v4-pro"
        );

        // Test fallback to amazon.nova-pro-v1:0 on bedrock for a pro/large model
        assert_eq!(
            map_model_for_provider("bedrock", "gpt-4o", &[]),
            "amazon.nova-pro-v1:0"
        );

        // Test fallback to Antigravity models
        assert_eq!(
            map_model_for_provider("antigravity", "gemini-2.5-flash", &[]),
            "gemini-3.8-flash-medium"
        );
        assert_eq!(
            map_model_for_provider("antigravity", "claude-3-5-sonnet", &[]),
            "claude-sonnet-4-6"
        );
        assert_eq!(
            map_model_for_provider("antigravity", "claude-3-opus", &[]),
            "claude-opus-4-6-thinking"
        );
        assert_eq!(
            map_model_for_provider("antigravity", "gpt-4o", &[]),
            "gemini-4-argon-medium"
        );
    }

    #[tokio::test]
    async fn test_plugin_pipeline_sorts_by_priority() {
        struct PriorityPlugin {
            name: &'static str,
            prio: i32,
            log: Arc<Mutex<Vec<&'static str>>>,
        }

        #[async_trait::async_trait]
        impl LlmPlugin for PriorityPlugin {
            fn name(&self) -> &str {
                self.name
            }
            fn priority(&self) -> i32 {
                self.prio
            }
            async fn pre_hook(
                &self,
                _req: &mut PylosRequest,
                _ctx: &mut RequestContext,
            ) -> Result<Option<PylosResponse>, PylosError> {
                self.log.lock().unwrap().push(self.name);
                Ok(None)
            }
        }

        let log = Arc::new(Mutex::new(Vec::new()));
        let plugins: Vec<Arc<dyn LlmPlugin>> = vec![
            Arc::new(PriorityPlugin { name: "otel", prio: 100, log: log.clone() }),
            Arc::new(PriorityPlugin { name: "guardrails", prio: -50, log: log.clone() }),
            Arc::new(PriorityPlugin { name: "cache", prio: 10, log: log.clone() }),
        ];
        let orch = InferenceOrchestrator::new(vec![], plugins);
        let mut req = PylosRequest::ChatCompletion(pylos_core::domain::openai::ChatCompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![],
            ..Default::default()
        });
        let mut ctx = RequestContext::default();
        for p in &orch.plugins {
            let _ = p.pre_hook(&mut req, &mut ctx).await;
        }
        assert_eq!(*log.lock().unwrap(), vec!["guardrails", "cache", "otel"]);
    }

    #[tokio::test]
    async fn test_streaming_post_hooks_restore_split_pii_and_preserve_usage() {
        use futures::StreamExt;
        use pylos_core::domain::config::{PluginConfig, PylosConfig};
        use pylos_core::domain::openai::{ChatCompletionRequest, Usage};
        use pylos_core::domain::request::{StreamChoice, StreamDelta};

        // Setup GuardrailsPlugin with PII masking/unmasking
        let mut cfg = PylosConfig::default();
        cfg.plugins.push(PluginConfig {
            name: "guardrails".into(),
            enabled: true,
            config: serde_json::json!({ "mask_pii": true }),
        });
        let store = Arc::new(crate::config_store::ConfigStore::from_config(cfg));
        let guardrails = Arc::new(crate::guardrails::GuardrailsPlugin::new(store));

        // Capture post_hook usage in a spy plugin
        let captured_usage: Arc<Mutex<Option<Usage>>> = Arc::new(Mutex::new(None));
        struct UsageSpy(Arc<Mutex<Option<Usage>>>);
        #[async_trait::async_trait]
        impl LlmPlugin for UsageSpy {
            fn name(&self) -> &str {
                "usage_spy"
            }
            async fn post_hook(
                &self,
                _req: &PylosRequest,
                resp: &mut PylosResponse,
                _ctx: &mut RequestContext,
            ) -> Result<(), PylosError> {
                if let PylosResponse::ChatCompletion(r) = resp {
                    *self.0.lock().unwrap() = r.usage.clone();
                }
                Ok(())
            }
        }

        let mut ctx = RequestContext::default();
        ctx.headers.insert(
            "x-pii-mapping".into(),
            r#"{"[EMAIL_1]":"alice@example.com"}"#.into(),
        );

        let make_chunk = |text: &str, usage: Option<Usage>| StreamChunk {
            id: "c1".into(),
            object: "chat.completion.chunk".into(),
            created: 1,
            model: "gpt-4o".into(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".into()),
                    content: Some(text.into()),
                    ..Default::default()
                },
                finish_reason: None,
            }],
            usage,
        };

        // Placeholder "[EMAIL_1]" is split across two chunks!
        let chunks = vec![
            Ok(make_chunk("Contact [EMA", None)),
            Ok(make_chunk(
                "IL_1] now.",
                Some(Usage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 15,
                    ..Default::default()
                }),
            )),
        ];

        let req = PylosRequest::ChatCompletion(ChatCompletionRequest {
            model: "gpt-4o".into(),
            messages: vec![],
            stream: Some(true),
            ..Default::default()
        });

        let wrapped = wrap_stream_with_post_hooks(
            Box::pin(futures::stream::iter(chunks)),
            vec![guardrails, Arc::new(UsageSpy(captured_usage.clone()))],
            req,
            ctx,
        );

        let received: Vec<StreamChunk> = wrapped.map(|r| r.unwrap()).collect().await;
        let full_text: String = received
            .iter()
            .filter_map(|c| c.choices.first().and_then(|ch| ch.delta.content.as_deref()))
            .collect();

        assert_eq!(full_text, "Contact alice@example.com now.");
        let u = captured_usage.lock().unwrap().clone().expect("Usage should be forwarded to post_hook");
        assert_eq!(u.total_tokens, 15);
    }
}
