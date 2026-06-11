//! ssh (T-005): SSH remote-task command construction + remote path layout.
//!
//! Ports the deterministic, verifiable core of `Invoke-XmachineRemoteTask.{ps1,sh}`:
//! remote path layout, task-id shape, SSH invocation args, and authentication-
//! failure classification. The live SSH round-trip (`SshTransport::run` spawning
//! `ssh`) is the cross-machine integration seam validated at T-013 (win→mac +
//! win→win), not in unit tests — there is no remote host here.
//!
//! Compose, don't rebuild: the remote execution body is a remote `gal` binary, so
//! this layer builds the *invocation*; it does not reimplement dispatch/routing.

use std::path::{Path, PathBuf};
use std::process::Command;

use pipeline::orchestration::{DispatchPlan, Transport, TransportOutcome};
use pipeline::PipelineError;

use crate::WorkPlatform;

/// SSH connection target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTarget {
    pub user: String,
    pub host: String,
}

impl RemoteTarget {
    pub fn new(user: impl Into<String>, host: impl Into<String>) -> Self {
        Self {
            user: user.into(),
            host: host.into(),
        }
    }

    /// `user@host` — the SSH destination.
    pub fn ssh_target(&self) -> String {
        format!("{}@{}", self.user, self.host)
    }
}

/// Where the remote task runs: a throwaway workspace (`execute`) or a worktree off
/// an existing repo clone (`repo`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionMode {
    /// No persistent checkout; a fresh workspace under the task temp dir.
    Execute,
    /// A detached git worktree off the named repo path.
    Repo { repo_path: String },
}

impl ExecutionMode {
    /// The `executionMode` value recorded in remote `status.json`.
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionMode::Execute => "execute",
            ExecutionMode::Repo { .. } => "repo",
        }
    }
}

/// Remote-side paths for one dispatched task (parity with
/// `Invoke-XmachineRemoteTask`'s `$remoteTemp` / `$remoteSpec` / `$remoteWorktree`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTaskPaths {
    pub temp_dir: String,
    pub spec_path: String,
    pub worktree_path: String,
    sep: char,
}

/// Build a deterministic task id (`<yyyymmdd>-<suffix>`); the legacy random
/// 6-char suffix is supplied by the caller so the shape stays testable.
pub fn remote_task_id(date_yyyymmdd: &str, suffix: &str) -> String {
    format!("{date_yyyymmdd}-{suffix}")
}

fn join(sep: char, base: &str, child: &str) -> String {
    format!("{base}{sep}{child}")
}

/// Compute the remote task paths for a platform + execution mode.
pub fn remote_task_paths(
    platform: WorkPlatform,
    task_id: &str,
    spec_file_name: &str,
    mode: &ExecutionMode,
) -> RemoteTaskPaths {
    let (base_tmp, sep) = match platform {
        WorkPlatform::Windows => (format!(r"C:\Windows\Temp\gal-xmachine\task-{task_id}"), '\\'),
        WorkPlatform::Posix => (format!("/tmp/gal-xmachine/task-{task_id}"), '/'),
    };
    let spec_path = join(sep, &base_tmp, spec_file_name);
    let worktree_path = match mode {
        ExecutionMode::Execute => join(sep, &base_tmp, "workspace"),
        ExecutionMode::Repo { repo_path } => format!("{repo_path}-xmachine-{task_id}"),
    };
    RemoteTaskPaths {
        temp_dir: base_tmp,
        spec_path,
        worktree_path,
        sep,
    }
}

impl RemoteTaskPaths {
    /// Remote `status.json` path under the task temp dir.
    pub fn status_path(&self) -> String {
        join(self.sep, &self.temp_dir, "status.json")
    }
}

/// SSH invocation argument vector: `-o BatchMode=yes <target> <remote_command>`.
/// `BatchMode=yes` forbids password prompts — passwordless key auth is required,
/// matching the legacy hard boundary (GAL never handles SSH credentials).
pub fn ssh_invocation_args(target: &str, remote_command: &str) -> Vec<String> {
    vec![
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        target.to_string(),
        remote_command.to_string(),
    ]
}

