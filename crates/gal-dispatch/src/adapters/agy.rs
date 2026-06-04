//! Antigravity CLI (agy) executor adapter (T-008).
//!
//! Spike-confirmed invocation (T-003):
//!   agy <spec-via-stdin>
//! Spec: via stdin.
//! Session id: NOT in stdout — stored as newest subdirectory in `~/.agy/brain/`.
//! The adapter scans this directory post-run for the most recently modified entry.
//! Resumable: `agy --conversation <uuid>`

use std::path::Path;
use super::{Adapter, AdapterInvocation, SpecDelivery};

pub struct AgyAdapter;

impl Adapter for AgyAdapter {
    fn executor_name(&self) -> &str { "agy" }

    fn build_invocation(&self, _model: &str, _workdir: &Path, _spec: &str) -> AdapterInvocation {
        // agy reads the spec from stdin; no JSON mode flag required.
        // Model injection is not supported via a standard flag in the current CLI.
        AdapterInvocation {
            args: vec![],
            delivery: SpecDelivery::Stdin,
        }
    }

    fn extract_session_id(&self, _stdout: &str) -> Option<String> {
        // agy does NOT emit the session id to stdout. Scan `~/.agy/brain/` for
        // the most recently modified subdirectory — that is the conversation id.
        scan_newest_brain_dir()
    }
}

/// Scan `~/.agy/brain/` and return the name of the most recently modified
/// subdirectory. Returns `None` if the directory doesn't exist or is empty.
fn scan_newest_brain_dir() -> Option<String> {
    let brain_dir = dirs::home_dir()?.join(".agy").join("brain");
    let entries = std::fs::read_dir(&brain_dir).ok()?;

    let mut newest: Option<(std::time::SystemTime, String)> = None;

    for entry in entries.flatten() {
        let meta = entry.metadata().ok()?;
        if !meta.is_dir() {
            continue;
        }
        let modified = meta.modified().ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        match &newest {
            None => newest = Some((modified, name)),
            Some((prev_time, _)) if modified > *prev_time => {
                newest = Some((modified, name));
            }
            _ => {}
        }
    }

    newest.map(|(_, name)| name)
}
