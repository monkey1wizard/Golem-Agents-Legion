//! Codex executor adapter.
//!
//! Invocation:
//!   codex exec --json --dangerously-bypass-approvals-and-sandbox -m <m>
//! Spec: via stdin.
//! Session id: `thread_id` field in `thread.started` NDJSON event.
//! Resumable: `codex exec resume <thread_id>`
//!
//! `--json` activates NDJSON streaming output (required for session id extraction).
//!
//! Security note: `--dangerously-bypass-approvals-and-sandbox` runs the child
//! without Codex's sandbox, the same full-access trust level the other
//! executors get from their permission-bypass flags. The child previously ran
//! under `-s workspace-write`, but the Codex Windows sandbox fails its setup
//! refresh before any command starts (`helper_unknown_error: setup refresh had
//! errors`), so every sandboxed dispatch ended without a receipt. Headless
//! dispatch cannot answer an approval prompt, so approvals are bypassed too.

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
            "--dangerously-bypass-approvals-and-sandbox".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("-m".to_string());
            args.push(input.model.to_string());
        }
        args.push("-c".to_string());
        args.push("project_doc_max_bytes=0".to_string());
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
            model: "test-codex-model",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: &[],
            timeout_secs: None,
        }
    }

    fn assert_project_doc_suppressed(args: &[String]) {
        let adjacent = args
            .windows(2)
            .any(|w| w[0] == "-c" && w[1] == "project_doc_max_bytes=0");
        assert!(
            adjacent,
            "codex invocation must carry an adjacent project_doc_max_bytes=0 override"
        );
        let count = args
            .windows(2)
            .filter(|w| w[0] == "-c" && w[1] == "project_doc_max_bytes=0")
            .count();
        assert_eq!(count, 1, "project_doc_max_bytes=0 must occur exactly once");
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
        assert_project_doc_suppressed(&inv.args);
    }

    #[test]
    fn no_effort_omits_config_override() {
        let inv = CodexAdapter.build_invocation(&input(None));
        assert!(!inv
            .args
            .iter()
            .any(|a| a.starts_with("model_reasoning_effort")));
        assert_project_doc_suppressed(&inv.args);
        assert_eq!(
            inv.args.iter().filter(|a| *a == "-c").count(),
            1,
            "only one -c flag should be present when effort is omitted"
        );
    }

    #[test]
    fn codex_supports_effort() {
        assert!(CodexAdapter.supports_effort());
    }

    #[test]
    fn codex_child_bypasses_sandbox_and_selects_no_sandbox_mode() {
        let inv = CodexAdapter.build_invocation(&input(None));
        assert_eq!(
            inv.args
                .iter()
                .filter(|a| *a == "--dangerously-bypass-approvals-and-sandbox")
                .count(),
            1,
            "codex child must carry the bypass flag exactly once, got {:?}",
            inv.args
        );
        assert!(
            !inv.args.iter().any(|a| a == "-s" || a == "--sandbox"),
            "codex child must not select a sandbox mode, got {:?}",
            inv.args
        );
    }
}
