//! Antigravity CLI (agy) executor adapter.
//!
//! Spike-confirmed invocation:
//!   agy --dangerously-skip-permissions --add-dir <workdir> [--print-timeout <n>s] <spec-via-stdin>
//! Spec: via stdin.
//! Session id: NOT in stdout — stored as newest subdirectory in `~/.agy/brain/`.
//! The adapter scans this directory post-run for the most recently modified entry.
//! Resumable: `agy --conversation <uuid>`

use super::{Adapter, AdapterInvocation, AdapterInvocationInput, SpecDelivery};
use std::path::PathBuf;

pub struct AgyAdapter;

impl Adapter for AgyAdapter {
    fn executor_name(&self) -> &str {
        "agy"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        // agy reads the spec from stdin. `--effort` vocabulary is low|medium|high.
        // Exclusive-or rule: for a model with effort tiers, the tier must be
        // supplied exactly once — either as a slug suffix or via `--effort`,
        // never both, never neither.
        // `--add-dir <workdir>` is required for agy to recognize the assigned worktree.
        // Without it, agy does not load workspace instruction context (e.g. AGENTS.md)
        // and file writes escape to its global scratch directory.
        // See `docs/architecture.md` § Executor Workdir Containment.
        let mut args = vec![
            "--dangerously-skip-permissions".to_string(),
            "--add-dir".to_string(),
            input.workdir.to_string_lossy().into_owned(),
        ];
        if !input.model.is_empty() {
            args.push("--model".to_string());
            args.push(input.model.to_string());
        }
        if let Some(effort) = input.effort {
            args.push("--effort".to_string());
            args.push(effort.to_string());
        }
        // agy's own `--print-timeout` defaults to `5m0s`, and piped stdin puts it in
        // print mode. Without this flag agy kills itself five minutes in, mid-work,
        // with `Error: timeout waiting for response` — regardless of how long the
        // dispatch layer was willing to wait, because the route's `timeoutSecs` only
        // governs GAL's own process-tree kill. Raising agy's wait to the same budget
        // makes the configured timeout the one that actually applies.
        if let Some(secs) = input.timeout_secs {
            args.push("--print-timeout".to_string());
            args.push(format!("{secs}s"));
        }
        AdapterInvocation {
            args,
            delivery: SpecDelivery::Stdin,
        }
    }

    fn supports_effort(&self) -> bool {
        true
    }

    fn extract_session_id(&self, _stdout: &str) -> Option<String> {
        // agy does NOT emit the session id to stdout. Scan `~/.agy/brain/` for
        // the most recently modified subdirectory — that is the conversation id.
        scan_newest_brain_dir()
    }
}

/// Scan agy brain directories and return the name of the most recently modified
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
    scan_newest_in(&candidates)
}

