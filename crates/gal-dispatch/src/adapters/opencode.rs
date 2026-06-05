//! OpenCode executor adapter (T-008).
//!
//! Spike-confirmed invocation (T-001):
//!   opencode run --format json --dangerously-skip-permissions -m <provider/model>
//! Spec: via stdin.
//! Session id: `sessionID` field in NDJSON stream events.
//! Resumable: `opencode run --session <sessionID>`
//!
//! `--dangerously-skip-permissions` (per <https://opencode.ai/docs/cli/>:
//! "Auto-approve permissions that are not explicitly denied") is required for
//! headless write-back — without it OpenCode auto-rejects the file `write`/`edit`
//! tool call in non-interactive mode, yielding `no-receipt`.

use std::path::Path;
use super::{Adapter, AdapterInvocation, SpecDelivery, extract_json_field};

pub struct OpenCodeAdapter;

impl Adapter for OpenCodeAdapter {
    fn executor_name(&self) -> &str { "opencode" }

    fn build_invocation(&self, model: &str, _workdir: &Path, _spec: &str) -> AdapterInvocation {
        let mut args = vec![
            "run".to_string(),
            "--format".to_string(),
            "json".to_string(),
            "--dangerously-skip-permissions".to_string(),
        ];
        if !model.is_empty() {
            args.push("-m".to_string());
            args.push(model.to_string());
        }
        AdapterInvocation { args, delivery: SpecDelivery::Stdin }
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        // OpenCode streams NDJSON events; `sessionID` appears in early events.
        extract_json_field(stdout, "sessionID")
    }
}
