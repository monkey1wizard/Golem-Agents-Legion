//! remote_run (T-005 orchestration): the control-node command sequence that
//! composes the building blocks into a remote dispatch + result-collection plan.
//!
//! This is the glue over `ssh` / `zellij` / `result` / `session_record`. It builds
//! the ordered ssh/scp invocations a control node would run — parity with
//! `Invoke-XmachineRemoteTask.{ps1,sh}` (dispatch) and `Get-XmachineRemoteResult`
//! (collect). It does NOT execute them: the live SSH round-trip is the T-013
//! integration seam (win→mac + win→win). Building the plan is pure and fully
//! unit-tested, so T-013 reduces to "run this plan against real machines and assert
//! parity", and the deterministic logic can't silently drift.

use crate::result::RESULT_FILES;
use crate::ssh::{
    remote_gal_task_command, remote_task_paths, scp_invocation_args, ssh_invocation_args,
    ExecutionMode, RemoteTaskPaths,
};
use crate::zellij::remote_launch_script;
use crate::WorkPlatform;

/// One control-node action against the remote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepAction {
    /// Run `remote_command` on the remote over ssh.
    Ssh { remote_command: String },
    /// Upload a local file to the remote over scp.
    ScpUp { local: String, remote: String },
    /// Download a remote file to local over scp.
    ScpDown { remote: String, local: String },
}

/// A named control-node step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteStep {
    pub name: &'static str,
    pub action: StepAction,
}

impl RemoteStep {
    /// Resolve this step to the program + argv that would run it.
    pub fn argv(&self, ssh_target: &str) -> (&'static str, Vec<String>) {
        match &self.action {
            StepAction::Ssh { remote_command } => {
                ("ssh", ssh_invocation_args(ssh_target, remote_command))
            }
            StepAction::ScpUp { local, remote } => (
                "scp",
                scp_invocation_args(local, &format!("{ssh_target}:{remote}")),
            ),
            StepAction::ScpDown { remote, local } => (
                "scp",
                scp_invocation_args(&format!("{ssh_target}:{remote}"), local),
            ),
        }
    }
}

/// Remote command to create a directory (platform-specific shell).
pub fn remote_mkdir_command(platform: WorkPlatform, dir: &str) -> String {
    match platform {
        WorkPlatform::Windows => {
            format!("New-Item -ItemType Directory -Force -Path '{dir}' | Out-Null")
        }
        WorkPlatform::Posix => format!("mkdir -p '{dir}'"),
    }
}

/// Remote command to prepare the workspace: a detached git worktree off the repo
/// (repo mode) or a fresh workspace dir (execute mode). `git -C` is cross-platform.
pub fn prepare_workspace_command(
    platform: WorkPlatform,
    mode: &ExecutionMode,
    worktree_path: &str,
) -> String {
    match mode {
        ExecutionMode::Repo { repo_path } => {
            format!("git -C '{repo_path}' worktree add --detach '{worktree_path}' HEAD")
        }
        ExecutionMode::Execute => remote_mkdir_command(platform, worktree_path),
    }
}

/// Remote command that launches the (detached) remote run. POSIX uses the zellij
/// detach/poll/run script; Windows uses a hidden `Start-Process`. Both invoke the
/// remote `gal` body (`inner_command`).
pub fn launch_command(
    platform: WorkPlatform,
    session: &str,
    output_dir: &str,
    inner_command: &str,
) -> String {
    match platform {
        WorkPlatform::Posix => remote_launch_script(session, output_dir, inner_command),
        WorkPlatform::Windows => {
            // Hidden detached process; the remote runner owns status.json (parity
            // with Invoke-XmachineRemoteTask's Start-Process launch).
            format!(
                "New-Item -ItemType Directory -Force -Path '{output_dir}' | Out-Null; \
                 Start-Process pwsh -ArgumentList '-NoProfile','-Command','{inner_command}' -WindowStyle Hidden"
            )
        }
    }
}

