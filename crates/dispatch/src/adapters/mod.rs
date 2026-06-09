//! Executor adapters — one per supported headless CLI tool (T-008).
//!
//! Each adapter knows:
//! - How to build the CLI invocation (args + spec delivery method)
//! - How to extract the provider-native session/job id from stdout
//!
//! Invocation flags are locked by the Phase 0 spike results (T-001 – T-003):
//! | Tool     | Flags                                            | Session id field               |
//! |----------|--------------------------------------------------|-------------------------------|
//! | claude   | `-p --model <m> --output-format json --dangerously-skip-permissions` | `session_id` (JSON root)      |
//! | codex    | `exec -m <m> --json -s workspace-write`          | `thread_id` in `thread.started` event |
//! | opencode | `run --format json -m <provider/model>`          | `sessionID` in stream events  |
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

/// Adapter trait implemented by each tool's module.
pub trait Adapter: Send + Sync {
    fn executor_name(&self) -> &str;

    /// Build the invocation given the model, working directory, and spec content.
    fn build_invocation(&self, model: &str, workdir: &Path, spec: &str) -> AdapterInvocation;

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
        "claude"    => Some(Box::new(claude::ClaudeAdapter)),
        "codex"     => Some(Box::new(codex::CodexAdapter)),
        "opencode"  => Some(Box::new(opencode::OpenCodeAdapter)),
        "copilot"   => Some(Box::new(copilot::CopilotAdapter)),
        "agy"       => Some(Box::new(agy::AgyAdapter)),
        _           => None,
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
            if let Some(s) = v.get(parent).and_then(|p| p.get(child)).and_then(|f| f.as_str()) {
                return Some(s.to_string());
            }
        }
    }
    None
}
