//! orchestration (R-01): local task-split + multi-provider dispatch + multi-stage.
//!
//! This layer *composes* `dispatch` — it does NOT rebuild spawn, write-back,
//! routing, or stage semantics:
//! - multi-stage: [`default_stage_plan`] is the per-task phase order, grounded in
//!   the `/gal pipeline` contract (implement → test → review/audit per task; the
//!   verifier runs once at the end, not per task).
//! - multi-provider: [`resolve_plan`] maps a phase to its role via
//!   `dispatch::stage::Phase::role`, looks the role up in the routing table, and
//!   asks the matching `dispatch` adapter to build the invocation. Different phases
//!   route to different executors/models.
//! - execution: [`LocalTransport`] turns a resolved plan into a
//!   `dispatch::SpawnConfig` and runs it through `dispatch::spawn_executor`.
//!
//! The `Transport` trait is owned here so xmachine can add the SSH+zellij impl
//! later (direction pipeline ← xmachine).

use std::path::{Path, PathBuf};

use dispatch::adapters::{self, SpecDelivery};
use dispatch::dispatch::{spawn_executor, SpawnConfig};
use dispatch::routing::RoutingTable;
use dispatch::stage::Phase;

use crate::PipelineError;

/// Default hard wall-clock limit for one local dispatch, in seconds.
pub const DEFAULT_TIMEOUT_SECS: u64 = 1800;

/// Per-task phase order for the local pipeline.
///
/// Mirrors the `/gal pipeline` contract: each `T-NNN` runs implement → test →
/// review(audit). `Verify` is a single end-of-run pass over the whole plan, so it
/// is intentionally not part of the per-task stage plan.
pub fn default_stage_plan() -> Vec<Phase> {
    vec![Phase::Implement, Phase::Test, Phase::Audit]
}

/// A fully-resolved unit of work: one task at one phase, routed to one executor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DispatchPlan {
    pub task_id: String,
    pub phase: Phase,
    pub executor: String,
    pub model: String,
    pub args: Vec<String>,
    /// Spec delivered via stdin. Empty when the adapter embeds the spec in `args`
    /// (e.g. copilot `-p <spec>`).
    pub stdin_spec: String,
}

/// Resolve one task+phase into an executor invocation by composing the routing
/// table and the `dispatch` adapter for the routed executor (multi-provider).
pub fn resolve_plan(
    task_id: &str,
    phase: Phase,
    routing: &RoutingTable,
    workdir: &Path,
    spec: &str,
) -> Result<DispatchPlan, PipelineError> {
    if task_id.trim().is_empty() {
        return Err(PipelineError::BlankTaskId);
    }

    let role = phase.role();
    let route = routing
        .get(role)
        .ok_or_else(|| PipelineError::UnroutedRole(role.to_string()))?;

    let adapter = adapters::get_adapter(&route.executor)
        .ok_or_else(|| PipelineError::UnknownExecutor(route.executor.clone()))?;

    let invocation = adapter.build_invocation(&route.model, workdir, spec);
    let stdin_spec = match invocation.delivery {
        SpecDelivery::Stdin => spec.to_string(),
        SpecDelivery::CliFlag(_) => String::new(),
    };

    Ok(DispatchPlan {
        task_id: task_id.to_string(),
        phase,
        executor: route.executor.clone(),
        model: route.model.clone(),
        args: invocation.args,
        stdin_spec,
    })
}

/// Normalized result of running a [`DispatchPlan`] through a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportOutcome {
    pub task_id: String,
    pub phase: Phase,
    pub transport: &'static str,
    pub executor: String,
    /// `dispatch::TerminalState` rendered via its canonical string form.
    pub terminal_state: String,
    pub log_path: PathBuf,
    pub session_id: Option<String>,
}

/// Transport abstraction owned by the pipeline layer. Local runs compose
/// `dispatch`; the remote SSH+zellij impl is added by `xmachine` (pipeline ←
/// xmachine).
pub trait Transport {
    fn label(&self) -> &'static str;

    fn run(
        &self,
        plan: &DispatchPlan,
        workdir: &Path,
        receipt_path: Option<PathBuf>,
    ) -> Result<TransportOutcome, PipelineError>;
}

/// Local transport: executes the plan in-process via `dispatch::spawn_executor`.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalTransport;

impl LocalTransport {
    /// Compose a `dispatch::SpawnConfig` from a resolved plan. Kept separate from
    /// [`Transport::run`] so the composition can be asserted without spawning.
    pub fn spawn_config(
        &self,
        plan: &DispatchPlan,
        workdir: &Path,
        receipt_path: Option<PathBuf>,
    ) -> SpawnConfig {
        SpawnConfig {
            executor: plan.executor.clone(),
            executor_args: plan.args.clone(),
            spec: plan.stdin_spec.clone(),
            workdir: workdir.to_path_buf(),
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            task_id: plan.task_id.clone(),
            phase: plan.phase.as_str().to_string(),
            actual_model: plan.model.clone(),
            log_dir: SpawnConfig::default_log_dir(workdir),
            receipt_path,
        }
    }
}