/// The full dispatch plan for one remote task: ensure temp dir → upload spec →
/// prepare workspace → launch. Mirrors `Invoke-XmachineRemoteTask`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteDispatchPlan {
    pub ssh_target: String,
    pub paths: RemoteTaskPaths,
    pub session: String,
    pub steps: Vec<RemoteStep>,
}

#[allow(clippy::too_many_arguments)]
pub fn build_dispatch_plan(
    platform: WorkPlatform,
    ssh_target: &str,
    mode: &ExecutionMode,
    task_id: &str,
    phase: &str,
    local_spec_path: &str,
    spec_file_name: &str,
    session: &str,
) -> RemoteDispatchPlan {
    let paths = remote_task_paths(platform, task_id, spec_file_name, mode);
    let inner = remote_gal_task_command(task_id, phase, &paths.spec_path);

    let steps = vec![
        RemoteStep {
            name: "ensure-temp-dir",
            action: StepAction::Ssh {
                remote_command: remote_mkdir_command(platform, &paths.temp_dir),
            },
        },
        RemoteStep {
            name: "upload-spec",
            action: StepAction::ScpUp {
                local: local_spec_path.to_string(),
                remote: paths.spec_path.clone(),
            },
        },
        RemoteStep {
            name: "prepare-workspace",
            action: StepAction::Ssh {
                remote_command: prepare_workspace_command(platform, mode, &paths.worktree_path),
            },
        },
        RemoteStep {
            name: "launch",
            action: StepAction::Ssh {
                remote_command: launch_command(platform, session, &paths.temp_dir, &inner),
            },
        },
    ];

    RemoteDispatchPlan {
        ssh_target: ssh_target.to_string(),
        paths,
        session: session.to_string(),
        steps,
    }
}

/// Remote command to read `status.json` for the poll loop (platform shell).
pub fn status_read_command(platform: WorkPlatform, status_path: &str) -> String {
    match platform {
        WorkPlatform::Windows => {
            format!("Get-Content -Path '{status_path}' -Raw -ErrorAction SilentlyContinue")
        }
        WorkPlatform::Posix => format!("cat '{status_path}' 2>/dev/null || true"),
    }
}

/// The collection plan: pull each result file down, then cleanup. Mirrors
/// `Get-XmachineRemoteResult` (the poll loop itself is driven by the executor using
/// `status_read_command` + `result::poll_should_continue`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteCollectPlan {
    pub ssh_target: String,
    pub status_read: String,
    pub pull_steps: Vec<RemoteStep>,
    pub cleanup: Vec<String>,
}

