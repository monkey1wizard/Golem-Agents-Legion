//! OpenCode executor adapter.
//!
//! Invocation (verified against installed OpenCode 1.17.18):
//!   opencode run --format json --auto --agent build -m <provider/model>
//! Spec: via stdin.
//! Session id: `sessionID` field in NDJSON stream events.
//! Resumable: `opencode run --session <sessionID>`
//!
//! `--auto` ("auto-approve permissions that are not explicitly denied (dangerous!)")
//! is the current permission-bypass flag required for headless write-back — without it
//! OpenCode auto-rejects the file `write`/`edit` tool call in non-interactive mode,
//! yielding `no-receipt`. It replaces the removed `--dangerously-skip-permissions`,
//! which is no longer a valid flag in current OpenCode releases.
//!
//! `--agent build` selects the write-capable agent (the default/`plan` agent is
//! read-only, which would silently produce no file changes).
//!
//! `effort` maps to `--variant` ("model variant (provider-specific reasoning effort,
//! e.g., high, max, minimal)").

use super::{extract_json_field, Adapter, AdapterInvocation, AdapterInvocationInput, SpecDelivery};

pub struct OpenCodeAdapter;

impl Adapter for OpenCodeAdapter {
    fn executor_name(&self) -> &str {
        "opencode"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--auto".to_string(),
            "--agent".to_string(),
            "build".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("-m".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            args.push("--variant".to_string());
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
        // OpenCode streams NDJSON events; `sessionID` appears in early events.
        extract_json_field(stdout, "sessionID")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn input<'a>(effort: Option<&'a str>) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "nvidia/nvidia/nemotron-3-ultra-550b-a55b",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: &[],
        }
    }

    #[test]
    fn argv_uses_auto_agent_build_and_omits_removed_flag() {
        let inv = OpenCodeAdapter.build_invocation(&input(None));
        let joined = inv.args.join(" ");
        assert!(
            joined.contains("run --format json --auto --agent build -m nvidia/nvidia/nemotron-3-ultra-550b-a55b"),
            "opencode argv should be run --format json --auto --agent build -m <model>, got {:?}",
            inv.args
        );
        assert!(
            !inv.args
                .iter()
                .any(|a| a == "--dangerously-skip-permissions"),
            "removed flag must not appear, got {:?}",
            inv.args
        );
    }

    #[test]
    fn effort_maps_to_variant_flag() {
        let inv = OpenCodeAdapter.build_invocation(&input(Some("max")));
        assert!(
            inv.args.join(" ").contains("--variant max"),
            "opencode argv should contain `--variant max`, got {:?}",
            inv.args
        );
    }

    #[test]
    fn opencode_supports_effort() {
        assert!(OpenCodeAdapter.supports_effort());
    }
}
