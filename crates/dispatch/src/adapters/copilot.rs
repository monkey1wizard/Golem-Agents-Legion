//! GitHub Copilot CLI executor adapter (T-008).
//!
//! Spike-confirmed invocation (T-001):
//!   copilot -C <workdir> --model <m> --allow-all --output-format json -p <spec>
//! Spec: passed via `-p` CLI flag (NOT stdin).
//! Session id: `result.sessionId` nested field in JSON output.
//! Resumable: `copilot --resume=<sessionId>`
//!
//! Security note: `--allow-all` grants the secondary CLI full filesystem, tool,
//! and URL access. Enable only in a trusted local environment.

use std::path::Path;
use super::{Adapter, AdapterInvocation, SpecDelivery, extract_nested_json_field};

pub struct CopilotAdapter;

impl Adapter for CopilotAdapter {
    fn executor_name(&self) -> &str { "copilot" }

    fn build_invocation(&self, model: &str, workdir: &Path, spec: &str) -> AdapterInvocation {
        let mut args = vec![
            "-C".to_string(),
            workdir.to_string_lossy().into_owned(),
            "--allow-all".to_string(),
            "--output-format".to_string(),
            "json".to_string(),
        ];
        if !model.is_empty() {
            args.push("--model".to_string());
            args.push(model.to_string());
        }
        // Spec is passed as the `-p` argument — copilot does not read from stdin.
        args.push("-p".to_string());
        args.push(spec.to_string());
        // stdin is empty because spec is in args
        AdapterInvocation { args, delivery: SpecDelivery::CliFlag("-p".to_string()) }
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        // Copilot JSON output has `result.sessionId` (nested).
        extract_nested_json_field(stdout, "result", "sessionId")
            .or_else(|| {
                // Some versions emit sessionId at root level too
                super::extract_json_field(stdout, "sessionId")
            })
    }
}
