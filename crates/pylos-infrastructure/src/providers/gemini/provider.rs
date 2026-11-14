use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use prometheus::{CounterVec, IntCounter, IntGaugeVec, Opts};
use reqwest::Client;
use std::sync::{Mutex, OnceLock};
use tracing::{debug, warn};

use pylos_core::domain::embedding::{EmbeddingRequest, EmbeddingResponse};
use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse};
use pylos_core::domain::traits::{ChunkStream, Provider};
use pylos_core::error::PylosError;

use super::converters::{
    from_gemini_embed_response, from_gemini_response, from_gemini_stream_chunk, map_gemini_error,
    to_gemini_embed_request, to_gemini_request, GeminiBatchEmbedResponse, GeminiResponse,
};

const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

// Prometheus metrics for the Gemini API Key pool
static KEY_SWITCHES: OnceLock<IntCounter> = OnceLock::new();
static PROJECT_STATUS: OnceLock<IntGaugeVec> = OnceLock::new();
static PROJECT_REQUESTS: OnceLock<CounterVec> = OnceLock::new();

fn get_key_switches_metric() -> &'static IntCounter {
    KEY_SWITCHES.get_or_init(|| {
        #[allow(clippy::expect_used)] // Static compile-time Prometheus metric descriptor
        let counter = IntCounter::new(
            "pylos_gemini_key_switches_total",
            "Total number of API key switches in the Gemini pool",
        )
        .expect("static prometheus metric");
        prometheus::register(Box::new(counter.clone())).ok();
        counter
    })
}

fn get_project_status_metric() -> &'static IntGaugeVec {
    PROJECT_STATUS.get_or_init(|| {
        #[allow(clippy::expect_used)] // Static compile-time Prometheus metric descriptor
        let gauge = IntGaugeVec::new(
            Opts::new(
                "pylos_gemini_project_status",
                "Status of each Gemini project API key (1 = Active, 0 = Quota Expired)",
            ),
            &["project_number"],
        )
        .expect("static prometheus metric");
        prometheus::register(Box::new(gauge.clone())).ok();
        gauge
    })
}

fn get_project_requests_metric() -> &'static CounterVec {
    PROJECT_REQUESTS.get_or_init(|| {
        #[allow(clippy::expect_used)] // Static compile-time Prometheus metric descriptor
        let counter = CounterVec::new(
            Opts::new(
                "pylos_gemini_requests_total",
                "Total number of Gemini requests distributed by project",
            ),
            &["project_number"],
        )
        .expect("static prometheus metric");
        prometheus::register(Box::new(counter.clone())).ok();
        counter
    })
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
struct GeminiKeyInfo {
    api_key: String,
    name: String,
    project_name: String,
    project_number: String,
    quota_exhausted_until: Option<std::time::Instant>,
}

// ─────────────────────────────────────────────────────────────────────────────
// GeminiProvider — Google Gemini API
// Bifrost source: core/providers/gemini/gemini.go
// ─────────────────────────────────────────────────────────────────────────────

pub struct GeminiProvider {
    client: Client,
    keys_pool: Mutex<Vec<GeminiKeyInfo>>,
}

impl GeminiProvider {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .unwrap_or_else(|_| Client::new());

        let mut keys = Vec::new();
        for suffix in &["AI1", "AI2", "AI3"] {
            let api_key_var = format!("{}_API_KEY", suffix);
            let name_var = format!("{}_NAME", suffix);
            let project_name_var = format!("{}_PROJECT_NAME", suffix);
            let project_number_var = format!("{}_PROJECT_NUMBER", suffix);

            if let Ok(api_key) = std::env::var(&api_key_var) {
                if !api_key.is_empty() {
                    let name = std::env::var(&name_var)
                        .unwrap_or_else(|_| format!("Gemini Key {}", suffix));
                    let project_name = std::env::var(&project_name_var).unwrap_or_default();
                    let project_number = std::env::var(&project_number_var).unwrap_or_default();
                    keys.push(GeminiKeyInfo {
                        api_key,
                        name,
                        project_name,
                        project_number,
                        quota_exhausted_until: None,
                    });
                }
            }
        }

