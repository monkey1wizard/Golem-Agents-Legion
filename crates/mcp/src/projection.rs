//! GAL-managed MCP projection file: metadata, struct, load/save/validate.

use gal_foundation::mcp::{McpError, McpManifest, McpServer, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Projection metadata + projection struct
// ─────────────────────────────────────────────────────────────────────────────

/// Metadata written into the GAL-managed projection file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProjectionMetadata {
    pub ownership: String,
    pub generated_at: String,
    pub generated_by: String,
    pub secret_bearing: bool,
    pub source_files: HashMap<String, String>,
    pub preservation: String,
}

/// The GAL-managed MCP projection file (written to `~/.gal/generated/mcp/managed.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProjection {
    pub schema_version: u32,
    pub mcp_servers: HashMap<String, McpServer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inputs: Option<serde_json::Value>,
    #[serde(rename = "_metadata")]
    pub metadata: McpProjectionMetadata,
}

// ─────────────────────────────────────────────────────────────────────────────
// Plugin-root guard + load/save helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Return `Err` when the plugin root is missing any of the three required dirs.
///
/// Guards against writing MCP configs before the canonical root is complete
/// (ports the check in `Invoke-UpdateMcp`).
pub fn check_plugin_root_complete(plugin_root: &Path) -> Result<()> {
    for dir in &["commands", "agents", "skills"] {
        let p = plugin_root.join(dir);
        if !p.exists() || !p.is_dir() {
            return Err(McpError::PluginRootIncomplete(dir.to_string()));
        }
    }
    Ok(())
}

/// Load an `McpManifest` from a JSON file.
pub fn load_manifest(path: &Path) -> Result<McpManifest> {
    let content = std::fs::read_to_string(path)?;
    Ok(serde_json::from_str(&content)?)
}

/// Write an `McpProjection` to `path`, only when `plugin_root` is complete.
pub fn save_projection(path: &Path, projection: &McpProjection, plugin_root: &Path) -> Result<()> {
    check_plugin_root_complete(plugin_root)?;
    let json = serde_json::to_string_pretty(projection)?;
    std::fs::write(path, json)?;
    Ok(())
}
