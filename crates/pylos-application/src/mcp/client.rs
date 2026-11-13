use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use pylos_core::domain::mcp_server::{
    McpContentItem, McpTool, McpToolCallResult, McpTransportConfig,
};
use pylos_core::error::PylosError;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Child;
use tokio::sync::Mutex;
use tracing::{debug, info};

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<i64>,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<i64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[async_trait]
pub trait McpClient: Send + Sync {
    /// Initialise la connexion MCP (JSON-RPC handshake)
    async fn initialize(&self) -> Result<(), PylosError>;

    /// Découvre et liste les outils exposés par le serveur
    async fn list_tools(&self) -> Result<Vec<McpTool>, PylosError>;

    /// Exécute un outil via tools/call
    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<McpToolCallResult, PylosError>;

    /// Identifiant du serveur ou transport
    fn transport_name(&self) -> &str;
}

// ─────────────────────────────────────────────────────────────────────────────
// Client HTTP
// ─────────────────────────────────────────────────────────────────────────────

pub struct HttpMcpClient {
    url: String,
    headers: std::collections::HashMap<String, String>,
    client: reqwest::Client,
    req_counter: AtomicI64,
}

impl HttpMcpClient {
    pub fn new(
        url: String,
        headers: Option<std::collections::HashMap<String, String>>,
        timeout_secs: Option<u64>,
    ) -> Result<Self, PylosError> {
        let timeout = Duration::from_secs(timeout_secs.unwrap_or(30));
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(|e| PylosError::Internal(format!("Failed to create reqwest client: {e}")))?;

        Ok(Self {
            url,
            headers: headers.unwrap_or_default(),
            client,
            req_counter: AtomicI64::new(1),
        })
    }

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, PylosError> {
        let id = self.req_counter.fetch_add(1, Ordering::SeqCst);
        let req_body = JsonRpcRequest {
            jsonrpc: "2.0",
            id: Some(id),
            method: method.to_string(),
            params,
        };

        let mut req = self.client.post(&self.url).json(&req_body);
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }

        let resp = req.send().await.map_err(|e| {
            PylosError::Internal(format!("MCP HTTP request to {} failed: {e}", self.url))
        })?;

        if !resp.status().is_success() {
            return Err(PylosError::ProviderError {
                provider: "mcp".into(),
                message: format!("MCP server HTTP error: status {}", resp.status()),
            });
        }

        let rpc_res: JsonRpcResponse = resp.json().await.map_err(|e| {
            PylosError::Internal(format!("Invalid JSON-RPC response from MCP server: {e}"))
        })?;

        if let Some(err) = rpc_res.error {
            return Err(PylosError::Internal(format!(
                "MCP JSON-RPC error {}: {}",
                err.code, err.message
            )));
        }

        rpc_res.result.ok_or_else(|| {
            PylosError::Internal("Empty result in JSON-RPC response from MCP server".into())
        })
    }
}

