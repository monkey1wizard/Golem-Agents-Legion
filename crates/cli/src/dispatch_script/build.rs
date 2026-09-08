//! Dispatch-block constructors: golem/intent/bound/offload blocks + the entry-point
//! `build_dispatch_block` dispatcher. Composes `super::model`.

use super::model::*;
use dispatch::routing::{resolve_worker_route, RoutingTable, WorkerRoute};
use dispatch::stage::Phase;

/// Build a golem dispatch block for the direct (non-pipeline) case. ACTION mirrors
/// gal.ps1 L950-957: when extra dispatch tokens are present they are echoed (joined);
/// otherwise the awaiting-instruction form. The pipeline-context ACTION + metadata and
/// the OFFLOAD path builders.
pub fn golem_block(golem: &str, mode: &str, dispatch_tokens: &[String]) -> DispatchBlock {
    let action = if dispatch_tokens.is_empty() {
        format!("Invoke {golem} — awaiting user instruction.")
    } else {
        dispatch_tokens.join(" ")
    };
    let mut b = DispatchBlock::new();
    b.push("ROLE", golem)
        .push("MODE", mode)
        .push("ACTION", action)
        .push("ON_COMPLETE", REPORT_TO_USER);
    b
}

/// Base `(ACTION, ON_COMPLETE)` for a subcommand intent (gal.ps1 L887-904), or `None`
/// if `intent` is not one of `init`/`research`/`deep-research`/`pipeline`.
pub fn intent_base(intent: &str) -> Option<(&'static str, &'static str)> {
    match intent {
        "init" => Some((
            "Initialize .dev/ for the target repo, then surface the manual next step.",
            "Tell the user to review .dev/project.md, then invoke /gal status for next steps.",
        )),
        "research" => Some((
            "Dispatch three research workers (WORKER_0/WORKER_1/WORKER_2) in parallel for structured investigation, then have ORCHESTRATOR compare, adjudicate, check, and document their findings.",
            "Synthesize findings and surface RESEARCH_COMPLETE to the user.",
        )),
        "deep-research" => Some((
            "Dispatch three research workers (WORKER_0/WORKER_1/WORKER_2) in parallel for multi-source investigation with cross-review, then have ORCHESTRATOR compare, adjudicate, check, and document their findings with independent reference verification.",
            "Synthesize findings, verify references, and surface RESEARCH_COMPLETE to the user.",
        )),
        "pipeline" => Some((
            "Load and follow the /gal-pipeline procedure (this selects the procedure, NOT immediate phase execution): pass the Entry-Latch preflight, then dispatch each phase with `gal dispatch-script … '#file:<prompt>'` (single-quote the token in PowerShell) to chain implement → test → audit using config.json#executorRouting for multi-vendor AI assignment.",
            "Report combined verdict: implement/test/audit status and whether the plan is READY TO FINALIZE (then /gal finalize).",
        )),
        _ => None,
    }
}

/// Build a subcommand-intent dispatch block (gal.ps1 L910-935) under the zero-config
/// routing baseline (no `config.json#executorRouting` entries). Locked test seam —
/// callers that hold a real `RoutingTable` (`build_dispatch_block`) must use
/// `intent_block_routed` instead so research/deep-research resolve worker actions
/// against the actual routing table rather than the zero-config default.
#[allow(dead_code)] // zero-config convenience wrapper; exercised directly by tests.rs,
                    // production callers go through `intent_block_routed` via `build_dispatch_block`.
pub fn intent_block(intent: &str) -> Option<DispatchBlock> {
    intent_block_routed(intent, &RoutingTable::default())
}

/// One `WORKER_<n>` field's ACTION value: `routed` names the headless CLI dispatch,
/// `subagent` (worker #0 always, or workers #1/#2 with no routed executor) instructs
/// the orchestrator to spawn an isolated native subagent — never the current session
/// researching itself.
fn worker_action(worker: u32, resolved: &WorkerRoute) -> String {
    if resolved.route.is_some() {
        format!("Run: gal research --worker {worker}.")
    } else {
        format!(
            "Spawn an isolated native subagent (worker {worker}) under plugins/gal-core/agents/golem-researcher.agent.md to research independently."
        )
    }
}

