//! result (T-007): remote result collection.
//!
//! Ports `Get-XmachineRemoteResult.ps1`: the retrieved file set, `status.json`
//! shape + the "still running" poll predicate, the scp pull invocation, and the
//! mode-aware remote cleanup. Network round-trips (the actual scp/ssh) are the
//! T-013 integration seam; the deterministic logic is what is unit-tested here.

use serde::Deserialize;

use crate::ssh::{scp_invocation_args, ExecutionMode};
use crate::WorkPlatform;

/// Files pulled back from the remote task output dir, in retrieval order.
pub const RESULT_FILES: [&str; 4] = ["status.json", "summary.md", "runtime.log", "result.patch"];

/// Parsed remote `status.json` (written by the remote runner). Unknown fields are
/// ignored; absent optional fields deserialize to `None`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    pub task_id: String,
    pub status: String,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub started_at: Option<String>,
    #[serde(default)]
    pub finished_at: Option<String>,
    #[serde(default)]
    pub error_message: Option<String>,
    #[serde(default)]
    pub worktree_path: Option<String>,
    #[serde(default)]
    pub execution_mode: Option<String>,
}

impl RemoteStatus {
    /// `true` while the remote task is still running (legacy `status -ne "running"`
    /// is the break condition; here we report the inverse).
    pub fn is_running(&self) -> bool {
        self.status == "running"
    }
}

/// Parse a `status.json` body. Returns `None` when the body is empty or not yet
/// valid JSON (the remote may not have written it yet) so callers keep polling.
pub fn parse_status(body: &str) -> Option<RemoteStatus> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

/// Poll decision: keep waiting while status is missing/unparsable or still
/// running; stop once a terminal status is observed.
pub fn poll_should_continue(status: Option<&RemoteStatus>) -> bool {
    match status {
        None => true,
        Some(s) => s.is_running(),
    }
}

/// scp argument vector to pull one result file from `remote_output_dir` into
/// `local_dir` (uses the same `-o BatchMode=yes -q` contract as dispatch).
pub fn pull_file_args(
    ssh_target: &str,
    remote_output_dir: &str,
    file: &str,
    local_dir: &str,
) -> Vec<String> {
    let from = format!("{ssh_target}:{remote_output_dir}/{file}");
    let to = format!("{local_dir}/{file}");
    scp_invocation_args(&from, &to)
}

fn remove_dir_command(platform: WorkPlatform, dir: &str) -> String {
    match platform {
        WorkPlatform::Windows => {
            format!("Remove-Item -Recurse -Force '{dir}' -ErrorAction SilentlyContinue")
        }
        WorkPlatform::Posix => format!("rm -rf '{dir}'"),
    }
}

/// Remote cleanup commands after retrieval (parity with the legacy cleanup
/// branches): repo mode unlocks + force-removes the worktree, then both modes
/// remove the output dir. Worktree removal is `git`, identical on either platform;
/// output-dir removal is platform-specific.
pub fn cleanup_commands(
    platform: WorkPlatform,
    mode: &ExecutionMode,
    output_dir: &str,
    worktree_path: &str,
) -> Vec<String> {
    let mut cmds = Vec::new();
    if matches!(mode, ExecutionMode::Repo { .. }) {
        cmds.push(format!("git worktree unlock '{worktree_path}'"));
        cmds.push(format!("git worktree remove --force '{worktree_path}'"));
    }
    cmds.push(remove_dir_command(platform, output_dir));
    cmds
}

#[cfg(test)]
mod tests {
    use super::*;

    const STATUS_JSON: &str = r#"{
        "taskId": "20260611-abc123",
        "status": "completed",
        "exitCode": 0,
        "startedAt": "2026-06-11T10:00:00Z",
        "finishedAt": "2026-06-11T10:05:00Z",
        "worktreePath": "/srv/repo-xmachine-20260611-abc123",
        "executionMode": "repo"
    }"#;

    #[test]
    fn result_file_set_matches_legacy() {
        assert_eq!(
            RESULT_FILES,
            ["status.json", "summary.md", "runtime.log", "result.patch"]
        );
    }

    #[test]
    fn parses_status_json() {
        let s = parse_status(STATUS_JSON).unwrap();
        assert_eq!(s.task_id, "20260611-abc123");
        assert_eq!(s.status, "completed");
        assert_eq!(s.exit_code, Some(0));
        assert_eq!(s.execution_mode.as_deref(), Some("repo"));
        assert_eq!(
            s.worktree_path.as_deref(),
            Some("/srv/repo-xmachine-20260611-abc123")
        );
        assert!(!s.is_running());
    }

    #[test]
    fn empty_or_invalid_status_is_none_and_keeps_polling() {
        assert!(parse_status("").is_none());
        assert!(parse_status("   ").is_none());
        assert!(parse_status("not json").is_none());
        assert!(poll_should_continue(None));
    }

    #[test]
    fn running_status_keeps_polling_terminal_stops() {
        let running = parse_status(r#"{"taskId":"t","status":"running"}"#).unwrap();
        assert!(running.is_running());
        assert!(poll_should_continue(Some(&running)));

        let done = parse_status(r#"{"taskId":"t","status":"completed"}"#).unwrap();
        assert!(!poll_should_continue(Some(&done)));
    }

    #[test]
    fn pull_file_builds_scp_from_remote_to_local() {
        let args = pull_file_args(
            "alice@mac-mini",
            "/tmp/gal-xmachine/task-20260611-abc123",
            "status.json",
            "./.tmp/gal-results/20260611-abc123",
        );
        assert_eq!(
            args,
            vec![
                "-o",
                "BatchMode=yes",
                "-q",
                "alice@mac-mini:/tmp/gal-xmachine/task-20260611-abc123/status.json",
                "./.tmp/gal-results/20260611-abc123/status.json",
            ]
        );
    }

    #[test]
    fn cleanup_repo_mode_removes_worktree_then_output_dir() {
        let cmds = cleanup_commands(
            WorkPlatform::Posix,
            &ExecutionMode::Repo { repo_path: "/srv/repo".into() },
            "/tmp/gal-xmachine/task-x",
            "/srv/repo-xmachine-x",
        );
        assert_eq!(cmds.len(), 3);
        assert_eq!(cmds[0], "git worktree unlock '/srv/repo-xmachine-x'");
        assert_eq!(cmds[1], "git worktree remove --force '/srv/repo-xmachine-x'");
        assert_eq!(cmds[2], "rm -rf '/tmp/gal-xmachine/task-x'");
    }

    #[test]
    fn cleanup_execute_mode_only_removes_output_dir() {
        let cmds = cleanup_commands(
            WorkPlatform::Windows,
            &ExecutionMode::Execute,
            r"C:\Windows\Temp\gal-xmachine\task-x",
            r"C:\Windows\Temp\gal-xmachine\task-x\workspace",
        );
        assert_eq!(cmds.len(), 1);
        assert_eq!(
            cmds[0],
            r"Remove-Item -Recurse -Force 'C:\Windows\Temp\gal-xmachine\task-x' -ErrorAction SilentlyContinue"
        );
    }
}