#[async_trait]
impl McpClient for HttpMcpClient {
    async fn initialize(&self) -> Result<(), PylosError> {
        debug!(url = %self.url, "Initializing MCP HTTP client");
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": false },
                "sampling": {}
            },
            "clientInfo": {
                "name": "pylos-gateway",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let _ = self.send_request("initialize", Some(params)).await?;

        // Notification initialized
        let notify = JsonRpcRequest {
            jsonrpc: "2.0",
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        let mut req = self.client.post(&self.url).json(&notify);
        for (k, v) in &self.headers {
            req = req.header(k, v);
        }
        let _ = req.send().await;

        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, PylosError> {
        let res = self.send_request("tools/list", Some(json!({}))).await?;
        parse_tools_list(&res)
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<McpToolCallResult, PylosError> {
        let params = json!({
            "name": name,
            "arguments": arguments
        });
        let res = self.send_request("tools/call", Some(params)).await?;
        parse_tool_call_result(&res)
    }

    fn transport_name(&self) -> &str {
        "http"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Client Stdio
// ─────────────────────────────────────────────────────────────────────────────

struct StdioProcess {
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
    _child: Child,
}

pub struct StdioMcpClient {
    command: String,
    args: Vec<String>,
    env: Option<std::collections::HashMap<String, String>>,
    current_dir: Option<String>,
    process: Arc<Mutex<Option<StdioProcess>>>,
    req_counter: AtomicI64,
}

impl StdioMcpClient {
    pub fn new(
        command: String,
        args: Vec<String>,
        env: Option<std::collections::HashMap<String, String>>,
        current_dir: Option<String>,
    ) -> Self {
        Self {
            command,
            args,
            env,
            current_dir,
            process: Arc::new(Mutex::new(None)),
            req_counter: AtomicI64::new(1),
        }
    }

    async fn ensure_started(&self) -> Result<(), PylosError> {
        let mut proc_guard = self.process.lock().await;
        if proc_guard.is_some() {
            return Ok(());
        }

        info!(command = %self.command, args = ?self.args, "Spawning MCP Stdio child process");
        let mut cmd = tokio::process::Command::new(&self.command);
        cmd.args(&self.args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        if let Some(ref dir) = self.current_dir {
            cmd.current_dir(dir);
        }

        if let Some(ref env) = self.env {
            for (k, v) in env {
                cmd.env(k, v);
            }
        }

        let mut child = cmd.spawn().map_err(|e| {
            PylosError::Internal(format!(
                "Failed to spawn MCP process '{}': {e}",
                self.command
            ))
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            PylosError::Internal("Failed to capture stdin of MCP child process".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            PylosError::Internal("Failed to capture stdout of MCP child process".into())
        })?;

        let reader = BufReader::new(stdout);
        *proc_guard = Some(StdioProcess {
            stdin,
            reader,
            _child: child,
        });

        Ok(())
    }

    async fn send_request(&self, method: &str, params: Option<Value>) -> Result<Value, PylosError> {
        self.ensure_started().await?;
        let mut proc_guard = self.process.lock().await;
        let proc = proc_guard
            .as_mut()
            .ok_or_else(|| PylosError::Internal("MCP process not available".into()))?;

        let id = self.req_counter.fetch_add(1, Ordering::SeqCst);
        let req_body = JsonRpcRequest {
            jsonrpc: "2.0",
            id: Some(id),
            method: method.to_string(),
            params,
        };

        let mut json_line = serde_json::to_string(&req_body)
            .map_err(|e| PylosError::Internal(format!("Failed to serialize JSON-RPC: {e}")))?;
        json_line.push('\n');

        proc.stdin
            .write_all(json_line.as_bytes())
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to write to MCP stdin: {e}")))?;
        proc.stdin
            .flush()
            .await
            .map_err(|e| PylosError::Internal(format!("Failed to flush MCP stdin: {e}")))?;

        let mut line = String::new();
        // Attente de la réponse correspondant à l'ID
        loop {
            line.clear();
            let bytes_read = proc.reader.read_line(&mut line).await.map_err(|e| {
                PylosError::Internal(format!("Failed to read line from MCP stdout: {e}"))
            })?;

            if bytes_read == 0 {
                return Err(PylosError::Internal(
                    "MCP process closed stdout unexpectedly".into(),
                ));
            }

            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            if let Ok(res) = serde_json::from_str::<JsonRpcResponse>(trimmed) {
                if res.id == Some(id) {
                    if let Some(err) = res.error {
                        return Err(PylosError::Internal(format!(
                            "MCP JSON-RPC error {}: {}",
                            err.code, err.message
                        )));
                    }
                    return res.result.ok_or_else(|| {
                        PylosError::Internal("Empty result in JSON-RPC response".into())
                    });
                }
            }
        }
    }
}

#[async_trait]
impl McpClient for StdioMcpClient {
    async fn initialize(&self) -> Result<(), PylosError> {
        let params = json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "roots": { "listChanged": false },
                "sampling": {}
            },
            "clientInfo": {
                "name": "pylos-gateway",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let _ = self.send_request("initialize", Some(params)).await?;

        // Send notifications/initialized
        self.ensure_started().await?;
        let mut proc_guard = self.process.lock().await;
        if let Some(proc) = proc_guard.as_mut() {
            let notify = JsonRpcRequest {
                jsonrpc: "2.0",
                id: None,
                method: "notifications/initialized".to_string(),
                params: None,
            };
            if let Ok(mut s) = serde_json::to_string(&notify) {
                s.push('\n');
                let _ = proc.stdin.write_all(s.as_bytes()).await;
                let _ = proc.stdin.flush().await;
            }
        }

        Ok(())
    }

    async fn list_tools(&self) -> Result<Vec<McpTool>, PylosError> {
        let res = self.send_request("tools/list", Some(json!({}))).await?;
        parse_tools_list(&res)
    }

    async fn call_tool(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<McpToolCallResult, PylosError> {
        let params = json!({
            "name": name,
            "arguments": arguments
        });
        let res = self.send_request("tools/call", Some(params)).await?;
        parse_tool_call_result(&res)
    }

    fn transport_name(&self) -> &str {
        "stdio"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers de parsing de réponse JSON-RPC
// ─────────────────────────────────────────────────────────────────────────────

fn parse_tools_list(res: &Value) -> Result<Vec<McpTool>, PylosError> {
    let tools_val = res
        .get("tools")
        .ok_or_else(|| PylosError::Internal("Missing 'tools' in tools/list response".into()))?;

    let tools_array = tools_val
        .as_array()
        .ok_or_else(|| PylosError::Internal("'tools' is not an array".into()))?;

    let mut tools = Vec::with_capacity(tools_array.len());
    for item in tools_array {
        let name = item
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| PylosError::Internal("Tool item missing 'name'".into()))?
            .to_string();

        let description = item
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let input_schema = item
            .get("inputSchema")
            .cloned()
            .unwrap_or_else(|| json!({"type": "object", "properties": {}}));

        tools.push(McpTool {
            name,
            description,
            input_schema,
            server_id: None,
        });
    }

    Ok(tools)
}

fn parse_tool_call_result(res: &Value) -> Result<McpToolCallResult, PylosError> {
    let is_error = res
        .get("isError")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let content_val = res.get("content");
    let content = if let Some(arr) = content_val.and_then(|v| v.as_array()) {
        let mut items = Vec::new();
        for item in arr {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("text");
            match item_type {
                "text" => {
                    let text = item
                        .get("text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    items.push(McpContentItem::Text { text });
                }
                "image" => {
                    let data = item
                        .get("data")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mime_type = item
                        .get("mimeType")
                        .and_then(|v| v.as_str())
                        .unwrap_or("image/png")
                        .to_string();
                    items.push(McpContentItem::Image { data, mime_type });
                }
                "resource" => {
                    let uri = item
                        .get("uri")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    let mime_type = item
                        .get("mimeType")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let text = item
                        .get("text")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    items.push(McpContentItem::Resource {
                        uri,
                        mime_type,
                        text,
                    });
                }
                _ => {
                    items.push(McpContentItem::Text {
                        text: item.to_string(),
                    });
                }
            }
        }
        items
    } else {
        vec![McpContentItem::Text {
            text: res.to_string(),
        }]
    };

    Ok(McpToolCallResult { content, is_error })
}

/// Usine de création de client MCP selon la configuration de transport
pub fn create_mcp_client(config: &McpTransportConfig) -> Result<Arc<dyn McpClient>, PylosError> {
    match config {
        McpTransportConfig::Http {
            url,
            headers,
            timeout_secs,
        } => {
            let client = HttpMcpClient::new(url.clone(), headers.clone(), *timeout_secs)?;
            Ok(Arc::new(client))
        }
        McpTransportConfig::Sse {
            url,
            headers,
            timeout_secs,
        } => {
            // Dans cette phase, le transport SSE utilise le client HTTP optimisé
            let client = HttpMcpClient::new(url.clone(), headers.clone(), *timeout_secs)?;
            Ok(Arc::new(client))
        }
        McpTransportConfig::Stdio {
            command,
            args,
            env,
            current_dir,
        } => {
            let client = StdioMcpClient::new(
                command.clone(),
                args.clone(),
                env.clone(),
                current_dir.clone(),
            );
            Ok(Arc::new(client))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_tools_list() {
        let json_val = json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "Echoes back the input",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string" }
                        }
                    }
                }
            ]
        });

        let tools = parse_tools_list(&json_val).unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(
            tools[0].description.as_deref(),
            Some("Echoes back the input")
        );
    }

    #[test]
    fn test_parse_tool_call_result_text() {
        let json_val = json!({
            "content": [
                {
                    "type": "text",
                    "text": "Hello world from tool"
                }
            ],
            "isError": false
        });

        let res = parse_tool_call_result(&json_val).unwrap();
        assert!(!res.is_error);
        assert_eq!(res.content.len(), 1);
        match &res.content[0] {
            McpContentItem::Text { text } => assert_eq!(text, "Hello world from tool"),
            _ => panic!("Expected Text content"),
        }
    }
}