/// Routing-aware subcommand-intent block builder. For `research`/`deep-research`, adds
/// one `WORKER_0`/`WORKER_1`/`WORKER_2` field (resolved via `resolve_worker_route`
/// against `routing`) plus an `ORCHESTRATOR` compare/adjudicate/check/document step —
/// replacing the single self-directed skill-activation `ACTION`. Other intents are
/// unaffected.
pub fn intent_block_routed(intent: &str, routing: &RoutingTable) -> Option<DispatchBlock> {
    let (base_action, on_complete) = intent_base(intent)?;
    let mut b = DispatchBlock::new();
    b.push("COMMAND", intent)
        .push("ACTION", base_action.to_string());
    if matches!(intent, "research" | "deep-research") {
        for worker in 0u32..3 {
            let resolved = resolve_worker_route(worker, routing);
            b.push(
                &format!("WORKER_{worker}"),
                worker_action(worker, &resolved),
            );
        }
        b.push(
            "ORCHESTRATOR",
            "Compare, adjudicate, check, and document the findings from all three workers.",
        );
    }
    b.push("ON_COMPLETE", on_complete);
    Some(b)
}
/// `PIPELINE_CONTEXT_FILES` list (gal.ps1 L343-348) — repo `.dev/project.md` +
/// `.dev/state.md` + active plan + source plan, de-duplicated, `; `-joined. Forward
/// slashes (platform-consistent — see D-001 note); `repo_root` is forward-slash form.
fn pipeline_context_files(
    repo_root: &str,
    active_plan: Option<&str>,
    source_plan: Option<&str>,
) -> String {
    let mut files: Vec<String> = vec![
        format!("{repo_root}/.dev/project.md"),
        format!("{repo_root}/.dev/state.md"),
    ];
    for p in [active_plan, source_plan].into_iter().flatten() {
        if !p.is_empty() && !files.contains(&p.to_string()) {
            files.push(p.to_string());
        }
    }
    files.join("; ")
}

/// Push the pipeline-phase metadata fields onto `block` (port of
/// `Get-PipelineDispatchMetadata`, gal.ps1 L302-353). Empty/None fields are dropped by
/// `DispatchBlock::push` (Write-Dispatch parity). `repo_root` is forward-slash form.
pub fn push_pipeline_metadata(
    block: &mut DispatchBlock,
    phase: &str,
    context_carry_supported: bool,
    state: &StateContext,
    repo_root: &str,
) {
    let context_mode = if context_carry_supported && phase != "implement" && phase != "scaffold" {
        "delta"
    } else {
        "full"
    };
    block
        .push(
            "CONTEXT_CARRY",
            if context_carry_supported {
                "true"
            } else {
                "false"
            },
        )
        .push("PIPELINE_CONTEXT_MODE", context_mode);
    if context_mode == "full" {
        // Plan-derived fields (ACTIVE_EXECUTION_PROMPT/SOURCE_PLAN/STATUS_*/CONVENTION_HINTS)
        // are non-empty only with an active plan; in the idle case they drop out.
        let active = state.active_plan_path.as_deref();
        block.push("ACTIVE_EXECUTION_PROMPT", active.unwrap_or_default());
        block.push(
            "WORKFLOW_STATE",
            state.workflow_state.clone().unwrap_or_default(),
        );
        block.push(
            "PIPELINE_CONTEXT_FILES",
            pipeline_context_files(repo_root, active, None),
        );
    }
}

/// Build a bound (pipeline-phase) golem dispatch block (gal.ps1 L959-982). ACTION =
/// "Invoke {golem} for pipeline phase '{phase}'." when no remaining tokens.
pub fn bound_golem_block(
    golem: &str,
    phase: &str,
    task_scope: &str,
    fix_mode: bool,
    context_carry_supported: bool,
    state: &StateContext,
    repo_root: &str,
) -> DispatchBlock {
    let mut b = DispatchBlock::new();
    b.push("ROLE", golem)
        .push("MODE", "bound")
        .push(
            "ACTION",
            format!("Invoke {golem} for pipeline phase '{phase}'."),
        )
        .push("ON_COMPLETE", REPORT_TO_USER)
        .push("DISPATCH_KIND", "pipeline-phase")
        .push("PIPELINE_PHASE", phase)
        .push("TASK_SCOPE", task_scope);
    if fix_mode {
        b.push("FIX_MODE", "true");
    }
    push_pipeline_metadata(&mut b, phase, context_carry_supported, state, repo_root);
    b
}
const BYPASS_PERMISSION_WARNING: &str = "SECURITY: headless executor runs with --dangerously-skip-permissions or --allow-all. Full trust of secondary CLI filesystem access. Enable only in a trusted local environment.";

/// Resolved OFFLOAD target: the phase role plus the routed executor + model + effort.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OffloadTarget {
    pub role: String,
    pub executor: String,
    pub model: String,
    pub effort: Option<String>,
}

