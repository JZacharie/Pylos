use async_trait::async_trait;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{oneshot, Mutex};
use tracing::{debug, info, warn};

// ─────────────────────────────────────────────────────────────────────────────
// Types pour l'API OpenAI Batch
// ─────────────────────────────────────────────────────────────────────────────

/// Statut d'un batch OpenAI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchStatus {
    Validating,
    Failed,
    InProgress,
    Finalizing,
    Completed,
    Expired,
    Cancelling,
    Cancelled,
}

/// Réponse de création d'un batch OpenAI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIBatchResponse {
    pub id: String,
    pub object: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub input_file_id: String,
    #[serde(default)]
    pub output_file_id: Option<String>,
    #[serde(default)]
    pub error_file_id: Option<String>,
    #[serde(default)]
    pub errors: Option<serde_json::Value>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub in_progress_at: Option<i64>,
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub completed_at: Option<i64>,
    #[serde(default)]
    pub failed_at: Option<i64>,
    #[serde(default)]
    pub expired_at: Option<i64>,
    #[serde(default)]
    pub cancelling_at: Option<i64>,
    #[serde(default)]
    pub cancelled_at: Option<i64>,
    #[serde(default)]
    pub request_counts: Option<BatchRequestCounts>,
    #[serde(default)]
    pub metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BatchRequestCounts {
    pub total: u32,
    pub completed: u32,
    pub failed: u32,
}

