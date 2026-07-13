//! MCP host config serializers.
//!
//! Converts the portable `.mcp.json` manifest into each MCP host's native
//! config format:
//! - Claude Desktop: stdio/command JSON
//! - Copilot CLI: local/http transport JSON
//! - Codex CLI: `[mcp_servers.name]` TOML sections in `~/.codex/config.toml`
//! - OpenCode: `{"mcp": {...}}` JSON in `opencode.json`

pub mod claude_mcp;
pub mod codex;
pub mod copilot;
pub mod opencode;

use gal_foundation::mcp::{McpManifest, McpServer, Result};

/// MCP host config trait.
///
/// `to_config_string` returns the host's native format (JSON for most hosts,
/// TOML for Codex). The name is format-agnostic so all four hosts can implement
/// the same trait.
pub trait McpHostConfig {
    /// Convert portable manifest to host-specific format.
    fn from_manifest(manifest: &McpManifest) -> Result<Self>
    where
        Self: Sized;

    /// Serialize to the host's native config format (JSON or TOML).
    fn to_config_string(&self) -> Result<String>;
}

/// Filter servers that have unresolved secrets
///
/// Returns true if the server config contains unresolved placeholder secrets
/// (e.g., ${API_KEY}, ${SECRET}, ${TOKEN}, ${PASSWORD})
pub fn has_unresolved_secrets(server: &McpServer) -> bool {
    // Compile the capturing pattern once (not per field/iteration).
    static PLACEHOLDER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let placeholder = PLACEHOLDER.get_or_init(|| regex::Regex::new(r"\$\{([A-Z0-9_]+)\}").unwrap());
    let secret_keywords = ["KEY", "SECRET", "TOKEN", "PASSWORD"];

    // Returns true if `value` contains any `${VAR}` placeholder whose name
    // looks like a secret (contains KEY/SECRET/TOKEN/PASSWORD).
    let is_unresolved_secret = |value: &str| -> bool {
        placeholder.captures_iter(value).any(|captures| {
            let var_name = &captures[1];
            secret_keywords
                .iter()
                .any(|&keyword| var_name.contains(keyword))
        })
    };

    // Scalar fields.
    if let Some(ref cmd) = server.command {
        if is_unresolved_secret(cmd) {
            return true;
        }
    }
    if let Some(ref url) = server.url {
        if is_unresolved_secret(url) {
            return true;
        }
    }

    // Collection fields.
    if let Some(ref args) = server.args {
        if args.iter().any(|a| is_unresolved_secret(a)) {
            return true;
        }
    }
    if let Some(ref env) = server.env {
        if env.values().any(|v| is_unresolved_secret(v)) {
            return true;
        }
    }
    if let Some(ref headers) = server.headers {
        if headers.values().any(|v| is_unresolved_secret(v)) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn server_with_env(value: &str) -> McpServer {
        let mut env = HashMap::new();
        env.insert("AUTH".to_string(), value.to_string());

        McpServer {
            server_type: None,
            command: None,
            args: None,
            env: Some(env),
            url: None,
            headers: None,
        }
    }

    #[test]
    fn detects_embedded_secret_placeholders() {
        let server = server_with_env("Bearer ${API_KEY}");

        assert!(has_unresolved_secrets(&server));
    }

    #[test]
    fn keeps_standalone_secret_detection() {
        let server = server_with_env("${API_KEY}");

        assert!(has_unresolved_secrets(&server));
    }

    #[test]
    fn ignores_non_secret_placeholders() {
        let server = server_with_env("${FOO}");

        assert!(!has_unresolved_secrets(&server));
    }
}
