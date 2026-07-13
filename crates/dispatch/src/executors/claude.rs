//! Claude Code executor adapter.
//!
//! Spike-confirmed invocation:
//!   claude -p --model <m> --output-format json --dangerously-skip-permissions
//! Spec: via stdin.
//! Session id: JSON root field `session_id`.
//! Resumable: `claude --resume <session_id>`
//!
//! Security note: `--dangerously-skip-permissions` grants the secondary CLI full
//! filesystem and terminal access. Enable only in a trusted local environment.

use super::{extract_json_field, Adapter, AdapterInvocation, AdapterInvocationInput, SpecDelivery};

pub struct ClaudeAdapter;

impl Adapter for ClaudeAdapter {
    fn executor_name(&self) -> &str {
        "claude"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "-p".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("--model".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            args.push("--effort".to_string());
            args.push(effort.to_string());
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
        // Claude outputs a JSON object with `session_id` at root level.
        extract_json_field(stdout, "session_id")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::AdapterInvocationInput;
    use std::path::Path;

    fn input<'a>(effort: Option<&'a str>) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "claude-sonnet-4-6",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: &[],
        }
    }

    #[test]
    fn effort_maps_to_effort_flag() {
        let inv = ClaudeAdapter.build_invocation(&input(Some("xhigh")));
        let joined = inv.args.join(" ");
        assert!(
            joined.contains("--effort xhigh"),
            "claude argv should contain `--effort xhigh`, got {:?}",
            inv.args
        );
    }

    #[test]
    fn no_effort_omits_flag() {
        let inv = ClaudeAdapter.build_invocation(&input(None));
        assert!(!inv.args.iter().any(|a| a == "--effort"));
    }

    #[test]
    fn claude_supports_effort() {
        assert!(ClaudeAdapter.supports_effort());
    }
}
