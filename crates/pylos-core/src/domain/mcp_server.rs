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
        cwd: Option<String>,
    },
    Http {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
    },
    Sse {
        url: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        headers: Option<HashMap<String, String>>,
    },
}

impl McpTransportConfig {
    pub fn transport_type(&self) -> &'static str {
        match self {
            Self::Stdio { .. } => "stdio",
            Self::Http { .. } => "http",
            Self::Sse { .. } => "sse",
        }
    }
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
    /// Return the explicit transport config or derive a fallback from `target_url` for backwards compatibility.
    pub fn effective_transport(&self) -> Option<McpTransportConfig> {
        if let Some(ref tc) = self.transport_config {
            Some(tc.clone())
        } else if let Some(ref url) = self.target_url {
            if url.ends_with("/sse") {
                Some(McpTransportConfig::Sse {
                    url: url.clone(),
                    headers: None,
                })
            } else {
                Some(McpTransportConfig::Http {
                    url: url.clone(),
                    headers: None,
                })
            }
        } else {
            None
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_transport_config_stdio_serde() {
        let mut env = HashMap::new();
        env.insert("DEBUG".to_string(), "1".to_string());
        let config = McpTransportConfig::Stdio {
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
            env: Some(env),
            cwd: Some("/workspace".to_string()),
        };

        assert_eq!(config.transport_type(), "stdio");

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"stdio\""));
        assert!(json.contains("\"command\":\"npx\""));

        let deserialized: McpTransportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_transport_config_http_serde() {
        let mut headers = HashMap::new();
        headers.insert("Authorization".to_string(), "Bearer token123".to_string());
        let config = McpTransportConfig::Http {
            url: "http://localhost:8000/mcp".to_string(),
            headers: Some(headers),
        };

        assert_eq!(config.transport_type(), "http");

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"http\""));

        let deserialized: McpTransportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_transport_config_sse_serde() {
        let config = McpTransportConfig::Sse {
            url: "http://localhost:8000/sse".to_string(),
            headers: None,
        };

        assert_eq!(config.transport_type(), "sse");

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("\"type\":\"sse\""));

        let deserialized: McpTransportConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_effective_transport_fallback() {
        let mut server = McpServer {
            id: "mcp-1".to_string(),
            name: "Server 1".to_string(),
            server_type: McpServerType::Node,
            status: McpServerStatus::Active,
            target_url: Some("http://localhost:8080/mcp".to_string()),
            container_image: None,
            env_vars: None,
            virtual_key_id: None,
            team_id: None,
            transport_config: None,
            created_at: 0,
            updated_at: 0,
        };

        // Fallback to HTTP
        let eff = server.effective_transport().unwrap();
        assert_eq!(eff.transport_type(), "http");

        // Fallback to SSE
        server.target_url = Some("http://localhost:8080/sse".to_string());
        let eff = server.effective_transport().unwrap();
        assert_eq!(eff.transport_type(), "sse");

        // Explicit transport config overrides target_url
        server.transport_config = Some(McpTransportConfig::Stdio {
            command: "python".to_string(),
            args: vec!["main.py".to_string()],
            env: None,
            cwd: None,
        });
        let eff = server.effective_transport().unwrap();
        assert_eq!(eff.transport_type(), "stdio");
    }
}
