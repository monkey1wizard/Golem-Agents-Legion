//! MCP (Model Context Protocol) shared data-model types.
//!
//! These are the serde-serializable manifest/server types plus the error and
//! `Result` alias shared between the MCP generation logic (currently in
//! `gal-engine::mcp`, split out at R-02/T-011) and the provider projection
//! layer (`gal-engine::providers`, split out at R-00/T-007).
//!
//! They live in `base` to break the provider→mcp dependency cycle that would
//! otherwise appear when `providers` becomes its own crate before `mcp` is
//! split (architect BUG-01). Pure data + error types; no GAL-crate deps.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

/// MCP-related errors.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("Unresolved secret placeholder: {0}")]
    UnresolvedSecret(String),

    #[error("Plugin root incomplete, missing required directory: {0}")]
    PluginRootIncomplete(String),

    #[error("Invalid variable name: {0}")]
    InvalidVariableName(String),
}

pub type Result<T> = std::result::Result<T, McpError>;

/// MCP server configuration.
///
/// Represents a single MCP server entry from a manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    pub server_type: Option<String>,

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

/// MCP manifest structure.
///
/// Represents the portable `.mcp.json` manifest from repo source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpManifest {
    pub servers: HashMap<String, McpServer>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,
}