        Self {
            client,
            keys_pool: Mutex::new(keys),
        }
    }

    fn base_url<'a>(&self, config: &'a ProviderConfig) -> &'a str {
        config.base_url.as_deref().unwrap_or(DEFAULT_BASE_URL)
    }

    fn select_key<'a>(&self, config: &'a ProviderConfig) -> Option<&'a str> {
        if config.keys.is_empty() {
            return None;
        }
        let total_weight: f64 = config.keys.iter().map(|k| k.weight).sum();
        if total_weight <= 0.0 {
            return Some(&config.keys[0].value);
        }
        let mut rng_val = fastrand::f64() * total_weight;
        for key in &config.keys {
            rng_val -= key.weight;
            if rng_val <= 0.0 {
                return Some(&key.value);
            }
        }
        config.keys.last().map(|k| k.value.as_str())
    }

    /// Normalise le nom du modèle : "google/gemini-2.0-flash" → "gemini-2.0-flash"
    fn normalize_model<'a>(&self, model: &'a str) -> &'a str {
        if let Some(stripped) = model.strip_prefix("google/") {
            return stripped;
        }
        if let Some(stripped) = model.strip_prefix("gemini/") {
            return stripped;
        }
        model
    }

    fn get_active_key(&self) -> Result<Option<GeminiKeyInfo>, PylosError> {
        let mut pool = self.keys_pool.lock().unwrap_or_else(|e| e.into_inner());
        if pool.is_empty() {
            return Ok(None);
        }

        let now = std::time::Instant::now();
        // Clean expired cooldowns and update gauges
        for key in pool.iter_mut() {
            if let Some(until) = key.quota_exhausted_until {
                if now >= until {
                    key.quota_exhausted_until = None;
                    get_project_status_metric()
                        .with_label_values(&[&key.project_number])
                        .set(1);
                } else {
                    get_project_status_metric()
                        .with_label_values(&[&key.project_number])
                        .set(0);
                }
            } else {
                get_project_status_metric()
                    .with_label_values(&[&key.project_number])
                    .set(1);
            }
        }

        // Find the first key that is not in cooldown
        let mut selected_idx = None;
        for (idx, key) in pool.iter().enumerate() {
            if key.quota_exhausted_until.is_none() {
                selected_idx = Some(idx);
                break;
            }
        }

        if let Some(idx) = selected_idx {
            let key = pool[idx].clone();
            // Increment requests counter for this project
            get_project_requests_metric()
                .with_label_values(&[&key.project_number])
                .inc();

            // Rotate the pool so that the element after idx becomes the first one to check
            let rotate_amount = (idx + 1) % pool.len();
            pool.rotate_left(rotate_amount);
            return Ok(Some(key));
        }

        // All keys are exhausted
        Err(PylosError::RateLimitExceeded(
            "All Gemini API keys in the pool have exhausted their quota".into(),
        ))
    }

    fn mark_key_exhausted(&self, api_key: &str) {
        let mut pool = self.keys_pool.lock().unwrap_or_else(|e| e.into_inner());
        let now = std::time::Instant::now();
        let cooldown = now + std::time::Duration::from_secs(60);

        let mut exhausted_index = None;
        for (idx, key) in pool.iter_mut().enumerate() {
            if key.api_key == api_key {
                key.quota_exhausted_until = Some(cooldown);
                get_project_status_metric()
                    .with_label_values(&[&key.project_number])
                    .set(0);
                exhausted_index = Some(idx);
                break;
            }
        }

        if let Some(idx) = exhausted_index {
            let exhausted_key = pool[idx].clone();
            get_key_switches_metric().inc();

            // Find next active key for logging
            let mut next_key_name = "NONE".to_string();
            for offset in 1..pool.len() {
                let next_idx = (idx + offset) % pool.len();
                if pool[next_idx].quota_exhausted_until.is_none() {
                    next_key_name = pool[next_idx].name.clone();
                    break;
                }
            }

            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();

            warn!(
                event = "gemini_quota_exceeded",
                secret_name = %exhausted_key.name,
                project_number = %exhausted_key.project_number,
                timestamp = %timestamp,
                next_key_name = %next_key_name,
                "Gemini API quota exceeded, switching keys"
            );
        }
    }
}

