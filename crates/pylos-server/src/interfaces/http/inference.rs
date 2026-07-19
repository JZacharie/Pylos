use std::sync::Arc;
use std::time::Instant;

use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    Json,
};
use futures::StreamExt;
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

use pylos_application::log_store::{build_log_entry_full, LogStatus};
use pylos_core::domain::openai::ChatCompletionRequest;
use pylos_core::domain::request::{PylosRequest, RequestContext, StreamChunk};
use pylos_core::error::PylosError;

fn guardrails_info_from_response(
    finish_reason: &Option<String>,
    output_preview: &Option<String>,
    ctx: &RequestContext,
) -> (Option<bool>, Option<String>, Option<String>, Option<String>) {
    let moderation_scores = ctx.headers.get("x-moderation-scores").cloned();
    if let Some(triggered_val) = ctx.headers.get("guardrail_triggered") {
        if triggered_val == "true" {
            return (
                Some(true),
                ctx.headers.get("guardrail_type").cloned(),
                ctx.headers.get("guardrail_detail").cloned(),
                moderation_scores,
            );
        }
    }
    let triggered = finish_reason.as_deref() == Some("content_filter");
    if !triggered {
        return (None, None, None, moderation_scores);
    }
    let (gtype, detail) = match output_preview.as_deref() {
        Some(msg) if msg.contains("prompt injection") => (
            Some("prompt_injection".into()),
            Some("Prompt injection attempt blocked".into()),
        ),
        Some(msg) if msg.contains("flagged") => (
            Some("keyword_block".into()),
            Some("Blocked keyword detected".into()),
        ),
        _ => (Some("content_filter".into()), None),
    };
    (Some(true), gtype, detail, moderation_scores)
}

use crate::middleware::request_id::RequestTrace;
use crate::middleware::virtual_key::VirtualKeyInfo;
use crate::provider_utils::guess_provider;
use crate::state::AppState;