/// Réponse du téléchargement de fichier OpenAI
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OpenAIFileResponse {
    pub id: String,
    pub object: String,
    #[serde(default)]
    pub bytes: i64,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub purpose: String,
    #[serde(default)]
    pub status: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// Mode de batching
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum BatchMode {
    Coalescing {
        delay_ms: u64,
        max_batch_size: usize,
    },
    AsyncBatch {
        openai_base_url: String,
        api_key: String,
        batch_model: String,
        poll_interval_secs: u64,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// BatchStore — stockage partagé des résultats de batch
// ─────────────────────────────────────────────────────────────────────────────

pub type BatchResultStore = Arc<Mutex<HashMap<String, PylosResponse>>>;

// ─────────────────────────────────────────────────────────────────────────────
// CoalescingBatch — queue à fenêtre temporelle
// ─────────────────────────────────────────────────────────────────────────────

struct BatchSignal {
    _model: String,
    release: oneshot::Sender<()>,
}

struct CoalescingBatch {
    tx: tokio::sync::mpsc::UnboundedSender<BatchSignal>,
}

impl CoalescingBatch {
    fn new(delay: Duration, max_batch_size: usize) -> Self {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(coalescing_loop(rx, delay, max_batch_size));
        Self { tx }
    }

    async fn enqueue(&self, model: String) -> oneshot::Receiver<()> {
        let (release_tx, release_rx) = oneshot::channel();
        let _ = self.tx.send(BatchSignal {
            _model: model,
            release: release_tx,
        });
        release_rx
    }
}

async fn coalescing_loop(
    mut rx: tokio::sync::mpsc::UnboundedReceiver<BatchSignal>,
    delay: Duration,
    max_batch_size: usize,
) {
    loop {
        let first = match rx.recv().await {
            Some(sig) => sig,
            None => return,
        };

        let mut batch = vec![first];
        let deadline = tokio::time::sleep(delay);
        tokio::pin!(deadline);

        loop {
            tokio::select! {
                sig = rx.recv() => {
                    match sig {
                        Some(sig) => {
                            batch.push(sig);
                            if batch.len() >= max_batch_size {
                                break;
                            }
                        }
                        None => break,
                    }
                }
                _ = &mut deadline => break,
            }
        }

        debug!("CoalescingBatch: releasing {} requests", batch.len());

        for sig in batch {
            let _ = sig.release.send(());
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// BatchingPlugin
// ─────────────────────────────────────────────────────────────────────────────

pub struct BatchingPlugin {
    mode: BatchMode,
    /// Queue de coalescing (mode coalescing)
    coalescer: Option<CoalescingBatch>,
    /// Stockage des requêtes en attente de batch (mode async)
    pending_requests: Option<Arc<Mutex<Vec<PylosRequest>>>>,
    /// Stockage des résultats de batch (mode async)
    batch_results: Option<BatchResultStore>,
    /// Compteur pour générer des IDs de batch uniques
    batch_id_counter: Arc<AtomicU64>,
}

impl BatchingPlugin {
    pub fn new(delay_ms: u64) -> Self {
        warn!(
            "BatchingPlugin: using legacy (stub) mode with {}ms delay. \
             Consider using with_coalescing() or with_async_batch().",
            delay_ms
        );
        let delay = Duration::from_millis(delay_ms);
        Self {
            mode: BatchMode::Coalescing {
                delay_ms,
                max_batch_size: usize::MAX,
            },
            coalescer: Some(CoalescingBatch::new(delay, usize::MAX)),
            pending_requests: None,
            batch_results: None,
            batch_id_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn with_coalescing(delay_ms: u64, max_batch_size: usize) -> Self {
        info!(
            "BatchingPlugin: coalescing mode (delay={}ms, max_batch_size={})",
            delay_ms, max_batch_size
        );
        let delay = Duration::from_millis(delay_ms);
        Self {
            mode: BatchMode::Coalescing {
                delay_ms,
                max_batch_size,
            },
            coalescer: Some(CoalescingBatch::new(delay, max_batch_size)),
            pending_requests: None,
            batch_results: None,
            batch_id_counter: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn with_async_batch(
        openai_base_url: String,
        api_key: String,
        batch_model: String,
        poll_interval_secs: u64,
    ) -> Self {
        info!(
            "BatchingPlugin: async batch mode (model={}, poll_interval={}s)",
            batch_model, poll_interval_secs
        );
        let pending = Arc::new(Mutex::new(Vec::new()));
        let results: BatchResultStore = Arc::new(Mutex::new(HashMap::new()));
        let results_clone = results.clone();
        let pending_clone = pending.clone();
        let id_counter = Arc::new(AtomicU64::new(1));
        let id_counter_clone = id_counter.clone();

        tokio::spawn(async_batch_loop(
            pending_clone,
            results_clone,
            id_counter_clone,
            openai_base_url.clone(),
            api_key.clone(),
            batch_model.clone(),
            poll_interval_secs,
        ));

        Self {
            mode: BatchMode::AsyncBatch {
                openai_base_url,
                api_key,
                batch_model,
                poll_interval_secs,
            },
            coalescer: None,
            pending_requests: Some(pending),
            batch_results: Some(results),
            batch_id_counter: id_counter,
        }
    }

    pub fn batch_results(&self) -> Option<BatchResultStore> {
        self.batch_results.clone()
    }
}

#[async_trait]
impl LlmPlugin for BatchingPlugin {
    fn name(&self) -> &str {
        "batching"
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        match &self.mode {
            BatchMode::Coalescing { .. } => {
                let model = request.model().to_string();
                debug!("BatchingPlugin: coalescing request for model '{}'", &model);

                if let Some(coalescer) = &self.coalescer {
                    let rx = coalescer.enqueue(model).await;
                    rx.await
                        .map_err(|_| PylosError::Internal("Batch cancelled".into()))?;
                }

                Ok(None)
            }
            BatchMode::AsyncBatch {
                batch_model,
                openai_base_url,
                ..
            } => {
                let model = request.model().to_string();
                debug!("BatchingPlugin: async batch request for model '{}'", &model);

                if let Some(pending) = &self.pending_requests {
                    pending.lock().await.push(request.clone());
                }

                let batch_id = format!(
                    "batch_{}_{}",
                    self.batch_id_counter.fetch_add(1, Ordering::SeqCst),
                    ctx.trace_id.as_deref().unwrap_or("unknown")
                );

                ctx.headers
                    .insert("x-pylos-batch-id".to_string(), batch_id.clone());
                ctx.headers
                    .insert("x-pylos-batch-status".to_string(), "pending".to_string());
                ctx.headers
                    .insert("x-pylos-batch-model".to_string(), batch_model.clone());

                // Store the trace ID so the background batch loop can correlate
                if let Some(ref trace_id) = ctx.trace_id {
                    ctx.headers
                        .insert("x-pylos-batch-trace-id".to_string(), trace_id.clone());
                }

                let batch_url = if openai_base_url.ends_with('/') {
                    format!("{}batches", openai_base_url)
                } else {
                    format!("{}/batches", openai_base_url)
                };

                warn!(
                    "BatchingPlugin: async batch request queued — submit via {:?}. \
                     Results available at x-pylos-batch-id header.",
                    batch_url
                );

                Ok(Some(PylosResponse::ChatCompletion(
                    pylos_core::domain::openai::ChatCompletionResponse {
                        id: batch_id,
                        object: "batch.result".to_string(),
                        created: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                        model,
                        choices: vec![pylos_core::domain::openai::ChatCompletionChoice {
                            index: 0,
                            message: pylos_core::domain::openai::ChatCompletionMessage {
                                role: pylos_core::domain::openai::MessageRole::Assistant,
                                content: Some(
                                    "Your request has been submitted for async batch processing. \
                                     Check the x-pylos-batch-id header for the batch ID."
                                        .to_string(),
                                ),
                                name: None,
                                tool_calls: None,
                                tool_call_id: None,
                                reasoning_content: None,
                            },
                            finish_reason: Some("stop".to_string()),
                        }],
                        usage: None,
                    },
                )))
            }
        }
    }

    async fn post_hook(
        &self,
        _request: &PylosRequest,
        _response: &mut PylosResponse,
        _ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Boucle de traitement asynchrone des batches (OpenAI Batch API)
// ─────────────────────────────────────────────────────────────────────────────

async fn async_batch_loop(
    pending: Arc<Mutex<Vec<PylosRequest>>>,
    results: BatchResultStore,
    id_counter: Arc<AtomicU64>,
    openai_base_url: String,
    api_key: String,
    batch_model: String,
    poll_interval_secs: u64,
) {
    let client = reqwest::Client::new();
    let interval = Duration::from_secs(poll_interval_secs);

    loop {
        tokio::time::sleep(interval).await;

        let batch_requests = {
            let mut p = pending.lock().await;
            if p.is_empty() {
                continue;
            }
            std::mem::take(&mut *p)
        };

        debug!(
            "AsyncBatch: processing {} queued requests for model '{}'",
            batch_requests.len(),
            &batch_model
        );

        if let Err(e) = submit_openai_batch(
            &client,
            &openai_base_url,
            &api_key,
            &batch_model,
            &batch_requests,
            &results,
            &id_counter,
        )
        .await
        {
            warn!("AsyncBatch: batch submission failed: {}. Re-queuing.", e);
            // Re-queue on failure
            let mut p = pending.lock().await;
            p.extend(batch_requests);
        }
    }
}

async fn submit_openai_batch(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    batch_model: &str,
    requests: &[PylosRequest],
    results: &BatchResultStore,
    id_counter: &AtomicU64,
) -> Result<(), PylosError> {
    // ── 1. Build JSONL lines ────────────────────────────────────────────
    let mut jsonl_lines = Vec::new();
    let mut custom_ids: Vec<String> = Vec::new();

    for req in requests {
        let custom_id = format!(
            "req_{}",
            id_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        );
        custom_ids.push(custom_id.clone());

        let body = match req {
            PylosRequest::ChatCompletion(chat) => {
                let mut body = serde_json::to_value(chat)
                    .map_err(|e| PylosError::Internal(format!("Serialization error: {}", e)))?;
                if let Some(obj) = body.as_object_mut() {
                    obj.insert(
                        "model".to_string(),
                        serde_json::Value::String(batch_model.to_string()),
                    );
                    obj.remove("stream");
                }
                body
            }
            _ => {
                warn!("AsyncBatch: skipping non-chat request");
                continue;
            }
        };

        let line = serde_json::json!({
            "custom_id": custom_id,
            "method": "POST",
            "url": "/v1/chat/completions",
            "body": body,
        });

        jsonl_lines.push(
            serde_json::to_string(&line)
                .map_err(|e| PylosError::Internal(format!("JSONL encoding error: {}", e)))?,
        );
    }

    if jsonl_lines.is_empty() {
        return Ok(());
    }

    let jsonl_content = jsonl_lines.join("\n");

    // ── 2. Upload file to OpenAI ────────────────────────────────────────
    let files_url = if base_url.ends_with('/') {
        format!("{}files", base_url)
    } else {
        format!("{}/files", base_url)
    };

    let file_part = reqwest::multipart::Part::text(jsonl_content)
        .file_name("batch_input.jsonl")
        .mime_str("application/jsonl")
        .map_err(|e| PylosError::Internal(format!("MIME error: {}", e)))?;

    let form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("purpose", "batch");

    let file_resp = client
        .post(&files_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| PylosError::Internal(format!("File upload failed: {}", e)))?;

    if !file_resp.status().is_success() {
        let status = file_resp.status();
        let body = file_resp
            .text()
            .await
            .unwrap_or_else(|_| "no body".to_string());
        return Err(PylosError::Internal(format!(
            "File upload failed ({}): {}",
            status, body
        )));
    }

    let file_data: OpenAIFileResponse = file_resp
        .json()
        .await
        .map_err(|e| PylosError::Internal(format!("File response parse error: {}", e)))?;

    let input_file_id = file_data.id;
    debug!("AsyncBatch: uploaded file {}", &input_file_id);

    // ── 3. Create batch ────────────────────────────────────────────────
    let batch_url = if base_url.ends_with('/') {
        format!("{}batches", base_url)
    } else {
        format!("{}/batches", base_url)
    };

    let create_body = serde_json::json!({
        "input_file_id": input_file_id,
        "endpoint": "/v1/chat/completions",
        "completion_window": "24h",
        "metadata": {
            "pylos_batch": "true",
        },
    });

    let batch_resp = client
        .post(&batch_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&create_body)
        .send()
        .await
        .map_err(|e| PylosError::Internal(format!("Batch creation failed: {}", e)))?;

    if !batch_resp.status().is_success() {
        let status = batch_resp.status();
        let body = batch_resp
            .text()
            .await
            .unwrap_or_else(|_| "no body".to_string());
        return Err(PylosError::Internal(format!(
            "Batch creation failed ({}): {}",
            status, body
        )));
    }

    let batch_data: OpenAIBatchResponse = batch_resp
        .json()
        .await
        .map_err(|e| PylosError::Internal(format!("Batch response parse error: {}", e)))?;

    let batch_id = batch_data.id.clone();
    info!(
        "AsyncBatch: created batch {} ({} requests, file {})",
        &batch_id,
        jsonl_lines.len(),
        &input_file_id
    );

    // ── 4. Poll until completion ───────────────────────────────────────
    let poll_interval = Duration::from_secs(30);
    let max_polls = 2880u32; // 24h at 30s intervals

    for poll_count in 0..max_polls {
        tokio::time::sleep(poll_interval).await;

        let status_resp = client
            .get(format!("{}/{}", batch_url, batch_id).as_str())
            .header("Authorization", format!("Bearer {}", api_key))
            .send()
            .await;

        let status_data: OpenAIBatchResponse = match status_resp {
            Ok(r) if r.status().is_success() => match r.json().await {
                Ok(d) => d,
                Err(e) => {
                    warn!("AsyncBatch: status parse error: {}", e);
                    continue;
                }
            },
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                warn!("AsyncBatch: status check failed ({}): {}", status, body);
                continue;
            }
            Err(e) => {
                warn!("AsyncBatch: status request error: {}", e);
                continue;
            }
        };

        match status_data.status.as_str() {
            "completed" => {
                info!("AsyncBatch: batch {} completed", &batch_id);

                if let Some(output_file_id) = &status_data.output_file_id {
                    if let Err(e) = download_and_store_results(
                        client,
                        base_url,
                        api_key,
                        output_file_id,
                        results,
                        &custom_ids,
                    )
                    .await
                    {
                        warn!(
                            "AsyncBatch: failed to download results for batch {}: {}",
                            &batch_id, e
                        );
                    }
                }
                return Ok(());
            }
            "failed" | "expired" | "cancelled" => {
                let status = &status_data.status;
                warn!(
                    "AsyncBatch: batch {} ended with status '{}'",
                    &batch_id, status
                );
                return Err(PylosError::Internal(format!(
                    "Batch {} ended with status '{}'",
                    batch_id, status
                )));
            }
            s => {
                if poll_count % 10 == 0 {
                    debug!(
                        "AsyncBatch: batch {} status '{}' (poll {}/{})",
                        &batch_id, s, poll_count, max_polls
                    );
                }
            }
        }
    }

    Err(PylosError::Internal(format!(
        "AsyncBatch: batch {} did not complete within the polling window",
        batch_id
    )))
}

async fn download_and_store_results(
    client: &reqwest::Client,
    base_url: &str,
    api_key: &str,
    output_file_id: &str,
    results: &BatchResultStore,
    _custom_ids: &[String],
) -> Result<(), PylosError> {
    let files_url = if base_url.ends_with('/') {
        format!("{}files/{}/content", base_url, output_file_id)
    } else {
        format!("{}/files/{}/content", base_url, output_file_id)
    };

    let resp = client
        .get(&files_url)
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await
        .map_err(|e| PylosError::Internal(format!("File download failed: {}", e)))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(PylosError::Internal(format!(
            "File download failed ({}): {}",
            status, body
        )));
    }

    let content = resp
        .text()
        .await
        .map_err(|e| PylosError::Internal(format!("File read error: {}", e)))?;

    let mut stored = results.lock().await;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<serde_json::Value>(line) {
            Ok(val) => {
                let custom_id = val
                    .get("custom_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let response_body = val.get("response").and_then(|v| v.get("body"));

                if let Some(body) = response_body {
                    match serde_json::from_value::<pylos_core::domain::openai::ChatCompletionResponse>(
                        body.clone(),
                    ) {
                        Ok(chat_resp) => {
                            stored.insert(
                                custom_id.clone(),
                                PylosResponse::ChatCompletion(chat_resp),
                            );
                        }
                        Err(_) => {
                            stored.insert(
                                custom_id.clone(),
                                PylosResponse::ChatCompletion(
                                    pylos_core::domain::openai::ChatCompletionResponse {
                                        id: custom_id.clone(),
                                        object: "chat.completion".to_string(),
                                        created: std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs()
                                            as i64,
                                        model: "unknown".to_string(),
                                        choices: vec![],
                                        usage: None,
                                    },
                                ),
                            );
                        }
                    }
                }

                // Also check for errors
                if let Some(error) = val.get("error") {
                    warn!("AsyncBatch: request {} had error: {:?}", custom_id, error);
                }
            }
            Err(e) => {
                warn!("AsyncBatch: failed to parse result line: {}", e);
            }
        }
    }

    info!(
        "AsyncBatch: stored {} results from output file {}",
        stored.len(),
        output_file_id
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_coalescing_basic() {
        let plugin = BatchingPlugin::with_coalescing(10, 10);
        assert!(matches!(plugin.mode, BatchMode::Coalescing { .. }));
        assert!(plugin.coalescer.is_some());
    }

    #[tokio::test]
    async fn test_async_batch_create() {
        let plugin = BatchingPlugin::with_async_batch(
            "https://api.openai.com/v1".to_string(),
            "sk-test".to_string(),
            "gpt-4o-mini".to_string(),
            60,
        );
        assert!(matches!(plugin.mode, BatchMode::AsyncBatch { .. }));
        assert!(plugin.batch_results.is_some());
        assert!(plugin.pending_requests.is_some());
    }
}