impl Default for GeminiProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Provider for GeminiProvider {
    fn name(&self) -> &str {
        "gemini"
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let model = self.normalize_model(&req.model);
                let url = format!("{}/models/{}:generateContent", base_url, model);
                let gemini_req = to_gemini_request(req);

                let pool_len = {
                    let pool = self.keys_pool.lock().unwrap_or_else(|e| e.into_inner());
                    pool.len()
                };

                if pool_len > 0 {
                    let mut attempts = 0;
                    loop {
                        let active_key = match self.get_active_key()? {
                            Some(k) => k,
                            None => {
                                return Err(PylosError::RateLimitExceeded(
                                    "All Gemini API keys in the pool have exhausted their quota"
                                        .into(),
                                ));
                            }
                        };
                        debug!(provider = "gemini", model = %model, url = %url, key_name = %active_key.name, "Sending Gemini generateContent with pool key");

                        let response = self
                            .client
                            .post(&url)
                            .header("x-goog-api-key", &active_key.api_key)
                            .json(&gemini_req)
                            .send()
                            .await
                            .map_err(|e| {
                                if e.is_timeout() {
                                    PylosError::Timeout(e.to_string())
                                } else {
                                    PylosError::ProviderError {
                                        provider: "gemini".into(),
                                        message: e.to_string(),
                                    }
                                }
                            })?;

                        let status = response.status().as_u16();
                        if status == 429 {
                            self.mark_key_exhausted(&active_key.api_key);
                            attempts += 1;
                            if attempts < pool_len {
                                continue;
                            }
                            let body = response.text().await.unwrap_or_default();
                            return Err(map_gemini_error(status, &body));
                        }

                        if !response.status().is_success() {
                            let body = response.text().await.unwrap_or_default();
                            warn!(provider = "gemini", status = status, body = %body, "Gemini returned error");
                            return Err(map_gemini_error(status, &body));
                        }

                        let gemini_resp: GeminiResponse = response.json().await.map_err(|e| {
                            PylosError::Internal(format!("Failed to parse Gemini response: {}", e))
                        })?;

                        debug!(provider = "gemini", model = %model, "Gemini generateContent successful");
                        return Ok(from_gemini_response(gemini_resp, model));
                    }
                } else {
                    let api_key = self.select_key(config).ok_or_else(|| {
                        PylosError::InvalidRequest("No API key configured for Gemini".into())
                    })?;

                    debug!(provider = "gemini", model = %model, url = %url, "Sending Gemini generateContent");

                    let response = self
                        .client
                        .post(&url)
                        .header("x-goog-api-key", api_key)
                        .json(&gemini_req)
                        .send()
                        .await
                        .map_err(|e| {
                            if e.is_timeout() {
                                PylosError::Timeout(e.to_string())
                            } else {
                                PylosError::ProviderError {
                                    provider: "gemini".into(),
                                    message: e.to_string(),
                                }
                            }
                        })?;

                    let status = response.status().as_u16();
                    if !response.status().is_success() {
                        let body = response.text().await.unwrap_or_default();
                        warn!(provider = "gemini", status = status, body = %body, "Gemini returned error");
                        return Err(map_gemini_error(status, &body));
                    }

                    let gemini_resp: GeminiResponse = response.json().await.map_err(|e| {
                        PylosError::Internal(format!("Failed to parse Gemini response: {}", e))
                    })?;

                    debug!(provider = "gemini", model = %model, "Gemini generateContent successful");
                    Ok(from_gemini_response(gemini_resp, model))
                }
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::InvalidRequest(
                "Request type not supported by complete() on Gemini".into(),
            )),
        }
    }

    async fn stream(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        let base_url = self.base_url(config);

        match request {
            PylosRequest::ChatCompletion(req) => {
                let model = self.normalize_model(&req.model).to_string();
                let url = format!(
                    "{}/models/{}:streamGenerateContent?alt=sse",
                    base_url, model
                );
                let gemini_req = to_gemini_request(req);

                let pool_len = {
                    let pool = self.keys_pool.lock().unwrap_or_else(|e| e.into_inner());
                    pool.len()
                };

                if pool_len > 0 {
                    let mut attempts = 0;
                    loop {
                        let active_key = match self.get_active_key()? {
                            Some(k) => k,
                            None => {
                                return Err(PylosError::RateLimitExceeded(
                                    "All Gemini API keys in the pool have exhausted their quota"
                                        .into(),
                                ));
                            }
                        };
                        debug!(provider = "gemini", model = %model, url = %url, key_name = %active_key.name, "Sending Gemini streaming request with pool key");

                        let response = self
                            .client
                            .post(&url)
                            .header("x-goog-api-key", &active_key.api_key)
                            .json(&gemini_req)
                            .send()
                            .await
                            .map_err(|e| {
                                if e.is_timeout() {
                                    PylosError::Timeout(e.to_string())
                                } else {
                                    PylosError::ProviderError {
                                        provider: "gemini".into(),
                                        message: e.to_string(),
                                    }
                                }
                            })?;

                        let status = response.status().as_u16();
                        if status == 429 {
                            self.mark_key_exhausted(&active_key.api_key);
                            attempts += 1;
                            if attempts < pool_len {
                                continue;
                            }
                            let body = response.text().await.unwrap_or_default();
                            return Err(map_gemini_error(status, &body));
                        }

                        if !response.status().is_success() {
                            let body = response.text().await.unwrap_or_default();
                            return Err(map_gemini_error(status, &body));
                        }

                        let model_clone = model.clone();
                        let stream =
                            response
                                .bytes_stream()
                                .eventsource()
                                .filter_map(move |event| {
                                    let m = model_clone.clone();
                                    async move {
                                        match event {
                                            Ok(e) => {
                                                let data = e.data.trim().to_string();
                                                if data.is_empty() {
                                                    return None;
                                                }
                                                match serde_json::from_str::<GeminiResponse>(&data)
                                                {
                                                    Ok(chunk) => {
                                                        from_gemini_stream_chunk(chunk, &m).map(Ok)
                                                    }
                                                    Err(_) => None,
                                                }
                                            }
                                            Err(e) => Some(Err(PylosError::ProviderError {
                                                provider: "gemini".into(),
                                                message: e.to_string(),
                                            })),
                                        }
                                    }
                                });

                        return Ok(Box::pin(stream));
                    }
                } else {
                    let api_key = self.select_key(config).ok_or_else(|| {
                        PylosError::InvalidRequest("No API key configured for Gemini".into())
                    })?;

                    debug!(provider = "gemini", model = %model, url = %url, "Sending Gemini streaming request");

                    let response = self
                        .client
                        .post(&url)
                        .header("x-goog-api-key", api_key)
                        .json(&gemini_req)
                        .send()
                        .await
                        .map_err(|e| {
                            if e.is_timeout() {
                                PylosError::Timeout(e.to_string())
                            } else {
                                PylosError::ProviderError {
                                    provider: "gemini".into(),
                                    message: e.to_string(),
                                }
                            }
                        })?;

                    let status = response.status().as_u16();
                    if !response.status().is_success() {
                        let body = response.text().await.unwrap_or_default();
                        return Err(map_gemini_error(status, &body));
                    }

                    let model_clone = model.clone();
                    let stream = response
                        .bytes_stream()
                        .eventsource()
                        .filter_map(move |event| {
                            let m = model_clone.clone();
                            async move {
                                match event {
                                    Ok(e) => {
                                        let data = e.data.trim().to_string();
                                        if data.is_empty() {
                                            return None;
                                        }
                                        match serde_json::from_str::<GeminiResponse>(&data) {
                                            Ok(chunk) => {
                                                from_gemini_stream_chunk(chunk, &m).map(Ok)
                                            }
                                            Err(_) => None,
                                        }
                                    }
                                    Err(e) => Some(Err(PylosError::ProviderError {
                                        provider: "gemini".into(),
                                        message: e.to_string(),
                                    })),
                                }
                            }
                        });

                    Ok(Box::pin(stream))
                }
            }
            PylosRequest::TextCompletion(_)
            | PylosRequest::Embedding(_)
            | PylosRequest::Image(_) => Err(PylosError::InvalidRequest(
                "Streaming not supported for this request type".into(),
            )),
        }
    }

    async fn embed(
        &self,
        request: &EmbeddingRequest,
        config: &ProviderConfig,
    ) -> Result<EmbeddingResponse, PylosError> {
        let base_url = self.base_url(config);
        let model = self.normalize_model(&request.model);
        let url = format!("{}/models/{}:batchEmbedContents", base_url, model);

        let texts = request.input.as_strings();
        let gemini_req = to_gemini_embed_request(model, texts, request.dimensions);

        let pool_len = {
            let pool = self.keys_pool.lock().unwrap_or_else(|e| e.into_inner());
            pool.len()
        };

        if pool_len > 0 {
            let mut attempts = 0;
            loop {
                let active_key = match self.get_active_key()? {
                    Some(k) => k,
                    None => {
                        return Err(PylosError::RateLimitExceeded(
                            "All Gemini API keys in the pool have exhausted their quota".into(),
                        ));
                    }
                };
                debug!(provider = "gemini", model = %model, url = %url, key_name = %active_key.name, "Sending Gemini batchEmbedContents with pool key");

                let response = self
                    .client
                    .post(&url)
                    .header("x-goog-api-key", &active_key.api_key)
                    .json(&gemini_req)
                    .send()
                    .await
                    .map_err(|e| PylosError::ProviderError {
                        provider: "gemini".into(),
                        message: e.to_string(),
                    })?;

                let status = response.status().as_u16();
                if status == 429 {
                    self.mark_key_exhausted(&active_key.api_key);
                    attempts += 1;
                    if attempts < pool_len {
                        continue;
                    }
                    let body = response.text().await.unwrap_or_default();
                    return Err(map_gemini_error(status, &body));
                }

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    return Err(map_gemini_error(status, &body));
                }

                let gemini_resp: GeminiBatchEmbedResponse = response.json().await.map_err(|e| {
                    PylosError::Internal(format!("Failed to parse Gemini embed response: {}", e))
                })?;

                return Ok(from_gemini_embed_response(gemini_resp, model));
            }
        } else {
            let api_key = self.select_key(config).ok_or_else(|| {
                PylosError::InvalidRequest("No API key configured for Gemini".into())
            })?;

            debug!(provider = "gemini", model = %model, url = %url, "Sending Gemini batchEmbedContents");

            let response = self
                .client
                .post(&url)
                .header("x-goog-api-key", api_key)
                .json(&gemini_req)
                .send()
                .await
                .map_err(|e| PylosError::ProviderError {
                    provider: "gemini".into(),
                    message: e.to_string(),
                })?;

            let status = response.status().as_u16();
            if !response.status().is_success() {
                let body = response.text().await.unwrap_or_default();
                return Err(map_gemini_error(status, &body));
            }

            let gemini_resp: GeminiBatchEmbedResponse = response.json().await.map_err(|e| {
                PylosError::Internal(format!("Failed to parse Gemini embed response: {}", e))
            })?;

            Ok(from_gemini_embed_response(gemini_resp, model))
        }
    }

    async fn health_check(&self, config: &ProviderConfig) -> Result<(), PylosError> {
        let base_url = self.base_url(config);
        let url = format!("{}/models", base_url);

        let pool_len = {
            let pool = self.keys_pool.lock().unwrap_or_else(|e| e.into_inner());
            pool.len()
        };

        if pool_len > 0 {
            let mut attempts = 0;
            loop {
                let active_key = match self.get_active_key()? {
                    Some(k) => k,
                    None => {
                        return Err(PylosError::RateLimitExceeded(
                            "All Gemini API keys in the pool have exhausted their quota".into(),
                        ));
                    }
                };
                debug!(provider = "gemini", url = %url, key_name = %active_key.name, "Testing provider connectivity with pool key");

                let response = self
                    .client
                    .get(&url)
                    .header("x-goog-api-key", &active_key.api_key)
                    .send()
                    .await
                    .map_err(|e| PylosError::ProviderError {
                        provider: "gemini".into(),
                        message: e.to_string(),
                    })?;

                let status = response.status().as_u16();
                if status == 429 {
                    self.mark_key_exhausted(&active_key.api_key);
                    attempts += 1;
                    if attempts < pool_len {
                        continue;
                    }
                    let body = response.text().await.unwrap_or_default();
                    return Err(map_gemini_error(status, &body));
                }

                if !response.status().is_success() {
                    let body = response.text().await.unwrap_or_default();
                    warn!(provider = "gemini", status = status, body = %body, "Health check failed");
                    return Err(map_gemini_error(status, &body));
                }

                return Ok(());
            }
        } else {
            let api_key = self.select_key(config).ok_or_else(|| {
                PylosError::InvalidRequest("No API key configured for Gemini".into())
            })?;

            debug!(provider = "gemini", url = %url, "Testing provider connectivity");

            let response = self
                .client
                .get(&url)
                .header("x-goog-api-key", api_key)
                .send()
                .await
                .map_err(|e| PylosError::ProviderError {
                    provider: "gemini".into(),
                    message: e.to_string(),
                })?;

            let status = response.status().as_u16();
            if !response.status().is_success() {
                let body = response.text().await.unwrap_or_default();
                warn!(provider = "gemini", status = status, body = %body, "Health check failed");
                return Err(map_gemini_error(status, &body));
            }

            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_model() {
        let p = GeminiProvider::new();
        assert_eq!(p.normalize_model("gemini-2.0-flash"), "gemini-2.0-flash");
        assert_eq!(
            p.normalize_model("google/gemini-2.0-flash"),
            "gemini-2.0-flash"
        );
        assert_eq!(
            p.normalize_model("gemini/gemini-2.0-flash"),
            "gemini-2.0-flash"
        );
    }

    #[test]
    fn test_pool_loading_and_failover() {
        std::env::set_var("AI1_API_KEY", "key1");
        std::env::set_var("AI1_NAME", "Gemini Key 1");
        std::env::set_var("AI1_PROJECT_NUMBER", "111");

        std::env::set_var("AI2_API_KEY", "key2");
        std::env::set_var("AI2_NAME", "Gemini Key 2");
        std::env::set_var("AI2_PROJECT_NUMBER", "222");

        let p = GeminiProvider::new();

        let k1 = p.get_active_key().unwrap().unwrap();
        assert_eq!(k1.api_key, "key1");
        assert_eq!(k1.name, "Gemini Key 1");
        assert_eq!(k1.project_number, "111");

        // Mark key 1 exhausted
        p.mark_key_exhausted("key1");

        let k2 = p.get_active_key().unwrap().unwrap();
        assert_eq!(k2.api_key, "key2");
        assert_eq!(k2.name, "Gemini Key 2");
        assert_eq!(k2.project_number, "222");

        // Mark key 2 exhausted
        p.mark_key_exhausted("key2");

        // Now both are exhausted
        let res = p.get_active_key();
        assert!(res.is_err());

        // Clean env vars
        std::env::remove_var("AI1_API_KEY");
        std::env::remove_var("AI1_NAME");
        std::env::remove_var("AI1_PROJECT_NUMBER");
        std::env::remove_var("AI2_API_KEY");
        std::env::remove_var("AI2_NAME");
        std::env::remove_var("AI2_PROJECT_NUMBER");
    }
}
