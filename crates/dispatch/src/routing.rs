//! Routing JSON parser — reads `~/.gal/config/executor-routing.json`.
//!
//! Schema:
//! ```json
//! {
//!   "executors": { "claude": "claude-haiku-4-5", "codex": "gpt-5.4-mini" },
//!   "CODER":     { "executor": "claude" },
//!   "TESTER":    { "executor": "codex", "model": "gpt-5.4-mini" }
//! }
//! ```
//!
//! `executors` defines the default model per tool. A role entry that omits `model`
//! inherits the default from `executors[executor]`. A role entry with an explicit
//! `model` always takes precedence.
//!
//! All failure modes degrade safely:
//! - File missing       → empty routing table (no routing available)
//! - File unreadable    → empty routing table + warning
//! - Invalid JSON       → empty routing table + warning
//! - Role key missing   → `None` from `get()`, caller decides to inline/degrade
//! - Unknown executor   → surfaced to caller as-is; not validated here

use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// One role's dispatch target (fully resolved — `model` is never empty after load).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEntry {
    /// CLI tool name: "claude" | "codex" | "copilot" | "opencode" | "agy"
    pub executor: String,
    /// Model identifier passed to the executor (e.g. "claude-haiku-4-5", "gpt-5.4-mini").
    /// Resolved from the role entry's `model` field, or from `executors[executor]` as fallback.
    pub model: String,
}

/// Raw role entry from JSON — `model` is optional; resolved against executor defaults at load time.
#[derive(Debug, Deserialize)]
struct RawRouteEntry {
    executor: String,
    #[serde(default)]
    model: Option<String>,
}

/// Full routing table loaded from `executor-routing.json`.
#[derive(Debug, Default, Clone)]
pub struct RoutingTable {
    pub entries: HashMap<String, RouteEntry>,
    /// Per-executor default models from the `executors` block (used as fallback during load).
    pub executor_defaults: HashMap<String, String>,
    /// Non-fatal warnings accumulated during load (e.g. parse errors on individual keys).
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
    let value: serde_json::Value = serde_json::from_str(&content)?;

    let obj = match value.as_object() {
        Some(o) => o,
        None => return Ok(RoutingTable::default()),
    };

    // Extract per-executor default models from the "executors" key.
    let executor_defaults: HashMap<String, String> = obj
        .get("executors")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default();

    let mut entries = HashMap::new();
    let mut warnings = vec![];

    for (key, val) in obj {
        // Skip meta keys ("executors" block and any "_*" comment keys).
        if key == "executors" || key.starts_with('_') {
            continue;
        }
        match serde_json::from_value::<RawRouteEntry>(val.clone()) {
            Ok(raw) => {
                // Role's own model takes precedence; fall back to executor default.
                let model = raw
                    .model
                    .filter(|m| !m.is_empty())
                    .or_else(|| executor_defaults.get(&raw.executor).cloned())
                    .unwrap_or_default();
                if model.is_empty() {
                    warnings.push(format!(
                        "executor-routing: role '{key}' has no model and executor '{}' has no default in `executors`",
                        raw.executor
                    ));
                }
                entries.insert(
                    key.clone(),
                    RouteEntry { executor: raw.executor, model },
                );
            }
            Err(e) => {
                warnings.push(format!("executor-routing: skipping key '{key}': {e}"));
            }
        }
    }

    Ok(RoutingTable { entries, executor_defaults, warnings })
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
    fn executor_default_used_when_model_omitted() {
        let f = write_json(r#"
        {
            "executors": { "claude": "claude-haiku-4-5", "codex": "gpt-5.4-mini" },
            "CODER":  { "executor": "claude" },
            "TESTER": { "executor": "codex" }
        }
        "#);
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.warnings.is_empty(), "unexpected warnings: {:?}", table.warnings);
        assert_eq!(table.get("CODER").unwrap().model, "claude-haiku-4-5");
        assert_eq!(table.get("TESTER").unwrap().model, "gpt-5.4-mini");
    }

    #[test]
    fn explicit_model_overrides_executor_default() {
        let f = write_json(r#"
        {
            "executors": { "claude": "claude-haiku-4-5" },
            "REVIEWER": { "executor": "claude", "model": "claude-opus-4-8" }
        }
        "#);
        let table = load_routing(&f.path().to_path_buf());
        assert_eq!(table.get("REVIEWER").unwrap().model, "claude-opus-4-8");
    }

    #[test]
    fn meta_keys_are_ignored() {
        let f = write_json(r#"
        {
            "_comment": "this is a comment",
            "executors": { "claude": "claude-haiku-4-5" },
            "CODER": { "executor": "claude" }
        }
        "#);
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("_comment").is_none());
        assert!(table.get("executors").is_none());
        assert!(table.get("CODER").is_some());
    }

    #[test]
    fn executor_defaults_exposed_on_table() {
        let f = write_json(r#"
        {
            "executors": { "claude": "claude-haiku-4-5", "codex": "gpt-5.4-mini" },
            "CODER": { "executor": "claude" }
        }
        "#);
        let table = load_routing(&f.path().to_path_buf());
        assert_eq!(table.executor_defaults.get("claude").map(|s| s.as_str()), Some("claude-haiku-4-5"));
        assert_eq!(table.executor_defaults.get("codex").map(|s| s.as_str()), Some("gpt-5.4-mini"));
    }

    #[test]
    fn warns_when_no_model_and_no_default() {
        let f = write_json(r#"{ "CODER": { "executor": "claude" } }"#);
        let table = load_routing(&f.path().to_path_buf());
        assert!(!table.warnings.is_empty(), "should warn when model cannot be resolved");
        assert!(table.warnings[0].contains("CODER"));
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
