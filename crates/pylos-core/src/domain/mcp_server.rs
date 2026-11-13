use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpTransportConfig {
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        env: Option<HashMap<String, String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        current_dir: Option<String>,
    },
    Http {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
    },
    Sse {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        timeout_secs: Option<u64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServer {
    pub id: String,
    pub name: String,
    pub server_type: McpServerType,
    pub status: McpServerStatus,
    pub target_url: Option<String>,
    pub container_image: Option<String>,
    pub env_vars: Option<serde_json::Value>,
    pub virtual_key_id: Option<String>,
    pub team_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport_config: Option<McpTransportConfig>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl McpServer {
    /// Retourne la configuration de transport effective :
    /// utilise `transport_config` si défini, sinon construit un transport HTTP
    /// à partir de `target_url` pour la rétro-compatibilité.
    pub fn effective_transport(&self) -> Option<McpTransportConfig> {
        if let Some(ref tc) = self.transport_config {
            Some(tc.clone())
        } else {
            self.target_url
                .as_ref()
                .map(|url| McpTransportConfig::Http {
                    url: url.clone(),
                    headers: None,
                    timeout_secs: None,
                })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpTool {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub input_schema: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct McpToolCallResult {
    pub content: Vec<McpContentItem>,
    #[serde(default)]
    pub is_error: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum McpContentItem {
    Text {
        text: String,
    },
    Image {
        data: String,
        mime_type: String,
    },
    Resource {
        uri: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerType {
    Python,
    Node,
    Custom(String),
}

impl std::fmt::Display for McpServerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Python => write!(f, "python"),
            Self::Node => write!(f, "node"),
            Self::Custom(s) => write!(f, "custom:{s}"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum McpServerStatus {
    Active,
    Inactive,
    Error,
}

impl std::fmt::Display for McpServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Inactive => write!(f, "inactive"),
            Self::Error => write!(f, "error"),
        }
    }
}
