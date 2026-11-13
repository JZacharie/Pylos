use std::collections::HashMap;
use std::sync::Arc;

use pylos_core::domain::mcp_server::{McpServer, McpServerStatus, McpTool, McpToolCallResult};
use pylos_core::domain::openai::Tool;
use pylos_core::error::PylosError;
use serde_json::Value;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use super::client::{create_mcp_client, McpClient};
use super::web_fetch::{execute_web_fetch, web_fetch_tool_def, WebFetchArgs, WEB_FETCH_TOOL_NAME};

#[allow(dead_code)]
struct RegisteredServer {
    server: McpServer,
    client: Arc<dyn McpClient>,
    tools: Vec<McpTool>,
}

#[derive(Clone)]
pub struct McpToolRegistry {
    servers: Arc<RwLock<HashMap<String, RegisteredServer>>>,
    // Mapping tool_name -> server_id
    tool_index: Arc<RwLock<HashMap<String, String>>>,
}

impl Default for McpToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl McpToolRegistry {
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            tool_index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Synchronise un serveur MCP : s'il est actif, initialise le transport et découvre ses outils.
    /// S'il est inactif, supprime ses outils du registre.
    pub async fn sync_server(&self, server: &McpServer) -> Result<(), PylosError> {
        if server.status != McpServerStatus::Active {
            self.remove_server(&server.id).await;
            return Ok(());
        }

        let transport = match server.effective_transport() {
            Some(t) => t,
            None => {
                warn!(server_id = %server.id, name = %server.name, "Active MCP server has no transport or target_url");
                self.remove_server(&server.id).await;
                return Ok(());
            }
        };

        info!(server_id = %server.id, name = %server.name, transport = ?transport, "Connecting to MCP server");
        let client = match create_mcp_client(&transport) {
            Ok(c) => c,
            Err(e) => {
                error!(server_id = %server.id, name = %server.name, error = %e, "Failed to create MCP client");
                return Err(e);
            }
        };

        if let Err(e) = client.initialize().await {
            error!(server_id = %server.id, name = %server.name, error = %e, "Failed to initialize MCP client");
            return Err(e);
        }

        let mut tools = match client.list_tools().await {
            Ok(t) => t,
            Err(e) => {
                error!(server_id = %server.id, name = %server.name, error = %e, "Failed to list MCP tools");
                return Err(e);
            }
        };

        // Associe le server_id à chaque outil
        for tool in &mut tools {
            tool.server_id = Some(server.id.clone());
        }

        info!(
            server_id = %server.id,
            name = %server.name,
            tool_count = tools.len(),
            "Discovered MCP tools"
        );

        // Mise à jour du registre
        {
            let mut servers_guard = self.servers.write().await;
            let mut index_guard = self.tool_index.write().await;

            // Retirer d'abord les anciens index de ce serveur
            index_guard.retain(|_, s_id| s_id != &server.id);

            for tool in &tools {
                index_guard.insert(tool.name.clone(), server.id.clone());
            }

            servers_guard.insert(
                server.id.clone(),
                RegisteredServer {
                    server: server.clone(),
                    client,
                    tools,
                },
            );
        }

        Ok(())
    }

    /// Enregistre directement un client MCP (utile pour les tests et mock servers - S-MCP-14)
    pub async fn register_client(
        &self,
        server: McpServer,
        client: Arc<dyn McpClient>,
        mut tools: Vec<McpTool>,
    ) {
        let mut servers_guard = self.servers.write().await;
        let mut index_guard = self.tool_index.write().await;

        index_guard.retain(|_, s_id| s_id != &server.id);
        for tool in &mut tools {
            tool.server_id = Some(server.id.clone());
            index_guard.insert(tool.name.clone(), server.id.clone());
        }

        servers_guard.insert(
            server.id.clone(),
            RegisteredServer {
                server,
                client,
                tools,
            },
        );
    }

    /// Supprime un serveur et ses outils du registre
    pub async fn remove_server(&self, server_id: &str) {
        let mut servers_guard = self.servers.write().await;
        let mut index_guard = self.tool_index.write().await;

        servers_guard.remove(server_id);
        index_guard.retain(|_, s_id| s_id != server_id);
    }

