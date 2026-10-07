//! Routing JSON parser.
//!
//! Primary source: `~/.gal/config/config.json` — routing lives under the
//! `executorRouting` subtree, grouped by consumer into `planning` and `pipeline`:
//! ```json
//! {
//!   "executorRouting": {
//!     "executors": { "claude": "claude-haiku-4-5", "codex": "example-codex-model" },
//!     "combinations": {
//!       "opencode-r1": { "executor": "opencode", "model": "openrouter/openai/gpt-5.6-luna" }
//!     },
//!     "planning": {
//!       "ARCHITECT": { "executor": "claude" }
//!     },
//!     "pipeline": {
//!       "CODER":  { "executor": "claude" },
//!       "TESTER": { "executor": "codex", "model": "example-codex-model" }
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
    /// Model identifier passed to the executor (e.g. "claude-haiku-4-5", "example-codex-model").
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
    /// Optional per-role dispatch timeout in seconds, overriding the caller's default
    /// when set. `None` when the role entry omits `timeoutSecs` or supplies `0`
    /// (rejected with a warning — a zero timeout is never meaningful).
    pub timeout_secs: Option<u64>,
}

/// Default route fields configured for one executor tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorDefault {
    pub model: String,
    pub effort: Option<String>,
    pub timeout_secs: Option<u64>,
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
    #[serde(default, rename = "timeoutSecs")]
    timeout_secs: Option<u64>,
}

/// Full routing table loaded from `config.json#executorRouting`.
#[derive(Debug, Default, Clone)]
pub struct RoutingTable {
    pub entries: HashMap<String, RouteEntry>,
    /// Named executor/model combinations validated at load time.
    pub combinations: HashMap<String, RouteEntry>,
    /// Per-executor default models from the `executors` block (used as fallback during load).
    pub executor_defaults: HashMap<String, ExecutorDefault>,
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
    gal_foundation::paths::machine_config_path()
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
    executor_defaults: &HashMap<String, ExecutorDefault>,
    combinations: &HashMap<String, RouteEntry>,
    warnings: &mut Vec<String>,
) -> Option<RouteEntry> {
    match serde_json::from_value::<RawRouteEntry>(val.clone()) {
        Ok(raw) => {
            let base = combinations.get(&raw.executor);
            let executor = base
                .map(|entry| entry.executor.clone())
                .unwrap_or_else(|| raw.executor.clone());

            // Role fields take precedence over a referenced combination, which takes
            // precedence over the literal executor's defaults.
            let model = raw
                .model
                .filter(|m| !m.is_empty())
                .or_else(|| {
                    base.and_then(|entry| (!entry.model.is_empty()).then(|| entry.model.clone()))
                })
                .or_else(|| {
                    executor_defaults
                        .get(&executor)
                        .map(|default| default.model.clone())
                })
                .unwrap_or_default();
            if model.is_empty() {
                warnings.push(format!(
                    "executor-routing: role '{key}' has no model and executor '{}' has no default in `executors`",
                    executor
                ));
            }
            let ssh_target = raw
                .ssh_target
                .or_else(|| base.and_then(|entry| entry.ssh_target.clone()));
            let remote_workdir = raw
                .remote_workdir
                .or_else(|| base.and_then(|entry| entry.remote_workdir.clone()));
            if ssh_target.is_some() != remote_workdir.is_some() {
                warnings.push(format!(
                    "executor-routing: role '{key}' has sshTarget/remoteWorkdir half-configured (both are required for remote dispatch)"
                ));
            }
            let timeout_secs = match raw.timeout_secs {
                Some(0) => {
                    warnings.push(format!(
                        "executor-routing: role '{key}' has timeoutSecs: 0, which is not a valid timeout; ignoring"
                    ));
                    None
                }
                Some(timeout_secs) => Some(timeout_secs),
                None => base.and_then(|entry| entry.timeout_secs).or_else(|| {
                    executor_defaults
                        .get(&executor)
                        .and_then(|default| default.timeout_secs)
                }),
            };
            Some(RouteEntry {
                executor: executor.clone(),
                model,
                ssh_target,
                remote_workdir,
                effort: raw.effort.filter(|e| !e.is_empty()).or_else(|| {
                    base.and_then(|entry| entry.effort.clone().filter(|e| !e.is_empty()))
                        .or_else(|| {
                            executor_defaults
                                .get(&executor)
                                .and_then(|default| default.effort.clone())
                        })
                }),
                timeout_secs,
            })
        }
        Err(e) => {
            warnings.push(format!("executor-routing: skipping key '{key}': {e}"));
            None
        }
    }
}