/// Pure inner scan over an explicit candidate list. A single unreadable directory
/// or entry is skipped (`continue`) instead of aborting the whole scan, so a
/// completed dispatch still yields the newest session id from a readable
/// candidate even when another candidate errors on I/O.
fn scan_newest_in(candidates: &[PathBuf]) -> Option<String> {
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for brain_dir in candidates {
        if !brain_dir.is_dir() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(brain_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if !meta.is_dir() {
                continue;
            }
            let Ok(modified) = meta.modified() else {
                continue;
            };
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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_newest_in_skips_unreadable_candidate() {
        use std::fs;
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        // candidate 1: absent path — read_dir errors; must not abort the scan.
        let missing = tmp.path().join("nope").join("brain");
        // candidate 2: a real brain dir with one session subdir.
        let brain = tmp.path().join("real").join("brain");
        fs::create_dir_all(brain.join("sess-abc")).unwrap();
        let got = scan_newest_in(&[missing, brain]);
        assert_eq!(
            got.as_deref(),
            Some("sess-abc"),
            "must return the session from the readable candidate, not None"
        );
    }

    fn input<'a>(
        model: &'a str,
        effort: Option<&'a str>,
        workdir: &'a std::path::Path,
        spec: &'a str,
    ) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model,
            workdir,
            spec,
            effort,
            mcp_disable_servers: &[],
            timeout_secs: None,
        }
    }

    fn input_with_timeout<'a>(
        model: &'a str,
        effort: Option<&'a str>,
        workdir: &'a std::path::Path,
        spec: &'a str,
        timeout_secs: Option<u64>,
    ) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model,
            workdir,
            spec,
            effort,
            mcp_disable_servers: &[],
            timeout_secs,
        }
    }

    #[test]
    fn build_invocation_args_contain_dangerously_skip_permissions() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv =
            adapter.build_invocation(&input("gemini-3.1-pro-preview", None, &workdir, "spec"));
        assert!(
            inv.args
                .iter()
                .any(|a| a == "--dangerously-skip-permissions"),
            "args should contain --dangerously-skip-permissions, got {:?}",
            inv.args
        );
    }

    #[test]
    fn build_invocation_delivery_is_stdin() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv =
            adapter.build_invocation(&input("gemini-3.1-pro-preview", None, &workdir, "spec"));
        assert!(
            matches!(inv.delivery, SpecDelivery::Stdin),
            "delivery should be Stdin"
        );
    }

    #[test]
    fn agy_supports_effort() {
        assert!(AgyAdapter.supports_effort());
    }

    #[test]
    fn build_invocation_emits_print_timeout_in_seconds_when_supplied() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv = adapter.build_invocation(&input_with_timeout(
            "gemini-3.8-flash-medium",
            None,
            &workdir,
            "spec",
            Some(1800),
        ));
        let pos = inv
            .args
            .iter()
            .position(|a| a == "--print-timeout")
            .expect("argv must carry --print-timeout when the caller supplied a budget");
        assert_eq!(
            inv.args.get(pos + 1).map(String::as_str),
            Some("1800s"),
            "the value must be the budget in seconds as a Go duration, got {:?}",
            inv.args
        );
    }

    #[test]
    fn build_invocation_omits_print_timeout_when_absent() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv = adapter.build_invocation(&input_with_timeout(
            "gemini-3.8-flash-medium",
            None,
            &workdir,
            "spec",
            None,
        ));
        assert!(
            !inv.args.iter().any(|a| a == "--print-timeout"),
            "no budget means baseline argv, got {:?}",
            inv.args
        );
    }

    #[test]
    fn build_invocation_print_timeout_follows_effort() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv = adapter.build_invocation(&input_with_timeout(
            "gemini-3.8-flash",
            Some("high"),
            &workdir,
            "spec",
            Some(900),
        ));
        assert_eq!(
            inv.args,
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                ".",
                "--model",
                "gemini-3.8-flash",
                "--effort",
                "high",
                "--print-timeout",
                "900s",
            ]
        );
    }

    #[test]
    fn build_invocation_argv_neither() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv = adapter.build_invocation(&input("", None, &workdir, "spec"));
        assert_eq!(
            inv.args,
            vec!["--dangerously-skip-permissions", "--add-dir", "."]
        );
    }

    #[test]
    fn build_invocation_argv_model_only() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv = adapter.build_invocation(&input("gemini-3.7-flash", None, &workdir, "spec"));
        assert_eq!(
            inv.args,
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                ".",
                "--model",
                "gemini-3.7-flash"
            ]
        );
    }

    #[test]
    fn build_invocation_argv_effort_only() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv = adapter.build_invocation(&input("", Some("medium"), &workdir, "spec"));
        assert_eq!(
            inv.args,
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                ".",
                "--effort",
                "medium"
            ]
        );
    }

    #[test]
    fn build_invocation_argv_both() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from(".");
        let inv =
            adapter.build_invocation(&input("gemini-3.7-flash", Some("medium"), &workdir, "spec"));
        assert_eq!(
            inv.args,
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                ".",
                "--model",
                "gemini-3.7-flash",
                "--effort",
                "medium"
            ]
        );
    }

    #[test]
    fn build_invocation_argv_space_or_non_ascii_workdir() {
        let adapter = AgyAdapter;
        let workdir = PathBuf::from("path with space/測試");
        let inv = adapter.build_invocation(&input("", None, &workdir, "spec"));
        assert!(
            inv.args
                .windows(2)
                .any(|w| w[0] == "--add-dir" && w[1] == "path with space/測試"),
            "args should contain --add-dir followed by workdir"
        );
    }
}
