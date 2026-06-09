//! Copilot CLI MCP configuration serializer
//!
//! Converts portable `.mcp.json` manifest to Copilot CLI's config format.
//!
//! Output format:
//! ```json
//! {
//!   "mcpServers": {
//!     "server-name": {
//!       "type": "local",
//!       "tools": ["*"],
//!       "command": "...",
//!       "args": ["..."],
//!       "env": {"...": "..."}
//!     }
//!   }
//! }
//! ```

use base::mcp::{McpManifest, McpServer, Result};
use crate::{has_unresolved_secrets, McpProviderConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Copilot CLI server entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotCliServerEntry {
    #[serde(rename = "type")]
    pub transport: String,

    pub tools: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<Vec<String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub env: Option<HashMap<String, String>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}

/// Copilot CLI MCP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CopilotCliMcpConfig {
    pub mcp_servers: HashMap<String, CopilotCliServerEntry>,
}

impl McpProviderConfig for CopilotCliMcpConfig {
    fn from_manifest(manifest: &McpManifest) -> Result<Self> {
        let mut mcp_servers = HashMap::new();

        for (server_name, server_config) in &manifest.servers {
            // Skip servers with unresolved secrets
            if has_unresolved_secrets(server_config) {
                continue;
            }

            // Convert to Copilot CLI format
            let entry = convert_to_copilot_entry(server_config);
            mcp_servers.insert(server_name.clone(), entry);
        }

        Ok(Self { mcp_servers })
    }

    fn to_config_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

/// Convert MCP server config to Copilot CLI entry
fn convert_to_copilot_entry(server_config: &McpServer) -> CopilotCliServerEntry {
    let transport = get_transport(server_config);

    // Base entry with type and tools
    let mut entry = CopilotCliServerEntry {
        transport: transport.clone(),
        tools: vec!["*".to_string()],
        command: None,
        args: None,
        env: None,
        url: None,
        headers: None,
    };

    // HTTP/SSE transport
    if transport == "http" || transport == "sse" {
        entry.url = server_config.url.clone();
        entry.headers = server_config.headers.clone();
        return entry;
    }

    // Local transport (stdio)
    entry.command = server_config.command.clone();
    entry.args = server_config.args.clone();
    entry.env = server_config.env.clone();

    entry
}

/// Determine transport type from server config
fn get_transport(server_config: &McpServer) -> String {
    if let Some(ref server_type) = server_config.server_type {
        return match server_type.as_str() {
            "http" => "http".to_string(),
            "sse" => "sse".to_string(),
            "stdio" => "local".to_string(),
            "local" => "local".to_string(),
            other => other.to_string(),
        };
    }

    // Infer from config fields
    if server_config.url.is_some() {
        return "http".to_string();
    }

    if server_config.command.is_some() {
        return "local".to_string();
    }

    "local".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_local_server() {
        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: None,
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();

        assert_eq!(config.mcp_servers.len(), 1);
        let entry = &config.mcp_servers["test-server"];

        assert_eq!(entry.transport, "local");
        assert_eq!(entry.tools, vec!["*"]);
        assert_eq!(entry.command.as_ref().unwrap(), "node");
        assert_eq!(
            entry.args.as_ref().unwrap(),
            &vec!["index.js".to_string()]
        );
    }

    #[test]
    fn test_convert_local_server_with_env() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret123".to_string());

        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: Some(env.clone()),
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
        let entry = &config.mcp_servers["test-server"];

        assert_eq!(entry.transport, "local");
        assert!(entry.env.is_some());
        assert_eq!(entry.env.as_ref().unwrap().get("API_KEY").unwrap(), "secret123");
    }

    #[test]
    fn test_convert_http_server() {
        let mut servers = HashMap::new();
        servers.insert(
            "http-server".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                command: None,
                args: None,
                env: None,
                url: Some("http://example.com".to_string()),
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
        let entry = &config.mcp_servers["http-server"];

        assert_eq!(entry.transport, "http");
        assert_eq!(entry.tools, vec!["*"]);
        assert_eq!(entry.url.as_ref().unwrap(), "http://example.com");
        assert!(entry.command.is_none());
        assert!(entry.args.is_none());
    }

    #[test]
    fn test_convert_sse_server() {
        let mut servers = HashMap::new();
        servers.insert(
            "sse-server".to_string(),
            McpServer {
                server_type: Some("sse".to_string()),
                command: None,
                args: None,
                env: None,
                url: Some("http://example.com/sse".to_string()),
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
        let entry = &config.mcp_servers["sse-server"];

        assert_eq!(entry.transport, "sse");
        assert_eq!(entry.url.as_ref().unwrap(), "http://example.com/sse");
    }

    #[test]
    fn test_skip_unresolved_secret() {
        let mut servers = HashMap::new();
        servers.insert(
            "secret-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("${API_KEY}".to_string()),
                args: None,
                env: None,
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
        assert_eq!(config.mcp_servers.len(), 0);
    }

    #[test]
    fn test_infer_transport_from_url() {
        let mut servers = HashMap::new();
        servers.insert(
            "inferred-http".to_string(),
            McpServer {
                server_type: None,
                command: None,
                args: None,
                env: None,
                url: Some("http://example.com".to_string()),
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
        let entry = &config.mcp_servers["inferred-http"];

        assert_eq!(entry.transport, "http");
    }

    #[test]
    fn test_infer_transport_from_command() {
        let mut servers = HashMap::new();
        servers.insert(
            "inferred-local".to_string(),
            McpServer {
                server_type: None,
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: None,
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
        let entry = &config.mcp_servers["inferred-local"];

        assert_eq!(entry.transport, "local");
    }
}