    /// Synchronise une liste complète de serveurs MCP
    pub async fn sync_all(&self, servers: &[McpServer]) {
        for server in servers {
            if let Err(e) = self.sync_server(server).await {
                warn!(server_id = %server.id, name = %server.name, error = %e, "Could not sync MCP server");
            }
        }
    }

    /// Liste tous les outils disponibles filtrés selon les autorisations (gouvernance Virtual Key)
    pub async fn list_tools(
        &self,
        allowed_tools: Option<&[String]>,
        denied_tools: Option<&[String]>,
    ) -> Vec<Tool> {
        let mut all_mcp_tools = Vec::new();

        // 1. Outil built-in web_fetch
        all_mcp_tools.push(web_fetch_tool_def());

        // 2. Outils des serveurs MCP enregistrés
        {
            let servers_guard = self.servers.read().await;
            for reg in servers_guard.values() {
                all_mcp_tools.extend(reg.tools.clone());
            }
        }

        // 3. Filtrage gouvernance
        all_mcp_tools
            .into_iter()
            .filter(|tool| is_tool_allowed(&tool.name, allowed_tools, denied_tools))
            .map(Tool::from)
            .collect()
    }

    /// Exécute un outil par son nom (S-MCP-06 / S-MCP-13)
    pub async fn execute_tool(
        &self,
        name: &str,
        arguments: Value,
        allowed_tools: Option<&[String]>,
        denied_tools: Option<&[String]>,
    ) -> Result<McpToolCallResult, PylosError> {
        // 1. Vérification de gouvernance (S-MCP-13)
        if !is_tool_allowed(name, allowed_tools, denied_tools) {
            return Err(PylosError::Unauthorized(format!(
                "Tool '{name}' is not allowed for this virtual key"
            )));
        }

        // 2. Outil built-in : web_fetch
        if name == WEB_FETCH_TOOL_NAME {
            let args: WebFetchArgs = serde_json::from_value(arguments).map_err(|e| {
                PylosError::InvalidRequest(format!("Invalid arguments for web_fetch: {e}"))
            })?;
            return execute_web_fetch(args).await;
        }

        // 3. Résolution du serveur MCP
        let (client, _server_id) = {
            let index_guard = self.tool_index.read().await;
            let server_id = index_guard.get(name).cloned().ok_or_else(|| {
                PylosError::NotFound(format!("Tool '{name}' not found on any active MCP server"))
            })?;

            let servers_guard = self.servers.read().await;
            let reg = servers_guard.get(&server_id).ok_or_else(|| {
                PylosError::NotFound(format!(
                    "MCP server for tool '{name}' is no longer available"
                ))
            })?;

            (reg.client.clone(), server_id)
        };

        // 4. Appel de l'outil
        client.call_tool(name, arguments).await
    }
}

/// Helper de vérification d'autorisation d'un outil
pub fn is_tool_allowed(
    tool_name: &str,
    allowed: Option<&[String]>,
    denied: Option<&[String]>,
) -> bool {
    // Si explicitement refusé -> false
    if let Some(denied_list) = denied {
        if denied_list.iter().any(|d| d == "*" || d == tool_name) {
            return false;
        }
    }

    // Si liste blanche fournie -> doit y figurer
    if let Some(allowed_list) = allowed {
        if !allowed_list.is_empty() {
            return allowed_list.iter().any(|a| a == "*" || a == tool_name);
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_tool_allowed() {
        assert!(is_tool_allowed("web_fetch", None, None));

        let allowed = vec!["web_fetch".to_string(), "echo".to_string()];
        assert!(is_tool_allowed("web_fetch", Some(&allowed), None));
        assert!(!is_tool_allowed("other_tool", Some(&allowed), None));

        let denied = vec!["forbidden_tool".to_string()];
        assert!(!is_tool_allowed("forbidden_tool", None, Some(&denied)));
        assert!(is_tool_allowed("other_tool", None, Some(&denied)));

        // Denied takes precedence over allowed
        let tool1_slice = vec!["tool1".to_string()];
        assert!(!is_tool_allowed(
            "tool1",
            Some(&tool1_slice),
            Some(&tool1_slice)
        ));
    }
}