/// `scp -o BatchMode=yes -q <from> <to>` argument vector.
pub fn scp_invocation_args(from: &str, to: &str) -> Vec<String> {
    vec![
        "-o".to_string(),
        "BatchMode=yes".to_string(),
        "-q".to_string(),
        from.to_string(),
        to.to_string(),
    ]
}

/// Classification of an SSH failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SshErrorKind {
    /// Key-based authentication failed (publickey / Permission denied).
    AuthFailed,
    /// Any other failure (host down, command error, …).
    Other,
}

/// Classify SSH stderr. Mirrors the legacy `Permission denied|publickey` probe.
pub fn classify_ssh_error(stderr: &str) -> SshErrorKind {
    if stderr.contains("Permission denied") || stderr.contains("publickey") {
        SshErrorKind::AuthFailed
    } else {
        SshErrorKind::Other
    }
}

/// Build the remote command that invokes the remote `gal` binary for one task
/// phase. The execution body on the remote is `gal`, never a shell-script fleet.
pub fn remote_gal_task_command(task_id: &str, phase: &str, spec_path: &str) -> String {
    format!("gal dispatch --pipeline-phase {phase} --task-scope {task_id} --task-spec {spec_path}")
}

/// Remote SSH transport. Holds the target + dispatch context; `run` builds the
/// SSH invocation and spawns `ssh`. The spawn is the T-013 integration seam.
#[derive(Debug, Clone)]
pub struct SshTransport {
    pub target: RemoteTarget,
    pub platform: WorkPlatform,
    pub mode: ExecutionMode,
    pub spec_file_name: String,
}

impl SshTransport {
    /// The `ssh` argument vector this transport would run for a plan (pure; this is
    /// what the unit tests assert, separate from the live spawn).
    pub fn ssh_args_for(&self, plan: &DispatchPlan) -> Vec<String> {
        let paths = remote_task_paths(
            self.platform,
            &plan.task_id,
            &self.spec_file_name,
            &self.mode,
        );
        let remote_cmd =
            remote_gal_task_command(&plan.task_id, plan.phase.as_str(), &paths.spec_path);
        ssh_invocation_args(&self.target.ssh_target(), &remote_cmd)
    }
}

