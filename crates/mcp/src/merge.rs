//! Safe GAL-managed merge + per-host non-destructive config writers.

use crate::{ensure_parent, McpUpdateError};
use gal_foundation::mcp::McpManifest;
use std::collections::HashMap;
use std::path::Path;

// ─────────────────────────────────────────────────────────────────────────────
// Safe-merge
// ─────────────────────────────────────────────────────────────────────────────

/// Safe merge of GAL-managed entries over user-owned entries.
///
/// GAL-managed servers (those listed in the manifest) replace any existing
/// entry; user-owned entries are preserved unchanged.
///
/// Ports `Merge-OrderedMap` + the GAL-managed guard from `Update-Mcp.ps1`.
#[derive(Debug)]
pub struct McpMerger {
    gal_managed_servers: Vec<String>,
}

impl McpMerger {
    pub fn new(gal_managed_servers: Vec<String>) -> Self {
        Self {
            gal_managed_servers,
        }
    }

    /// Merge `gal_manifest` over `existing` (if any).
    ///
    /// - GAL-managed names are always replaced.
    /// - User-owned names in `existing` are preserved.
    pub fn merge(&self, gal_manifest: &McpManifest, existing: Option<&McpManifest>) -> McpManifest {
        let mut servers = HashMap::new();
        for (name, server) in &gal_manifest.servers {
            servers.insert(name.clone(), server.clone());
        }
        if let Some(ex) = existing {
            for (name, server) in &ex.servers {
                if !self.gal_managed_servers.contains(name) {
                    servers.insert(name.clone(), server.clone());
                }
            }
        }
        McpManifest {
            servers,
            inputs: gal_manifest.inputs.clone(),
        }
    }
}
/// Non-destructively overlay GAL-managed entries into a JSON provider config.
///
/// Preserves user-owned server entries and any unrelated top-level keys.
/// `servers_key` is the provider's server-map field (`mcpServers` / `mcp`).
/// `generated` is the freshly serialized provider config (`{servers_key: {…}}`).
/// Returns the number of managed entries written. An existing file that is not
/// a JSON object is treated as an error rather than being overwritten.
pub(crate) fn write_json_provider_merged(
    dest: &Path,
    servers_key: &str,
    generated: &serde_json::Value,
) -> std::result::Result<usize, McpUpdateError> {
    let managed = generated
        .get(servers_key)
        .and_then(|v| v.as_object())
        .cloned()
        .unwrap_or_default();

    let mut root = if dest.exists() {
        let content = std::fs::read_to_string(dest)?;
        if content.trim().is_empty() {
            serde_json::Value::Object(serde_json::Map::new())
        } else {
            match serde_json::from_str::<serde_json::Value>(&content)? {
                v @ serde_json::Value::Object(_) => v,
                _ => {
                    return Err(McpUpdateError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!(
                            "{} is not a JSON object — refusing to overwrite",
                            dest.display()
                        ),
                    )))
                }
            }
        }
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };

    let obj = root.as_object_mut().expect("root is an object");
    let servers = obj
        .entry(servers_key.to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    if !servers.is_object() {
        *servers = serde_json::Value::Object(serde_json::Map::new());
    }
    let servers_obj = servers.as_object_mut().expect("servers slot is an object");

    let count = managed.len();
    for (name, entry) in managed {
        servers_obj.insert(name, entry);
    }

    ensure_parent(dest)?;
    std::fs::write(dest, serde_json::to_string_pretty(&root)?)?;
    Ok(count)
}

/// Extract the server name from a Codex TOML table header line.
///
/// Returns `Some(name)` for `[mcp_servers.NAME]` / `[mcp_servers.NAME.env]`
/// (bare or quoted), else `None`. Ports `Get-CodexTableServerName`.
pub(crate) fn codex_table_server_name(line: &str) -> Option<String> {
    let t = line.trim();
    let inner = t.strip_prefix('[')?.strip_suffix(']')?;
    let remainder = inner.strip_prefix("mcp_servers.")?;
    if remainder.is_empty() {
        return None;
    }
    if let Some(rest) = remainder.strip_prefix('"') {
        let mut out = String::new();
        let mut prev = '"';
        for c in rest.chars() {
            if c == '"' && prev != '\\' {
                return Some(out.replace("\\\"", "\"").replace("\\\\", "\\"));
            }
            out.push(c);
            prev = c;
        }
        return None;
    }
    Some(remainder.split('.').next().unwrap_or(remainder).to_string())
}

/// True for a single-bracket TOML table header `[name]` (not `[[array]]`).
fn is_toml_table_header(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('[') && !t.starts_with("[[") && t.ends_with(']') && t.len() > 2
}

/// Remove managed `[mcp_servers.NAME]` sections (and their sub-tables) for the
/// given names, preserving all other content (user MCP entries + non-MCP config).
///
/// Ports `Remove-CodexManagedServersFromToml`.
pub(crate) fn remove_codex_managed_sections(
    raw: &str,
    managed: &std::collections::HashSet<String>,
) -> String {
    if raw.trim().is_empty() {
        return String::new();
    }
    let mut out: Vec<&str> = Vec::new();
    let mut skip = false;
    for line in raw.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if let Some(name) = codex_table_server_name(line) {
            skip = managed.contains(&name);
        } else if is_toml_table_header(line) {
            skip = false;
        }
        if !skip {
            out.push(line);
        }
    }
    out.join("\r\n").trim_end_matches(['\r', '\n']).to_string()
}

/// Write Codex `config.toml` by removing managed sections then appending fresh
/// ones, preserving non-MCP content. `sections` is the codex serializer output
/// (already `\r\n`-joined with a trailing newline).
pub(crate) fn write_codex_merged(
    dest: &Path,
    managed_names: &std::collections::HashSet<String>,
    sections: &str,
) -> std::result::Result<(), McpUpdateError> {
    let existing = if dest.exists() {
        std::fs::read_to_string(dest)?
    } else {
        String::new()
    };
    let remaining = remove_codex_managed_sections(&existing, managed_names);

    let mut new_raw = remaining;
    if !new_raw.trim().is_empty() {
        new_raw = new_raw.trim_end_matches(['\r', '\n']).to_string();
        new_raw.push_str("\r\n\r\n");
    }
    new_raw.push_str(sections);
    if !new_raw.ends_with("\r\n") {
        new_raw.push_str("\r\n");
    }

    ensure_parent(dest)?;
    std::fs::write(dest, new_raw)?;
    Ok(())
}
