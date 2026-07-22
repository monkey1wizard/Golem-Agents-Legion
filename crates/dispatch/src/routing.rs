//! Routing JSON parser.
//!
//! Primary source: `~/.gal/config/config.json` — routing lives under the
//! `executorRouting` subtree, grouped by consumer into `planning` and `pipeline`:
//! ```json
//! {
//!   "executorRouting": {
//!     "executors": { "claude": "claude-haiku-4-5", "codex": "gpt-5.4-mini" },
//!     "planning": {
//!       "ARCHITECT": { "executor": "claude" }
//!     },
//!     "pipeline": {
//!       "CODER":  { "executor": "claude" },
//!       "TESTER": { "executor": "codex", "model": "gpt-5.4-mini" }
//!     }
//!   }
//! }
//! ```
//!
//! Two consumer groups, closed role allowlists:
//! - `pipeline` ← {CODER, TESTER, AUDITOR} — dispatched by `/gal pipeline`; entered
//!   into the routing table (`RoutingTable::get`).
//! - `planning` ← {ARCHITECT, ANALYST, DESIGNER, RELEASER} — consult roles read by
//!   projection (`resolve_codex_routing`) for Codex native subagents; **validated for
//!   diagnostics only, never entered** into the dispatch table.
//!
//! Hard cut: the flat shape (top-level role keys directly under `executorRouting`) is
//! **no longer parsed**. A leftover flat role key produces a visible retirement warning
//! naming the key and its target group — no alias, no fallback. A role placed in the
//! wrong group, or an unknown role inside a group, is skipped with a warning naming the
//! correct group.
//!
//! config.json is the sole routing source. A config.json missing the
//! `executorRouting` key yields an empty (degraded) routing table.
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
//! - Retired flat key   → not parsed; retirement warning names key + target group
//! - Wrong-group / unknown role → skipped with a warning naming the correct group
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
    /// Optional SSH target (`user@host` or a configured `ssh` alias) for remote dispatch.
    /// Both `ssh_target` and `remote_workdir` must be present together, or neither — a
    /// half-configured entry loads with a warning and fails loud at dispatch time.
    pub ssh_target: Option<String>,
    /// Remote workdir the SSH lane `cd`s into before running the executor. A GAL-dedicated
    /// checkout — the remote file-return flow cleans it destructively after a successful apply.
    pub remote_workdir: Option<String>,
    /// Optional reasoning-intensity hint for this role, read from the same `effort`
    /// route-entry key `resolve_codex_routing` (projection) already consumes. The dispatch
    /// layer maps it to each executor's native flag (and rejects it for an executor that
    /// cannot honor it). `None` when the role entry omits `effort`.
    pub effort: Option<String>,
}

impl RouteEntry {
    /// `true` when both `ssh_target` and `remote_workdir` are set — the route dispatches remote.
    pub fn is_remote(&self) -> bool {
        self.ssh_target.is_some() && self.remote_workdir.is_some()
    }
}

/// Raw role entry from JSON — `model` is optional; resolved against executor defaults at load time.
#[derive(Debug, Deserialize)]
struct RawRouteEntry {
    executor: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default, rename = "sshTarget")]
    ssh_target: Option<String>,
    #[serde(default, rename = "remoteWorkdir")]
    remote_workdir: Option<String>,
    #[serde(default)]
    effort: Option<String>,
}

/// Full routing table loaded from `config.json#executorRouting`.
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

/// Default routing path: `~/.gal/config/config.json` (contains `executorRouting` subtree).
pub fn default_routing_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".gal").join("config").join("config.json"))
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

/// Load routing from the default path (`config.json#executorRouting`).
/// Returns an empty table with a warning when the home directory is unavailable.
pub fn load_routing_default() -> RoutingTable {
    match default_routing_path() {
        None => {
            let mut t = RoutingTable::default();
            t.warnings
                .push("executor-routing: cannot determine home directory".to_string());
            t
        }
        Some(cfg_path) => load_routing(&cfg_path),
    }
}

/// Roles dispatched by `/gal pipeline` — parsed from the `pipeline` group into the table.
const PIPELINE_ROLES: &[&str] = &["CODER", "TESTER", "AUDITOR"];
/// Consult roles read by projection for Codex native subagents — validated in the
/// `planning` group for diagnostics only, never entered into the dispatch table.
const PLANNING_ROLES: &[&str] = &["ARCHITECT", "ANALYST", "DESIGNER", "RELEASER"];

/// The consumer group a known role belongs to (`"pipeline"` or `"planning"`), or `None`
/// when the name is not a recognized role.
fn role_group(role: &str) -> Option<&'static str> {
    if PIPELINE_ROLES.contains(&role) {
        Some("pipeline")
    } else if PLANNING_ROLES.contains(&role) {
        Some("planning")
    } else {
        None
    }
}