impl Transport for SshTransport {
    fn label(&self) -> &'static str {
        "ssh"
    }

    fn run(
        &self,
        plan: &DispatchPlan,
        _workdir: &Path,
        _receipt_path: Option<PathBuf>,
    ) -> Result<TransportOutcome, PipelineError> {
        if plan.task_id.trim().is_empty() {
            return Err(PipelineError::BlankTaskId);
        }

        let args = self.ssh_args_for(plan);
        let output = Command::new("ssh")
            .args(&args)
            .output()
            .map_err(|e| PipelineError::Dispatch(format!("ssh spawn failed: {e}")))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let detail = match classify_ssh_error(&stderr) {
                SshErrorKind::AuthFailed => format!(
                    "ssh authentication failed (passwordless key login required): {}",
                    stderr.trim()
                ),
                SshErrorKind::Other => format!("remote dispatch failed: {}", stderr.trim()),
            };
            return Err(PipelineError::Dispatch(detail));
        }

        Ok(TransportOutcome {
            task_id: plan.task_id.clone(),
            phase: plan.phase,
            transport: self.label(),
            executor: plan.executor.clone(),
            terminal_state: "dispatched".to_string(),
            log_path: PathBuf::from(remote_task_paths(
                self.platform,
                &plan.task_id,
                &self.spec_file_name,
                &self.mode,
            )
            .status_path()),
            session_id: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatch::stage::Phase;

    fn plan(task_id: &str) -> DispatchPlan {
        DispatchPlan {
            task_id: task_id.to_string(),
            phase: Phase::Implement,
            executor: "claude".to_string(),
            model: "m".to_string(),
            args: vec![],
            stdin_spec: String::new(),
        }
    }

    #[test]
    fn ssh_target_is_user_at_host() {
        assert_eq!(RemoteTarget::new("alice", "mac-mini").ssh_target(), "alice@mac-mini");
    }

    #[test]
    fn task_id_shape_matches_legacy() {
        assert_eq!(remote_task_id("20260611", "abc123"), "20260611-abc123");
    }

    #[test]
    fn windows_remote_paths_repo_mode_match_legacy() {
        let p = remote_task_paths(
            WorkPlatform::Windows,
            "20260611-abc123",
            "T-005-implement.md",
            &ExecutionMode::Repo { repo_path: r"C:\Code\Repo".to_string() },
        );
        assert_eq!(p.temp_dir, r"C:\Windows\Temp\gal-xmachine\task-20260611-abc123");
        assert_eq!(p.spec_path, r"C:\Windows\Temp\gal-xmachine\task-20260611-abc123\T-005-implement.md");
        assert_eq!(p.worktree_path, r"C:\Code\Repo-xmachine-20260611-abc123");
        assert_eq!(p.status_path(), r"C:\Windows\Temp\gal-xmachine\task-20260611-abc123\status.json");
    }

    #[test]
    fn windows_remote_paths_execute_mode_workspace_under_temp() {
        let p = remote_task_paths(
            WorkPlatform::Windows,
            "20260611-abc123",
            "spec.md",
            &ExecutionMode::Execute,
        );
        assert_eq!(p.worktree_path, r"C:\Windows\Temp\gal-xmachine\task-20260611-abc123\workspace");
    }

    #[test]
    fn posix_remote_paths_match_legacy() {
        let p = remote_task_paths(
            WorkPlatform::Posix,
            "20260611-abc123",
            "spec.md",
            &ExecutionMode::Repo { repo_path: "/srv/repo".to_string() },
        );
        assert_eq!(p.temp_dir, "/tmp/gal-xmachine/task-20260611-abc123");
        assert_eq!(p.spec_path, "/tmp/gal-xmachine/task-20260611-abc123/spec.md");
        assert_eq!(p.worktree_path, "/srv/repo-xmachine-20260611-abc123");
        assert_eq!(p.status_path(), "/tmp/gal-xmachine/task-20260611-abc123/status.json");
    }

    #[test]
    fn execution_mode_status_strings() {
        assert_eq!(ExecutionMode::Execute.as_str(), "execute");
        assert_eq!(
            ExecutionMode::Repo { repo_path: "/x".into() }.as_str(),
            "repo"
        );
    }

    #[test]
    fn ssh_invocation_enforces_batchmode() {
        let args = ssh_invocation_args("alice@host", "gal --version");
        assert_eq!(args, vec!["-o", "BatchMode=yes", "alice@host", "gal --version"]);
    }

    #[test]
    fn scp_invocation_is_quiet_batchmode() {
        let args = scp_invocation_args("./spec.md", "alice@host:/tmp/spec.md");
        assert_eq!(
            args,
            vec!["-o", "BatchMode=yes", "-q", "./spec.md", "alice@host:/tmp/spec.md"]
        );
    }

    #[test]
    fn classify_publickey_and_permission_denied_as_auth_failure() {
        assert_eq!(classify_ssh_error("alice@host: Permission denied (publickey)."), SshErrorKind::AuthFailed);
        assert_eq!(classify_ssh_error("no matching publickey found"), SshErrorKind::AuthFailed);
        assert_eq!(classify_ssh_error("ssh: connect to host port 22: Connection refused"), SshErrorKind::Other);
    }

    #[test]
    fn remote_command_invokes_remote_gal_binary() {
        let cmd = remote_gal_task_command("T-005", "implement", "/tmp/x/spec.md");
        assert!(cmd.starts_with("gal dispatch"));
        assert!(cmd.contains("--pipeline-phase implement"));
        assert!(cmd.contains("--task-scope T-005"));
        assert!(cmd.contains("--task-spec /tmp/x/spec.md"));
    }

    #[test]
    fn ssh_transport_builds_batchmode_invocation_to_remote_gal() {
        let transport = SshTransport {
            target: RemoteTarget::new("alice", "mac-mini"),
            platform: WorkPlatform::Posix,
            mode: ExecutionMode::Execute,
            spec_file_name: "T-005-implement.md".to_string(),
        };
        let args = transport.ssh_args_for(&plan("T-005"));
        assert_eq!(args[0], "-o");
        assert_eq!(args[1], "BatchMode=yes");
        assert_eq!(args[2], "alice@mac-mini");
        assert!(args[3].starts_with("gal dispatch"));
        assert!(args[3].contains("/tmp/gal-xmachine/task-T-005/T-005-implement.md"));
        assert_eq!(transport.label(), "ssh");
    }
}
