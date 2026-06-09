//! Provider-specific MCP configuration serializers
//!
//! This module contains serializers for converting the portable `.mcp.json` manifest
//! into provider-specific configuration formats:
//! - Claude Desktop: stdio/command format
//! - Copilot CLI: local/http transport format
//! - AGY (Antigravity): three-surface projection (CLI/IDE/GUI-config)
//!
//! Corresponds to T-009 (MCP) and T-010 (AGY) of the bootstrap convergence plan (M1 phase).

pub mod agy;
pub mod claude;
pub mod copilot;

use base::mcp::{McpManifest, McpServer, Result};

/// Provider-specific MCP configuration
pub trait McpProviderConfig {
    /// Convert portable manifest to provider-specific format
    fn from_manifest(manifest: &McpManifest) -> Result<Self>
    where
        Self: Sized;

    /// Serialize to provider-specific JSON format
    fn to_json_pretty(&self) -> Result<String>;
}

/// Filter servers that have unresolved secrets
///
/// Returns true if the server config contains unresolved placeholder secrets
/// (e.g., ${API_KEY}, ${SECRET}, ${TOKEN}, ${PASSWORD})
pub fn has_unresolved_secrets(server: &McpServer) -> bool {
    // Compile the capturing pattern once (not per field/iteration).
    static PLACEHOLDER: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let placeholder =
        PLACEHOLDER.get_or_init(|| regex::Regex::new(r"^\$\{([A-Z0-9_]+)\}$").unwrap());
    let secret_keywords = ["KEY", "SECRET", "TOKEN", "PASSWORD"];

    // Returns true if `value` is a bare `${VAR}` placeholder whose name looks
    // like a secret (contains KEY/SECRET/TOKEN/PASSWORD).
    let is_unresolved_secret = |value: &str| -> bool {
        if let Some(captures) = placeholder.captures(value) {
            let var_name = &captures[1];
            return secret_keywords
                .iter()
                .any(|&keyword| var_name.contains(keyword));
        }
        false
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
