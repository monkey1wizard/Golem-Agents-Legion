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

/// Scan agy brain directory and return the name of the most recently modified
/// subdirectory (the conversation id).
///
/// On Linux/macOS: `~/.agy/brain/`
/// On Windows:     `~\.gemini\antigravity-cli\brain\`
/// Tries both; returns `None` if neither exists or both are empty.
fn scan_newest_brain_dir() -> Option<String> {
    let home = dirs::home_dir()?;
    let candidates = [
        home.join(".agy").join("brain"),
        home.join(".gemini").join("antigravity-cli").join("brain"),
    ];
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for brain_dir in &candidates {
        if !brain_dir.is_dir() { continue; }
        let entries = std::fs::read_dir(brain_dir).ok()?;

        for entry in entries.flatten() {
            let meta = entry.metadata().ok()?;
            if !meta.is_dir() {
                continue;
            }
            let modified = meta.modified().ok()?;
            let name = entry.file_name().to_string_lossy().into_owned();
            match &best {
                None => best = Some((modified, name)),
                Some((prev_time, _)) if modified > *prev_time => {
                    best = Some((modified, name));
                }
                _ => {}
            }
        }
    }

    best.map(|(_, name)| name)
}
 