//! Codex CLI MCP configuration serializer (TOML format).
//!
//! Converts the portable `.mcp.json` manifest to Codex's `[mcp_servers.name]`
//! TOML sections written into `~/.codex/config.toml`.
//!
//! Output format (one section per server):
//! ```toml
//! [mcp_servers."server-name"]
//! command = "npx"
//! args = ["-y", "@mcp/server"]
//!
//! [mcp_servers."server-name".env]
//! KEY = "value"
//! ```
//!
//! HTTP servers:
//! ```toml
//! [mcp_servers."server-name"]
//! type = "http"
//! url = "https://..."
//! ```
//!
//! Ports `ConvertTo-CodexMcpConfig` + `ConvertTo-CodexMcpSection` from
//! `scripts/Update-Mcp.ps1`.

use super::McpHostConfig;
use gal_foundation::mcp::{McpManifest, McpServer, Result};
use std::collections::BTreeMap;

/// A single Codex MCP server entry (for serialization purposes).
#[derive(Debug, Clone)]
pub struct CodexServerEntry {
    pub server_type: Option<String>, // "http" or None (stdio)
    pub command: Option<String>,
    pub args: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub url: Option<String>,
}

/// Codex CLI MCP configuration — collection of server entries.
#[derive(Debug, Clone)]
pub struct CodexMcpConfig {
    /// Ordered (BTreeMap) so output is deterministic across test runs.
    pub servers: BTreeMap<String, CodexServerEntry>,
}

impl McpHostConfig for CodexMcpConfig {
    fn from_manifest(manifest: &McpManifest) -> Result<Self> {
        let mut servers = BTreeMap::new();
        for (name, config) in &manifest.servers {
            servers.insert(name.clone(), convert_to_codex_entry(config));
        }
        Ok(Self { servers })
    }

    /// Serialize to TOML section text.
    ///
    /// Each server becomes a `[mcp_servers."name"]` block followed by its
    /// key-value fields, and a nested `[mcp_servers."name".env]` block when
    /// env vars are present. Sections are separated by a blank line.
    fn to_config_string(&self) -> Result<String> {
        let mut sections: Vec<String> = Vec::new();
        for (name, entry) in &self.servers {
            sections.push(format_codex_section(name, entry));
        }
        Ok(sections.join("\r\n\r\n") + "\r\n")
    }
}

fn convert_to_codex_entry(server: &McpServer) -> CodexServerEntry {
    let is_http = server.server_type.as_deref() == Some("http")
        || (server.command.is_none() && server.url.is_some());

    if is_http {
        CodexServerEntry {
            server_type: Some("http".to_string()),
            command: None,
            args: Vec::new(),
            env: BTreeMap::new(),
            url: server.url.clone(),
        }
    } else {
        let mut env = BTreeMap::new();
        if let Some(ref e) = server.env {
            for (k, v) in e {
                env.insert(k.clone(), v.clone());
            }
        }
        CodexServerEntry {
            server_type: None,
            command: server.command.clone(),
            args: server.args.clone().unwrap_or_default(),
            env,
            url: None,
        }
    }
}

/// Format one server as TOML section(s).
///
/// Ports `ConvertTo-CodexMcpSection` → `ConvertTo-TomlTableSections`.
fn format_codex_section(name: &str, entry: &CodexServerEntry) -> String {
    let key = format_toml_key_segment(name);
    let header = format!("[mcp_servers.{}]", key);
    let mut lines = vec![header];

    if let Some(ref t) = entry.server_type {
        lines.push(format!("type = {}", toml_string(t)));
    }
    if let Some(ref url) = entry.url {
        lines.push(format!("url = {}", toml_string(url)));
    }
    if let Some(ref cmd) = entry.command {
        lines.push(format!("command = {}", toml_string(cmd)));
    }
    if !entry.args.is_empty() {
        let args_toml = entry
            .args
            .iter()
            .map(|a| toml_string(a))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("args = [{}]", args_toml));
    }

    let section_body = lines.join("\r\n");

    if entry.env.is_empty() {
        section_body
    } else {
        let env_header = format!("[mcp_servers.{}.env]", key);
        let mut env_lines = vec![env_header];
        for (k, v) in &entry.env {
            env_lines.push(format!("{} = {}", k, toml_string(v)));
        }
        format!("{}\r\n\r\n{}", section_body, env_lines.join("\r\n"))
    }
}