impl Transport for LocalTransport {
    fn label(&self) -> &'static str {
        "local"
    }

    fn run(
        &self,
        plan: &DispatchPlan,
        workdir: &Path,
        receipt_path: Option<PathBuf>,
    ) -> Result<TransportOutcome, PipelineError> {
        if plan.task_id.trim().is_empty() {
            return Err(PipelineError::BlankTaskId);
        }

        let cfg = self.spawn_config(plan, workdir, receipt_path);
        let result =
            spawn_executor(&cfg).map_err(|e| PipelineError::Dispatch(e.to_string()))?;

        Ok(TransportOutcome {
            task_id: plan.task_id.clone(),
            phase: plan.phase,
            transport: self.label(),
            executor: plan.executor.clone(),
            terminal_state: result.terminal_state.as_str().to_string(),
            log_path: result.log_path,
            session_id: result.session_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dispatch::routing::RouteEntry;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn routing(pairs: &[(&str, &str, &str)]) -> RoutingTable {
        let mut entries = HashMap::new();
        for (role, executor, model) in pairs {
            entries.insert(
                role.to_string(),
                RouteEntry {
                    executor: executor.to_string(),
                    model: model.to_string(),
                },
            );
        }
        RoutingTable {
            entries,
            executor_defaults: HashMap::new(),
            warnings: vec![],
        }
    }

    #[test]
    fn default_stage_plan_is_implement_test_audit() {
        assert_eq!(
            default_stage_plan(),
            vec![Phase::Implement, Phase::Test, Phase::Audit]
        );
    }

    #[test]
    fn resolve_plan_routes_role_to_stdin_executor() {
        let table = routing(&[("CODER", "claude", "claude-haiku-4-5")]);
        let workdir = Path::new(".");
        let plan = resolve_plan("T-003", Phase::Implement, &table, workdir, "SPEC").unwrap();

        assert_eq!(plan.executor, "claude");
        assert_eq!(plan.model, "claude-haiku-4-5");
        // claude takes the spec on stdin and carries --model in args.
        assert_eq!(plan.stdin_spec, "SPEC");
        assert!(plan.args.contains(&"--model".to_string()));
        assert!(plan.args.contains(&"claude-haiku-4-5".to_string()));
    }

    #[test]
    fn resolve_plan_embeds_spec_in_args_for_cliflag_executor() {
        let table = routing(&[("CODER", "copilot", "gpt-5.4")]);
        let workdir = Path::new(".");
        let plan = resolve_plan("T-003", Phase::Implement, &table, workdir, "SPEC").unwrap();

        assert_eq!(plan.executor, "copilot");
        // copilot delivers the spec via `-p`, so stdin stays empty and args carry it.
        assert!(plan.stdin_spec.is_empty());
        assert!(plan.args.contains(&"-p".to_string()));
        assert!(plan.args.contains(&"SPEC".to_string()));
    }

    #[test]
    fn resolve_plan_is_multi_provider_per_phase() {
        let table = routing(&[
            ("CODER", "claude", "claude-haiku-4-5"),
            ("TESTER", "codex", "gpt-5.4-mini"),
        ]);
        let workdir = Path::new(".");
        let coder = resolve_plan("T-003", Phase::Implement, &table, workdir, "S").unwrap();
        let tester = resolve_plan("T-003", Phase::Test, &table, workdir, "S").unwrap();

        assert_eq!(coder.executor, "claude");
        assert_eq!(tester.executor, "codex");
    }

    #[test]
    fn resolve_plan_blank_task_id_errors() {
        let table = routing(&[("CODER", "claude", "m")]);
        let err = resolve_plan("  ", Phase::Implement, &table, Path::new("."), "S").unwrap_err();
        assert_eq!(err, PipelineError::BlankTaskId);
    }

    #[test]
    fn resolve_plan_unrouted_role_errors() {
        let table = routing(&[]);
        let err =
            resolve_plan("T-003", Phase::Implement, &table, Path::new("."), "S").unwrap_err();
        assert_eq!(err, PipelineError::UnroutedRole("CODER".to_string()));
    }

    #[test]
    fn resolve_plan_unknown_executor_errors() {
        let table = routing(&[("CODER", "not-a-real-tool", "m")]);
        let err =
            resolve_plan("T-003", Phase::Implement, &table, Path::new("."), "S").unwrap_err();
        assert_eq!(
            err,
            PipelineError::UnknownExecutor("not-a-real-tool".to_string())
        );
    }

    #[test]
    fn local_transport_composes_spawn_config_from_plan() {
        let plan = DispatchPlan {
            task_id: "T-003".to_string(),
            phase: Phase::Implement,
            executor: "claude".to_string(),
            model: "claude-haiku-4-5".to_string(),
            args: vec!["-p".to_string()],
            stdin_spec: "SPEC".to_string(),
        };
        let workdir = Path::new("/tmp/work");
        let cfg = LocalTransport.spawn_config(&plan, workdir, None);

        assert_eq!(cfg.executor, "claude");
        assert_eq!(cfg.actual_model, "claude-haiku-4-5");
        assert_eq!(cfg.phase, "implement");
        assert_eq!(cfg.task_id, "T-003");
        assert_eq!(cfg.spec, "SPEC");
        assert_eq!(cfg.log_dir, workdir.join(".dev").join("executor-logs"));
    }

    #[test]
    fn local_transport_run_unavailable_executor_yields_outcome_not_panic() {
        // No real CLI named this exists → dispatch records `unavailable` without
        // panicking. Proves LocalTransport genuinely routes through spawn_executor.
        let tmp = TempDir::new().unwrap();
        let plan = DispatchPlan {
            task_id: "T-003".to_string(),
            phase: Phase::Implement,
            executor: "gal-no-such-executor-xyz".to_string(),
            model: "m".to_string(),
            args: vec![],
            stdin_spec: String::new(),
        };

        let outcome = LocalTransport.run(&plan, tmp.path(), None).unwrap();
        assert_eq!(outcome.transport, "local");
        assert_eq!(outcome.executor, "gal-no-such-executor-xyz");
        assert_eq!(outcome.terminal_state, "unavailable");
    }
}
