use async_trait::async_trait;
use tracing::{debug, warn};

use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionResponse, MessageRole};
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

pub struct StructuredOutputPlugin {
    client: reqwest::Client,
    correction_model: Option<String>,
    max_retries: u32,
    pylos_base_url: Option<String>,
    pylos_api_key: Option<String>,
}

#[allow(clippy::new_without_default)]
impl StructuredOutputPlugin {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            correction_model: None,
            max_retries: 0,
            pylos_base_url: None,
            pylos_api_key: None,
        }
    }

    pub fn with_correction(
        correction_model: String,
        max_retries: u32,
        pylos_base_url: String,
        pylos_api_key: Option<String>,
    ) -> Self {
        let client = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .unwrap_or_default();
        Self {
            client,
            correction_model: Some(correction_model),
            max_retries,
            pylos_base_url: Some(pylos_base_url),
            pylos_api_key,
        }
    }

    fn validate_content(
        content: &str,
        format_type: &str,
        schema_val: Option<&serde_json::Value>,
    ) -> Result<(), String> {
        let parsed = serde_json::from_str::<serde_json::Value>(content)
            .map_err(|e| format!("Response content is not valid JSON: {}", e))?;

        if format_type == "json_schema" {
            if let Some(schema) = schema_val {
                let compiled = jsonschema::JSONSchema::compile(schema)
                    .map_err(|e| format!("Invalid JSON Schema in request: {}", e))?;

                let validation_result = compiled.validate(&parsed);
                if let Err(errors) = validation_result {
                    let err_msgs: Vec<String> = errors
                        .map(|err| format!("Path: {}, Error: {}", err.instance_path, err))
                        .collect();
                    return Err(format!(
                        "Response format error: LLM output failed JSON Schema validation. Errors: {}",
                        err_msgs.join("; ")
                    ));
                }
            }
        }

        Ok(())
    }

    fn extract_json(text: &str) -> Option<String> {
        if let Some(start) = text.find("```json") {
            let after_fence = &text[start + 7..];
            if let Some(end) = after_fence.find("```") {
                return Some(after_fence[..end].trim().to_string());
            }
        }
        if let Some(start) = text.find('{') {
            let mut depth = 0u32;
            let mut in_string = false;
            let mut escaped = false;
            for (i, c) in text[start..].char_indices() {
                if escaped {
                    escaped = false;
                    continue;
                }
                match c {
                    '"' => in_string = !in_string,
                    '\\' if in_string => escaped = true,
                    '{' if !in_string => depth = depth.saturating_add(1),
                    '}' if !in_string => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Some(text[start..start + i + 1].to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        None
    }

    async fn attempt_correction(
        &self,
        request: &pylos_core::domain::openai::ChatCompletionRequest,
        invalid_content: &str,
        validation_error: &str,
    ) -> Result<String, PylosError> {
        let base_url = self
            .pylos_base_url
            .as_deref()
            .unwrap_or("http://localhost:3000");
        let url = format!("{}/v1/chat/completions", base_url.trim_end_matches('/'));

        let model = self.correction_model.as_deref().unwrap_or("gpt-4o-mini");

        let correction_instruction = format!(
            "The following response was generated but failed validation:\n\n```json\n{}\n```\n\nValidation error:\n{}\n\nPlease provide a corrected version that is valid JSON. Return ONLY the corrected JSON with no additional text, markdown formatting, or explanation.",
            invalid_content, validation_error
        );

        let mut messages = request.messages.clone();
        messages.push(ChatCompletionMessage {
            role: MessageRole::User,
            content: Some(correction_instruction),
            ..Default::default()
        });

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": false,
        });

        let mut req = self.client.post(&url).json(&body);
        if let Some(ref key) = self.pylos_api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let resp = req.send().await.map_err(|e| {
            warn!("StructuredOutputPlugin: Correction request failed: {}", e);
            PylosError::Internal(format!("Correction request to '{}' failed: {}", model, e))
        })?;

        let status = resp.status();
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            warn!("StructuredOutputPlugin: Correction API error: {}", err_text);
            return Err(PylosError::Internal(format!(
                "Correction API returned status {}: {}",
                status, err_text
            )));
        }

        let completion: ChatCompletionResponse = resp.json().await.map_err(|e| {
            warn!(
                "StructuredOutputPlugin: Failed to parse correction response: {}",
                e
            );
            PylosError::Internal(format!("Failed to parse correction response: {}", e))
        })?;

        let raw = completion
            .choices
            .first()
            .and_then(|c| c.message.content.as_deref())
            .unwrap_or("")
            .to_string();

        Ok(Self::extract_json(&raw).unwrap_or(raw))
    }
}

#[async_trait]
impl LlmPlugin for StructuredOutputPlugin {
    fn name(&self) -> &str {
        "structured_output"
    }

    async fn post_hook(
        &self,
        request: &PylosRequest,
        response: &mut PylosResponse,
        _ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        let chat_req = match request {
            PylosRequest::ChatCompletion(ref req) => req,
            _ => return Ok(()),
        };

        if chat_req.stream.unwrap_or(false) {
            return Ok(());
        }

        let response_format = match &chat_req.response_format {
            Some(fmt) => fmt,
            None => return Ok(()),
        };

        let format_type = &response_format.format_type;
        if format_type != "json_object" && format_type != "json_schema" {
            return Ok(());
        }

        let chat_resp = match response {
            PylosResponse::ChatCompletion(ref mut resp) => resp,
            _ => return Ok(()),
        };

        for choice in &mut chat_resp.choices {
            let content = match &choice.message.content {
                Some(c) => c.clone(),
                None => continue,
            };

            let schema_val = response_format.json_schema.as_ref();

            match Self::validate_content(&content, format_type, schema_val) {
                Ok(()) => {
                    debug!("StructuredOutputPlugin: Validation succeeded");
                }
                Err(validation_error) => {
                    if self.correction_model.is_none() || self.max_retries == 0 {
                        return Err(PylosError::InvalidRequest(validation_error));
                    }

                    warn!(
                        "StructuredOutputPlugin: Validation failed, attempting correction (max_retries={})",
                        self.max_retries
                    );

                    let mut corrected = content;
                    let mut current_error = validation_error.clone();
                    let mut success = false;

                    for attempt in 1..=self.max_retries {
                        match self
                            .attempt_correction(chat_req, &corrected, &current_error)
                            .await
                        {
                            Ok(new_content) => {
                                match Self::validate_content(&new_content, format_type, schema_val)
                                {
                                    Ok(()) => {
                                        debug!(
                                            "StructuredOutputPlugin: Correction succeeded on attempt {}",
                                            attempt
                                        );
                                        corrected = new_content;
                                        success = true;
                                        break;
                                    }
                                    Err(new_error) => {
                                        warn!(
                                            "StructuredOutputPlugin: Correction attempt {} still invalid: {}",
                                            attempt, new_error
                                        );
                                        corrected = new_content;
                                        current_error = new_error;
                                    }
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "StructuredOutputPlugin: Correction attempt {} failed: {}",
                                    attempt, e
                                );
                                return Err(PylosError::InvalidRequest(format!(
                                    "Response format error: {}. Correction failed: {}",
                                    validation_error, e
                                )));
                            }
                        }
                    }

                    if success {
                        choice.message.content = Some(corrected);
                    } else {
                        return Err(PylosError::InvalidRequest(format!(
                            "Response format error: {}. Correction attempted {} time(s) but still invalid.",
                            validation_error, self.max_retries
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}
