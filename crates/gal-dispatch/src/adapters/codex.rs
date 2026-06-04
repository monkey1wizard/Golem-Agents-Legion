//! Codex executor adapter (T-008).
//!
//! Spike-confirmed invocation (T-002):
//!   codex exec -m <m> --json -s workspace-write
//! Spec: via stdin.
//! Session id: `thread_id` field in `thread.started` NDJSON event.
//! Resumable: `codex exec resume <thread_id>`
//!
//! `--json` activates NDJSON streaming output (required for session id extraction).
//! `-s workspace-write` is the sandbox level that permits file writes.
//! No `--dangerously-skip-permissions` or equivalent required — codex uses
//! workspace-write sandbox instead.

use std::path::Path;
use super::{Adapter, AdapterInvocation, SpecDelivery};

pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    fn executor_name(&self) -> &str { "codex" }

    fn build_invocation(&self, model: &str, _workdir: &Path, _spec: &str) -> AdapterInvocation {
        let mut args = vec![
            "exec".to_string(),
            "--json".to_string(),
            "-s".to_string(),
            "workspace-write".to_string(),
        ];
        if !model.is_empty() {
            args.push("-m".to_string());
            args.push(model.to_string());
        }
        AdapterInvocation { args, delivery: SpecDelivery::Stdin }
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        // Codex streams NDJSON events. Look for the `thread.started` event and
        // extract `thread_id` from it.
        for line in stdout.lines() {
            let line = line.trim();
            if line.is_empty() || !line.starts_with('{') {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(line) {
                // Event type is at root `type` or `event` field
                let is_thread_started =
                    v.get("type").and_then(|t| t.as_str()) == Some("thread.started")
                    || v.get("event").and_then(|t| t.as_str()) == Some("thread.started");
                if is_thread_started {
                    if let Some(id) = v.get("thread_id").and_then(|f| f.as_str()) {
                        return Some(id.to_string());
                    }
                }
                // Also try direct `thread_id` at root (some codex versions)
                if let Some(id) = v.get("thread_id").and_then(|f| f.as_str()) {
                    return Some(id.to_string());
                }
            }
        }
        None
    }
}
