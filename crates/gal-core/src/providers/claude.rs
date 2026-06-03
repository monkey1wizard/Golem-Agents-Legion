//! Claude Desktop MCP configuration serializer
//!
//! Converts portable `.mcp.json` manifest to Claude Desktop's config format.
//! On Windows, wraps env vars into PowerShell wrapper for stdio servers.
//!
//! Output format:
//! ```json
//! {
//!   "mcpServers": {
//!     "server-name": {
//!       "command": "...",
//!       "args": ["..."]
//!     }
//!   }
//! }
//! ```

use crate::mcp::{McpManifest, McpServer, Result};
use crate::providers::{has_unresolved_secrets, McpProviderConfig};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Claude Desktop server entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeDesktopServerEntry {
    pub command: String,
    pub args: Vec<String>,
}

/// Claude Desktop MCP configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeDesktopMcpConfig {
    pub mcp_servers: HashMap<String, ClaudeDesktopServerEntry>,
}

impl McpProviderConfig for ClaudeDesktopMcpConfig {
    fn from_manifest(manifest: &McpManifest) -> Result<Self> {
        let mut mcp_servers = HashMap::new();

        for (server_name, server_config) in &manifest.servers {
            // Skip HTTP servers (no command)
            if server_config.command.is_none() {
                continue;
            }

            // Skip servers with type=http explicitly
            if let Some(ref server_type) = server_config.server_type {
                if server_type == "http" {
                    continue;
                }
            }

            // Skip servers with unresolved secrets
            if has_unresolved_secrets(server_config) {
                continue;
            }

            // Convert to Claude Desktop format
            let entry = convert_to_claude_entry(server_name, server_config);
            mcp_servers.insert(server_name.clone(), entry);
        }

        Ok(Self { mcp_servers })
    }

    fn to_json_pretty(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

/// Convert MCP server config to Claude Desktop entry
///
/// On Windows, wraps env vars into PowerShell wrapper
fn convert_to_claude_entry(
    _server_name: &str,
    server_config: &McpServer,
) -> ClaudeDesktopServerEntry {
    let command = server_config.command.as_ref().unwrap().clone();
    let args = server_config
        .args.clone()
        .unwrap_or_default();

    // Check if we need PowerShell wrapper (Windows only, and env vars present)
    if cfg!(target_os = "windows") {
        if let Some(ref env) = server_config.env {
            if !env.is_empty() {
                return wrap_with_powershell(&command, &args, env);
            }
        }
    }

    ClaudeDesktopServerEntry { command, args }
}

/// Wrap command with PowerShell to set environment variables
///
/// Generates:
/// ```powershell
/// powershell -NoProfile -Command "$env:VAR='value'; & 'command' 'arg1' 'arg2'"
/// ```
fn wrap_with_powershell(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> ClaudeDesktopServerEntry {
    let mut script_parts = Vec::new();

    // Add env var assignments
    for (name, value) in env {
        let escaped_value = value.replace('\'', "''");
        script_parts.push(format!("$env:{} = '{}'", name, escaped_value));
    }

    // Build command invocation
    let mut command_parts = vec!["&".to_string()];
    command_parts.push(format!("'{}'", command.replace('\'', "''")));
    for arg in args {
        command_parts.push(format!("'{}'", arg.replace('\'', "''")));
    }
    script_parts.push(command_parts.join(" "));

    ClaudeDesktopServerEntry {
        command: "powershell".to_string(),
        args: vec![
            "-NoProfile".to_string(),
            "-Command".to_string(),
            script_parts.join("; "),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_simple_server() {
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

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();

        assert_eq!(config.mcp_servers.len(), 1);
        assert!(config.mcp_servers.contains_key("test-server"));

        let entry = &config.mcp_servers["test-server"];
        assert_eq!(entry.command, "node");
        assert_eq!(entry.args, vec!["index.js"]);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_wrap_with_env_on_windows() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret123".to_string());

        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: Some(env),
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
        let entry = &config.mcp_servers["test-server"];

        assert_eq!(entry.command, "powershell");
        assert_eq!(entry.args[0], "-NoProfile");
        assert_eq!(entry.args[1], "-Command");
        assert!(entry.args[2].contains("$env:API_KEY = 'secret123'"));
        assert!(entry.args[2].contains("& 'node' 'index.js'"));
    }

    #[test]
    fn test_skip_http_server() {
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

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
        assert_eq!(config.mcp_servers.len(), 0);
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

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
        assert_eq!(config.mcp_servers.len(), 0);
    }
}