pub fn build_collect_plan(
    platform: WorkPlatform,
    ssh_target: &str,
    mode: &ExecutionMode,
    task_id: &str,
    spec_file_name: &str,
    local_dir: &str,
) -> RemoteCollectPlan {
    let paths = remote_task_paths(platform, task_id, spec_file_name, mode);
    let pull_steps = RESULT_FILES
        .iter()
        .map(|file| RemoteStep {
            name: "pull-result",
            action: StepAction::ScpDown {
                remote: format!("{}/{file}", paths.temp_dir),
                local: format!("{local_dir}/{file}"),
            },
        })
        .collect();
    let cleanup = crate::result::cleanup_commands(platform, mode, &paths.temp_dir, &paths.worktree_path);

    RemoteCollectPlan {
        ssh_target: ssh_target.to_string(),
        status_read: status_read_command(platform, &paths.status_path()),
        pull_steps,
        cleanup,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_plan_posix_repo_mode_sequence() {
        let plan = build_dispatch_plan(
            WorkPlatform::Posix,
            "alice@mac-mini",
            &ExecutionMode::Repo { repo_path: "/srv/repo".into() },
            "20260611-abc",
            "implement",
            "./.dev/task-specs/T-005-implement.md",
            "T-005-implement.md",
            "xmachine-20260611-abc",
        );
        let names: Vec<&str> = plan.steps.iter().map(|s| s.name).collect();
        assert_eq!(names, ["ensure-temp-dir", "upload-spec", "prepare-workspace", "launch"]);

        // mkdir temp
        assert_eq!(
            plan.steps[0].action,
            StepAction::Ssh { remote_command: "mkdir -p '/tmp/gal-xmachine/task-20260611-abc'".into() }
        );
        // scp spec up
        assert_eq!(
            plan.steps[1].action,
            StepAction::ScpUp {
                local: "./.dev/task-specs/T-005-implement.md".into(),
                remote: "/tmp/gal-xmachine/task-20260611-abc/T-005-implement.md".into(),
            }
        );
        // repo worktree
        assert_eq!(
            plan.steps[2].action,
            StepAction::Ssh {
                remote_command:
                    "git -C '/srv/repo' worktree add --detach '/srv/repo-xmachine-20260611-abc' HEAD".into()
            }
        );
        // launch via zellij, running remote gal pipeline
        let StepAction::Ssh { remote_command } = &plan.steps[3].action else { panic!() };
        assert!(remote_command.contains("zellij --session 'xmachine-20260611-abc' run --close-on-exit"));
        assert!(remote_command.contains("gal pipeline /tmp/gal-xmachine/task-20260611-abc/T-005-implement.md --phase implement --task 20260611-abc"));
    }

    #[test]
    fn dispatch_plan_execute_mode_makes_workspace_dir() {
        let plan = build_dispatch_plan(
            WorkPlatform::Posix,
            "bob@scratch",
            &ExecutionMode::Execute,
            "t1",
            "implement",
            "spec.md",
            "spec.md",
            "s1",
        );
        assert_eq!(
            plan.steps[2].action,
            StepAction::Ssh { remote_command: "mkdir -p '/tmp/gal-xmachine/task-t1/workspace'".into() }
        );
    }

    #[test]
    fn windows_launch_uses_hidden_start_process() {
        let cmd = launch_command(WorkPlatform::Windows, "s", r"C:\Temp\out", "gal pipeline x --phase implement --task t");
        assert!(cmd.contains("Start-Process pwsh"));
        assert!(cmd.contains("-WindowStyle Hidden"));
        assert!(cmd.contains("gal pipeline x"));
    }

    #[test]
    fn step_argv_builds_ssh_and_scp() {
        let ssh_step = RemoteStep { name: "x", action: StepAction::Ssh { remote_command: "gal --version".into() } };
        assert_eq!(ssh_step.argv("u@h"), ("ssh", vec!["-o".into(), "BatchMode=yes".into(), "u@h".into(), "gal --version".into()]));

        let up = RemoteStep { name: "x", action: StepAction::ScpUp { local: "a".into(), remote: "/b".into() } };
        assert_eq!(up.argv("u@h"), ("scp", vec!["-o".into(), "BatchMode=yes".into(), "-q".into(), "a".into(), "u@h:/b".into()]));

        let down = RemoteStep { name: "x", action: StepAction::ScpDown { remote: "/b".into(), local: "a".into() } };
        assert_eq!(down.argv("u@h"), ("scp", vec!["-o".into(), "BatchMode=yes".into(), "-q".into(), "u@h:/b".into(), "a".into()]));
    }

    #[test]
    fn collect_plan_pulls_all_result_files_then_cleans_up() {
        let plan = build_collect_plan(
            WorkPlatform::Posix,
            "alice@mac-mini",
            &ExecutionMode::Repo { repo_path: "/srv/repo".into() },
            "t1",
            "spec.md",
            "./.tmp/gal-results/t1",
        );
        assert_eq!(plan.pull_steps.len(), 4);
        assert_eq!(
            plan.pull_steps[0].action,
            StepAction::ScpDown {
                remote: "/tmp/gal-xmachine/task-t1/status.json".into(),
                local: "./.tmp/gal-results/t1/status.json".into(),
            }
        );
        assert_eq!(plan.status_read, "cat '/tmp/gal-xmachine/task-t1/status.json' 2>/dev/null || true");
        // repo-mode cleanup removes worktree then temp dir
        assert_eq!(plan.cleanup.len(), 3);
        assert!(plan.cleanup[1].contains("git worktree remove --force"));
    }
}
