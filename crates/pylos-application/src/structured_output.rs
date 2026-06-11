use async_trait::async_trait;
use tracing::{debug, warn};

use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;

#[derive(Default)]
pub struct StructuredOutputPlugin;

impl StructuredOutputPlugin {
    pub fn new() -> Self {
        Self
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

        // Skip streaming requests for structured validation
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

        for choice in &chat_resp.choices {
            if let Some(content) = &choice.message.content {
                // 1. Validate it is valid JSON
                let parsed_json: serde_json::Value = match serde_json::from_str(content) {
                    Ok(val) => val,
                    Err(e) => {
                        warn!(
                            "StructuredOutputPlugin: Response content is not valid JSON: {}",
                            e
                        );
                        return Err(PylosError::InvalidRequest(format!(
                            "Response format error: LLM output is not valid JSON. Error: {}",
                            e
                        )));
                    }
                };

                // 2. Validate against schema if json_schema is specified
                if format_type == "json_schema" {
                    if let Some(ref schema_val) = response_format.json_schema {
                        let validator = match jsonschema::validator_for(schema_val) {
                            Ok(v) => v,
                            Err(e) => {
                                warn!(
                                    "StructuredOutputPlugin: Invalid JSON Schema in request: {}",
                                    e
                                );
                                return Err(PylosError::InvalidRequest(format!(
                                    "Request schema error: The provided JSON Schema is invalid. Error: {}",
                                    e
                                )));
                            }
                        };

                        let errors: Vec<_> = validator.iter_errors(&parsed_json).collect();
                        if !errors.is_empty() {
                            let mut err_msgs = Vec::new();
                            for err in &errors {
                                err_msgs.push(format!(
                                    "Path: {}, Error: {}",
                                    err.instance_path(),
                                    err
                                ));
                            }
                            let error_summary = err_msgs.join("; ");
                            warn!(
                                "StructuredOutputPlugin: Schema validation failed: {}",
                                error_summary
                            );
                            return Err(PylosError::InvalidRequest(format!(
                                "Response format error: LLM output failed JSON Schema validation. Errors: {}",
                                error_summary
                            )));
                        }
                        debug!("StructuredOutputPlugin: Schema validation succeeded");
                    }
                }
            }
        }

        Ok(())
    }
}
