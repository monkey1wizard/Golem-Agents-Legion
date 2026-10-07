//! GitHub Copilot CLI executor adapter.
//!
//! Spike-confirmed invocation:
//!   copilot -C <workdir> --model <m> --allow-all --output-format json -p <spec>
//! Spec: passed via `-p` CLI flag (NOT stdin).
//! Session id: `result.sessionId` nested field in JSON output.
//! Resumable: `copilot --resume=<sessionId>`
//!
//! Security note: `--allow-all` grants the secondary CLI full filesystem, tool,
//! and URL access. Enable only in a trusted local environment.

use super::{
    extract_nested_json_field, Adapter, AdapterInvocation, AdapterInvocationInput, SpecDelivery,
};

pub struct CopilotAdapter;

impl Adapter for CopilotAdapter {
    fn executor_name(&self) -> &str {
        "copilot"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        let mut args = vec![
            "-C".to_string(),
            input.workdir.to_string_lossy().into_owned(),
            "--allow-all".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
            // Context slimming (adapter-owned, always on): Copilot Free's headless
            // prompt-mode fails when it loads the large generated custom-instructions
            // context and built-in MCP servers, overflowing before it can write the
            // receipt. The task spec + its referenced agent-contract file are
            // self-sufficient (dispatch verifies the contract path before dispatch),
            // so the custom instructions are not needed. Default tools are kept — the
            // receipt write-back needs the file `write` tool, and `--available-tools
            // write` was empirically over-restrictive (no real write-back).
            "--no-custom-instructions".to_string(),
            "--disable-builtin-mcps".to_string(),
        ];
        if !input.model.is_empty() {
            args.push("--model".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            args.push("--reasoning-effort".to_string());
            args.push(effort.to_string());
        }
        // Disable each host-configured MCP server the dispatch layer resolved
        // (from `~/.copilot/mcp-config.json`), one flag per name.
        for name in input.mcp_disable_servers {
            args.push("--disable-mcp-server".to_string());
            args.push(name.clone());
        }
        // Spec is passed as the `-p` argument — copilot does not read from stdin.
        args.push("-p".to_string());
        args.push(input.spec.to_string());
        // stdin is empty because spec is in args
        AdapterInvocation {
            args,
            delivery: SpecDelivery::CliFlag("-p".to_string()),
        }
    }

    fn supports_effort(&self) -> bool {
        true
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        // Copilot JSON output has `result.sessionId` (nested).
        extract_nested_json_field(stdout, "result", "sessionId").or_else(|| {
            // Some versions emit sessionId at root level too
            super::extract_json_field(stdout, "sessionId")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn input<'a>(effort: Option<&'a str>, disable: &'a [String]) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "auto",
            workdir: Path::new("/wd"),
            spec: "spec",
            effort,
            mcp_disable_servers: disable,
            timeout_secs: None,
        }
    }

    #[test]
    fn effort_maps_to_reasoning_effort_flag() {
        let inv = CopilotAdapter.build_invocation(&input(Some("high"), &[]));
        assert!(
            inv.args.join(" ").contains("--reasoning-effort high"),
            "copilot argv should contain `--reasoning-effort high`, got {:?}",
            inv.args
        );
    }

    #[test]
    fn copilot_supports_effort() {
        assert!(CopilotAdapter.supports_effort());
    }

    #[test]
    fn context_slimming_flags_always_present_default_tools_kept() {
        let inv = CopilotAdapter.build_invocation(&input(None, &[]));
        let joined = inv.args.join(" ");
        assert!(
            joined.contains("--no-custom-instructions"),
            "got {:?}",
            inv.args
        );
        assert!(
            joined.contains("--disable-builtin-mcps"),
            "got {:?}",
            inv.args
        );
        // Base flags preserved.
        assert!(joined.contains("--allow-all"));
        assert!(joined.contains("--output-format json"));
        assert!(inv.args.iter().any(|a| a == "-C"));
        assert!(inv.args.iter().any(|a| a == "-p"));
        // Default tool surface kept — no --available-tools restriction.
        assert!(
            !inv.args.iter().any(|a| a == "--available-tools"),
            "got {:?}",
            inv.args
        );
    }

    #[test]
    fn one_disable_mcp_server_flag_per_entry() {
        let disable = vec!["srv-a".to_string(), "srv-b".to_string()];
        let inv = CopilotAdapter.build_invocation(&input(None, &disable));
        let count = inv
            .args
            .iter()
            .filter(|a| *a == "--disable-mcp-server")
            .count();
        assert_eq!(
            count, 2,
            "one --disable-mcp-server per name, got {:?}",
            inv.args
        );
        assert!(inv.args.iter().any(|a| a == "srv-a"));
        assert!(inv.args.iter().any(|a| a == "srv-b"));
    }
}
