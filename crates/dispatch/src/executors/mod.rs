//! Executor adapters — one per supported headless CLI tool.
//!
//! Each adapter knows:
//! - How to build the CLI invocation (args + spec delivery method)
//! - How to extract the provider-native session/job id from stdout
//!
//! Invocation flags are locked by the Phase 0 spike results:
//! | Tool     | Flags                                            | Session id field               |
//! |----------|--------------------------------------------------|-------------------------------|
//! | claude   | `-p --model <m> --output-format json --dangerously-skip-permissions` | `session_id` (JSON root)      |
//! | codex    | `exec -m <m> --json -s workspace-write`          | `thread_id` in `thread.started` event |
//! | opencode | `run --format json --dangerously-skip-permissions -m <provider/model>` | `sessionID` in stream events |
//! | copilot  | `-C <workdir> --model <m> --allow-all --output-format json -p <spec>` | `result.sessionId` (JSON root) |
//! | agy      | (stdin spec)                                     | newest `~/.agy/brain/<uuid>/`  |

use std::path::Path;

pub mod agy;
pub mod claude;
pub mod codex;
pub mod copilot;
pub mod opencode;

/// How the task spec is delivered to the executor.
pub enum SpecDelivery {
    /// Spec is written to the process stdin.
    Stdin,
    /// Spec is embedded as the value of a named CLI flag (e.g. copilot `-p`).
    CliFlag(String),
}

/// The resolved invocation produced by an adapter.
pub struct AdapterInvocation {
    /// CLI arguments to pass to the executor.
    pub args: Vec<String>,
    /// Spec delivery method. Determines whether `spec` goes to stdin or a CLI flag.
    pub delivery: SpecDelivery,
}

/// The single input struct every adapter's `build_invocation` receives.
///
/// Carries the existing model/workdir/spec plus two cross-cutting optional fields:
/// - `effort`: reasoning-intensity hint mapped to each executor's native flag
///   (only adapters that opt in via [`Adapter::supports_effort`] honor it).
/// - `mcp_disable_servers`: MCP server names an adapter should disable (currently
///   only Copilot consumes this via `--disable-mcp-server`).
///
/// Fields are borrowed — the struct is constructed fresh at the dispatch call site
/// and lives only for the `build_invocation` call, so no ownership transfer is needed.
/// When `effort` is `None` and `mcp_disable_servers` is empty, every adapter produces
/// byte-identical argv to the pre-struct behavior.
pub struct AdapterInvocationInput<'a> {
    /// Model identifier passed to the executor. Empty string means "omit the model flag".
    pub model: &'a str,
    /// Working directory of the dispatched process.
    pub workdir: &'a Path,
    /// Task spec content (delivered via stdin or a CLI flag per [`SpecDelivery`]).
    pub spec: &'a str,
    /// Optional reasoning-intensity hint. Adapters that return `true` from
    /// [`Adapter::supports_effort`] map this to their native flag; others ignore it
    /// (the dispatch layer rejects an `effort` set on an unsupporting executor before spawn).
    pub effort: Option<&'a str>,
    /// MCP server names to disable for this invocation. Empty for every executor
    /// except Copilot, which appends one `--disable-mcp-server <name>` per entry.
    pub mcp_disable_servers: &'a [String],
}

/// Adapter trait implemented by each tool's module.
pub trait Adapter: Send + Sync {
    fn executor_name(&self) -> &str;

    /// Build the invocation from the shared [`AdapterInvocationInput`].
    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation;

    /// Whether this executor can honor a route-entry `effort` value.
    ///
    /// Fail-closed: the trait default is `false`, so a newly added adapter that
    /// forgets to opt in can never silently accept-and-drop an `effort` value —
    /// the dispatch layer rejects the route with `unsupported-effort` instead.
    /// Adapters that map `effort` to a native flag override this to `true`.
    fn supports_effort(&self) -> bool {
        false
    }

    /// Extract the provider-native session/job id from the executor's stdout.
    ///
    /// Returns `None` when the id cannot be extracted (e.g. not present in output).
    /// For tools whose session id is stored on the filesystem (agy), this scans
    /// the appropriate directory.
    fn extract_session_id(&self, stdout: &str) -> Option<String>;
}

