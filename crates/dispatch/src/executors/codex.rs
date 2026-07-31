//! Codex executor adapter.
//!
//! Spike-confirmed invocation:
//!   codex exec -m <m> --json -s workspace-write
//! Spec: via stdin.
//! Session id: `thread_id` field in `thread.started` NDJSON event.
//! Resumable: `codex exec resume <thread_id>`
//!
//! `--json` activates NDJSON streaming output (required for session id extraction).
//! `-s workspace-write` is the sandbox level that permits file writes.
//! No `--dangerously-skip-permissions` or equivalent required — codex uses
//! workspace-write sandbox instead.

use super::{Adapter, AdapterInvocation, AdapterInvocationInput, SpecDelivery};

pub struct CodexAdapter;

impl Adapter for CodexAdapter {
    fn executor_name(&self) -> &str {
        "codex"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "exec".to_string(),
            "--json".to_string(),
            "-s".to_string(),
            "workspace-write".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("-m".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            // Codex has no first-class effort flag; set it via a `-c` config override.
            // The value is embedded as a quoted TOML string. argv tokens are passed
            // directly (no shell), so the literal double quotes reach codex, which
            // parses `model_reasoning_effort="high"` as the TOML string `high`. The
            // dispatch layer validates `effort` against `[A-Za-z0-9._-]+` before spawn,
            // so the value can never contain a quote/whitespace that breaks the override.
            args.push("-c".to_string());
            args.push(format!("model_reasoning_effort=\"{effort}\""));
        }
        AdapterInvocation {
            args,
            delivery: SpecDelivery::Stdin,
        }
    }

    fn supports_effort(&self) -> bool {
        true
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
                let is_thread_started = v.get("type").and_then(|t| t.as_str())
                    == Some("thread.started")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn input<'a>(effort: Option<&'a str>) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "gpt-5.4-mini",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: &[],
        }
    }

    #[test]
    fn effort_maps_to_quoted_config_override() {
        let inv = CodexAdapter.build_invocation(&input(Some("high")));
        // The `-c` flag and the quoted TOML override are two adjacent argv tokens.
        assert!(
            inv.args
                .join(" ")
                .contains(r#"-c model_reasoning_effort="high""#),
            "codex argv should contain the quoted config override, got {:?}",
            inv.args
        );
        // The value token carries literal double quotes (no shell strips them).
        assert!(inv
            .args
            .iter()
            .any(|a| a == r#"model_reasoning_effort="high""#));
    }

    #[test]
    fn no_effort_omits_config_override() {
        let inv = CodexAdapter.build_invocation(&input(None));
        assert!(!inv
            .args
            .iter()
            .any(|a| a.starts_with("model_reasoning_effort")));
        assert!(!inv.args.iter().any(|a| a == "-c"));
    }

    #[test]
    fn codex_supports_effort() {
        assert!(CodexAdapter.supports_effort());
    }
}