/// Parse one role entry `val` into a fully-resolved [`RouteEntry`], resolving the model
/// against `executor_defaults` and accumulating any model/ssh/malformed warnings into
/// `warnings`. Returns `None` (with a warning) when the entry is malformed.
///
/// Shared by both groups: the `pipeline` caller inserts the returned entry into the
/// dispatch table; the `planning` caller discards it (validation-only) while still
/// surfacing the same diagnostics.
fn parse_route_entry(
    key: &str,
    val: &serde_json::Value,
    executor_defaults: &HashMap<String, String>,
    warnings: &mut Vec<String>,
) -> Option<RouteEntry> {
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
            if raw.ssh_target.is_some() != raw.remote_workdir.is_some() {
                warnings.push(format!(
                    "executor-routing: role '{key}' has sshTarget/remoteWorkdir half-configured (both are required for remote dispatch)"
                ));
            }
            Some(RouteEntry {
                executor: raw.executor,
                model,
                ssh_target: raw.ssh_target,
                remote_workdir: raw.remote_workdir,
                effort: raw.effort.filter(|e| !e.is_empty()),
            })
        }
        Err(e) => {
            warnings.push(format!("executor-routing: skipping key '{key}': {e}"));
            None
        }
    }
}

/// Parse the roles inside one consumer group object. `group` is `"pipeline"` or
/// `"planning"`; `enter` controls whether a correctly-grouped role is inserted into
/// `entries` (pipeline) or merely validated (planning). Wrong-group and unknown roles
/// are skipped with a warning naming the correct group; `_*` comment keys are silent.
fn parse_group(
    group: &str,
    obj: &serde_json::Map<String, serde_json::Value>,
    executor_defaults: &HashMap<String, String>,
    entries: &mut HashMap<String, RouteEntry>,
    warnings: &mut Vec<String>,
    enter: bool,
) {
    for (key, val) in obj {
        if key.starts_with('_') {
            continue;
        }
        match role_group(key) {
            Some(g) if g == group => {
                if let Some(entry) = parse_route_entry(key, val, executor_defaults, warnings) {
                    if enter {
                        entries.insert(key.clone(), entry);
                    }
                }
            }
            Some(other) => warnings.push(format!(
                "executor-routing: role '{key}' is in the '{group}' group but belongs to the '{other}' group; skipping"
            )),
            None => warnings.push(format!(
                "executor-routing: unknown role '{key}' in the '{group}' group; skipping"
            )),
        }
    }
}