/// Escape and quote a TOML string value: `"text"`.
fn toml_string(value: &str) -> String {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{}\"", escaped)
}

/// Format a TOML key segment — bare if alphanumeric/hyphen/underscore, else quoted.
fn format_toml_key_segment(key: &str) -> String {
    if key
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        key.to_string()
    } else {
        toml_string(key)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_manifest(servers: HashMap<String, McpServer>) -> McpManifest {
        McpManifest {
            servers,
            inputs: None,
        }
    }

    #[test]
    fn stdio_server_produces_command_and_args() {
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
        let cfg = CodexMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let toml = cfg.to_config_string().unwrap();
        assert!(
            toml.contains("[mcp_servers.memory]"),
            "section header missing"
        );
        assert!(toml.contains("command = \"npx\""), "command missing");
        assert!(
            toml.contains("args = [\"-y\", \"@mcp/server-memory\"]"),
            "args missing"
        );
    }

    #[test]
    fn http_server_produces_type_and_url() {
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
        let cfg = CodexMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let toml = cfg.to_config_string().unwrap();
        assert!(
            toml.contains("[mcp_servers.remote]"),
            "section header missing"
        );
        assert!(toml.contains("type = \"http\""), "type missing");
        assert!(
            toml.contains("url = \"https://api.example.com/mcp/\""),
            "url missing"
        );
    }

    #[test]
    fn env_vars_produce_nested_env_section() {
        let mut env = HashMap::new();
        env.insert(
            "MEMORY_FILE_PATH".to_string(),
            "/home/user/mem.json".to_string(),
        );
        let mut servers = HashMap::new();
        servers.insert(
            "memory".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "@mcp/server-memory".to_string()]),
                env: Some(env),
                url: None,
                headers: None,
            },
        );
        let cfg = CodexMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let toml = cfg.to_config_string().unwrap();
        assert!(
            toml.contains("[mcp_servers.memory.env]"),
            "env section header missing"
        );
        assert!(
            toml.contains("MEMORY_FILE_PATH = \"/home/user/mem.json\""),
            "env key missing"
        );
    }

    #[test]
    fn server_name_with_special_chars_is_quoted() {
        let mut servers = HashMap::new();
        servers.insert(
            "upstash/context7".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("npx".to_string()),
                args: Some(vec!["-y".to_string(), "@upstash/context7".to_string()]),
                env: None,
                url: None,
                headers: None,
            },
        );
        let cfg = CodexMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let toml = cfg.to_config_string().unwrap();
        // Name with '/' must be quoted in TOML key
        assert!(
            toml.contains("[mcp_servers.\"upstash/context7\"]"),
            "quoted key missing: {toml}"
        );
    }

    #[test]
    fn empty_manifest_produces_single_newline() {
        let cfg = CodexMcpConfig::from_manifest(&McpManifest {
            servers: HashMap::new(),
            inputs: None,
        })
        .unwrap();
        let toml = cfg.to_config_string().unwrap();
        assert_eq!(toml, "\r\n");
    }

    #[test]
    fn url_only_server_treated_as_http() {
        let mut servers = HashMap::new();
        servers.insert(
            "remote".to_string(),
            McpServer {
                server_type: None, // no explicit type
                command: None,
                args: None,
                env: None,
                url: Some("https://example.com/mcp/".to_string()),
                headers: None,
            },
        );
        let cfg = CodexMcpConfig::from_manifest(&make_manifest(servers)).unwrap();
        let entry = cfg.servers.get("remote").unwrap();
        assert_eq!(entry.server_type.as_deref(), Some("http"));
    }
}
