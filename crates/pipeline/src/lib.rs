//! pipeline: GAL orchestration scaffold.
//!
//! T-002 establishes the dependency-correct skeleton only:
//! - the `Transport` trait lives here (so xmachine can implement it later)
//! - the local transport marker also lives here
//! - pipeline phases reuse `dispatch::stage::Phase` instead of redefining them
//!
//! This crate intentionally does NOT implement task splitting or task-spec yet.

use dispatch::stage::Phase;
use thiserror::Error;

/// One bounded pipeline execution request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRequest {
    pub task_id: String,
    pub phase: Phase,
}

/// Result of sending one bounded request through a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportReceipt {
    pub task_id: String,
    pub phase: Phase,
    pub transport: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkPlatform {
    Windows,
    Posix,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemotePipelinePaths {
    pub output_dir: String,
    pub plan_snapshot_path: String,
    pub state_snapshot_path: String,
    pub pipeline_status_path: String,
    pub pipeline_events_path: String,
    pub worktree_path: String,
    pub runner_path: String,
    pub session_name: String,
    pub launcher: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRunRecord {
    pub run_id: String,
    pub status: &'static str,
    pub work_node: String,
    pub target: String,
    pub platform: WorkPlatform,
    pub target_project_root: String,
    pub local_plan_path: String,
    pub plan_snapshot_hash: String,
    pub branch: String,
    pub base_commit: String,
    pub remote_project_repo_path: String,
    pub remote_runtime_repo_path: String,
    pub remote_output_dir: String,
    pub remote_worktree_path: String,
    pub remote_pipeline_status_path: String,
    pub remote_pipeline_events_path: String,
    pub remote_session_name: String,
    pub launcher: &'static str,
    pub dispatch_time_utc: String,
    pub converge_command: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRunContext<'a> {
    pub run_id: &'a str,
    pub work_node: &'a str,
    pub target: &'a str,
    pub platform: WorkPlatform,
    pub target_project_root: &'a str,
    pub local_plan_path: &'a str,
    pub plan_snapshot_hash: &'a str,
    pub branch: &'a str,
    pub base_commit: &'a str,
    pub remote_project_repo_path: &'a str,
    pub remote_runtime_repo_path: &'a str,
    pub dispatch_time_utc: &'a str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
    #[error("task id must not be blank")]
    BlankTaskId,
}

/// Transport abstraction owned by the pipeline layer.
///
/// Later tasks add a real local executor path and the SSH+zellij xmachine impl.
pub trait Transport {
    fn label(&self) -> &'static str;
    fn dispatch(&self, request: &PipelineRequest) -> Result<TransportReceipt, PipelineError>;
}

/// Local transport marker.
///
/// T-002 proves the ownership boundary only: local execution belongs to pipeline,
/// but phase/role semantics come from `dispatch` rather than being rebuilt here.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalTransport;

impl Transport for LocalTransport {
    fn label(&self) -> &'static str {
        "local"
    }

    fn dispatch(&self, request: &PipelineRequest) -> Result<TransportReceipt, PipelineError> {
        if request.task_id.trim().is_empty() {
            return Err(PipelineError::BlankTaskId);
        }

        Ok(TransportReceipt {
            task_id: request.task_id.clone(),
            phase: request.phase,
            transport: self.label(),
        })
    }
}

pub fn remote_pipeline_paths(
    platform: WorkPlatform,
    run_id: &str,
    remote_project_repo_path: &str,
    remote_runtime_repo_path: &str,
) -> RemotePipelinePaths {
    match platform {
        WorkPlatform::Windows => {
            let output_dir = format!(r"C:\Windows\Temp\gal-xmachine-pipeline\{run_id}");
            RemotePipelinePaths {
                plan_snapshot_path: format!(r"{output_dir}\plan.prompt.md"),
                state_snapshot_path: format!(r"{output_dir}\state.md"),
                pipeline_status_path: format!(r"{output_dir}\pipeline-status.json"),
                pipeline_events_path: format!(r"{output_dir}\pipeline-events.jsonl"),
                worktree_path: format!("{remote_project_repo_path}-xpipeline-{run_id}"),
                runner_path: format!(r"{remote_runtime_repo_path}\scripts\Start-XmachinePipeline.ps1"),
                session_name: format!("pipeline-{run_id}"),
                output_dir,
                launcher: "start-process",
            }
        }
        WorkPlatform::Posix => {
            let output_dir = format!("/tmp/gal-xmachine-pipeline/{run_id}");
            RemotePipelinePaths {
                plan_snapshot_path: format!("{output_dir}/plan.prompt.md"),
                state_snapshot_path: format!("{output_dir}/state.md"),
                pipeline_status_path: format!("{output_dir}/pipeline-status.json"),
                pipeline_events_path: format!("{output_dir}/pipeline-events.jsonl"),
                worktree_path: format!("{remote_project_repo_path}-xpipeline-{run_id}"),
                runner_path: format!("{remote_runtime_repo_path}/scripts/Start-XmachinePipeline.sh"),
                session_name: format!("pipeline-{run_id}"),
                output_dir,
                launcher: "zellij",
            }
        }
    }
}

pub fn dispatched_state_note(run_id: &str, plan_file_name: &str) -> String {
    format!("Active run: {run_id}; plan: {plan_file_name}; status: dispatched")
}

pub fn running_state_note(run_id: &str, session_name: &str) -> String {
    format!("Active run: {run_id}; session: {session_name}; status: running")
}

pub fn planned_run_record(
    context: &PipelineRunContext<'_>,
    remote_paths: &RemotePipelinePaths,
) -> PipelineRunRecord {
    PipelineRunRecord {
        run_id: context.run_id.to_string(),
        status: "dispatched",
        work_node: context.work_node.to_string(),
        target: context.target.to_string(),
        platform: context.platform,
        target_project_root: context.target_project_root.to_string(),
        local_plan_path: context.local_plan_path.to_string(),
        plan_snapshot_hash: context.plan_snapshot_hash.to_string(),
        branch: context.branch.to_string(),
        base_commit: context.base_commit.to_string(),
        remote_project_repo_path: context.remote_project_repo_path.to_string(),
        remote_runtime_repo_path: context.remote_runtime_repo_path.to_string(),
        remote_output_dir: remote_paths.output_dir.clone(),
        remote_worktree_path: remote_paths.worktree_path.clone(),
        remote_pipeline_status_path: remote_paths.pipeline_status_path.clone(),
        remote_pipeline_events_path: remote_paths.pipeline_events_path.clone(),
        remote_session_name: remote_paths.session_name.clone(),
        launcher: remote_paths.launcher,
        dispatch_time_utc: context.dispatch_time_utc.to_string(),
        converge_command: "/gal status",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_transport_accepts_valid_request() {
        let transport = LocalTransport;
        let request = PipelineRequest {
            task_id: "T-002".to_string(),
            phase: Phase::Implement,
        };

        let receipt = transport.dispatch(&request).unwrap();
        assert_eq!(receipt.task_id, "T-002");
        assert_eq!(receipt.phase, Phase::Implement);
        assert_eq!(receipt.transport, "local");
    }

    #[test]
    fn local_transport_rejects_blank_task_id() {
        let transport = LocalTransport;
        let request = PipelineRequest {
            task_id: "   ".to_string(),
            phase: Phase::Implement,
        };

        let err = transport.dispatch(&request).unwrap_err();
        assert_eq!(err, PipelineError::BlankTaskId);
    }

    #[test]
    fn pipeline_reuses_dispatch_phase_model() {
        let request = PipelineRequest {
            task_id: "T-002".to_string(),
            phase: Phase::Audit,
        };

        assert_eq!(request.phase.role(), "AUDITOR");
        assert_eq!(request.phase.as_str(), "audit");
    }

    #[test]
    fn windows_remote_pipeline_paths_match_legacy_layout() {
        let paths = remote_pipeline_paths(
            WorkPlatform::Windows,
            "20260611-abc123",
            r"C:\work\repo",
            r"D:\runtime",
        );

        assert_eq!(paths.output_dir, r"C:\Windows\Temp\gal-xmachine-pipeline\20260611-abc123");
        assert_eq!(paths.plan_snapshot_path, r"C:\Windows\Temp\gal-xmachine-pipeline\20260611-abc123\plan.prompt.md");
        assert_eq!(paths.pipeline_status_path, r"C:\Windows\Temp\gal-xmachine-pipeline\20260611-abc123\pipeline-status.json");
        assert_eq!(paths.worktree_path, r"C:\work\repo-xpipeline-20260611-abc123");
        assert_eq!(paths.runner_path, r"D:\runtime\scripts\Start-XmachinePipeline.ps1");
        assert_eq!(paths.session_name, "pipeline-20260611-abc123");
        assert_eq!(paths.launcher, "start-process");
    }

    #[test]
    fn posix_remote_pipeline_paths_match_legacy_layout() {
        let paths = remote_pipeline_paths(
            WorkPlatform::Posix,
            "20260611-abc123",
            "/srv/repo",
            "/opt/gal",
        );

        assert_eq!(paths.output_dir, "/tmp/gal-xmachine-pipeline/20260611-abc123");
        assert_eq!(paths.plan_snapshot_path, "/tmp/gal-xmachine-pipeline/20260611-abc123/plan.prompt.md");
        assert_eq!(paths.pipeline_events_path, "/tmp/gal-xmachine-pipeline/20260611-abc123/pipeline-events.jsonl");
        assert_eq!(paths.worktree_path, "/srv/repo-xpipeline-20260611-abc123");
        assert_eq!(paths.runner_path, "/opt/gal/scripts/Start-XmachinePipeline.sh");
        assert_eq!(paths.session_name, "pipeline-20260611-abc123");
        assert_eq!(paths.launcher, "zellij");
    }

    #[test]
    fn planned_run_record_matches_legacy_dispatched_state() {
        let paths = remote_pipeline_paths(
            WorkPlatform::Posix,
            "20260611-abc123",
            "/srv/repo",
            "/opt/gal",
        );
        let context = PipelineRunContext {
            run_id: "20260611-abc123",
            work_node: "mac-mini",
            target: "user@mac-mini",
            platform: WorkPlatform::Posix,
            target_project_root: "C:/Code/Golem-Agents-Legion",
            local_plan_path: ".dev/plans/refactor-gal-xmachine-rust-port.prompt.md",
            plan_snapshot_hash: "abc123hash",
            branch: "main",
            base_commit: "deadbeef",
            remote_project_repo_path: "/srv/repo",
            remote_runtime_repo_path: "/opt/gal",
            dispatch_time_utc: "2026-06-11T10:00:00Z",
        };

        let record = planned_run_record(&context, &paths);

        assert_eq!(record.run_id, "20260611-abc123");
        assert_eq!(record.status, "dispatched");
        assert_eq!(record.work_node, "mac-mini");
        assert_eq!(record.target, "user@mac-mini");
        assert_eq!(record.platform, WorkPlatform::Posix);
        assert_eq!(record.remote_output_dir, "/tmp/gal-xmachine-pipeline/20260611-abc123");
        assert_eq!(record.remote_pipeline_status_path, "/tmp/gal-xmachine-pipeline/20260611-abc123/pipeline-status.json");
        assert_eq!(record.remote_session_name, "pipeline-20260611-abc123");
        assert_eq!(record.launcher, "zellij");
        assert_eq!(record.converge_command, "/gal status");
    }

    #[test]
    fn state_notes_match_legacy_strings() {
        assert_eq!(
            dispatched_state_note("20260611-abc123", "refactor-gal-xmachine-rust-port.prompt.md"),
            "Active run: 20260611-abc123; plan: refactor-gal-xmachine-rust-port.prompt.md; status: dispatched"
        );
        assert_eq!(
            running_state_note("20260611-abc123", "pipeline-20260611-abc123"),
            "Active run: 20260611-abc123; session: pipeline-20260611-abc123; status: running"
        );
    }
}
