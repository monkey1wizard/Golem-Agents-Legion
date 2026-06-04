//! Claude Code executor adapter (T-008).
//!
//! Spike-confirmed invocation (T-001):
//!   claude -p --model <m> --output-format json --dangerously-skip-permissions
//! Spec: via stdin.
//! Session id: JSON root field `session_id`.
//! Resumable: `claude --resume <session_id>`
//!
//! Security note: `--dangerously-skip-permissions` grants the secondary CLI full
//! filesystem and terminal access. Enable only in a trusted local environment.

use std::path::Path;
use super::{Adapter, AdapterInvocation, SpecDelivery, extract_json_field};

pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn executor_name(&self) -> &str { "claude" }

    fn build_invocation(&self, model: &str, _workdir: &Path, _spec: &str) -> AdapterInvocation {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];
        if !model.is_empty() {
            args.push("--model".to_string());
            args.push(model.to_string());
        }
        AdapterInvocation { args, delivery: SpecDelivery::Stdin }
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        // Claude outputs a JSON object with `session_id` at root level.
        extract_json_field(stdout, "session_id")
    }
}