/// Handler POST /v1/chat/completions
pub async fn chat_completions(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Extension(vk_info): Extension<Option<VirtualKeyInfo>>,
    Extension(trace): Extension<RequestTrace>,
    Json(mut payload): Json<ChatCompletionRequest>,
) -> Response {
    let caveman_mode = headers
        .get("x-caveman-mode")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<crate::compression::CavemanMode>().ok())
        .unwrap_or(crate::compression::CavemanMode::Off);

    let shrink_input = headers
        .get("x-caveman-compress")
        .and_then(|h| h.to_str().ok())
        .map(|s| s == "true" || s == "1")
        .unwrap_or(false);

    let saved_bytes =
        crate::compression::optimize_request(&mut payload, caveman_mode, shrink_input);

    let caveman_input = (caveman_mode != crate::compression::CavemanMode::Off || shrink_input)
        .then(|| crate::compression::caveman_input_json(&payload))
        .flatten();

    let is_stream = payload.stream.unwrap_or(false);
    let model = payload.model.clone();
    let input_preview = payload
        .messages
        .iter()
        .rev()
        .find(|m| matches!(m.role, pylos_core::domain::openai::MessageRole::User))
        .and_then(|m| m.content.clone());

    let request_id = trace.request_id.clone();
    let source = trace.source.clone();

    info!(
        request_id = %request_id,
        source = %source.as_deref().unwrap_or("api"),
        model = %model,
        stream = is_stream,
        "[Request] POST /v1/chat/completions — Starting inference"
    );

    let force_provider = headers
        .get("x-pylos-force-provider")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let force_model = headers
        .get("x-pylos-force-model")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string());

    let request = PylosRequest::ChatCompletion(payload);
    let mut ctx = RequestContext {
        trace_id: Some(request_id.clone()),
        force_provider,
        force_model,
        ..Default::default()
    };
    if let Some(vk) = &vk_info {
        ctx.virtual_key = Some(vk.name.clone());
        ctx.provider_configs = vk.provider_configs.clone();
    }

    if is_stream {
        stream_response(
            state,
            request,
            ctx,
            model,
            input_preview,
            vk_info,
            saved_bytes,
            request_id,
            source,
            caveman_input,
        )
        .await
    } else {
        complete_response(
            state,
            request,
            ctx,
            model,
            input_preview,
            vk_info,
            saved_bytes,
            request_id,
            source,
            caveman_input,
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn complete_response(
    state: AppState,
    request: PylosRequest,
    ctx: RequestContext,
    model: String,
    input_preview: Option<String>,
    vk_info: Option<VirtualKeyInfo>,
    saved_bytes: usize,
    request_id: String,
    source: Option<String>,
    caveman_input: Option<String>,
) -> Response {
    let start = Instant::now();
    let req_type = "chat_completion";
    let provider_name = guess_provider(&model);

    state.metrics.inc_requests(&provider_name, &model, req_type);

    match state.orchestrator.complete(request, ctx).await {
        Ok((pylos_core::domain::request::PylosResponse::ChatCompletion(resp), final_ctx)) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            state.metrics.inc_success(&provider_name, &model);
            state
                .metrics
                .observe_duration(&provider_name, &model, latency / 1000.0);
            if let Some(ref usage) = resp.usage {
                state
                    .metrics
                    .add_prompt_tokens(&provider_name, &model, usage.prompt_tokens as u64);
                state.metrics.add_completion_tokens(
                    &provider_name,
                    &model,
                    usage.completion_tokens as u64,
                );
            }
            if saved_bytes > 0 {
                state
                    .metrics
                    .add_saved_bytes(&provider_name, &model, saved_bytes as u64);
            }

            let output_preview = resp.choices.first().and_then(|c| c.message.content.clone());
            let finish_reason = resp.choices.first().and_then(|c| c.finish_reason.clone());
            let (gr_triggered, gr_type, gr_detail, moderation_scores) =
                guardrails_info_from_response(&finish_reason, &output_preview, &final_ctx);
            let provider = guess_provider(&resp.model);
            let vk_name = vk_info.map(|v| v.name);

            info!(
                request_id = %request_id,
                source = %source.as_deref().unwrap_or("api"),
                provider = %provider,
                model = %resp.model,
                latency_ms = format!("{:.2}", latency),
                guardrail_triggered = ?gr_triggered,
                guardrail_type = ?gr_type,
                moderation_scores = ?moderation_scores,
                "[Complete] Inference succeeded"
            );

            let obfuscated_input = final_ctx.headers.get("x-obfuscated-input").cloned();
            let pii_mapping = final_ctx.headers.get("x-pii-mapping").cloned();

            let entry = build_log_entry_full(
                &provider,
                &resp.model,
                false,
                LogStatus::Success,
                latency,
                resp.usage.as_ref(),
                finish_reason,
                None,
                input_preview,
                output_preview,
                vk_name,
                saved_bytes,
                gr_triggered,
                gr_type,
                gr_detail,
                obfuscated_input,
                pii_mapping,
                moderation_scores,
                caveman_input,
            );

            let state_clone = state.clone();
            tokio::spawn(async move {
                state_clone.log_store.push(entry).await;
            });

            let actual_model = resp.model.clone();
            let actual_provider = provider.clone();
            let mut response = Json(resp).into_response();
            response.headers_mut().insert(
                axum::http::header::HeaderName::from_static("x-pylos-provider"),
                axum::http::header::HeaderValue::from_str(&actual_provider).unwrap(),
            );
            response.headers_mut().insert(
                axum::http::header::HeaderName::from_static("x-pylos-model"),
                axum::http::header::HeaderValue::from_str(&actual_model).unwrap(),
            );
            response
        }
        Ok((pylos_core::domain::request::PylosResponse::Image(resp), _)) => {
            state.metrics.inc_success(&provider_name, &model);
            info!(
                request_id = %request_id,
                source = %source.as_deref().unwrap_or("api"),
                provider = %provider_name,
                "[Image] Inference succeeded"
            );
            Json(resp).into_response()
        }
        Ok((pylos_core::domain::request::PylosResponse::Embedding(resp), _)) => {
            state.metrics.inc_success(&provider_name, &model);
            info!(
                request_id = %request_id,
                source = %source.as_deref().unwrap_or("api"),
                provider = %provider_name,
                "[Embedding] Inference succeeded"
            );
            Json(resp).into_response()
        }
        Ok((pylos_core::domain::request::PylosResponse::TextCompletion(resp), _)) => {
            state.metrics.inc_success(&provider_name, &model);
            info!(
                request_id = %request_id,
                source = %source.as_deref().unwrap_or("api"),
                provider = %provider_name,
                "[TextCompletion] Inference succeeded"
            );
            Json(resp).into_response()
        }
        Err(e) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            state.metrics.inc_error(&provider_name, e.error_type());
            let provider = guess_provider(&model);
            let vk_name = vk_info.map(|v| v.name);

            error!(
                request_id = %request_id,
                source = %source.as_deref().unwrap_or("api"),
                provider = %provider,
                model = %model,
                latency_ms = format!("{:.2}", latency),
                error = %e,
                error_type = %e.error_type(),
                "[Complete] Inference FAILED — root cause: {}. Type: {}",
                e, e.error_type()
            );

            let entry = build_log_entry_full(
                &provider,
                &model,
                false,
                LogStatus::Error,
                latency,
                None,
                None,
                Some(e.to_string()),
                input_preview,
                None,
                vk_name,
                saved_bytes,
                None,
                None,
                None,
                None,
                None,
                None,
                caveman_input,
            );
            let state_clone = state.clone();
            tokio::spawn(async move {
                state_clone.log_store.push(entry).await;
            });
            error_response(&e)
        }
    }
}