/// Flatten a config/invocation-derived value to one logical line: CR and LF collapse
/// to a space so an embedded newline can never forge an extra `KEY: value` field when
/// interpolated into a dispatch block or marker line.
fn single_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
}

/// Decide whether a pipeline-phase dispatch should offload, composing
/// `Phase::role` + the routing table (gal.ps1 L994-999). Returns `None` (→ regular
/// text dispatch, shell-parity fallback) when the phase is invalid or no executor is
/// routed for the role.
pub fn resolve_offload_target(phase: &str, routing: &RoutingTable) -> Option<OffloadTarget> {
    let role = Phase::from_str(phase).ok()?.role();
    let entry = routing.get(role)?;
    if entry.executor.is_empty() {
        return None;
    }
    Some(OffloadTarget {
        role: role.to_string(),
        executor: entry.executor.clone(),
        model: entry.model.clone(),
        effort: entry.effort.clone(),
    })
}

/// Build the headless OFFLOAD dispatch block (A2 — self-contained `gal pipeline`
/// with a deterministic receipt contract). `dispatch_target` is the explicit plan/prompt
/// path that `gal pipeline` materializes the per-(task,phase) spec from — it is the
/// single source of truth (the router only emits OFFLOAD when a plan token resolved);
/// `repo_root` is forward-slash form. There is no generated-spec fallback: the dispatch
/// never points at a path GAL has not guaranteed exists.
pub fn offload_block(
    target: &OffloadTarget,
    phase: &str,
    task_scope: &str,
    dispatch_target: &str,
    repo_root: &str,
    fix_mode: bool,
) -> DispatchBlock {
    let model_display = if target.model.is_empty() {
        "(default)".to_string()
    } else {
        single_line(&target.model)
    };
    let effort_display = match target.effort.as_deref() {
        Some(e) if !e.is_empty() => single_line(e),
        _ => "(default)".to_string(),
    };
    let executor_display = single_line(&target.executor);
    let task_scope_display = single_line(task_scope);
    let fix_flag = if fix_mode { " --fix" } else { "" };
    let action = format!(
        "Run: gal.exe pipeline '{dispatch_target}' --phase {phase} --task {task_scope_display} --workdir '{repo_root}'{fix_flag}. \
The single gal binary reads routing, assembles and dispatches the executor in-process with no sibling dispatch executable, and emits a Dispatch: marker line. \
For test/audit/implement it auto-resolves a deterministic receipt (.dev/pipeline/receipts/<plan-scope-key>/{task_scope_display}-{phase}.receipt.md), embeds it in the spec, invalidates managed stale evidence before spawn, and accepts only a non-empty receipt written by the current run; pass --receipt only to select another path, where a pre-existing external path fails closed. \
Exit 0 -> completed (write-back verified). Exit 1 -> no-receipt or disconnected-partial. Exit 2 -> unavailable/degraded. \
Fall back to role-playing the {role} golem only on exit 2.",
        role = target.role,
        fix_flag = fix_flag,
    );
    let on_complete = format!(
        "Record in the execution prompt the Dispatch: marker line emitted by gal pipeline \
(phase={phase} task={task_scope_display} role={role} executor={executor_display} model={model_display} state=<state> session_id=<id> log=<path>).",
        role = target.role,
    );
    let phase_display = if fix_mode {
        format!("{phase} (fix)")
    } else {
        phase.to_string()
    };
    let report_line = format!(
        "Dispatched: {phase_display} {task_scope_display} - {role} as {executor_display}, model {model_display}, effort {effort_display}",
        role = target.role,
    );
    let mut b = DispatchBlock::new();
    b.push("COMMAND", "offload")
        .push("OFFLOAD", "headless-executor")
        .push("DISPATCH_MODE", "offload")
        .push("EXECUTOR", &executor_display)
        .push("MODEL", model_display)
        .push("EFFORT", effort_display)
        .push("ROLE", &target.role)
        .push("PIPELINE_PHASE", phase)
        .push("TASK_SCOPE", &task_scope_display)
        .push("TASK_SPEC", dispatch_target)
        .push("ACTION", action)
        .push("ON_COMPLETE", on_complete)
        .push("REPORT_LINE", report_line)
        .push("BYPASS_PERMISSION_WARNING", BYPASS_PERMISSION_WARNING);
    b
}

/// Build an error dispatch block (`COMMAND: error` + `ACTION`).
pub fn error_block(action: &str) -> DispatchBlock {
    let mut b = DispatchBlock::new();
    b.push("COMMAND", "error").push("ACTION", action);
    b
}