/// Look up the adapter for a given executor name. Returns `None` for unknown executors.
pub fn get_adapter(executor: &str) -> Option<Box<dyn Adapter>> {
    match executor {
        "claude" => Some(Box::new(claude::ClaudeAdapter)),
        "codex" => Some(Box::new(codex::CodexAdapter)),
        "opencode" => Some(Box::new(opencode::OpenCodeAdapter)),
        "copilot" => Some(Box::new(copilot::CopilotAdapter)),
        "agy" => Some(Box::new(agy::AgyAdapter)),
        _ => None,
    }
}

// ── Shared JSON extraction helpers ────────────────────────────────────────────

/// Extract a top-level string field from JSON (handles streaming NDJSON lines too).
pub(crate) fn extract_json_field(output: &str, field: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(s) = v.get(field).and_then(|f| f.as_str()) {
                return Some(s.to_string());
            }
        }
    }
    None
}

/// Extract a nested field `parent.child` from JSON lines.
pub(crate) fn extract_nested_json_field(output: &str, parent: &str, child: &str) -> Option<String> {
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || !line.starts_with('{') {
            continue;
        }
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(s) = v
                .get(parent)
                .and_then(|p| p.get(child))
                .and_then(|f| f.as_str())
            {
                return Some(s.to_string());
            }
        }
    }
    None
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a baseline input: no effort, no MCP disable list — the state that must
    /// reproduce the pre-struct-migration argv byte-for-byte.
    fn baseline<'a>(
        model: &'a str,
        workdir: &'a Path,
        spec: &'a str,
    ) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model,
            workdir,
            spec,
            effort: None,
            mcp_disable_servers: &[],
        }
    }

    /// Every adapter's `build_invocation` argv is byte-identical to the
    /// spike-locked baseline when `effort` is absent and the disable list is empty.
    #[test]
    fn build_invocation_argv_byte_identical_baseline() {
        let wd = Path::new("/wd");
        let inp = baseline("m", wd, "the-spec");

        assert_eq!(
            claude::ClaudeAdapter.build_invocation(&inp).args,
            vec![
                "-p",
                "--output-format",
                "json",
                "--dangerously-skip-permissions",
                "--model",
                "m"
            ]
        );
        assert_eq!(
            codex::CodexAdapter.build_invocation(&inp).args,
            vec!["exec", "--json", "-s", "workspace-write", "-m", "m"]
        );
        // OpenCode's baseline argv intentionally uses `--auto --agent build`
        // (verified against installed OpenCode 1.17.18); the older
        // `--dangerously-skip-permissions` flag was removed from the CLI. This is a
        // deliberate flag change, not a migration regression.
        assert_eq!(
            opencode::OpenCodeAdapter.build_invocation(&inp).args,
            vec!["run", "--format", "json", "--auto", "--agent", "build", "-m", "m"]
        );
        // Copilot's baseline always carries the adapter-owned context-slimming flags
        // `--no-custom-instructions` + `--disable-builtin-mcps` (a deliberate change so
        // Copilot Free headless does not overflow before writing the receipt); they are
        // not gated on effort/disable-list.
        assert_eq!(
            copilot::CopilotAdapter.build_invocation(&inp).args,
            vec![
                "-C",
                &*wd.to_string_lossy(),
                "--allow-all",
                "--output-format",
                "json",
                "--no-custom-instructions",
                "--disable-builtin-mcps",
                "--model",
                "m",
                "-p",
                "the-spec",
            ]
        );
        assert_eq!(
            agy::AgyAdapter.build_invocation(&inp).args,
            vec!["--dangerously-skip-permissions"]
        );
    }

    /// Empty model omits the `--model`/`-m` flag pair (baseline behavior preserved).
    #[test]
    fn build_invocation_empty_model_omits_model_flag() {
        let wd = Path::new("/wd");
        let inp = baseline("", wd, "spec");
        assert_eq!(
            claude::ClaudeAdapter.build_invocation(&inp).args,
            vec![
                "-p",
                "--output-format",
                "json",
                "--dangerously-skip-permissions"
            ]
        );
        assert!(!codex::CodexAdapter
            .build_invocation(&inp)
            .args
            .iter()
            .any(|a| a == "-m"));
    }

    /// Fail-closed capability default: an adapter that does not override
    /// `supports_effort` reports `false` (agy is the canonical example).
    #[test]
    fn supports_effort_defaults_false_for_agy() {
        assert!(!agy::AgyAdapter.supports_effort());
    }
}