fn try_load(path: &PathBuf) -> Result<RoutingTable, RoutingLoadError> {
    if !path.exists() {
        return Err(RoutingLoadError::NotFound(path.clone()));
    }
    let content = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)?;

    let root = match value.as_object() {
        Some(o) => o,
        None => return Ok(RoutingTable::default()),
    };

    let Some(obj) = root.get("executorRouting").and_then(|v| v.as_object()) else {
        return Ok(RoutingTable::default());
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

    // Hard cut: recognize only `executors` / `planning` / `pipeline` / `_*` at the top
    // level. Any other top-level key is a retired flat-shape role entry — never parsed.
    for key in obj.keys() {
        if key == "executors" || key == "planning" || key == "pipeline" || key.starts_with('_') {
            continue;
        }
        match role_group(key) {
            Some(group) => warnings.push(format!(
                "executor-routing: flat top-level role '{key}' is retired; move it under the '{group}' group (executorRouting.{group}.{key})"
            )),
            None => warnings.push(format!(
                "executor-routing: unrecognized top-level key '{key}' under executorRouting (expected: executors, planning, pipeline)"
            )),
        }
    }

    // pipeline.* → dispatch table (entered). planning.* → diagnostics only (not entered).
    if let Some(pipeline) = obj.get("pipeline").and_then(|v| v.as_object()) {
        parse_group(
            "pipeline",
            pipeline,
            &executor_defaults,
            &mut entries,
            &mut warnings,
            true,
        );
    }
    if let Some(planning) = obj.get("planning").and_then(|v| v.as_object()) {
        parse_group(
            "planning",
            planning,
            &executor_defaults,
            &mut entries,
            &mut warnings,
            false,
        );
    }

    Ok(RoutingTable {
        entries,
        executor_defaults,
        warnings,
    })
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
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "pipeline": {
                    "CODER":   { "executor": "claude",   "model": "claude-sonnet-4-6" },
                    "TESTER":  { "executor": "opencode", "model": "openai/gpt-5.4" },
                    "AUDITOR": { "executor": "codex",    "model": "o3" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
        let coder = table.get("CODER").unwrap();
        assert_eq!(coder.executor, "claude");
        assert_eq!(coder.model, "claude-sonnet-4-6");
        let tester = table.get("TESTER").unwrap();
        assert_eq!(tester.executor, "opencode");
    }

    #[test]
    fn executor_default_used_when_model_omitted() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5", "codex": "gpt-5.4-mini" },
                "pipeline": {
                    "CODER":  { "executor": "claude" },
                    "TESTER": { "executor": "codex" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
        assert_eq!(table.get("CODER").unwrap().model, "claude-haiku-4-5");
        assert_eq!(table.get("TESTER").unwrap().model, "gpt-5.4-mini");
    }

    #[test]
    fn explicit_model_overrides_executor_default() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5" },
                "pipeline": {
                    "AUDITOR": { "executor": "claude", "model": "claude-opus-4-8" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert_eq!(table.get("AUDITOR").unwrap().model, "claude-opus-4-8");
    }

    #[test]
    fn meta_keys_are_ignored() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "_comment": "this is a comment",
                "executors": { "claude": "claude-haiku-4-5" },
                "pipeline": {
                    "_note": "a comment inside a group is ignored too",
                    "CODER": { "executor": "claude" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
        assert!(table.get("_comment").is_none());
        assert!(table.get("executors").is_none());
        assert!(table.get("CODER").is_some());
    }

    #[test]
    fn executor_defaults_exposed_on_table() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5", "codex": "gpt-5.4-mini" },
                "pipeline": { "CODER": { "executor": "claude" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert_eq!(
            table.executor_defaults.get("claude").map(|s| s.as_str()),
            Some("claude-haiku-4-5")
        );
        assert_eq!(
            table.executor_defaults.get("codex").map(|s| s.as_str()),
            Some("gpt-5.4-mini")
        );
    }

    #[test]
    fn warns_when_no_model_and_no_default() {
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "CODER": { "executor": "claude" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            !table.warnings.is_empty(),
            "should warn when model cannot be resolved"
        );
        assert!(table.warnings.iter().any(|w| w.contains("CODER")));
    }

    // ── Two-group parsing: hard cut, allowlists, diagnostics ──

    #[test]
    fn flat_top_level_role_is_retired_not_parsed() {
        // The flat shape is hard-cut: a top-level CODER is NOT parsed as a role and
        // produces a retirement warning naming the key + its target group.
        let f = write_json(
            r#"{ "executorRouting": { "CODER": { "executor": "claude", "model": "m" } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.get("CODER").is_none(),
            "flat top-level role must not be parsed"
        );
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("CODER") && w.contains("retired") && w.contains("pipeline")),
            "expected a retirement warning naming CODER + pipeline group, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn unrecognized_top_level_key_warns() {
        // A top-level key that is not a known role (e.g. retired LOCAL/RESEARCHER) →
        // unrecognized-key warning, not parsed.
        let f = write_json(r#"{ "executorRouting": { "LOCAL": { "executor": "claude" } } }"#);
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("LOCAL").is_none());
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("LOCAL") && w.contains("unrecognized")),
            "expected unrecognized-key warning, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn wrong_group_role_is_skipped_with_warning() {
        // planning.CODER (pipeline role in the planning group) and pipeline.ARCHITECT
        // (planning role in the pipeline group) are both skipped with a warning naming
        // the correct group.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5" },
                "planning": { "CODER": { "executor": "claude" } },
                "pipeline": { "ARCHITECT": { "executor": "claude" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.get("CODER").is_none(),
            "CODER in planning must not enter the table"
        );
        assert!(
            table.get("ARCHITECT").is_none(),
            "ARCHITECT is a planning role, never entered"
        );
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("CODER") && w.contains("planning") && w.contains("pipeline")),
            "expected wrong-group warning for CODER, got: {:?}",
            table.warnings
        );
        assert!(
            table.warnings.iter().any(|w| w.contains("ARCHITECT")
                && w.contains("pipeline")
                && w.contains("planning")),
            "expected wrong-group warning for ARCHITECT, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn unknown_role_in_group_is_skipped_with_warning() {
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "FOO": { "executor": "claude", "model": "m" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("FOO").is_none());
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("FOO") && w.contains("unknown") && w.contains("pipeline")),
            "expected unknown-role warning, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn planning_group_validated_but_not_entered() {
        // A well-formed planning role is validated (no warning) but never inserted into
        // the dispatch table — projection reads planning separately.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5" },
                "planning": { "ARCHITECT": { "executor": "claude" } },
                "pipeline": { "CODER": { "executor": "claude" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
        assert!(
            table.get("ARCHITECT").is_none(),
            "planning role must not be in the dispatch table"
        );
        assert!(table.get("CODER").is_some(), "pipeline role is entered");
    }

    #[test]
    fn planning_group_surfaces_diagnostics_without_entering() {
        // planning.ARCHITECT with a half-configured ssh pair still warns (validation),
        // even though it is never entered into the dispatch table.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5" },
                "planning": { "ARCHITECT": { "executor": "claude", "sshTarget": "user@box" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("ARCHITECT").is_none());
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("ARCHITECT") && w.contains("half-configured")),
            "planning validation should still surface the half-configured warning, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn in_group_half_configured_ssh_warns() {
        // A pipeline role with sshTarget but no remoteWorkdir still warns at load.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "pipeline": {
                    "CODER": {
                        "executor": "claude",
                        "model": "claude-sonnet-4-6",
                        "sshTarget": "user@build-box"
                    }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        let coder = table.get("CODER").unwrap();
        assert!(!coder.is_remote());
        assert!(
            table.warnings.iter().any(|w| w.contains("half-configured")),
            "expected half-configured warning, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn missing_file_returns_empty_no_warning() {
        let path = PathBuf::from("/nonexistent/path/config.json");
        let table = load_routing(&path);
        assert!(table.entries.is_empty());
        assert!(table.warnings.is_empty(), "missing file should be silent");
    }

    #[test]
    fn malformed_json_returns_empty_with_warning() {
        let f = write_json("{ not valid json ]]]");
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.entries.is_empty());
        assert!(
            !table.warnings.is_empty(),
            "malformed JSON should produce a warning"
        );
        assert!(table.warnings[0].contains("executor-routing"));
    }

    #[test]
    fn missing_role_returns_none() {
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "CODER": { "executor": "claude", "model": "claude-sonnet-4-6" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("CODER").is_some());
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
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "CODER": { "executor": "claude", "model": "claude-haiku-4-5" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.has_routing());
    }

    // config.json#executorRouting subtree handling

    #[test]
    fn config_json_executor_routing_subtree_parsed() {
        let f = write_json(
            r#"{
            "planLanguage": "en",
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5" },
                "pipeline": {
                    "CODER": { "executor": "claude" },
                    "TESTER": { "executor": "copilot", "model": "gpt-5.4" }
                }
            }
        }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
        assert_eq!(table.get("CODER").unwrap().executor, "claude");
        assert_eq!(table.get("CODER").unwrap().model, "claude-haiku-4-5");
        assert_eq!(table.get("TESTER").unwrap().model, "gpt-5.4");
        // Top-level keys from config.json must not leak into routing entries.
        assert!(table.get("planLanguage").is_none());
    }

    #[test]
    fn route_without_ssh_target_is_local() {
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "CODER": { "executor": "claude", "model": "claude-sonnet-4-6" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        let coder = table.get("CODER").unwrap();
        assert_eq!(coder.ssh_target, None);
        assert_eq!(coder.remote_workdir, None);
        assert!(!coder.is_remote());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
    }

    #[test]
    fn route_with_both_ssh_fields_is_remote() {
        let f = write_json(
            r#"{
            "executorRouting": {
                "pipeline": {
                    "CODER": {
                        "executor": "claude",
                        "model": "claude-sonnet-4-6",
                        "sshTarget": "user@build-box",
                        "remoteWorkdir": "/home/user/gal-remote"
                    }
                }
            }
        }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        let coder = table.get("CODER").unwrap();
        assert_eq!(coder.ssh_target.as_deref(), Some("user@build-box"));
        assert_eq!(
            coder.remote_workdir.as_deref(),
            Some("/home/user/gal-remote")
        );
        assert!(coder.is_remote());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
    }

    #[test]
    fn parses_effort_when_present_and_none_when_absent() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5", "codex": "gpt-5.4-mini" },
                "pipeline": {
                    "CODER":  { "executor": "claude", "effort": "xhigh" },
                    "TESTER": { "executor": "codex" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
        // Present → Some, preserving the exact string.
        assert_eq!(table.get("CODER").unwrap().effort.as_deref(), Some("xhigh"));
        // Absent → None; unrelated fields still resolve (old config compatibility).
        assert_eq!(table.get("TESTER").unwrap().effort, None);
        assert_eq!(table.get("TESTER").unwrap().model, "gpt-5.4-mini");
    }

    #[test]
    fn empty_effort_string_normalizes_to_none() {
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "CODER": { "executor": "claude", "model": "m", "effort": "" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        // An empty effort string is treated as unset (no bogus empty native flag downstream).
        assert_eq!(table.get("CODER").unwrap().effort, None);
    }

    #[test]
    fn default_routing_path_points_to_config_json() {
        let path = default_routing_path();
        if let Some(p) = path {
            assert!(
                p.ends_with("config/config.json") || p.to_string_lossy().contains("config.json"),
                "default_routing_path must resolve to config.json, got: {}",
                p.display()
            );
        }
    }
}