/// The entry-point dispatcher: route `intent`/tokens to the right block, composing all
/// the block builders (port of the `dispatch` branch, gal.ps1 L858-1054 — the
/// live block kinds; the dead auto-detect suggest path is intentionally omitted).
///
/// `routing` + `state` drive the OFFLOAD decision: OFFLOAD fires only with a routed
/// executor for the phase role AND an active plan to assemble the spec from; otherwise
/// the bound block is emitted (shell-parity fallback). In the idle repo state (no active
/// plan), bound is the live result.
pub fn build_dispatch_block(
    intent: &str,
    tokens: &[String],
    routing: &RoutingTable,
    state: &StateContext,
    repo_root: &str,
) -> DispatchBlock {
    let pipeline = parse_pipeline_context(tokens);
    let finalize_branch_audit = tokens.iter().any(|t| t == "--finalize-branch-audit");

    if let Some(err) = &pipeline.error {
        return error_block(err);
    }

    // Subcommand intents.
    if intent_base(intent).is_some() {
        let mut b = intent_block_routed(intent, routing).expect("intent_base matched");
        if let Some(f) = &pipeline.from {
            b.push("FROM", f);
        }
        if let Some(s) = &pipeline.stop_at {
            b.push("STOP_AT", s);
        }
        return b;
    }

    // Golem dispatch — also handles `/gal discuss <role>` (in-context consult dual-mode,
    // four roles only: architect, analyst, designer, releaser).
    if let Some((golem, in_context)) = parse_discuss_context(intent, &pipeline.remaining) {
        // Discuss gate: only the four consult-dual-mode roles support in-context invocation.
        // All other roles fail closed with a role-specific recovery ACTION.
        if in_context && !is_discuss_capable(&golem) {
            return error_block(&discuss_recovery_action(&golem));
        }
        let mode = golem_mode(&golem, pipeline.requested, finalize_branch_audit);
        if mode == "orchestrated-only" {
            return error_block(&format!(
                "'{golem}' is an orchestrated-only role and cannot be invoked directly. \
                 Use /gal pipeline (implementer/tester/auditor) or /gal research (researcher) instead."
            ));
        }
        if finalize_branch_audit && golem == "golem-auditor" {
            return error_block(
                "Whole-branch audit is not dispatchable through dispatch-script. \
                 Finalize runs its top-down review in the same runtime, marks \
                 DEGRADED_SAME_RUNTIME, and writes the Requirement by L1-L4 table.",
            );
        }
        if mode == "bound" {
            let phase = pipeline.phase.as_deref().unwrap_or("implement");
            let task_scope = pipeline.task_scope.as_deref().unwrap_or("");
            // OFFLOAD when a routed executor exists for the phase role. Pipeline context is
            // proven by mode==bound + a non-empty task_scope; the active-plan signal is NOT
            // re-derived from `.dev/state.md` (that narrow header-table parser made OFFLOAD
            // dormant whenever state.md used a bullet list, degrading every phase to
            // in-process role-play). The real safety gate (routing + is_available +
            // executor_readiness) lives downstream in `dispatch::run::run_dispatch`, so
            // widening this emit-gate does not weaken safety.
            if !task_scope.is_empty() {
                if let Some(target) = resolve_offload_target(phase, routing) {
                    // A2: the dispatch target must be a plan/prompt path `gal pipeline`
                    // can materialize. The pipeline phase dispatch commands thread it
                    // explicitly (SKILL); we never fall back to a generated-spec path
                    // GAL has not written. No plan token resolved → error, not a phantom.
                    match pipeline.plan_path.as_deref() {
                        Some(plan_path) => {
                            return offload_block(
                                &target,
                                phase,
                                task_scope,
                                plan_path,
                                repo_root,
                                pipeline.fix_mode,
                            );
                        }
                        None => {
                            return error_block(
                                "OFFLOAD requires a resolvable plan/prompt path: pass the execution prompt via `'#file:<prompt>'` (single-quote it in PowerShell — a bare `#` starts a comment there and strips the token before gal sees it) or `@<prompt>` so `gal pipeline` can materialize the task spec. Refusing to emit a dispatch pointing at an unmaterialized generated-spec path.",
                            );
                        }
                    }
                }
            }
            return bound_golem_block(
                &golem,
                phase,
                task_scope,
                pipeline.fix_mode,
                true,
                state,
                repo_root,
            );
        }
        // direct
        let mut b = golem_block(&golem, mode, tokens);
        if in_context {
            b.push("CONSULT_MODE", "in-context");
        }
        return b;
    }

    // Unknown intent.
    error_block(&format!(
        "Unknown argument: '{intent}'. Use a subcommand (init/research/deep-research/pipeline) or a golem name."
    ))
}