/// Parse a named combination without resolving another combination or applying role defaults.
/// Combination diagnostics name combinations rather than roles because this path validates the
/// registry as an independent, non-nested collection.
fn parse_combination_entry(
    key: &str,
    val: &serde_json::Value,
    warnings: &mut Vec<String>,
) -> Option<RouteEntry> {
    match serde_json::from_value::<RawRouteEntry>(val.clone()) {
        Ok(raw) => {
            if raw.ssh_target.is_some() != raw.remote_workdir.is_some() {
                warnings.push(format!(
                    "executor-routing: combination '{key}' has sshTarget/remoteWorkdir half-configured (both are required for remote dispatch)"
                ));
            }
            let timeout_secs = match raw.timeout_secs {
                Some(0) => {
                    warnings.push(format!(
                        "executor-routing: combination '{key}' has timeoutSecs: 0, which is not a valid timeout; ignoring"
                    ));
                    None
                }
                other => other,
            };
            Some(RouteEntry {
                executor: raw.executor,
                model: raw.model.filter(|m| !m.is_empty()).unwrap_or_default(),
                ssh_target: raw.ssh_target,
                remote_workdir: raw.remote_workdir,
                effort: raw.effort.filter(|e| !e.is_empty()),
                timeout_secs,
            })
        }
        Err(e) => {
            warnings.push(format!(
                "executor-routing: skipping combination '{key}': {e}"
            ));
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
    executor_defaults: &HashMap<String, ExecutorDefault>,
    combinations: &HashMap<String, RouteEntry>,
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
                if let Some(entry) = parse_route_entry(
                    key,
                    val,
                    executor_defaults,
                    combinations,
                    warnings,
                ) {
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

/// Parse the optional `research` group: numbered worker keys (`"1"`, `"2"`, ...) only.
/// Unlike `pipeline`/`planning`, this group has no closed role allowlist — every worker
/// shares the `RESEARCHER` role and is distinguished by its numbered key. Worker `"0"`
/// is always a native subagent and is never routed here; a role-name key (including
/// `RESEARCHER` itself) is the wrong shape for this group. Both cases are skipped with
/// a warning. `_*` comment keys are silent. A correctly-shaped entry is inserted into
/// `entries` under the worker-qualified key `RESEARCHER#<N>`.
fn parse_research_group(
    obj: &serde_json::Map<String, serde_json::Value>,
    executor_defaults: &HashMap<String, ExecutorDefault>,
    combinations: &HashMap<String, RouteEntry>,
    entries: &mut HashMap<String, RouteEntry>,
    warnings: &mut Vec<String>,
) {
    for (key, val) in obj {
        if key.starts_with('_') {
            continue;
        }
        match key.parse::<u32>() {
            Ok(0) => warnings.push(
                "executor-routing: research worker '0' is skipped; worker #0 is always a native subagent and is never routed"
                    .to_string(),
            ),
            Ok(n) => {
                if let Some(entry) = parse_route_entry(
                    key,
                    val,
                    executor_defaults,
                    combinations,
                    warnings,
                ) {
                    entries.insert(format!("RESEARCHER#{n}"), entry);
                }
            }
            Err(_) => warnings.push(format!(
                "executor-routing: role '{key}' in the 'research' group must be a numbered worker key (e.g. \"1\", \"2\"), not a role name; skipping"
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

    let mut entries = HashMap::new();
    let mut warnings = vec![];

    // Extract per-executor defaults from the "executors" key. Both the legacy
    // model string and the structured form are accepted independently.
    let executor_defaults: HashMap<String, ExecutorDefault> = obj
        .get("executors")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(key, value)| {
                    let default = if let Some(model) = value.as_str() {
                        Some(ExecutorDefault {
                            model: model.to_string(),
                            effort: None,
                            timeout_secs: None,
                        })
                    } else if let Some(object) = value.as_object() {
                        let model = object.get("model").and_then(|v| v.as_str());
                        if model.is_none()
                            || object
                                .get("effort")
                                .is_some_and(|v| !v.is_string() && !v.is_null())
                            || object
                                .get("timeoutSecs")
                                .is_some_and(|v| !v.is_u64())
                        {
                            warnings.push(format!(
                                "executor-routing: skipping executor default '{key}': expected an object with a string model, optional string effort, and optional unsigned timeoutSecs"
                            ));
                            return None;
                        }
                        let timeout_secs = match object.get("timeoutSecs").and_then(|v| v.as_u64()) {
                            Some(0) => {
                                warnings.push(format!(
                                    "executor-routing: executor default '{key}' has timeoutSecs: 0, which is not a valid timeout; ignoring"
                                ));
                                None
                            }
                            other => other,
                        };
                        Some(ExecutorDefault {
                            model: model.unwrap().to_string(),
                            effort: object
                                .get("effort")
                                .and_then(|v| v.as_str())
                                .filter(|effort| !effort.is_empty())
                                .map(str::to_string),
                            timeout_secs,
                        })
                    } else {
                        warnings.push(format!(
                            "executor-routing: skipping executor default '{key}': expected a model string or an object with model defaults"
                        ));
                        None
                    };
                    default.map(|default| (key.clone(), default))
                })
                .collect()
        })
        .unwrap_or_default();

    // Hard cut: recognize only `executors` / `combinations` / `planning` / `pipeline` /
    // `research` / `_*`
    // at the top level. Any other top-level key is a retired flat-shape role entry —
    // never parsed. `RESEARCHER` gets its own retirement wording naming the `research`
    // group, since it is not a member of `PIPELINE_ROLES`/`PLANNING_ROLES`.
    for key in obj.keys() {
        if key == "executors"
            || key == "combinations"
            || key == "planning"
            || key == "pipeline"
            || key == "research"
            || key.starts_with('_')
        {
            continue;
        }
        if key == "RESEARCHER" {
            warnings.push(
                "executor-routing: flat top-level role 'RESEARCHER' is retired; move it under the 'research' group (executorRouting.research.\"<N>\")"
                    .to_string(),
            );
            continue;
        }
        match role_group(key) {
            Some(group) => warnings.push(format!(
                "executor-routing: flat top-level role '{key}' is retired; move it under the '{group}' group (executorRouting.{group}.{key})"
            )),
            None => warnings.push(format!(
                "executor-routing: unrecognized top-level key '{key}' under executorRouting (expected: executors, combinations, planning, pipeline, research)"
            )),
        }
    }

    // Build the registry independently. In particular, never resolve an in-progress or already
    // parsed combination here; every entry must name a literal executor directly.
    let mut combinations = HashMap::new();
    if let Some(raw_combinations) = obj.get("combinations").and_then(|v| v.as_object()) {
        for (key, val) in raw_combinations {
            if key.starts_with('_') {
                continue;
            }
            if crate::executors::get_adapter(key).is_some() {
                warnings.push(format!(
                    "executor-routing: combination '{key}' has a literal executor tool collision; rejected at load time and not entered into the registry"
                ));
                continue;
            }
            let Some(entry) = parse_combination_entry(key, val, &mut warnings) else {
                continue;
            };
            if crate::executors::get_adapter(&entry.executor).is_none() {
                warnings.push(format!(
                    "executor-routing: combination '{key}' executor '{}' names another combination instead of a literal tool; rejected at load time and not entered into the registry",
                    entry.executor
                ));
                continue;
            }
            combinations.insert(key.clone(), entry);
        }
    }

    // pipeline.* → dispatch table (entered). planning.* → diagnostics only (not entered).
    if let Some(pipeline) = obj.get("pipeline").and_then(|v| v.as_object()) {
        parse_group(
            "pipeline",
            pipeline,
            &executor_defaults,
            &combinations,
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
            &combinations,
            &mut entries,
            &mut warnings,
            false,
        );
    }
    // research.* → dispatch table (entered), keyed by worker number rather than role
    // name. Optional group: absent entirely emits no warning.
    if let Some(research) = obj.get("research").and_then(|v| v.as_object()) {
        parse_research_group(
            research,
            &executor_defaults,
            &combinations,
            &mut entries,
            &mut warnings,
        );
    }

    Ok(RoutingTable {
        entries,
        combinations,
        executor_defaults,
        warnings,
    })
}

/// Per-worker resolution result. `route` is the resolved dispatch target, `None`
/// for a native-subagent worker. `winning_rule` names which resolution branch
/// produced the result (recordable evidence for the caller that dispatches per-worker
/// routing).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerRoute {
    pub route: Option<RouteEntry>,
    pub winning_rule: &'static str,
}

/// Winning-rule token: worker `#0`, always native, never looked up.
const RULE_NATIVE_SUBAGENT: &str = "native-subagent";
/// Winning-rule token: an explicit `research."N"` entry beat agy preference.
const RULE_EXPLICIT_RESEARCH: &str = "explicit-research";
/// Winning-rule token: no explicit entry, but `executors.agy` is defined.
const RULE_AGY_PREFERENCE: &str = "agy-preference";

/// Resolve worker `worker`'s dispatch route from `table`.
///
/// Worker `#0` always resolves to a native subagent and the table is never
/// consulted for it. Workers `#1`/`#2` (and any other non-zero worker) resolve
/// independently: an explicit `research."N"` entry (`RESEARCHER#N` in the
/// table) wins over the `executors.agy` preference route, which itself wins
/// over falling back to a native subagent. The agy-preference route carries no
/// `effort` — it is a plain executor+model pairing, not a routed role entry.
pub fn resolve_worker_route(worker: u32, table: &RoutingTable) -> WorkerRoute {
    if worker == 0 {
        return WorkerRoute {
            route: None,
            winning_rule: RULE_NATIVE_SUBAGENT,
        };
    }

    if let Some(entry) = table.get(&format!("RESEARCHER#{worker}")) {
        return WorkerRoute {
            route: Some(entry.clone()),
            winning_rule: RULE_EXPLICIT_RESEARCH,
        };
    }

    if let Some(default) = table.executor_defaults.get("agy") {
        return WorkerRoute {
            route: Some(RouteEntry {
                executor: "agy".to_string(),
                model: default.model.clone(),
                ssh_target: None,
                remote_workdir: None,
                effort: None,
                timeout_secs: None,
            }),
            winning_rule: RULE_AGY_PREFERENCE,
        };
    }

    WorkerRoute {
        route: None,
        winning_rule: RULE_NATIVE_SUBAGENT,
    }
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
                "executors": { "claude": "claude-haiku-4-5", "codex": "test-codex-model" },
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
        assert_eq!(table.get("TESTER").unwrap().model, "test-codex-model");
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
    fn parse_route_entry_resolves_registered_combination_with_inline_override_and_rejects_zero_timeout(
    ) {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5" },
                "combinations": {
                    "remote-fast": {
                        "executor": "claude",
                        "effort": "high",
                        "sshTarget": "user@build-box",
                        "remoteWorkdir": "/srv/gal",
                        "timeoutSecs": 90
                    }
                },
                "pipeline": {
                    "CODER": {
                        "executor": "remote-fast",
                        "model": "role-model",
                        "timeoutSecs": 0
                    }
                }
            }
        }
        "#,
        );

        let table = load_routing(&f.path().to_path_buf());
        let coder = table.get("CODER").expect("resolved role should be present");

        println!(
            "parse_route_entry resolves a role's executor to a registered combination's fields with per-field inline override and rejects an explicit zero timeoutSecs instead of falling through to the combination"
        );
        assert_eq!(
            coder.executor, "claude",
            "parse_route_entry resolves a role's executor to a registered combination's fields with per-field inline override and rejects an explicit zero timeoutSecs instead of falling through to the combination"
        );
        assert_eq!(coder.model, "role-model");
        assert_eq!(coder.effort.as_deref(), Some("high"));
        assert_eq!(coder.ssh_target.as_deref(), Some("user@build-box"));
        assert_eq!(coder.remote_workdir.as_deref(), Some("/srv/gal"));
        assert_eq!(coder.timeout_secs, None);
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
                "executors": { "claude": "claude-haiku-4-5", "codex": "test-codex-model" },
                "pipeline": { "CODER": { "executor": "claude" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert_eq!(
            table
                .executor_defaults
                .get("claude")
                .map(|default| default.model.as_str()),
            Some("claude-haiku-4-5")
        );
        assert_eq!(
            table
                .executor_defaults
                .get("codex")
                .map(|default| default.model.as_str()),
            Some("test-codex-model")
        );
    }

    #[test]
    fn executors_opencode_accepts_an_object_with_model_effort_and_timeout_secs_alongside_the_existing_bare_string_shorthand(
    ) {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": {
                    "claude": "claude-haiku-4-5",
                    "opencode": {
                        "model": "openai/gpt-5.4",
                        "effort": "medium",
                        "timeoutSecs": 900
                    }
                },
                "pipeline": {
                    "CODER": { "executor": "claude" },
                    "TESTER": { "executor": "opencode" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        println!(
            "executors.opencode accepts an object with model effort and timeoutSecs alongside the existing bare string shorthand"
        );
        let coder = table.get("CODER").expect("CODER should resolve");
        assert_eq!(coder.executor, "claude");
        assert_eq!(coder.model, "claude-haiku-4-5");

        let tester = table.get("TESTER").expect(
            "executors.opencode accepts an object with model effort and timeoutSecs alongside the existing bare string shorthand",
        );
        assert_eq!(
            tester.model, "openai/gpt-5.4",
            "executors.opencode accepts an object with model effort and timeoutSecs alongside the existing bare string shorthand"
        );
        assert_eq!(
            tester.effort.as_deref(),
            Some("medium"),
            "executors.opencode accepts an object with model effort and timeoutSecs alongside the existing bare string shorthand"
        );
        assert_eq!(
            tester.timeout_secs,
            Some(900),
            "executors.opencode accepts an object with model effort and timeoutSecs alongside the existing bare string shorthand"
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

    #[test]
    fn rejects_combination_named_like_literal_executor() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "combinations": {
                    "claude": { "executor": "opencode", "model": "m" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        println!(
            "a combination named the same as a literal executor tool is rejected at load time and not entered into the registry"
        );
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("combination")
                    && w.contains("claude")
                    && w.contains("literal")
                    && w.contains("collision")),
            "a combination named the same as a literal executor tool is rejected at load time and not entered into the registry; warnings: {:?}",
            table.warnings
        );
        assert!(
            table.get("claude").is_none(),
            "a rejected combination must not enter role routing"
        );
    }

    #[test]
    fn rejects_combination_that_references_another_combination() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "combinations": {
                    "base": { "executor": "claude", "model": "m" },
                    "fast": { "executor": "base", "model": "m" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        println!(
            "a combination whose executor field names another combination instead of a literal tool is rejected at load time and not entered into the registry"
        );
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("combination")
                    && w.contains("fast")
                    && w.contains("base")
                    && w.contains("literal")),
            "a combination whose executor field names another combination instead of a literal tool is rejected at load time and not entered into the registry; warnings: {:?}",
            table.warnings
        );
        assert!(
            table.get("fast").is_none(),
            "a rejected chained combination must not enter role routing"
        );
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
                "executors": { "claude": "claude-haiku-4-5", "codex": "test-codex-model" },
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
        assert_eq!(table.get("TESTER").unwrap().model, "test-codex-model");
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
    fn parses_timeout_secs_when_present_and_none_when_absent() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "pipeline": {
                    "CODER":  { "executor": "claude", "model": "m", "timeoutSecs": 900 },
                    "TESTER": { "executor": "claude", "model": "m" }
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
        assert_eq!(table.get("CODER").unwrap().timeout_secs, Some(900));
        assert_eq!(table.get("TESTER").unwrap().timeout_secs, None);
    }

    #[test]
    fn zero_timeout_secs_is_rejected_with_warning() {
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "CODER": { "executor": "claude", "model": "m", "timeoutSecs": 0 } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert_eq!(table.get("CODER").unwrap().timeout_secs, None);
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("CODER") && w.contains("timeoutSecs")),
            "expected zero-timeout warning naming CODER, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn parse_route_entry_resolves_registered_combination_with_field_overrides() {
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "opencode": "executor-default-model" },
                "combinations": {
                    "fallback": {
                        "executor": "opencode",
                        "effort": "base-effort",
                        "sshTarget": "user@build-box",
                        "remoteWorkdir": "/srv/base",
                        "timeoutSecs": 120
                    },
                    "modelled": {
                        "executor": "opencode",
                        "model": "combination-model"
                    }
                },
                "pipeline": {
                    "CODER": {
                        "executor": "fallback",
                        "model": "",
                        "effort": "inline-effort",
                        "remoteWorkdir": "/srv/role",
                        "timeoutSecs": 0
                    },
                    "TESTER": { "executor": "modelled" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());

        let coder = table.get("CODER").unwrap();
        assert_eq!(coder.executor, "opencode");
        assert_eq!(coder.model, "executor-default-model");
        assert_eq!(coder.effort.as_deref(), Some("inline-effort"));
        assert_eq!(coder.ssh_target.as_deref(), Some("user@build-box"));
        assert_eq!(coder.remote_workdir.as_deref(), Some("/srv/role"));
        assert_eq!(coder.timeout_secs, None);

        let tester = table.get("TESTER").unwrap();
        assert_eq!(tester.executor, "opencode");
        assert_eq!(tester.model, "combination-model");
    }

    // ── optional `research` group, numbered keys ──

    #[test]
    fn research_numbered_keys_enter_dispatch_table() {
        // research."1" + research."2" → two entries under worker-qualified keys
        // (RESEARCHER#1 / RESEARCHER#2, per R3's marker convention), no warnings.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "claude": "claude-haiku-4-5" },
                "research": {
                    "1": { "executor": "claude" },
                    "2": { "executor": "codex", "model": "gpt-5.4" }
                }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.get("RESEARCHER#1").is_some() && table.get("RESEARCHER#2").is_some(),
            "research numbered keys enter the dispatch table"
        );
        assert!(
            table.warnings.is_empty(),
            "unexpected warnings: {:?}",
            table.warnings
        );
        assert_eq!(table.get("RESEARCHER#1").unwrap().executor, "claude");
        assert_eq!(table.get("RESEARCHER#2").unwrap().model, "gpt-5.4");
    }

    #[test]
    fn absent_research_group_emits_no_warning() {
        // No `research` key at all under executorRouting → silent, nothing to warn about.
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "CODER": { "executor": "claude", "model": "m" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(
            table.warnings.is_empty(),
            "absent research group must emit no warning, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn research_worker_zero_warns_never_routed() {
        // research."0" is skipped with a warning: worker #0 is always a native subagent.
        let f = write_json(
            r#"{ "executorRouting": { "research": { "0": { "executor": "claude", "model": "m" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("RESEARCHER#0").is_none());
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains('0') && w.contains("native")),
            "expected a warning naming worker #0 as always native, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn research_role_name_key_warns_numbered_key_shape() {
        // research.RESEARCHER (a role-name key, not a numbered key) is skipped with a
        // warning describing the required numbered-key shape.
        let f = write_json(
            r#"{ "executorRouting": { "research": { "RESEARCHER": { "executor": "claude", "model": "m" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("RESEARCHER").is_none());
        assert!(
            table
                .warnings
                .iter()
                .any(|w| w.contains("RESEARCHER") && w.contains("research")),
            "expected a numbered-key-shape warning naming RESEARCHER, got: {:?}",
            table.warnings
        );
    }

    #[test]
    fn pipeline_numbered_key_warns_and_stays_out_of_table() {
        // pipeline."1" is a numbered key inside the wrong (pipeline) group; it must warn
        // and never enter the dispatch table.
        let f = write_json(
            r#"{ "executorRouting": { "pipeline": { "1": { "executor": "claude", "model": "m" } } } }"#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(table.get("1").is_none());
        assert!(table.entries.is_empty());
        assert!(
            !table.warnings.is_empty(),
            "expected pipeline.\"1\" to warn, got no warnings"
        );
    }

    // ── per-worker resolution, route + winning rule ──

    #[test]
    fn worker_zero_resolves_to_native_subagent_never_looked_up() {
        // Worker #0 is always native, regardless of any research config present —
        // the table must not even be consulted for #0.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "agy": "agy-default-model" },
                "research": { "1": { "executor": "agy" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        let resolved = resolve_worker_route(0, &table);
        assert!(
            resolved.route.is_none(),
            "worker 0 resolves to native subagent"
        );
        assert_eq!(resolved.winning_rule, "native-subagent");
    }

    #[test]
    fn explicit_research_entry_beats_agy_preference() {
        // Worker #1 with both an explicit research."1" entry and executors.agy defined
        // must resolve to the explicit entry, not the agy-preference route.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "agy": "agy-default-model" },
                "research": { "1": { "executor": "codex", "model": "gpt-5.4" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        let resolved = resolve_worker_route(1, &table);
        let route = resolved
            .route
            .expect("explicit research entry must resolve");
        assert_eq!(route.executor, "codex");
        assert_eq!(route.model, "gpt-5.4");
        assert_eq!(resolved.winning_rule, "explicit-research");
    }

    #[test]
    fn agy_preference_used_when_no_explicit_entry_and_agy_defined() {
        // Worker #2 with no explicit research."2" entry but executors.agy defined
        // routes to agy at its default model, and carries no effort.
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "agy": "agy-default-model" }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        let resolved = resolve_worker_route(2, &table);
        let route = resolved.route.expect("agy preference must resolve a route");
        assert_eq!(route.executor, "agy");
        assert_eq!(route.model, "agy-default-model");
        assert_eq!(route.effort, None, "agy preference route sets no effort");
        assert_eq!(resolved.winning_rule, "agy-preference");
    }

    #[test]
    fn zero_config_gives_three_native_workers() {
        // No research group and no executors.agy: #0, #1, #2 all resolve native.
        let f = write_json(r#"{ "executorRouting": {} }"#);
        let table = load_routing(&f.path().to_path_buf());
        for worker in 0..=2u32 {
            let resolved = resolve_worker_route(worker, &table);
            assert!(
                resolved.route.is_none(),
                "worker {worker} must resolve native under zero config"
            );
            assert_eq!(resolved.winning_rule, "native-subagent");
        }
    }

    #[test]
    fn every_case_reports_a_winning_rule() {
        // Every branch (native #0, explicit research, agy preference, native fallback)
        // must expose a non-empty winning rule (recordable evidence for the caller).
        let f = write_json(
            r#"
        {
            "executorRouting": {
                "executors": { "agy": "agy-default-model" },
                "research": { "1": { "executor": "codex", "model": "gpt-5.4" } }
            }
        }
        "#,
        );
        let table = load_routing(&f.path().to_path_buf());
        assert!(!resolve_worker_route(0, &table).winning_rule.is_empty());
        assert!(!resolve_worker_route(1, &table).winning_rule.is_empty());
        assert!(!resolve_worker_route(2, &table).winning_rule.is_empty());
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
