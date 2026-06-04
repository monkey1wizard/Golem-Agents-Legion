//! Routing JSON parser — reads `~/.gal/config/executor-routing.json`.
//!
//! Schema: `{ "ROLE_NAME": { "executor": "tool-name", "model": "model-id" }, ... }`
//!
//! All failure modes degrade safely:
//! - File missing       → empty routing table (no routing available)
//! - File unreadable    → empty routing table + warning
//! - Invalid JSON       → empty routing table + warning
//! - Role key missing   → `None` from `get()`, caller decides to inline/degrade
//! - Unknown executor   → surfaced to caller as-is; not validated here

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// One role's dispatch target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteEntry {
    /// CLI tool name: "claude" | "codex" | "copilot" | "opencode" | "agy"
    pub executor: String,
    /// Model identifier passed to the executor (e.g. "claude-sonnet-4-6", "openai/gpt-5.4")
    pub model: String,
}

/// Full routing table loaded from `executor-routing.json`.
#[derive(Debug, Default, Clone)]
pub struct RoutingTable {
    pub entries: HashMap<String, RouteEntry>,
    /// Non-fatal warnings accumulated during load (e.g. parse errors).
    pub warnings: Vec<String>,
}

impl RoutingTable {
    /// Returns the entry for `role`, or `None` if not configured.
    pub fn get(&self, role: &str) -> Option<&RouteEntry> {
        self.entries.get(role)
    }

    /// `true` if the table has at least one entry (not empty/degraded).
    pub fn has_routing(&self) -> bool {
        !self.entries.is_empty()
    }
}

/// Errors that can occur during routing load — all are non-fatal at the call site;
/// the caller receives a `RoutingTable` with warnings instead of a hard error.
#[derive(Debug, Error)]
pub enum RoutingLoadError {
    #[error("routing file not found: {0}")]
    NotFound(PathBuf),
    #[error("routing file unreadable: {0}")]
    Io(#[from] std::io::Error),
    #[error("routing file is not valid JSON: {0}")]
    Json(#[from] serde_json::Error),
}

/// Default path: `~/.gal/config/executor-routing.json`
pub fn default_routing_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".gal").join("config").join("executor-routing.json"))
}

/// Load the routing table from `path`.
///
/// Never panics. On any error, returns an empty `RoutingTable` with a warning message.
pub fn load_routing(path: &PathBuf) -> RoutingTable {
    match try_load(path) {
        Ok(table) => table,
        Err(RoutingLoadError::NotFound(_)) => RoutingTable::default(), // silent: no routing configured
        Err(e) => {
            let mut t = RoutingTable::default();
            t.warnings.push(format!("executor-routing: {e}"));
            t
        }
    }
}

/// Load from the default path (`~/.gal/config/executor-routing.json`).
/// Returns empty table with warning when the home dir is unavailable.
pub fn load_routing_default() -> RoutingTable {
    match default_routing_path() {
        None => {
            let mut t = RoutingTable::default();
            t.warnings.push("executor-routing: cannot determine home directory".to_string());
            t
        }
        Some(path) => load_routing(&path),
    }
}

fn try_load(path: &PathBuf) -> Result<RoutingTable, RoutingLoadError> {
    if !path.exists() {
        return Err(RoutingLoadError::NotFound(path.clone()));
    }
    let content = std::fs::read_to_string(path)?;
    let raw: HashMap<String, RouteEntry> = serde_json::from_str(&content)?;
    Ok(RoutingTable { entries: raw, warnings: vec![] })
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn write_json(content: &str) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(content.as_bytes()).unwrap();
        f
    }

    #[test]
    fn parses_valid_routing_json() {
        let f = write_json(r#"
        {
            "CODER":    { "executor": "claude",   "model": "claude-sonnet-4-6" },
            "TESTER":   { "executor": "opencode", "model": "openai/gpt-5.4" },
            "REVIEWER": { "executor": "codex",    "model": "o3" },
            "VERIFIER": { "executor": "copilot",  "model": "gpt-5.4" }
        }
        "#);
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.warnings.is_empty());
        let coder = table.get("CODER").unwrap();
        assert_eq!(coder.executor, "claude");
        assert_eq!(coder.model, "claude-sonnet-4-6");
        let tester = table.get("TESTER").unwrap();
        assert_eq!(tester.executor, "opencode");
    }

    #[test]
    fn missing_file_returns_empty_no_warning() {
        let path = PathBuf::from("/nonexistent/path/executor-routing.json");
        let table = load_routing(&path);
        assert!(table.entries.is_empty());
        assert!(table.warnings.is_empty(), "missing file should be silent");
    }

    #[test]
    fn malformed_json_returns_empty_with_warning() {
        let f = write_json("{ not valid json ]]]");
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.entries.is_empty());
        assert!(!table.warnings.is_empty(), "malformed JSON should produce a warning");
        assert!(table.warnings[0].contains("executor-routing"));
    }

    #[test]
    fn missing_role_returns_none() {
        let f = write_json(r#"{ "CODER": { "executor": "claude", "model": "claude-sonnet-4-6" } }"#);
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("TESTER").is_none());
    }

    #[test]
    fn empty_json_object_is_valid_empty_table() {
        let f = write_json("{}");
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.entries.is_empty());
        assert!(table.warnings.is_empty());
        assert!(!table.has_routing());
    }

    #[test]
    fn has_routing_true_when_entries_present() {
        let f = write_json(r#"{ "CODER": { "executor": "claude", "model": "claude-haiku-4-5" } }"#);
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.has_routing());
    }
}
