//! OpenCode MCP configuration serializer (JSON, `mcp.*` section).
//!
//! Converts the portable `.mcp.json` manifest to OpenCode's format, which
//! nests server entries under a top-level `"mcp"` key in `opencode.json`.
//!
//! Output format:
//! ```json
//! {
//!   "mcp": {
//!     "server-name": {
//!       "type": "local",
//!       "command": ["cmd", "arg1", "arg2"],
//!       "enabled": true
//!     }
//!   }
//! }
//! ```
//!
//! For HTTP/remote servers:
//! ```json
//! {
//!   "mcp": {
//!     "server-name": {
//!       "type": "remote",
//!       "url": "https://...",
//!       "enabled": true
//!     }
//!   }
//! }
//! ```
//!
//! Ports `ConvertTo-OpenCodeMcpConfig` from `scripts/Update-Mcp.ps1`.

use super::McpHostConfig;
use gal_foundation::mcp::{McpManifest, McpServer, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single OpenCode MCP server entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenCodeServerEntry {
    #[serde(rename = "type")]
    pub transport: String,

    /// For local (stdio) servers: command + args as one flat array.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<Vec<String>>,

    /// For remote (http/sse) servers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub environment: Option<HashMap<String, String>>,

    pub enabled: bool,
}

/// OpenCode MCP configuration — wraps server map under `"mcp"` key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenCodeMcpConfig {
    pub mcp: HashMap<String, OpenCodeServerEntry>,
}

impl McpHostConfig for OpenCodeMcpConfig {
    fn from_manifest(manifest: &McpManifest) -> Result<Self> {
        let mut mcp = HashMap::new();
        for (name, config) in &manifest.servers {
            mcp.insert(name.clone(), convert_to_opencode_entry(config));
        }
        Ok(Self { mcp })
    }

    fn to_config_string(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

fn convert_to_opencode_entry(server: &McpServer) -> OpenCodeServerEntry {
    let is_remote = server.server_type.as_deref() == Some("http")
        || server.server_type.as_deref() == Some("sse")
        || (server.command.is_none() && server.url.is_some());

    if is_remote {
        OpenCodeServerEntry {
            transport: "remote".to_string(),
            command: None,
            url: server.url.clone(),
            environment: None,
            enabled: true,
        }
    } else {
        // Local (stdio): command + args merged into one `command` array.
        let mut cmd = Vec::new();
        if let Some(ref c) = server.command {
            cmd.push(c.clone());
        }
        if let Some(ref args) = server.args {
            cmd.extend(args.iter().cloned());
        }

        let environment = server
            .env
            .as_ref()
            .map(|env| env.iter().map(|(k, v)| (k.clone(), v.clone())).collect());

        OpenCodeServerEntry {
            transport: "local".to_string(),
            command: Some(cmd),
            url: None,
            environment,
            enabled: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn make_manifest(servers: HashMap<String, McpServer>) -> McpManifest {
        McpManifest {
            servers,
            inputs: None,
        }
    }

    #[test]
    fn local_server_uses_command_array() {
        let mut servers = HashMap::new();
        servers.insert(
            "memory".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "@mcp/server-memory".to_string()]),
                env: None,
                url: None,
                headers: None,
            },
        );
        let cfg = OpenCodeMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let json: Value = serde_json::from_str(&cfg.to_config_string().unwrap()).unwrap();

        let entry = &json["mcp"]["memory"];
        assert_eq!(entry["type"], "local");
        assert_eq!(
            entry["command"],
            serde_json::json!(["npx", "-y", "@mcp/server-memory"])
        );
        assert_eq!(entry["enabled"], true);
    }

    #[test]
    fn remote_server_uses_url() {
        let mut servers = HashMap::new();
        servers.insert(
            "remote".to_string(),
            McpServer {
                server_type: Some("http".to_string()),
                command: None,
                args: None,
                env: None,
                url: Some("https://api.example.com/mcp/".to_string()),
                headers: None,
            },
        );
        let cfg = OpenCodeMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let json: Value = serde_json::from_str(&cfg.to_config_string().unwrap()).unwrap();

        let entry = &json["mcp"]["remote"];
        assert_eq!(entry["type"], "remote");
        assert_eq!(entry["url"], "https://api.example.com/mcp/");
        assert_eq!(entry["enabled"], true);
    }

    #[test]
    fn env_vars_appear_as_environment() {
        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret".to_string());
        let mut servers = HashMap::new();
        servers.insert(
            "tool".to_string(),
            McpServer {
                server_type: None,
                command: Some("mytool".to_string()),
                args: None,
                env: Some(env),
                url: None,
                headers: None,
            },
        );
        let cfg = OpenCodeMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let json: Value = serde_json::from_str(&cfg.to_config_string().unwrap()).unwrap();

        let entry = &json["mcp"]["tool"];
        assert_eq!(entry["environment"]["API_KEY"], "secret");
    }

    #[test]
    fn sse_type_treated_as_remote() {
        let mut servers = HashMap::new();
        servers.insert(
            "sse-server".to_string(),
            McpServer {
                server_type: Some("sse".to_string()),
                command: None,
                args: None,
                env: None,
                url: Some("https://sse.example.com/stream".to_string()),
                headers: None,
            },
        );
        let cfg = OpenCodeMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let entry = &cfg.mcp["sse-server"];
        assert_eq!(entry.transport, "remote");
    }

    #[test]
    fn url_only_treated_as_remote() {
        let mut servers = HashMap::new();
        servers.insert(
            "url-only".to_string(),
            McpServer {
                server_type: None,
                command: None,
                args: None,
                env: None,
                url: Some("https://example.com/mcp/".to_string()),
                headers: None,
            },
        );
        let cfg = OpenCodeMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let entry = &cfg.mcp["url-only"];
        assert_eq!(entry.transport, "remote");
    }

    #[test]
    fn serialized_output_has_mcp_key() {
        let cfg = OpenCodeMcpConfig::from_manifest(&McpManifest {
            servers: HashMap::new(),
            inputs: None,
        })
        .unwrap();
        let json: Value = serde_json::from_str(&cfg.to_config_string().unwrap()).unwrap();
        assert!(json.get("mcp").is_some());
    }
}