#[derive(Default)]
struct StreamAccumulator {
    output_parts: Vec<String>,
    finish_reason: Option<String>,
    completion_tokens: usize,
    first_token_time: Option<Instant>,
}

impl StreamAccumulator {
    fn collect(&mut self, chunk: &StreamChunk) {
        for choice in &chunk.choices {
            if let Some(content) = &choice.delta.content {
                if self.first_token_time.is_none() {
                    self.first_token_time = Some(Instant::now());
                }
                self.completion_tokens += (content.len() / 4).max(1);
                self.output_parts.push(content.clone());
            }
            if let Some(fr) = &choice.finish_reason {
                self.finish_reason = Some(fr.clone());
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn stream_response(
    state: AppState,
    request: PylosRequest,
    ctx: RequestContext,
    model: String,
    input_preview: Option<String>,
    vk_info: Option<VirtualKeyInfo>,
    saved_bytes: usize,
    request_id: String,
    source: Option<String>,
    caveman_input: Option<String>,
) -> Response {
    let start = Instant::now();
    let req_type = "chat_completion";

    state
        .metrics
        .inc_requests(&guess_provider(&model), &model, req_type);

    match state.orchestrator.stream(request, ctx).await {
        Ok((chunk_stream, actual_provider, final_ctx)) => {
            let vk_name = vk_info.map(|v| v.name);

            info!(
                request_id = %request_id,
                source = %source.as_deref().unwrap_or("api"),
                provider = %actual_provider,
                model = %model,
                "[Stream] Streaming started — actual provider: {}",
                actual_provider,
            );

            let accumulator = Arc::new(std::sync::Mutex::new(StreamAccumulator::default()));
            let acc_stream = Arc::clone(&accumulator);

            let (log_tx, mut log_rx) = mpsc::channel::<()>(1);
            let log_tx = Arc::new(log_tx);
            let log_tx_done = Arc::clone(&log_tx);

            let req_id_for_stream = request_id.clone();
            let src_for_stream = source.clone();
            // Stream principal : accumule les chunks + émet les événements SSE
            let sse_stream = chunk_stream.map(move |result| {
                match &result {
                    Ok(chunk) => {
                        if let Ok(mut acc) = acc_stream.lock() {
                            acc.collect(chunk);
                        }
                    }
                    Err(e) => {
                        error!(
                            request_id = %req_id_for_stream,
                            source = %src_for_stream.as_deref().unwrap_or("api"),
                            error = %e,
                            "SSE chunk error during streaming"
                        );
                    }
                }
                let data = match result {
                    Ok(chunk) => serde_json::to_string(&chunk).unwrap_or_default(),
                    Err(e) => json!({ "error": e.to_string() }).to_string(),
                };
                Ok::<Event, axum::Error>(Event::default().data(data))
            });

            // Événement terminal [DONE] — déclenche le logging
            let done_event = futures::stream::once(async move {
                let _ = log_tx_done.send(()).await;
                Ok::<Event, axum::Error>(Event::default().data("[DONE]"))
            });

            // Task de logging : attend le signal [DONE] puis persiste
            let state_for_log = state.clone();
            let model_clone = model.clone();
            let input_prev = input_preview.clone();
            let acc_log = Arc::clone(&accumulator);
            let provider_clone = actual_provider.clone();
            let req_id_for_log = request_id.clone();
            let src_for_log = source.clone();
            let final_ctx_clone = final_ctx.clone();
            tokio::spawn(async move {
                if log_rx.recv().await.is_some() {
                    let elapsed_total = start.elapsed().as_secs_f64();
                    let latency = elapsed_total * 1000.0;
                    let (completion_tokens, output_preview, finish, first_token_time) = {
                        let acc = acc_log.lock().unwrap_or_else(|e| e.into_inner());
                        let tokens = acc.completion_tokens as i32;
                        let preview = if acc.output_parts.is_empty() {
                            None
                        } else {
                            Some(acc.output_parts.join(""))
                        };
                        let finish = acc.finish_reason.clone().or_else(|| Some("stop".into()));
                        (tokens, preview, finish, acc.first_token_time)
                    };

                    if output_preview.as_ref().is_none_or(|s| s.trim().is_empty()) {
                        warn!(
                            request_id = %req_id_for_log,
                            trace_id = %req_id_for_log,
                            source = %src_for_log.as_deref().unwrap_or("api"),
                            provider = %provider_clone,
                            model = %model_clone,
                            latency_ms = format!("{:.2}", latency),
                            "[Stream] Streaming ended with EMPTY content"
                        );
                    } else {
                        info!(
                            request_id = %req_id_for_log,
                            trace_id = %req_id_for_log,
                            source = %src_for_log.as_deref().unwrap_or("api"),
                            provider = %provider_clone,
                            model = %model_clone,
                            latency_ms = format!("{:.2}", latency),
                            tokens = completion_tokens,
                            "[Stream] Streaming completed successfully"
                        );
                    }

                    // Enregistrer le TTFT si disponible
                    if let Some(ft_time) = first_token_time {
                        let ttft = ft_time.duration_since(start).as_secs_f64();
                        state_for_log
                            .metrics
                            .inference_ttft_seconds
                            .with_label_values(&[&provider_clone, &model_clone])
                            .observe(ttft);
                    }

                    // Enregistrer le TPS si disponible
                    if completion_tokens > 0 && elapsed_total > 0.0 {
                        let tps = completion_tokens as f64 / elapsed_total;
                        state_for_log
                            .metrics
                            .inference_tps
                            .with_label_values(&[&provider_clone, &model_clone])
                            .observe(tps);
                    }

                    state_for_log
                        .metrics
                        .inc_success(&provider_clone, &model_clone);
                    state_for_log.metrics.observe_duration(
                        &provider_clone,
                        &model_clone,
                        elapsed_total,
                    );
                    if saved_bytes > 0 {
                        state_for_log.metrics.add_saved_bytes(
                            &provider_clone,
                            &model_clone,
                            saved_bytes as u64,
                        );
                    }

                    let pseudo_usage = pylos_core::domain::openai::Usage {
                        prompt_tokens: 0,
                        completion_tokens,
                        total_tokens: completion_tokens,
                        ..Default::default()
                    };

                    let (gr_triggered, gr_type, gr_detail, moderation_scores) =
                        guardrails_info_from_response(&finish, &output_preview, &final_ctx_clone);
                    let obfuscated_input = final_ctx_clone.headers.get("x-obfuscated-input").cloned();
                    let pii_mapping = final_ctx_clone.headers.get("x-pii-mapping").cloned();

                    let entry = build_log_entry_full(
                        &provider_clone,
                        &model_clone,
                        true,
                        LogStatus::Success,
                        latency,
                        Some(&pseudo_usage),
                        finish,
                        None,
                        input_prev,
                        output_preview,
                        vk_name,
                        saved_bytes,
                        gr_triggered,
                        gr_type,
                        gr_detail,
                        obfuscated_input,
                        pii_mapping,
                        moderation_scores,
                        caveman_input.clone(),
                    );
                    state_for_log.log_store.push(entry).await;
                }
            });

            let mut response = Sse::new(sse_stream.chain(done_event))
                .keep_alive(KeepAlive::default())
                .into_response();
            response.headers_mut().insert(
                axum::http::header::HeaderName::from_static("x-pylos-provider"),
                axum::http::header::HeaderValue::from_str(&actual_provider).unwrap(),
            );
            response.headers_mut().insert(
                axum::http::header::HeaderName::from_static("x-pylos-model"),
                axum::http::header::HeaderValue::from_str(&model).unwrap(),
            );
            response
        }
        Err(e) => {
            let latency = start.elapsed().as_secs_f64() * 1000.0;
            let provider = guess_provider(&model);
            state.metrics.inc_error(&provider, e.error_type());
            let vk_name = vk_info.map(|v| v.name);

            error!(
                request_id = %request_id,
                source = %source.as_deref().unwrap_or("api"),
                provider = %provider,
                model = %model,
                latency_ms = format!("{:.2}", latency),
                error = %e,
                error_type = %e.error_type(),
                "[Stream] Streaming inference FAILED — root cause: {}. Type: {}",
                e, e.error_type()
            );

            let entry = build_log_entry_full(
                &provider,
                &model,
                true,
                LogStatus::Error,
                latency,
                None,
                None,
                Some(e.to_string()),
                input_preview,
                None,
                vk_name,
                saved_bytes,
                None,
                None,
                None,
                None,
                None,
                None,
                caveman_input.clone(),
            );
            tokio::spawn(async move {
                state.log_store.push(entry).await;
            });
            error_response(&e)
        }
    }
}

pub fn error_response(error: &PylosError) -> Response {
    let (status, code) = match error {
        PylosError::Unauthorized(_) => (StatusCode::UNAUTHORIZED, "unauthorized"),
        PylosError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, "invalid_request_error"),
        PylosError::NotFound(_) => (StatusCode::NOT_FOUND, "not_found"),
        PylosError::RateLimitExceeded(_) => (StatusCode::TOO_MANY_REQUESTS, "rate_limit_error"),
        PylosError::Timeout(_) => (StatusCode::GATEWAY_TIMEOUT, "timeout"),
        PylosError::AllProvidersFailed(_) | PylosError::ProviderError { .. } => {
            (StatusCode::BAD_GATEWAY, "provider_error")
        }
        PylosError::Unsupported(_) => (StatusCode::NOT_IMPLEMENTED, "not_implemented"),
        PylosError::BudgetExceeded(_) => (StatusCode::PAYMENT_REQUIRED, "budget_exceeded"),
        PylosError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error"),
    };

    let body = json!({
        "error": {
            "message": error.to_string(),
            "type": code,
            "code": status.as_u16()
        }
    });

    (status, Json(body)).into_response()
}
