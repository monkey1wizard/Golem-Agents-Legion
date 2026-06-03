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

use crate::mcp::{McpManifest, McpServer, Result};

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
    let secret_pattern = regex::Regex::new(r"^\$\{[A-Z0-9_]+\}$").unwrap();
    let secret_keywords = ["KEY", "SECRET", "TOKEN", "PASSWORD"];

    // Check command
    if let Some(ref cmd) = server.command {
        if secret_pattern.is_match(cmd) {
            if let Some(captures) = regex::Regex::new(r"^\$\{([A-Z0-9_]+)\}$")
                .unwrap()
                .captures(cmd)
            {
                let var_name = &captures[1];
                if secret_keywords
                    .iter()
                    .any(|&keyword| var_name.contains(keyword))
                {
                    return true;
                }
            }
        }
    }

    // Check URL
    if let Some(ref url) = server.url {
        if secret_pattern.is_match(url) {
            if let Some(captures) = regex::Regex::new(r"^\$\{([A-Z0-9_]+)\}$")
                .unwrap()
                .captures(url)
            {
                let var_name = &captures[1];
                if secret_keywords
                    .iter()
                    .any(|&keyword| var_name.contains(keyword))
                {
                    return true;
                }
            }
        }
    }

    // Check args
    if let Some(ref args) = server.args {
        for arg in args {
            if secret_pattern.is_match(arg) {
                if let Some(captures) = regex::Regex::new(r"^\$\{([A-Z0-9_]+)\}$")
                    .unwrap()
                    .captures(arg)
                {
                    let var_name = &captures[1];
                    if secret_keywords
                        .iter()
                        .any(|&keyword| var_name.contains(keyword))
                    {
                        return true;
                    }
                }
            }
        }
    }

    // Check env values
    if let Some(ref env) = server.env {
        for value in env.values() {
            if secret_pattern.is_match(value) {
                if let Some(captures) = regex::Regex::new(r"^\$\{([A-Z0-9_]+)\}$")
                    .unwrap()
                    .captures(value)
                {
                    let var_name = &captures[1];
                    if secret_keywords
                        .iter()
                        .any(|&keyword| var_name.contains(keyword))
                    {
                        return true;
                    }
                }
            }
        }
    }

    // Check headers values
    if let Some(ref headers) = server.headers {
        for value in headers.values() {
            if secret_pattern.is_match(value) {
                if let Some(captures) = regex::Regex::new(r"^\$\{([A-Z0-9_]+)\}$")
                    .unwrap()
                    .captures(value)
                {
                    let var_name = &captures[1];
                    if secret_keywords
                        .iter()
                        .any(|&keyword| var_name.contains(keyword))
                    {
                        return true;
                    }
                }
            }
        }
    }

    false
}
