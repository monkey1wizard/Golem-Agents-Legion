//! Guarded Codex pipeline entry and preflight latch.

use gal_engine::ExitCode;
use pipeline::coordinator::{
    AttemptOwnership, CheckpointKind, CheckpointReceipt, ContinuationProfile, CoordinatorState,
    ExecutionBinding, NextAction, OwnerObservation, ResumeHostSubagent,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const MAX_RECORD: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContinuationAction {
    Entry,
    Status,
    SubmitCheckpointReceipt,
    Resume,
    ResumeHost,
}

/// Reuse the guarded-entry parser to identify only a new Entry action.
pub(crate) fn fresh_entry_prompt(args: &[String]) -> Option<PathBuf> {
    let parsed = parse(args).ok()?;
    (parsed.action == ContinuationAction::Entry
        && !parsed.phase_supplied
        && parsed.task.is_none()
        && super::dispatch::resolve_pipeline_input(&parsed.prompt)
            == super::dispatch::PipelineInput::Prompt
        && !super::pipeline_preflight::preflight_pin_exists(&parsed.prompt))
    .then_some(parsed.prompt)
}

/// Hold entry custody while checking durable coordinator state and recovering
/// the linked worktree. This runs before coordinator creation and executable binding.
pub(crate) fn recover_entry_worktree(prompt: &Path, worktree: &Path) -> Result<bool, String> {
    let worktree = fs::canonicalize(worktree)
        .map_err(|error| format!("worktree unavailable for recovery: {error}"))?;
    // Recovery is only for linked Git worktrees. Ordinary repositories and
    // non-Git directories retain the established entry behavior.
    let git_path = |argument: &str| -> Option<PathBuf> {
        let output = std::process::Command::new("git")
            .current_dir(&worktree)
            .args(["rev-parse", argument])
            .output()
            .ok()?;
        output.status.success().then(|| {
            let path = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            fs::canonicalize(if path.is_absolute() {
                path
            } else {
                worktree.join(path)
            })
            .ok()
        })?
    };
    let (Some(git_dir), Some(common_dir)) =
        (git_path("--absolute-git-dir"), git_path("--git-common-dir"))
    else {
        return Ok(false);
    };
    if git_dir == common_dir {
        return Ok(false);
    }
    let prompt = if prompt.exists() {
        fs::canonicalize(prompt)
            .map_err(|error| format!("entry prompt unavailable for recovery: {error}"))?
    } else if prompt.is_absolute() {
        prompt.to_path_buf()
    } else {
        worktree.join(prompt)
    };
    let prompt = if prompt.exists() {
        fs::canonicalize(&prompt)
            .map_err(|error| format!("entry prompt unavailable for recovery: {error}"))?
    } else {
        let mut ancestor = prompt.as_path();
        let mut suffix = Vec::new();
        while !ancestor.exists() {
            suffix.push(
                ancestor
                    .file_name()
                    .ok_or("entry prompt has no existing parent")?
                    .to_os_string(),
            );
            ancestor = ancestor
                .parent()
                .ok_or("entry prompt has no existing parent")?;
        }
        let canonical_parent = fs::canonicalize(ancestor)
            .map_err(|error| format!("cannot resolve entry prompt parent: {error}"))?;
        if !canonical_parent.starts_with(&worktree) {
            return Err("entry prompt parent resolves outside the linked worktree".into());
        }
        suffix
            .into_iter()
            .rev()
            .fold(canonical_parent, |path, part| path.join(part))
    };
    if !prompt.starts_with(&worktree)
        || prompt
            .components()
            .any(|part| part == std::path::Component::ParentDir)
    {
        return Err("entry prompt is outside the linked worktree".into());
    }
    let scope = safe_scope(&prompt)?;
    let root = worktree.join(".dev/pipeline").join(scope);
    let journal = root.join("projection-journal.json");
    if root.exists() && journal.exists() {
        return Err("pending projection journal exists; refusing worktree recovery".into());
    }
    if root.join(".coordinator-revision-lock").exists() {
        return Err("retained coordinator lock exists; refusing worktree recovery".into());
    }
    if root.join("coordinator.json").is_file() {
        let state: CoordinatorState = read_bounded(&root.join("coordinator.json"))
            .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
            .map_err(|error| {
                format!("coordinator unavailable; refusing worktree recovery: {error}")
            })?;
        if state.active_attempt.is_some() {
            return Ok(false);
        }
        if state
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.consumed_receipt_id.is_none())
        {
            return Ok(false);
        }
    }
    fs::create_dir_all(&root).map_err(|error| format!("cannot prepare entry custody: {error}"))?;
    let lock = EntryLock::acquire(root.join(".coordinator-revision-lock"))
        .map_err(|_| "retained coordinator lock exists; refusing worktree recovery".to_string())?;
    let result = super::format_repair::recover_primary_tip(&worktree, &prompt);
    drop(lock);
    result
}

#[derive(Debug)]
struct EntryArgs {
    prompt: PathBuf,
    phase: String,
    task: Option<String>,
    action: ContinuationAction,
    host_resume: Option<ResumeHostSubagent>,
    phase_supplied: bool,
}

fn parse(args: &[String]) -> Result<EntryArgs, String> {
    let mut prompt = None;
    let mut phase = "implement".to_string();
    let mut phase_supplied = false;
    let mut task = None;
    let mut action = None;
    let mut host_resume = None;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--require-codex-stop-v1" => {}
            "--status" | "--submit-checkpoint-receipt" | "--resume" => {
                if action.is_some() {
                    return Err("ambiguous guarded continuation action".into());
                }
                action = Some(match args[i].as_str() {
                    "--status" => ContinuationAction::Status,
                    "--submit-checkpoint-receipt" => ContinuationAction::SubmitCheckpointReceipt,
                    _ => ContinuationAction::Resume,
                });
            }
            "--phase" | "--task" => {
                let key = args[i].as_str();
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| format!("{key} requires a value"))?
                    .clone();
                if key == "--phase" {
                    phase = value;
                    phase_supplied = true;
                } else {
                    task = Some(value);
                }
            }
            v if v.starts_with('{') => {
                if action.is_some() {
                    return Err("ambiguous guarded continuation action".into());
                }
                let value: Value = serde_json::from_str(v)
                    .map_err(|e| format!("invalid guarded continuation action: {e}"))?;
                let kind = value
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or("guarded continuation action requires a kind")?;
                let binding = value
                    .get("binding")
                    .and_then(Value::as_object)
                    .ok_or("guarded continuation action requires a binding object")?;
                match kind {
                    "start_task" | "resume_task" | "retry_task" => {
                        task = binding
                            .get("task_id")
                            .and_then(Value::as_str)
                            .filter(|v| !v.is_empty())
                            .map(str::to_string);
                        phase = binding
                            .get("phase")
                            .and_then(Value::as_str)
                            .filter(|v| !v.is_empty())
                            .ok_or("guarded task action requires task and phase")?
                            .to_string();
                        phase_supplied = true;
                        if task.is_none() {
                            return Err("guarded task action requires task and phase".into());
                        }
                        action = Some(ContinuationAction::Resume);
                    }
                    "resume_host_subagent" => {
                        host_resume = Some(
                            serde_json::from_value(Value::Object(binding.clone()))
                                .map_err(|e| format!("invalid host resume binding: {e}"))?,
                        );
                        action = Some(ContinuationAction::ResumeHost);
                    }
                    "await_orchestrator" => {
                        if binding
                            .get("checkpoint_id")
                            .and_then(Value::as_str)
                            .is_none_or(str::is_empty)
                        {
                            return Err("guarded checkpoint action requires checkpoint id".into());
                        }
                        action = Some(ContinuationAction::SubmitCheckpointReceipt);
                    }
                    "complete" | "blocked" => action = Some(ContinuationAction::Status),
                    "recover_attempt" => {
                        return Err("recover-attempt requires a task and phase".into());
                    }
                    _ => return Err(format!("unknown guarded continuation kind '{kind}'")),
                }
            }
            v if v.starts_with("--") => return Err(format!("unknown guarded option '{v}'")),
            v => {
                if prompt.replace(PathBuf::from(v)).is_some() {
                    return Err("unexpected extra prompt path".into());
                }
            }
        }
        i += 1;
    }
    let prompt = prompt.ok_or("guarded pipeline requires an execution prompt path")?;
    if !matches!(
        phase.as_str(),
        "implement" | "test" | "audit" | "fix" | "converge"
    ) {
        return Err(format!("unsupported guarded phase '{phase}'"));
    }
    let action = action.unwrap_or(ContinuationAction::Entry);
    if action == ContinuationAction::Resume && (task.is_none() || !phase_supplied) {
        return Err("guarded resume requires --task and --phase".into());
    }
    Ok(EntryArgs {
        prompt,
        phase,
        task,
        action,
        host_resume,
        phase_supplied,
    })
}

fn continuation(
    args: &EntryArgs,
    root: &Path,
    repair: &mut Option<super::format_repair::RepairInvocation>,
) -> Option<ExitCode> {
    if args.action == ContinuationAction::Entry {
        return None;
    }
    let path = root.join("coordinator.json");
    let mut state: CoordinatorState = match read_bounded(&path)
        .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    {
        Ok(value) => value,
        Err(error) => return Some(fail(format!("coordinator unavailable: {error}"))),
    };
    if state.profile != ContinuationProfile::CodexStopV1
        || state.plan_scope
            != root
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
    {
        return Some(fail("coordinator does not match guarded plan root"));
    }
    match args.action {
        ContinuationAction::Status => {
            println!("{}", state.bounded_status());
            Some(ExitCode::Success)
        }
        ContinuationAction::SubmitCheckpointReceipt => {
            let receipt_path = root.join("orchestrator-receipt.json");
            if !receipt_path.is_file() {
                return Some(fail("checkpoint receipt missing or invalid"));
            }
            let lock_path = root.join(".coordinator-revision-lock");
            let _lock = match EntryLock::acquire(lock_path) {
                Ok(lock) => lock,
                Err(error) => {
                    return Some(fail(format!(
                        "coordinator revision lock unavailable: {error}"
                    )));
                }
            };
            let journal = root.join("projection-journal.json");
            if journal.exists() {
                if let Err(error) = pipeline::projection_journal::recover(&journal) {
                    return Some(fail(format!(
                        "pending projection journal recovery failed: {error}"
                    )));
                }
            }
            state = match read_bounded(&path)
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
            {
                Ok(value) => value,
                Err(error) => {
                    return Some(fail(format!(
                        "coordinator unavailable after recovery: {error}"
                    )));
                }
            };
            let receipt_bytes = match fs::read(&receipt_path)
                .ok()
                .filter(|b| b.len() as u64 <= MAX_RECORD)
            {
                Some(bytes) => bytes,
                None => return Some(fail("checkpoint receipt missing or invalid")),
            };
            let receipt: CheckpointReceipt = match serde_json::from_slice(&receipt_bytes) {
                Ok(value) => value,
                Err(_) => return Some(fail("checkpoint receipt missing or invalid")),
            };
            let head = std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output();
            let head = match head {
                Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_owned(),
                _ => return Some(fail("checkpoint receipt HEAD unavailable")),
            };
            let outcome = match state.checkpoint.as_ref().map(|c| c.kind) {
                Some(CheckpointKind::TaskQuality) => {
                    submit_task_quality(&mut state, root, receipt, &head)
                }
                Some(CheckpointKind::BoundaryWidening) => {
                    submit_boundary_widening(&mut state, root, receipt, &head)
                }
                Some(CheckpointKind::GoalBackward) => Err(
                    "goal-backward checkpoint receipts are consumed by pipeline-handback-check"
                        .into(),
                ),
                _ => Err("checkpoint receipt task-quality or commit binding mismatch".into()),
            };
            if let Err(error) = outcome {
                return Some(fail(error));
            }
            if let Err(error) = atomic_write(
                &path,
                &serde_json::to_vec_pretty(&state).unwrap_or_default(),
            ) {
                return Some(fail(error.to_string()));
            }
            if let Err(error) = fs::remove_file(receipt_path) {
                return Some(fail(format!("checkpoint receipt removal failed: {error}")));
            }
            println!("{}", state.bounded_status());
            Some(ExitCode::Success)
        }
        ContinuationAction::ResumeHost => Some(
            match resume_host(root, args.host_resume.clone().unwrap(), repair) {
                Ok(()) => ExitCode::Success,
                Err(error) => fail(error),
            },
        ),
        ContinuationAction::Resume => {
            if resume_phase_handler(&args.phase).is_none() {
                return Some(fail(format!(
                    "unhandled guarded resume phase '{}'",
                    args.phase
                )));
            }
            if state.active_attempt.is_some() {
                return Some(fail("active attempt exists; refusing provider start"));
            }
            if state
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.consumed_receipt_id.is_none())
            {
                return Some(fail("unconsumed checkpoint exists; refusing resume"));
            }
            let journal = root.join("projection-journal.json");
            if journal.exists() && pipeline::projection_journal::recover(&journal).is_err() {
                return Some(fail("pending projection journal recovery failed"));
            }
            let prompt = super::pipeline_execution::current_worktree_root()
                .ok()
                .map(|cwd| {
                    cwd.join(".dev/plans")
                        .join(format!("{}.prompt.md", state.plan_scope))
                });
            let prompt = match prompt.and_then(|p| fs::canonicalize(p).ok()) {
                Some(p) => p,
                None => return Some(fail("resume entry-latch prompt unavailable")),
            };
            let segment = digest(
                format!(
                    "resume:{}:{}",
                    state.revision,
                    SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                )
                .as_bytes(),
            );
            let receipt_path = root.join(format!("resume-preflight-{segment}.receipt.md"));
            let preflight = super::pipeline_preflight::cmd_pipeline_preflight(&[
                "pipeline-preflight".into(),
                prompt.to_string_lossy().into_owned(),
                "--receipt".into(),
                receipt_path.to_string_lossy().into_owned(),
            ]);
            let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
            if preflight != ExitCode::Success
                || !receipt.lines().any(|line| line.trim() == "overall: pass")
            {
                return Some(fail("resume entry latch did not pass"));
            }
            if let Ok(pin) = super::pipeline_preflight::read_pin(&prompt) {
                if pin.dispatch_mode == super::pipeline_preflight::DispatchMode::HostSubagent {
                    state.pinned_mode = Some("host-subagent".into());
                    state.pinned_host = pin.host;
                }
            }
            let task = args.task.as_deref().unwrap_or_default();
            state.task_id = task.to_string();
            state.phase = args.phase.clone();
            state.next_action = NextAction::ResumeTask {
                task_id: task.to_string(),
                phase: args.phase.clone(),
            };
            state.revision = state.revision.saturating_add(1);
            if let Err(error) = atomic_write(
                &path,
                &serde_json::to_vec_pretty(&state).unwrap_or_default(),
            ) {
                return Some(fail(error.to_string()));
            }
            if matches!(args.phase.as_str(), "implement" | "fix") {
                let task = args.task.as_deref().unwrap_or_default();
                return Some(implement_segment(
                    &mut state,
                    &path,
                    root,
                    &prompt,
                    task,
                    args.phase == "fix",
                    repair,
                ));
            }
            if args.phase == "converge" {
                let task = args.task.as_deref().unwrap_or_default();
                let prompt_arg = format!(".dev/plans/{}.prompt.md", state.plan_scope);
                let converge_receipt = root.join(format!("{task}-converge.receipt.md"));
                let converge_args = vec![
                    "pipeline-converge-check".into(),
                    prompt_arg.clone(),
                    "--task".into(),
                    task.to_string(),
                    "--receipt".into(),
                    converge_receipt.to_string_lossy().into_owned(),
                ];
                let convergence =
                    super::converge_check::cmd_pipeline_converge_check(&converge_args);
                if convergence != ExitCode::Success {
                    println!("{}", state.bounded_status());
                    return Some(convergence);
                }
                let boundary_receipt = root.join(format!("{task}-state-boundary.receipt.md"));
                let boundary_args = vec![
                    "boundary-check".into(),
                    prompt_arg,
                    "--task".into(),
                    task.to_string(),
                    "--boundary-kind".into(),
                    "state-recording".into(),
                    "--receipt".into(),
                    boundary_receipt.to_string_lossy().into_owned(),
                ];
                let boundary = super::boundary_check::cmd_boundary_check(&boundary_args);
                if boundary != ExitCode::Success {
                    println!("{}", state.bounded_status());
                    return Some(boundary);
                }
                let prompt_text = match fs::read_to_string(&prompt) {
                    Ok(text) => text,
                    Err(error) => {
                        return Some(fail(format!("converged prompt unavailable: {error}")));
                    }
                };
                let slug = match prompt
                    .file_name()
                    .and_then(|name| name.to_str())
                    .and_then(|name| name.strip_suffix(".prompt.md"))
                {
                    Some(slug) => slug,
                    None => return Some(fail("converged prompt has invalid name")),
                };
                let repo = match super::pipeline_execution::current_worktree_root() {
                    Ok(repo) => repo,
                    Err(error) => {
                        return Some(fail(format!("repository path unavailable: {error}")));
                    }
                };
                let source = repo.join(".dev/plans").join(format!("{slug}.md"));
                let state_surface = repo.join(".dev/state.md");
                let source_bytes = match fs::read(&source) {
                    Ok(bytes) => bytes,
                    Err(error) => return Some(fail(format!("source plan unavailable: {error}"))),
                };
                let state_bytes = match fs::read(&state_surface) {
                    Ok(bytes) => bytes,
                    Err(error) => return Some(fail(format!("state surface unavailable: {error}"))),
                };
                let prompt_bytes = prompt_text.as_bytes().to_vec();
                let remaining_tasks =
                    super::finalize_check::task_checkbox_projection(&prompt_text).unchecked;
                let journal = root.join("projection-journal.json");
                let projections = [
                    pipeline::projection_journal::Projection {
                        path: prompt.clone(),
                        bytes: prompt_bytes,
                    },
                    pipeline::projection_journal::Projection {
                        path: source,
                        bytes: source_bytes,
                    },
                    pipeline::projection_journal::Projection {
                        path: state_surface,
                        bytes: state_bytes,
                    },
                ];
                if let Err(error) = pipeline::projection_journal::apply(&journal, projections) {
                    return Some(fail(format!(
                        "verified progress projection failed: {error}"
                    )));
                }
                state.last_verified_evidence = Some(format!(
                    "converge-check:pass;boundary-state-recording:pass;task:{task}"
                ));
                if remaining_tasks.is_empty() {
                    if state
                        .checkpoint(
                            format!("goal-backward-{task}"),
                            CheckpointKind::GoalBackward,
                            None,
                        )
                        .is_err()
                    {
                        return Some(fail("goal-backward checkpoint failed"));
                    }
                    let checkpoint_id = state
                        .checkpoint
                        .as_ref()
                        .map(|c| c.checkpoint_id.clone())
                        .unwrap_or_default();
                    state.next_action = NextAction::AwaitOrchestrator { checkpoint_id };
                } else {
                    state.revision = state.revision.saturating_add(1);
                }
                if let Err(error) = atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&state).unwrap_or_default(),
                ) {
                    return Some(fail(format!(
                        "convergence evidence persistence failed: {error}"
                    )));
                }
                println!("{}", state.bounded_status());
                return Some(ExitCode::Success);
            }
            if matches!(args.phase.as_str(), "test" | "audit") {
                let task = args.task.as_deref().unwrap_or_default();
                if retry_ceiling_blocks(&mut state) {
                    if let Err(error) = atomic_write(
                        &path,
                        &serde_json::to_vec_pretty(&state).unwrap_or_default(),
                    ) {
                        return Some(fail(format!("retry ceiling persistence failed: {error}")));
                    }
                    println!("{}", state.bounded_status());
                    return Some(fail(
                        "retry limit exhausted; refusing further provider starts",
                    ));
                }
                let phases: Vec<&str> = if args.phase == "test" {
                    vec!["test", "audit"]
                } else {
                    vec!["audit"]
                };
                for phase in phases {
                    let (result, verdict) = match guarded_phase_dispatch(
                        &mut state, &path, &prompt, task, phase, repair,
                    ) {
                        Ok(outcome) => outcome,
                        Err(error) => return Some(fail(error)),
                    };
                    if state.has_pending_host_checkpoint() {
                        return Some(result);
                    }
                    if let Err(error) = sync_retry_handoff(&state, &prompt, task, phase, verdict) {
                        return Some(fail(error));
                    }
                    if matches!(
                        state.next_action,
                        NextAction::Blocked { .. } | NextAction::RetryTask { .. }
                    ) {
                        if let NextAction::RetryTask {
                            task_id, attempt, ..
                        } = state.next_action.clone()
                        {
                            state.next_action = NextAction::RetryTask {
                                task_id,
                                phase: "fix".into(),
                                attempt,
                            };
                            if let Err(error) = atomic_write(
                                &path,
                                &serde_json::to_vec_pretty(&state).unwrap_or_default(),
                            ) {
                                return Some(fail(format!(
                                    "retry action persistence failed: {error}"
                                )));
                            }
                        }
                        println!("{}", state.bounded_status());
                        return Some(if result == ExitCode::Success {
                            ExitCode::Success
                        } else {
                            result
                        });
                    }
                }
                state.phase = "converge".into();
                state.next_action = NextAction::StartTask {
                    task_id: task.to_string(),
                    phase: "converge".into(),
                };
                if let Err(error) = atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&state).unwrap_or_default(),
                ) {
                    return Some(fail(format!(
                        "convergence action persistence failed: {error}"
                    )));
                }
                println!("{}", state.bounded_status());
                return Some(ExitCode::Success);
            }
            Some(fail(format!(
                "unhandled guarded resume phase '{}'",
                args.phase
            )))
        }
        ContinuationAction::Entry => None,
    }
}

/// Validate a task-quality receipt against its prompt and convention, then
/// consume it. The receipt must bind to HEAD.
fn submit_task_quality(
    state: &mut CoordinatorState,
    root: &Path,
    receipt: CheckpointReceipt,
    head: &str,
) -> Result<(), String> {
    let prompt_path = root
        .parent()
        .and_then(|_| super::pipeline_execution::current_worktree_root().ok())
        .and_then(|cwd| {
            fs::canonicalize(
                cwd.join(".dev/plans")
                    .join(format!("{}.prompt.md", state.plan_scope)),
            )
            .ok()
        });
    let prompt_text = match prompt_path
        .as_ref()
        .and_then(|p| fs::read_to_string(p).ok())
    {
        Some(text) => text,
        None => return Err("task-quality receipt prompt unavailable".into()),
    };
    let convention = Path::new("plugins/gal-core/conventions/task-quality.md");
    if !convention.is_file()
        || pipeline::task_spec::extract_plan_task_goal(&prompt_text, &state.task_id).is_err()
    {
        return Err("task-quality receipt validation failed".into());
    }
    let spec =
        match pipeline::task_spec::assemble_plan_task_spec(&pipeline::task_spec::TaskSpecInput {
            task_scope: &state.task_id,
            phase: &state.phase,
            prompt_path: prompt_path.as_ref().unwrap().to_string_lossy().as_ref(),
            prompt_body: &prompt_text,
            generated: "",
            git_branch: "",
            git_head: "",
            convention_hints: Some("plugins/gal-core/conventions/task-quality.md"),
            receipt_path: None,
            agent_contract_body: None,
            fix_mode: false,
        }) {
            Ok(spec) => spec,
            Err(error) => return Err(format!("task-quality task spec invalid: {error}")),
        };
    if receipt.commit.as_deref() != Some(head) || spec.markdown.is_empty() {
        return Err("checkpoint receipt task-quality or commit binding mismatch".into());
    }
    state
        .consume_checkpoint_receipt(receipt)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Consume a bound boundary-widening receipt, then recheck the task boundary
/// with the implement branch's arguments. A passing recheck moves the task to
/// `test`. A rejected receipt blocks with `boundary-violation`.
fn submit_boundary_widening(
    state: &mut CoordinatorState,
    root: &Path,
    receipt: CheckpointReceipt,
    head: &str,
) -> Result<(), String> {
    let worktree = super::pipeline_execution::current_worktree_root()
        .map_err(|error| format!("repository path unavailable: {error}"))?;
    let bound = state
        .checkpoint
        .as_ref()
        .and_then(|checkpoint| checkpoint.commit.as_deref())
        .is_some_and(|commit| commit == head || is_ancestor(&worktree, commit, head));
    if !bound {
        return Err("boundary-widening checkpoint commit binding mismatch".into());
    }
    let accepted = receipt.accepted;
    state
        .consume_checkpoint_receipt(receipt)
        .map_err(|error| error.to_string())?;
    if !accepted {
        state.next_action = NextAction::Blocked {
            reason: "boundary-violation".into(),
        };
        return Ok(());
    }
    let prompt = fs::canonicalize(
        worktree
            .join(".dev/plans")
            .join(format!("{}.prompt.md", state.plan_scope)),
    )
    .map_err(|error| format!("boundary-widening prompt unavailable: {error}"))?;
    let recheck_receipt = root.join(format!(
        "boundary-widening-recheck-{}.receipt.md",
        state.revision
    ));
    let boundary_args = vec![
        "boundary-check".into(),
        prompt.to_string_lossy().into_owned(),
        "--task".into(),
        state.task_id.clone(),
        "--receipt".into(),
        recheck_receipt.to_string_lossy().into_owned(),
    ];
    let boundary = super::boundary_check::cmd_boundary_check(&boundary_args);
    let boundary_text = fs::read_to_string(&recheck_receipt).unwrap_or_default();
    if boundary == ExitCode::Success && boundary_text.contains("pass") {
        state.phase = "test".into();
        state.next_action = NextAction::StartTask {
            task_id: state.task_id.clone(),
            phase: "test".into(),
        };
    } else {
        state.next_action = NextAction::Blocked {
            reason: "boundary-widening-recheck-failed".into(),
        };
    }
    Ok(())
}

/// Report whether `commit` is an ancestor of `head` in the worktree.
fn is_ancestor(worktree: &Path, commit: &str, head: &str) -> bool {
    std::process::Command::new("git")
        .current_dir(worktree)
        .args(["merge-base", "--is-ancestor", commit, head])
        .status()
        .is_ok_and(|status| status.success())
}

/// Apply the retry floor and report whether the retry ceiling blocks a provider
/// start. A blocked result sets `Blocked { reason: "retry-limit-exhausted" }`
/// but leaves persistence to the caller.
fn retry_ceiling_blocks(state: &mut CoordinatorState) -> bool {
    state.retry.max_attempts = state.retry.max_attempts.max(3);
    if state.retry.max_attempts > 0 && state.retry.attempt >= state.retry.max_attempts {
        state.next_action = NextAction::Blocked {
            reason: "retry-limit-exhausted".into(),
        };
        return true;
    }
    false
}

/// Run one implement segment for the guarded `implement` phase or, with
/// `fix_mode`, the guarded `fix` phase. Both share the claim, dispatch, and
/// boundary check. `fix_mode` first applies the retry ceiling, appends `--fix`
/// to the dispatch, and records a retry when the dispatch fails. That failure
/// skips the boundary check.
fn implement_segment(
    state: &mut CoordinatorState,
    path: &Path,
    root: &Path,
    prompt: &Path,
    task: &str,
    fix_mode: bool,
    repair: &mut Option<super::format_repair::RepairInvocation>,
) -> ExitCode {
    if fix_mode && retry_ceiling_blocks(state) {
        if let Err(error) =
            atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default())
        {
            return fail(format!("retry ceiling persistence failed: {error}"));
        }
        println!("{}", state.bounded_status());
        return fail("retry limit exhausted; refusing further provider starts");
    }
    let attempt_id = digest(
        format!(
            "{}:{}:{}",
            task,
            state.revision,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
        .as_bytes(),
    );
    if state.claim_attempt(attempt_id.clone()).is_err() {
        return fail("implement attempt claim failed");
    }
    if let Err(error) = atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default()) {
        return fail(format!("implement claim persistence failed: {error}"));
    }
    if state
        .bind_process(
            std::process::id(),
            dispatch::dispatch::process_birth_identity(std::process::id()),
        )
        .is_err()
    {
        return fail("implement attempt bind failed");
    }
    if let Err(error) = atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default()) {
        return fail(format!("implement bind persistence failed: {error}"));
    }
    let mut dispatch_args = vec![
        "pipeline".into(),
        prompt.to_string_lossy().into_owned(),
        "--phase".into(),
        "implement".into(),
        "--task".into(),
        task.to_string(),
    ];
    if state.pinned_mode.as_deref() == Some("host-subagent") {
        dispatch_args.extend(["--dispatch".into(), "host-subagent".into()]);
    }
    if fix_mode {
        dispatch_args.push("--fix".into());
    }
    let dispatched = super::dispatch::cmd_pipeline_inner(&dispatch_args, repair);
    let published: CoordinatorState = match serde_json::from_value(match read_bounded(path) {
        Ok(v) => v,
        Err(e) => return fail(e),
    }) {
        Ok(published) => published,
        Err(error) => {
            return fail(format!("published coordinator invalid: {error}"));
        }
    };
    if published.has_pending_host_checkpoint() {
        return dispatched;
    }
    if state.pinned_mode.as_deref() == Some("host-subagent") {
        return fail("host dispatch did not publish a checkpoint");
    }
    let label = if fix_mode {
        "implement-fix-dispatch"
    } else {
        "implement-dispatch"
    };
    if fix_mode && dispatched != ExitCode::Success {
        if state
            .finish_attempt(
                &attempt_id,
                Some(format!("{label}:{dispatched:?};boundary:skipped")),
            )
            .is_err()
        {
            return fail("implement attempt finish failed");
        }
        state.record_retry("fix-dispatch-failed".into());
        if let Err(error) =
            atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default())
        {
            return fail(format!("fix dispatch persistence failed: {error}"));
        }
        println!("{}", state.bounded_status());
        return dispatched;
    }
    let boundary_receipt = root.join(format!("{attempt_id}-boundary.receipt.md"));
    let boundary_args = vec![
        "boundary-check".into(),
        prompt.to_string_lossy().into_owned(),
        "--task".into(),
        task.to_string(),
        "--receipt".into(),
        boundary_receipt.to_string_lossy().into_owned(),
    ];
    let boundary = super::boundary_check::cmd_boundary_check(&boundary_args);
    let boundary_text = fs::read_to_string(&boundary_receipt).unwrap_or_default();
    let passed = boundary == ExitCode::Success && boundary_text.contains("pass");
    if state
        .finish_attempt(
            &attempt_id,
            Some(format!(
                "{label}:{dispatched:?};boundary:{}",
                if passed { "pass" } else { "fail" }
            )),
        )
        .is_err()
    {
        return fail("implement attempt finish failed");
    }
    if passed {
        state.phase = "test".into();
        state.next_action = NextAction::StartTask {
            task_id: task.to_string(),
            phase: "test".into(),
        };
    } else {
        let head = match super::pipeline_execution::current_worktree_root()
            .map_err(|error| format!("repository path unavailable: {error}"))
            .and_then(|worktree| super::dispatch::host_head(&worktree))
        {
            Ok(head) => head,
            Err(error) => return fail(error),
        };
        if state
            .checkpoint(
                format!("boundary-widening-{attempt_id}"),
                CheckpointKind::BoundaryWidening,
                Some(head),
            )
            .is_err()
        {
            return fail("boundary widening checkpoint failed");
        }
    }
    if let Err(error) = atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default()) {
        return fail(format!("implement terminal persistence failed: {error}"));
    }
    println!("{}", state.bounded_status());
    dispatched
}

/// Publish custody only after the immutable host attempt and started log exist.
pub(crate) fn publish_host_checkpoint(
    manifest: &dispatch::host_attempt_state::HostAttemptState,
) -> Result<(), String> {
    let root = manifest
        .worktree
        .join(".dev/pipeline")
        .join(safe_scope(&manifest.prompt)?);
    let _lock =
        EntryLock::acquire(root.join(".coordinator-revision-lock")).map_err(|e| e.to_string())?;
    let path = root.join("coordinator.json");
    let mut state: CoordinatorState =
        serde_json::from_value(read_bounded(&path)?).map_err(|e| e.to_string())?;
    if manifest.prompt_sha256.as_deref() != Some(&state.prompt_sha256)
        || state
            .active_attempt
            .as_ref()
            .is_none_or(|a| a.attempt_id != manifest.attempt_id)
    {
        return Err("host checkpoint prompt or attempt mismatch".into());
    }
    let checkpoint_id = format!("host-{}", manifest.attempt_id);
    state
        .checkpoint_host_subagent(
            checkpoint_id.clone(),
            manifest.run_id.clone(),
            Some(super::dispatch::host_head(&manifest.worktree)?),
        )
        .map_err(|e| e.to_string())?;
    atomic_write(
        &path,
        &serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let checkpoint = state.checkpoint.as_ref().unwrap();
    let binding = ResumeHostSubagent {
        checkpoint_id,
        expected_revision: state.revision,
        run_id: manifest.run_id.clone(),
        task_id: manifest.task.clone(),
        phase: manifest.phase.clone(),
        attempt_id: manifest.attempt_id.clone(),
        session_ref: "<session_ref>".into(),
        prompt_sha256: checkpoint.prompt_sha256.clone(),
        execution_binding_sha256: checkpoint.execution_binding_sha256.clone(),
        commit: checkpoint.commit.clone(),
    };
    println!(
        "CLOSE: gal pipeline '{}' --require-codex-stop-v1 '{}'",
        manifest.prompt.display(),
        json!({"kind":"resume_host_subagent","binding":binding})
    );
    Ok(())
}

fn publish_host_terminal(path: &Path, bytes: &[u8]) -> Result<(), String> {
    // Publish the digest first so a crash immediately after the log rename is recoverable.
    atomic_write(
        &path.with_extension("terminal.sha256"),
        digest(bytes).as_bytes(),
    )
    .map_err(|e| e.to_string())?;
    atomic_write(path, bytes).map_err(|e| e.to_string())
}

fn resume_host(
    root: &Path,
    binding: ResumeHostSubagent,
    repair: &mut Option<super::format_repair::RepairInvocation>,
) -> Result<(), String> {
    use dispatch::host_attempt_state::{HostAttemptState, HostTerminalState};
    let _lock = EntryLock::acquire(root.join(".coordinator-revision-lock"))
        .map_err(|e| format!("coordinator revision lock unavailable: {e}"))?;
    let path = root.join("coordinator.json");
    let mut state: CoordinatorState =
        serde_json::from_value(read_bounded(&path)?).map_err(|e| e.to_string())?;
    let custody = state
        .resume_host_subagent(binding.clone())
        .map_err(|e| e.to_string())?;
    let worktree = super::pipeline_execution::current_worktree_root()
        .and_then(fs::canonicalize)
        .map_err(|e| e.to_string())?;
    let executable = std::env::current_exe()
        .and_then(fs::canonicalize)
        .map_err(|e| e.to_string())?;
    if state.execution_binding.as_ref()
        != Some(&coordinator_execution_binding(
            &state.plan_scope,
            &worktree,
            &executable,
        )?)
    {
        return Err("coordinator execution binding mismatch".into());
    }
    let task_scope = root.join(&state.task_id);
    let manifest_path = HostAttemptState::manifest_path_for(&task_scope, &binding.attempt_id)
        .map_err(|e| e.to_string())?;
    let manifest =
        HostAttemptState::load(&manifest_path, &binding.attempt_id).map_err(|e| e.to_string())?;
    if fs::canonicalize(&manifest.worktree).map_err(|e| e.to_string())? != worktree {
        return Err("host manifest worktree mismatch".into());
    }
    manifest
        .matches_identity(
            &manifest.worktree,
            &binding.task_id,
            &binding.phase,
            state.pinned_host.as_deref().unwrap_or_default(),
        )
        .map_err(|e| e.to_string())?;
    if manifest.profile != "codex_stop_v1"
        || manifest.run_id != binding.run_id
        || manifest.prompt_sha256.as_deref() != Some(&binding.prompt_sha256)
        || safe_scope(&manifest.prompt)? != state.plan_scope
        || binding.commit.as_deref() != Some(&super::dispatch::host_head(&worktree)?)
    {
        return Err("host manifest binding mismatch".into());
    }
    let task_scope = manifest
        .worktree
        .join(".dev/pipeline")
        .join(&state.plan_scope)
        .join(&state.task_id);
    let terminal_before = fs::read(&manifest.log_path).map_err(|e| e.to_string())?;
    let log = String::from_utf8_lossy(&terminal_before);
    let field = |key: &str| {
        log.lines()
            .take_while(|line| *line != "---STDOUT---")
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(name, _)| *name == key)
                    .map(|(_, value)| value.trim())
            })
    };
    let completed = field("terminal_state") == Some("completed");
    if !completed {
        if state
            .host_subagent
            .as_ref()
            .and_then(|h| h.terminal_digest.as_ref())
            .is_some()
        {
            return Err("attached host terminal is missing".into());
        }
        let snapshot = fs::read(&manifest.snapshot_path).map_err(|e| e.to_string())?;
        if manifest.snapshot_sha256.as_deref() != Some(&digest(&snapshot)) {
            return Err("host snapshot digest mismatch".into());
        }
        let mut placement = guarded_host_placement(&manifest, &snapshot, repair);
        let home = gal_foundation::paths::user_home().ok_or("host home unavailable")?;
        let closed = dispatch::run::close_host_attempt_with_typed_placement(
            &manifest.worktree,
            &task_scope,
            &manifest.task,
            &manifest.attempt_id,
            &manifest.task,
            &manifest.phase,
            &manifest.host,
            &binding.session_ref,
            &home,
            projection::ORCHESTRATED_ONLY_GOLEM_NAMES,
            &mut placement,
            &dispatch::run::HostAttemptCloseHooks {
                publish_terminal: publish_host_terminal,
                ..Default::default()
            },
        );
        if closed.is_ok() {
            super::dispatch::capture_attempt_usage(
                &manifest.worktree,
                Some(manifest.prompt.as_path()),
                &manifest.attempt_id,
                Some(manifest.log_path.as_path()),
                None,
            );
            super::dispatch::capture_host_child_usage(
                &manifest.worktree,
                manifest.prompt.as_path(),
                &manifest.attempt_id,
                &manifest.task,
                &manifest.phase,
                &manifest.started_at,
                &binding.session_ref,
            );
        } else if closed.as_ref().err() == Some(&dispatch::run::HostAttemptCloseError::Session) {
            eprintln!(
                "gal usage: usage not collected for attempt {}: the host session was not verified, so the attempt is left to explicit collection",
                manifest.attempt_id
            );
        }
        closed.map_err(|e| e.to_string())?;
    }
    let terminal = fs::read(&manifest.log_path).map_err(|e| e.to_string())?;
    let terminal_digest = digest(&terminal);
    let expected = fs::read_to_string(manifest.log_path.with_extension("terminal.sha256"))
        .map_err(|e| format!("host terminal digest unavailable: {e}"))?;
    let text = String::from_utf8_lossy(&terminal);
    let field = |key: &str| {
        text.lines()
            .take_while(|line| *line != "---STDOUT---")
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(name, _)| *name == key)
                    .map(|(_, value)| value.trim())
            })
    };
    let reference = dispatch::host_session::HostSessionRef::parse(&binding.session_ref)
        .map_err(|e| e.to_string())?;
    let session = dispatch::host_session::verify(
        &reference,
        &manifest.worktree,
        &gal_foundation::paths::user_home().ok_or("host home unavailable")?,
        &manifest.started_at,
        &manifest.attempt_id,
        &manifest.phase,
        projection::ORCHESTRATED_ONLY_GOLEM_NAMES,
    )
    .map_err(|e| e.to_string())?;
    if terminal_digest != expected
        || field("terminal_state") != Some("completed")
        || field("attempt_id") != Some(&manifest.attempt_id)
        || field("task_id") != Some(&manifest.task)
        || field("phase") != Some(&manifest.phase)
        || field("host") != Some(&manifest.host)
        || field("session_id") != Some(&session.session_id)
    {
        return Err("host terminal binding or digest mismatch".into());
    }
    if state
        .host_subagent
        .as_ref()
        .and_then(|h| h.terminal_digest.as_deref())
        .is_some_and(|attached| attached != terminal_digest)
    {
        return Err("attached host terminal digest mismatch".into());
    }
    recover_host_cleanup(&manifest, &manifest_path)?;
    let custody = state
        .attach_host_subagent_terminal(
            custody,
            HostTerminalState::Completed,
            terminal_digest.clone(),
        )
        .map_err(|e| e.to_string())?;
    atomic_write(
        &path,
        &serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    state
        .consume_host_subagent_checkpoint(custody, &terminal_digest)
        .map_err(|e| e.to_string())?;
    atomic_write(
        &path,
        &serde_json::to_vec_pretty(&state).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    println!("{}", state.bounded_status());
    Ok(())
}

fn guarded_host_placement<'a>(
    manifest: &'a dispatch::host_attempt_state::HostAttemptState,
    snapshot: &'a [u8],
    repair: &'a mut Option<super::format_repair::RepairInvocation>,
) -> impl FnMut(&[u8]) -> Result<(), dispatch::run::HostAttemptPlacementFailure> + 'a {
    move |bytes| {
        if !pipeline::task_spec::is_writeback_in_scope(&manifest.phase) {
            return Ok(());
        }
        let derived = match repair.as_mut() {
            Some(invocation) => {
                invocation
                    .candidate(bytes)
                    .map_err(super::dispatch::rejected_host_placement)?
                    .0
            }
            None => bytes.to_vec(),
        };
        let rendered = pipeline::task_spec::render_phase_writeback(
            snapshot,
            &derived,
            &manifest.phase,
            &manifest.task,
        )
        .map_err(|error| super::dispatch::rejected_host_placement(error.to_string()))?;
        atomic_write(&manifest.prompt, &rendered)
            .map_err(|error| super::dispatch::uncertain_host_placement(error.to_string()))
    }
}

fn recover_host_cleanup(
    manifest: &dispatch::host_attempt_state::HostAttemptState,
    manifest_path: &Path,
) -> Result<(), String> {
    // Cleanup is idempotent after exact terminal validation. Never remove another owner's lease.
    let owner = format!("owner_token={}", manifest.attempt_id);
    let locks = manifest.worktree.join(dispatch::dispatch::LEASE_ROOT);
    let receipt = manifest
        .receipt_path
        .as_ref()
        .ok_or("host receipt path missing")?;
    for entry in fs::read_dir(&locks).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if !entry.file_type().map_err(|e| e.to_string())?.is_file() {
            continue;
        }
        let body = fs::read_to_string(entry.path()).map_err(|e| e.to_string())?;
        if body.lines().nth(1) != Some(&owner) {
            continue;
        }
        let recorded = body
            .lines()
            .next()
            .and_then(|v| v.strip_prefix("receipt="))
            .ok_or("host receipt lease malformed")?;
        if !path_matches(recorded, &receipt.to_string_lossy()) {
            return Err("host receipt lease path mismatch".into());
        }
        fs::remove_file(entry.path()).map_err(|e| e.to_string())?;
    }
    match dispatch::dispatch::PromptWriterLease::reattach(
        &manifest.worktree,
        &manifest.prompt,
        &manifest.attempt_id,
    ) {
        Ok(mut lease) => lease.release_confirmed().map_err(|e| e.to_string())?,
        Err(dispatch::dispatch::DispatchError::LeaseMissing) => {}
        Err(error) => return Err(error.to_string()),
    }
    match fs::remove_dir(manifest_path.with_extension("close-claim")) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.to_string()),
    }
    for path in [&manifest.snapshot_path, &manifest.spec_path] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    let flags = windows_sys::Win32::Storage::FileSystem::MOVEFILE_REPLACE_EXISTING
        | windows_sys::Win32::Storage::FileSystem::MOVEFILE_WRITE_THROUGH;
    let ok = unsafe {
        windows_sys::Win32::Storage::FileSystem::MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            flags,
        )
    };
    if ok == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}
#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

pub(super) fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent"))?;
    fs::create_dir_all(parent)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let tmp = path.with_extension(format!("tmp-{nonce}"));
    fs::write(&tmp, bytes)?;
    if let Err(error) = replace_file(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(error);
    }
    Ok(())
}
fn read_bounded(path: &Path) -> Result<Value, String> {
    let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_RECORD {
        return Err("record exceeds size limit".into());
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn safe_scope(prompt: &Path) -> Result<String, String> {
    let name = prompt
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("invalid prompt filename")?;
    let scope = name
        .strip_suffix(".prompt.md")
        .ok_or("guarded mode requires an execution prompt")?;
    if scope.is_empty()
        || !scope
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("invalid prompt scope".into());
    }
    Ok(scope.to_string())
}
fn fail(message: impl AsRef<str>) -> ExitCode {
    eprintln!("gal pipeline: {}", message.as_ref());
    ExitCode::Error
}

/// Reconcile an existing attempt before the entry path can consume a grant or
/// start any later work. Unknown ownership is deliberately read-only.
fn reconcile_attempt(
    root: &Path,
    coordinator_path: &Path,
    state: &mut CoordinatorState,
) -> Option<ExitCode> {
    if state.has_pending_host_checkpoint() {
        println!("{}", state.bounded_status());
        return Some(ExitCode::Success);
    }
    let attempt = state.active_attempt.clone()?;
    let (verdict, observed_pid, observed_birth) = match attempt.process_id {
        Some(pid) => {
            let birth = dispatch::dispatch::process_birth_identity(pid);
            match birth {
                Some(identity) if Some(identity.clone()) == attempt.process_birth => {
                    (AttemptOwnership::Live, Some(pid), Some(identity))
                }
                Some(_) => (AttemptOwnership::Unknown, Some(pid), birth),
                None if process_is_running(pid) => (AttemptOwnership::Unknown, Some(pid), None),
                None => (AttemptOwnership::Dead, Some(pid), None),
            }
        }
        None => (AttemptOwnership::Unknown, None, None),
    };
    let mut candidate = state.clone();
    let ownership = match candidate.recover_owner(OwnerObservation {
        verdict,
        observed_pid,
        observed_birth,
    }) {
        Ok(value) => value,
        Err(error) => return Some(fail(format!("attempt recovery failed: {error}"))),
    };
    match ownership {
        AttemptOwnership::Unknown => Some(fail(
            "attempt-owner-unknown: refusing kill, reclaim, or redispatch",
        )),
        AttemptOwnership::Live => {
            if let Err(error) = atomic_write(
                coordinator_path,
                &serde_json::to_vec_pretty(&candidate).unwrap_or_default(),
            ) {
                return Some(fail(format!(
                    "attempt wait transition persistence failed: {error}"
                )));
            }
            *state = candidate;
            println!(
                "gal pipeline: attempt={} ownership=live action=wait",
                attempt.attempt_id
            );
            Some(ExitCode::Success)
        }
        AttemptOwnership::Dead => {
            let terminal = find_terminal_attempt_log(root, &attempt.attempt_id);
            let Some((log_path, log)) = terminal else {
                return Some(fail(
                    "attempt-owner-unknown: no terminal attempt log proves completion",
                ));
            };
            let verified = format!(
                "attempt-log:{}:{}",
                log_path.display(),
                digest(log.as_bytes())
            );
            if candidate
                .finish_attempt(&attempt.attempt_id, Some(verified))
                .is_err()
            {
                return Some(fail("attempt terminal transition failed"));
            }
            if let Err(error) = atomic_write(
                coordinator_path,
                &serde_json::to_vec_pretty(&candidate).unwrap_or_default(),
            ) {
                return Some(fail(format!(
                    "attempt terminal transition persistence failed: {error}"
                )));
            }
            *state = candidate;
            println!(
                "gal pipeline: attempt={} ownership=dead action=attached log={}",
                attempt.attempt_id,
                log_path.display()
            );
            Some(ExitCode::Success)
        }
        AttemptOwnership::NotApplicable => {
            Some(fail("attempt recovery observation is not applicable"))
        }
    }
}

fn find_terminal_attempt_log(root: &Path, attempt_id: &str) -> Option<(PathBuf, String)> {
    for entry in fs::read_dir(root).ok()?.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(log) = fs::read_to_string(&path) else {
            continue;
        };
        if log
            .lines()
            .any(|line| line.trim() == format!("attempt_id:       {attempt_id}"))
            && log
                .lines()
                .any(|line| line.starts_with("terminal_state:") && !line.ends_with("started"))
        {
            return Some((path, log));
        }
    }
    None
}

fn completed_bound_attempt(root: &Path, attempt_id: &str, task: &str, phases: &[&str]) -> bool {
    let Some((_, log)) = find_terminal_attempt_log(root, attempt_id) else {
        return false;
    };
    let field = |name: &str| {
        log.lines()
            .find_map(|line| line.strip_prefix(&format!("{name}:")))
            .map(str::trim)
    };
    field("task_id") == Some(task)
        && field("phase").is_some_and(|phase| phases.contains(&phase))
        && field("terminal_state") == Some("completed")
}

fn task_result(prompt: &str, section: &str, task: &str) -> Option<String> {
    let lines: Vec<_> = prompt.lines().collect();
    let section_start = lines
        .iter()
        .position(|line| line.trim() == format!("## {section}"))?;
    let section_end = lines[section_start + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map(|offset| section_start + 1 + offset)
        .unwrap_or(lines.len());
    let task_heading = format!("### [{task}]");
    let task_start = (section_start + 1..section_end)
        .rfind(|index| lines[*index].trim().starts_with(&task_heading))?;
    let task_end = (task_start + 1..section_end)
        .find(|index| lines[*index].starts_with("### "))
        .unwrap_or(section_end);
    Some(lines[task_start..task_end].join("\n"))
}

/// Read the explicit safety verdict from the phase's appended evidence line.
/// Free-form prose is deliberately never interpreted as a safety decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PipelineSafety {
    security_failed: bool,
    requires_out_of_allowlist_fix: bool,
    high_or_critical_finding_open: bool,
}

fn structured_security_gate(result: &str, phase: &str, task: &str) -> Option<PipelineSafety> {
    let mut evidence = result
        .lines()
        .filter_map(|line| line.strip_prefix("Pipeline Evidence:"));
    let raw = evidence.next()?.trim();
    if evidence.next().is_some() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_str(raw).ok()?;
    let object = value.as_object()?;
    let keys = [
        "schema_version",
        "task_id",
        "repair_round",
        "attempt_id",
        "implement_fix_attempt_id",
        "prompt_sha256",
        "head_sha",
        "producer",
        "executor_terminal_state",
        "security_gate",
        "requires_out_of_allowlist_fix",
        "gates",
    ];
    if object.len() != keys.len()
        || keys.iter().any(|key| !object.contains_key(*key))
        || object.get("schema_version")?.as_u64()? != 1
        || object.get("task_id")?.as_str()?.is_empty()
        || !object.get("task_id")?.as_str()?.eq_ignore_ascii_case(task)
        || object.get("attempt_id")?.as_str()?.is_empty()
        || object.get("executor_terminal_state")?.as_str()? != "completed"
        || !matches!(
            object
                .get("producer")?
                .as_str()?
                .to_ascii_uppercase()
                .as_str(),
            "TESTER" | "AUDITOR"
        )
        || !object
            .get("producer")?
            .as_str()?
            .eq_ignore_ascii_case(if phase == "audit" {
                "AUDITOR"
            } else {
                "TESTER"
            })
        || !object.get("requires_out_of_allowlist_fix")?.is_boolean()
        || !valid_evidence_hash(object.get("prompt_sha256")?.as_str()?, 64)
        || !valid_evidence_hash(object.get("head_sha")?.as_str()?, 40)
        || !valid_pipeline_gates(object.get("gates")?)
    {
        return None;
    }
    let round = object.get("repair_round")?.as_u64()?;
    let fix_attempt = object.get("implement_fix_attempt_id")?;
    if (round == 0 && !fix_attempt.is_null())
        || (round > 0 && fix_attempt.as_str().is_none_or(str::is_empty))
    {
        return None;
    }
    let security_failed = match object.get("security_gate")?.as_str()? {
        "fail" => true,
        "pass" => false,
        _ => return None,
    };
    Some(PipelineSafety {
        security_failed,
        requires_out_of_allowlist_fix: object.get("requires_out_of_allowlist_fix")?.as_bool()?,
        high_or_critical_finding_open: pipeline_has_open_high_or_critical(object.get("gates")?),
    })
}

fn valid_evidence_hash(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn valid_pipeline_gates(value: &serde_json::Value) -> bool {
    let Some(gates) = value.as_array() else {
        return false;
    };
    if gates.is_empty() {
        return false;
    }
    let mut gate_ids = std::collections::BTreeSet::new();
    let mut finding_ids = std::collections::BTreeSet::new();
    for gate in gates {
        let Some(gate) = gate.as_object() else {
            return false;
        };
        let keys = [
            "gate_id",
            "premise_id",
            "premise_text",
            "outcome",
            "findings",
        ];
        if gate.len() != keys.len()
            || keys.iter().any(|key| !gate.contains_key(*key))
            || gate
                .get("gate_id")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            || gate
                .get("premise_id")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            || gate
                .get("premise_text")
                .and_then(serde_json::Value::as_str)
                .is_none_or(str::is_empty)
            || !matches!(
                gate.get("outcome").and_then(serde_json::Value::as_str),
                Some("pass" | "fail")
            )
            || !gate_ids.insert(
                gate.get("gate_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap()
                    .to_string(),
            )
        {
            return false;
        }
        let Some(findings) = gate.get("findings").and_then(serde_json::Value::as_array) else {
            return false;
        };
        for finding in findings {
            let Some(finding) = finding.as_object() else {
                return false;
            };
            let keys = ["finding_id", "severity", "origin", "resolution", "evidence"];
            if finding.len() != keys.len()
                || keys.iter().any(|key| !finding.contains_key(*key))
                || finding
                    .get("finding_id")
                    .and_then(serde_json::Value::as_str)
                    .is_none_or(str::is_empty)
                || !matches!(
                    finding.get("severity").and_then(serde_json::Value::as_str),
                    Some("critical" | "high" | "medium" | "low")
                )
                || !matches!(
                    finding.get("origin").and_then(serde_json::Value::as_str),
                    Some("initial" | "fix-regression" | "missing-contract" | "pre-existing")
                )
                || !matches!(
                    finding
                        .get("resolution")
                        .and_then(serde_json::Value::as_str),
                    Some("open" | "fixed" | "dismissed")
                )
                || !finding_ids.insert(
                    finding
                        .get("finding_id")
                        .and_then(serde_json::Value::as_str)
                        .unwrap()
                        .to_string(),
                )
                || finding
                    .get("evidence")
                    .and_then(serde_json::Value::as_array)
                    .is_none_or(|items| {
                        items.is_empty()
                            || items
                                .iter()
                                .any(|item| item.as_str().is_none_or(str::is_empty))
                    })
            {
                return false;
            }
        }
    }
    true
}

fn pipeline_has_open_high_or_critical(value: &serde_json::Value) -> bool {
    value.as_array().is_some_and(|gates| {
        gates.iter().any(|gate| {
            gate.get("findings")
                .and_then(serde_json::Value::as_array)
                .is_some_and(|findings| {
                    findings.iter().any(|finding| {
                        matches!(
                            finding.get("severity").and_then(serde_json::Value::as_str),
                            Some("high" | "critical")
                        ) && finding
                            .get("resolution")
                            .and_then(serde_json::Value::as_str)
                            == Some("open")
                    })
                })
        })
    })
}

fn repeated_repair_failure(prompt: &str, task: &str, root: &Path) -> Option<String> {
    let mut rounds: std::collections::BTreeMap<u64, (String, Vec<serde_json::Value>)> =
        std::collections::BTreeMap::new();
    let mut attempts = std::collections::BTreeSet::new();
    let mut fix_attempts = std::collections::BTreeSet::new();
    for line in prompt.lines() {
        let Some(raw) = line.strip_prefix("Pipeline Evidence:") else {
            continue;
        };
        let value: serde_json::Value = serde_json::from_str(raw.trim()).ok()?;
        let producer = value.get("producer")?.as_str()?;
        if !producer.eq_ignore_ascii_case("AUDITOR") {
            continue;
        }
        if structured_security_gate(line, "audit", task).is_none() {
            return None;
        }
        let round = value.get("repair_round")?.as_u64()?;
        if round == 0 {
            continue;
        }
        let fix_id = value.get("implement_fix_attempt_id")?.as_str()?.to_string();
        let attempt_id = value.get("attempt_id")?.as_str()?.to_string();
        if !completed_bound_attempt(root, &attempt_id, task, &["audit"])
            || !completed_bound_attempt(root, &fix_id, task, &["fix", "implement"])
            || !fix_attempts.insert(fix_id.clone())
        {
            return None;
        }
        if !attempts.insert(attempt_id) {
            return None;
        }
        let row = rounds
            .entry(round)
            .or_insert_with(|| (fix_id.clone(), Vec::new()));
        if row.0 != fix_id {
            return None;
        }
        row.1.push(value);
    }
    let mut latest_resolution: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();
    let mut by_gate: std::collections::BTreeMap<
        (String, String, String),
        Vec<(u64, bool, Vec<String>)>,
    > = std::collections::BTreeMap::new();
    for (round, (_, records)) in &rounds {
        let mut gate_rows: std::collections::BTreeMap<
            (String, String, String),
            (bool, std::collections::BTreeSet<String>),
        > = std::collections::BTreeMap::new();
        for record in records {
            for gate in record.get("gates")?.as_array()? {
                let key = (
                    gate.get("gate_id")?.as_str()?.to_string(),
                    gate.get("premise_id")?.as_str()?.to_string(),
                    gate.get("premise_text")?.as_str()?.to_string(),
                );
                let failed = gate.get("outcome")?.as_str()? == "fail";
                let row = gate_rows
                    .entry(key)
                    .or_insert_with(|| (false, Default::default()));
                row.0 |= failed;
                for finding in gate.get("findings")?.as_array()? {
                    let id = finding.get("finding_id")?.as_str()?.to_string();
                    let resolution = finding.get("resolution")?.as_str()?.to_string();
                    latest_resolution.insert(id.clone(), resolution.clone());
                    if failed
                        && matches!(
                            finding.get("origin")?.as_str()?,
                            "fix-regression" | "missing-contract"
                        )
                        && resolution == "open"
                    {
                        row.1.insert(id);
                    }
                }
            }
        }
        for (key, (failed, finding_ids)) in gate_rows {
            by_gate.entry(key).or_default().push((
                *round,
                failed,
                finding_ids.into_iter().collect(),
            ));
        }
    }
    for ((gate, premise, _premise_text), observations) in by_gate {
        let mut previous_failure = None;
        for (round, failed, finding_ids) in observations {
            let failure = failed
                && finding_ids.iter().any(|id| {
                    latest_resolution
                        .get(id)
                        .is_some_and(|state| state == "open")
                });
            if failure && previous_failure == Some(round.saturating_sub(1)) {
                let reason = format!(
                    "design-defect:gate={gate};premise={premise};rounds={}-{}",
                    round - 1,
                    round
                );
                return Some(reason);
            }
            previous_failure = if failure { Some(round) } else { None };
        }
    }
    None
}

/// Verdict of one guarded `test` or `audit` dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseVerdict {
    Pass,
    Fail,
    SecurityOrProtectedPath,
    HostPending,
}

/// Branch of `Resume` that runs for a guarded phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResumePhaseHandler {
    Implement,
    Fix,
    TestOrAudit,
    Converge,
}

/// Map a guarded resume phase to the `Resume` branch that handles it. Returns
/// `None` for every phase `Resume` does not handle, so the caller can fail
/// closed before it writes the coordinator.
fn resume_phase_handler(phase: &str) -> Option<ResumePhaseHandler> {
    match phase {
        "implement" => Some(ResumePhaseHandler::Implement),
        "fix" => Some(ResumePhaseHandler::Fix),
        "test" | "audit" => Some(ResumePhaseHandler::TestOrAudit),
        "converge" => Some(ResumePhaseHandler::Converge),
        _ => None,
    }
}

fn guarded_phase_dispatch(
    state: &mut CoordinatorState,
    path: &Path,
    prompt: &Path,
    task: &str,
    phase: &str,
    repair: &mut Option<super::format_repair::RepairInvocation>,
) -> Result<(ExitCode, PhaseVerdict), String> {
    let attempt_id = digest(
        format!(
            "{}:{}:{}:{}",
            task,
            phase,
            state.revision,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
        .as_bytes(),
    );
    state.phase = phase.to_string();
    state.task_id = task.to_string();
    state.retry.max_attempts = state.retry.max_attempts.max(3);
    state
        .claim_attempt(attempt_id.clone())
        .map_err(|e| e.to_string())?;
    atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default())
        .map_err(|e| format!("{phase} claim persistence failed: {e}"))?;
    state
        .bind_process(
            std::process::id(),
            dispatch::dispatch::process_birth_identity(std::process::id()),
        )
        .map_err(|e| e.to_string())?;
    atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default())
        .map_err(|e| format!("{phase} bind persistence failed: {e}"))?;
    let mut dispatch_args = vec![
        "pipeline".into(),
        prompt.to_string_lossy().into_owned(),
        "--phase".into(),
        phase.to_string(),
        "--task".into(),
        task.to_string(),
    ];
    if state.pinned_mode.as_deref() == Some("host-subagent") {
        dispatch_args.extend(["--dispatch".into(), "host-subagent".into()]);
    }
    let dispatched = super::dispatch::cmd_pipeline_inner(&dispatch_args, repair);
    let published: CoordinatorState =
        serde_json::from_value(read_bounded(path)?).map_err(|e| e.to_string())?;
    if published.has_pending_host_checkpoint() {
        *state = published;
        return Ok((dispatched, PhaseVerdict::HostPending));
    }
    if state.pinned_mode.as_deref() == Some("host-subagent") {
        return Err("host dispatch did not publish a checkpoint".into());
    }
    let result_section = if phase == "test" {
        "Test Results"
    } else {
        "Review Results"
    };
    let text = fs::read_to_string(prompt)
        .map_err(|e| format!("{phase} result prompt unavailable: {e}"))?;
    let result = task_result(&text, result_section, task).unwrap_or_default();
    let result_lower = result.to_ascii_lowercase();
    let safety_status = structured_security_gate(&result, phase, task);
    let repo_root = path
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .and_then(Path::parent)
        .ok_or("coordinator path has no repository root")?;
    let design_defect_reason = (phase == "audit")
        .then(|| repeated_repair_failure(&text, task, repo_root))
        .flatten();
    let safety_stop = phase == "audit"
        && safety_status.is_some_and(|status| {
            status.security_failed
                || status.requires_out_of_allowlist_fix
                || status.high_or_critical_finding_open
        });
    let dangerous = safety_stop || design_defect_reason.is_some();
    let passed = dispatched == ExitCode::Success
        && if phase == "test" {
            result_lower.contains("pass")
                && !result_lower.contains("fail")
                && safety_status.is_some_and(|status| {
                    !status.security_failed
                        && !status.requires_out_of_allowlist_fix
                        && !status.high_or_critical_finding_open
                })
        } else {
            (result_lower.contains("approve") || result_lower.contains("pass"))
                && !result_lower.contains("blocking")
                && !result_lower.contains("reject")
                && safety_status.is_some_and(|status| {
                    !status.security_failed
                        && !status.requires_out_of_allowlist_fix
                        && !status.high_or_critical_finding_open
                })
        };
    state
        .finish_attempt(
            &attempt_id,
            Some(format!(
                "{phase}-dispatch:{dispatched:?};verdict:{}",
                if passed {
                    "pass"
                } else if dangerous {
                    "security-or-protected-path"
                } else {
                    "fail"
                }
            )),
        )
        .map_err(|e| e.to_string())?;
    let verdict = if dangerous {
        PhaseVerdict::SecurityOrProtectedPath
    } else if passed {
        PhaseVerdict::Pass
    } else {
        PhaseVerdict::Fail
    };
    if dangerous {
        if safety_stop {
            state.next_action = NextAction::Blocked {
                reason: "security-or-protected-path-finding".into(),
            };
        } else if let Some(reason) = design_defect_reason {
            let event = state
                .bind_design_stop(reason)
                .map_err(|error| error.to_string())?;
            state
                .record_design_stop(event)
                .map_err(|error| error.to_string())?;
        }
    } else if passed {
        state.last_verified_evidence = Some(format!("{phase}-verdict:pass"));
    } else {
        state.record_retry(format!("{phase}-verdict-failed"));
    }
    atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default())
        .map_err(|e| format!("{phase} terminal persistence failed: {e}"))?;
    Ok((dispatched, verdict))
}

/// Keep the task's retry handoff in step with one guarded `test` or `audit`
/// verdict. A failed verdict that schedules a retry or exhausts the ceiling
/// opens or refreshes that origin's block. A passing verdict resolves it. A
/// security or Protected Path stop writes nothing.
fn sync_retry_handoff(
    state: &CoordinatorState,
    prompt: &Path,
    task: &str,
    phase: &str,
    verdict: PhaseVerdict,
) -> Result<(), String> {
    let origin = if phase == "test" { "TEST" } else { "AUDIT" };
    let text = fs::read_to_string(prompt)
        .map_err(|e| format!("{phase} handoff prompt unavailable: {e}"))?;
    let exhausted = matches!(
        &state.next_action,
        NextAction::Blocked { reason } if reason == "retry-limit-exhausted"
    );
    let retrying = matches!(state.next_action, NextAction::RetryTask { .. });
    let updated = match verdict {
        PhaseVerdict::Fail if retrying || exhausted => {
            let worktree = super::pipeline_execution::current_worktree_root()
                .map_err(|e| format!("repository path unavailable: {e}"))?;
            let head = super::dispatch::host_head(&worktree)?;
            let date: String = gal_foundation::time::now_timestamp()
                .chars()
                .take(10)
                .collect();
            write_retry_handoff(
                &text,
                task,
                origin,
                &format!("{phase}-verdict-failed"),
                &date,
                &head,
            )
        }
        PhaseVerdict::Pass => resolve_retry_handoff(
            &text,
            task,
            origin,
            &format!("Cleared by a passing {origin} verdict."),
        ),
        _ => return Ok(()),
    };
    if updated != text {
        atomic_write(prompt, updated.as_bytes())
            .map_err(|e| format!("{phase} handoff persistence failed: {e}"))?;
    }
    Ok(())
}

fn split_lines(prompt: &str) -> Vec<String> {
    prompt.lines().map(str::to_string).collect()
}

/// Join lines with the prompt's own line ending so a CRLF file stays CRLF.
fn render_lines(lines: &[String], original: &str) -> String {
    let eol = if original.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = lines.join(eol);
    if original.ends_with('\n') {
        out.push_str(eol);
    }
    out
}

fn retry_handoff_heading(task: &str, origin: &str) -> String {
    format!("#### Retry Handoff — {task} / {origin}")
}

/// Half-open line range of the `### Handoff Notes` body under `## Status`.
fn handoff_notes_range(lines: &[String]) -> Option<(usize, usize)> {
    let status = lines.iter().position(|line| line.trim() == "## Status")?;
    let status_end = (status + 1..lines.len())
        .find(|&i| lines[i].starts_with("## "))
        .unwrap_or(lines.len());
    let heading = (status + 1..status_end).find(|&i| lines[i].trim() == "### Handoff Notes")?;
    let start = heading + 1;
    let end = (start..status_end)
        .find(|&i| lines[i].starts_with("### ") || lines[i].starts_with("## "))
        .unwrap_or(status_end);
    Some((start, end))
}

/// Add `## Status > ### Handoff Notes` when the prompt does not have it.
fn create_handoff_notes(lines: &mut Vec<String>) {
    let Some(status) = lines.iter().position(|line| line.trim() == "## Status") else {
        if lines.last().is_some_and(|line| !line.trim().is_empty()) {
            lines.push(String::new());
        }
        lines.extend(["## Status", "", "### Handoff Notes", ""].map(String::from));
        return;
    };
    let at = (status + 1..lines.len())
        .find(|&i| lines[i].starts_with("## "))
        .unwrap_or(lines.len());
    let mut insert = Vec::new();
    if at > 0 && !lines[at - 1].trim().is_empty() {
        insert.push(String::new());
    }
    insert.extend(["### Handoff Notes".to_string(), String::new()]);
    lines.splice(at..at, insert);
}

/// Line range of the OPEN block headed `heading` inside the notes range, as
/// (heading line, end-exclusive block end). Resolved history is skipped.
fn open_handoff_block(
    lines: &[String],
    notes: (usize, usize),
    heading: &str,
) -> Option<(usize, usize)> {
    let (start, end) = notes;
    (start..end)
        .filter(|&i| lines[i].trim() == heading)
        .map(|i| {
            let block_end = (i + 1..end)
                .find(|&j| lines[j].trim_start().starts_with("#### "))
                .unwrap_or(end);
            (i, block_end)
        })
        .find(|&(i, block_end)| {
            lines[i + 1..block_end]
                .iter()
                .any(|line| line.trim() == "- Status: OPEN")
        })
}

fn is_attempt_line(line: &str) -> bool {
    line.trim_start()
        .split_once(". ")
        .is_some_and(|(number, _)| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
}

fn attempt_lines(
    number: usize,
    date: &str,
    origin: &str,
    failure: &str,
    head: &str,
) -> Vec<String> {
    vec![
        format!("  {number}. {date} — {origin} verdict failed ({failure})."),
        "     - Result: Failed verdict recorded at this commit. No fix commit yet.".into(),
        format!(
            "     - Validation: guarded {} phase verdict: fail",
            origin.to_ascii_lowercase()
        ),
        format!("     - Commit: {head}"),
    ]
}

/// Open a retry handoff for a failed `origin` verdict, or append an attempt to
/// the OPEN block of the same origin. Fields follow `handoff-notes.md`.
/// Returns the updated prompt text.
fn write_retry_handoff(
    prompt: &str,
    task: &str,
    origin: &str,
    failure: &str,
    date: &str,
    head: &str,
) -> String {
    let mut lines = split_lines(prompt);
    if handoff_notes_range(&lines).is_none() {
        create_handoff_notes(&mut lines);
    }
    if let Some((start, end)) = handoff_notes_range(&lines) {
        let placeholders: Vec<usize> = (start..end)
            .filter(|&i| lines[i].trim() == "None.")
            .collect();
        for index in placeholders.into_iter().rev() {
            lines.remove(index);
        }
    }
    let (start, end) = handoff_notes_range(&lines).expect("handoff notes exist after creation");
    let heading = retry_handoff_heading(task, origin);
    let problem = format!("{origin} verdict failed ({failure}).");
    match open_handoff_block(&lines, (start, end), &heading) {
        Some((index, block_end)) => {
            if let Some(problem_line) =
                (index + 1..block_end).find(|&i| lines[i].trim_start().starts_with("- Problem:"))
            {
                lines[problem_line] = format!("- Problem: {problem}");
            }
            let number = (index + 1..block_end)
                .filter(|&i| is_attempt_line(&lines[i]))
                .count()
                + 1;
            let insert_at = (index + 1..block_end)
                .find(|&i| lines[i].trim_start().starts_with("- Next human step:"))
                .unwrap_or(block_end);
            lines.splice(
                insert_at..insert_at,
                attempt_lines(number, date, origin, failure, head),
            );
        }
        None => {
            let (test_evidence, review_evidence, evidence_section) = if origin == "TEST" {
                (
                    format!("`## Test Results` > `### [{task}]`"),
                    "not-applicable".to_string(),
                    "## Test Results",
                )
            } else {
                (
                    "not-applicable".to_string(),
                    format!("`## Review Results` > `### [{task}]`"),
                    "## Review Results",
                )
            };
            let mut block = Vec::new();
            if end > 0 && !lines[end - 1].trim().is_empty() {
                block.push(String::new());
            }
            block.push(heading);
            block.push(String::new());
            block.push("- Status: OPEN".into());
            block.push(format!("- Problem: {problem}"));
            block.push("- Evidence:".into());
            block.push(format!("  - Test Results: {test_evidence}"));
            block.push(format!("  - Review Results: {review_evidence}"));
            block.push("  - Security Review: not-applicable".into());
            block.push("- Attempts:".into());
            block.extend(attempt_lines(1, date, origin, failure, head));
            block.push(format!(
                "- Next human step: Fix executor: repair the failures recorded in `{evidence_section}` > `### [{task}]` and change only the files on the task allowlist."
            ));
            if end < lines.len() {
                block.push(String::new());
            }
            lines.splice(end..end, block);
        }
    }
    render_lines(&lines, prompt)
}

/// Mark the OPEN block for `task` and `origin` as RESOLVED and replace its next
/// human step with `confirmation`. Returns the prompt unchanged when no such
/// block is open.
fn resolve_retry_handoff(prompt: &str, task: &str, origin: &str, confirmation: &str) -> String {
    let mut lines = split_lines(prompt);
    let Some(notes) = handoff_notes_range(&lines) else {
        return prompt.to_string();
    };
    let heading = retry_handoff_heading(task, origin);
    let Some((index, block_end)) = open_handoff_block(&lines, notes, &heading) else {
        return prompt.to_string();
    };
    for line in &mut lines[index + 1..block_end] {
        if line.trim() == "- Status: OPEN" {
            *line = "- Status: RESOLVED".into();
        } else if line.trim_start().starts_with("- Next human step:") {
            *line = format!("- Next human step: {confirmation}");
        }
    }
    render_lines(&lines, prompt)
}

fn process_is_running(pid: u32) -> bool {
    #[cfg(target_os = "linux")]
    {
        Path::new("/proc").join(pid.to_string()).exists()
    }
    #[cfg(target_os = "windows")]
    {
        let _ = pid;
        false
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        let _ = pid;
        false
    }
}

fn repo_snapshot(root: &str) -> Result<(String, String), String> {
    let head = std::process::Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    if !head.status.success() {
        return Err("git rev-parse HEAD failed".into());
    }
    let head = String::from_utf8(head.stdout)
        .map_err(|e| e.to_string())?
        .trim()
        .to_string();
    let diff = std::process::Command::new("git")
        .current_dir(root)
        .args(["diff", "--binary", "HEAD"])
        .output()
        .map_err(|e| e.to_string())?;
    if !diff.status.success() {
        return Err("git diff HEAD failed".into());
    }
    Ok((head, digest(&diff.stdout)))
}

struct EntryLock(PathBuf);
impl EntryLock {
    fn acquire(path: PathBuf) -> std::io::Result<Self> {
        fs::create_dir(&path)?;
        Ok(Self(path))
    }
}
impl Drop for EntryLock {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}

fn path_matches(a: &str, b: &str) -> bool {
    let norm = |s: &str| s.replace('\\', "/").trim_start_matches("//?/").to_string();
    norm(a).eq_ignore_ascii_case(&norm(b))
}

fn consume_grant(
    plugin: &Path,
    control: &Path,
    expected_worktree: &str,
    expected_prompt_hash: &str,
) -> Result<String, String> {
    let lock_path = control
        .parent()
        .ok_or("grant control path has no parent")?
        .join(".entry-lock");
    let _lock = EntryLock::acquire(lock_path)
        .map_err(|e| format!("host-continuation-not-ready: grant lock unavailable: {e}"))?;
    let mut grant = read_bounded(&plugin.join("ready-grant.json"))
        .map_err(|_| "host-continuation-not-ready: ready grant missing or invalid")?;
    let mut record = read_bounded(control)
        .map_err(|_| "host-continuation-not-ready: control grant missing or invalid")?;
    if grant.get("state").and_then(Value::as_str) != Some("ReadyGrant") {
        return Err("host-continuation-not-ready: grant is stale or already consumed".into());
    }
    let binding = grant
        .get("binding")
        .ok_or("host-continuation-not-ready: grant binding missing")?;
    let rec_worktree = record
        .get("worktree")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let binding_worktree = binding
        .get("worktree")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !path_matches(rec_worktree, expected_worktree)
        || grant.get("presented_turn_id") != binding.get("turn_id")
        || record.get("binding").is_some_and(|v| v != binding)
        || record.get("turn_id").and_then(Value::as_str).is_none()
        || record.get("nonce_digest").and_then(Value::as_str).is_none()
        || !path_matches(binding_worktree, expected_worktree)
        || record.get("prompt_hash").and_then(Value::as_str) != Some(expected_prompt_hash)
        || binding.get("prompt_hash").and_then(Value::as_str) != Some(expected_prompt_hash)
        || record.get("prompt_hash") != binding.get("prompt_hash")
        || record.get("session_id") != binding.get("session_id")
        || record.get("turn_id") != binding.get("turn_id")
        || record.get("generation") != binding.get("generation")
        || record.get("handler_digest") != binding.get("handler_digest")
        || grant
            .get("expires")
            .and_then(Value::as_u64)
            .is_none_or(|t| {
                t < SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()
            })
    {
        return Err("host-continuation-not-ready: grant binding mismatch or expiry".into());
    }
    let activation = digest(&serde_json::to_vec(&record).map_err(|e| e.to_string())?);
    grant["state"] = json!("Consumed");
    atomic_write(
        &plugin.join("ready-grant.json"),
        &serde_json::to_vec(&grant).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("grant consumption failed: {e}"))?;
    record["state"] = json!("Consumed");
    atomic_write(
        control,
        &serde_json::to_vec(&record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("grant control consumption failed: {e}"))?;
    Ok(activation)
}

/// Build the coordinator execution binding the guarded entry persists, in the
/// same path form that receipt writers and verifiers compare against.
pub(crate) fn coordinator_execution_binding(
    plan_scope: &str,
    worktree: &Path,
    executable: &Path,
) -> Result<ExecutionBinding, String> {
    let executable_sha256 = fs::read(executable)
        .map(|bytes| digest(&bytes))
        .map_err(|error| format!("executable hash failed: {error}"))?;
    Ok(ExecutionBinding {
        version: ExecutionBinding::VERSION,
        binding_scope: format!("coordinator:{plan_scope}"),
        worktree_root: super::pipeline_execution::binding_path_text(worktree),
        executable_path: super::pipeline_execution::binding_path_text(executable),
        executable_sha256,
    })
}

pub(crate) fn cmd_guarded_pipeline_entry_inner(
    args: &[String],
    repair: &mut Option<super::format_repair::RepairInvocation>,
) -> ExitCode {
    let parsed = match parse(args) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("gal pipeline: {e}");
            return ExitCode::Usage;
        }
    };
    let canonical_prompt = match fs::canonicalize(&parsed.prompt) {
        Ok(p) => p,
        Err(e) => return fail(format!("prompt unavailable: {e}")),
    };
    let prompt_bytes = match fs::read(&canonical_prompt) {
        Ok(b) => b,
        Err(e) => return fail(format!("prompt unavailable: {e}")),
    };
    let mut prompt_hash = digest(&prompt_bytes);
    let worktree_canon =
        match super::pipeline_execution::current_worktree_root().and_then(fs::canonicalize) {
            Ok(p) => p,
            Err(e) => return fail(format!("worktree unavailable: {e}")),
        };
    let worktree = super::pipeline_execution::binding_path_text(&worktree_canon);
    let scope = match safe_scope(&canonical_prompt) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let root = worktree_canon.join(".dev/pipeline").join(&scope);
    if let Some(exit) = continuation(&parsed, &root, repair) {
        return exit;
    }
    let coordinator_path = root.join("coordinator.json");
    let mut prior_generation = None;
    if coordinator_path.is_file() {
        let mut existing: CoordinatorState = match read_bounded(&coordinator_path)
            .and_then(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        {
            Ok(state) => state,
            Err(error) => return fail(format!("coordinator unavailable: {error}")),
        };
        if existing.profile == ContinuationProfile::CodexStopV1 {
            if let Some(exit) = reconcile_attempt(&root, &coordinator_path, &mut existing) {
                return exit;
            }
            if existing.active_attempt.is_some() {
                return fail("active attempt exists; refusing coordinator rebuild");
            }
            if existing
                .checkpoint
                .as_ref()
                .is_some_and(|checkpoint| checkpoint.consumed_receipt_id.is_none())
            {
                return fail("unconsumed checkpoint exists; refusing coordinator rebuild");
            }
            prior_generation = match fs::read(&coordinator_path) {
                Ok(bytes) => Some(bytes),
                Err(error) => return fail(format!("coordinator generation unavailable: {error}")),
            };
        }
    }
    let journal = root.join("projection-journal.json");
    if journal.exists() {
        return fail("pending projection journal exists; refusing coordinator rebuild");
    }
    let control = root.join("codex-hook-grant.json");
    let plugin = match std::env::var_os("PLUGIN_DATA") {
        Some(p) => PathBuf::from(p),
        None => return fail("host-continuation-not-ready: plugin grant store unavailable"),
    };
    let activation = match consume_grant(&plugin, &control, &worktree, &prompt_hash) {
        Ok(a) => a,
        Err(e) => return fail(e),
    };
    let task = match parsed.task {
        Some(task) => task,
        None => {
            let prompt_text = String::from_utf8_lossy(&prompt_bytes);
            match super::finalize_check::task_checkbox_projection(&prompt_text)
                .unchecked
                .into_iter()
                .next()
            {
                Some(task) => task,
                None => {
                    return fail("guarded entry found no unchecked task in the execution prompt");
                }
            }
        }
    };
    let mut state = CoordinatorState::new_legacy(&scope, prompt_hash.clone(), task, parsed.phase);
    state.profile = ContinuationProfile::CodexStopV1;
    state.activation_digest = Some(activation);
    let bound_executable = match std::env::current_exe().and_then(fs::canonicalize) {
        Ok(path) => path,
        Err(error) => return fail(format!("executable resolution failed: {error}")),
    };
    let binding = match coordinator_execution_binding(&scope, &worktree_canon, &bound_executable) {
        Ok(binding) => binding,
        Err(error) => return fail(error),
    };
    if let Err(error) = state.guarded_entry(binding) {
        return fail(format!("execution binding rejected: {error}"));
    }
    state.next_action = NextAction::Blocked {
        reason: "entry-latch-pending".into(),
    };
    let coordinator_path = root.join("coordinator.json");
    if let Some(bytes) = prior_generation {
        let generation_path = root
            .join("generations")
            .join(format!("coordinator-{}.json", digest(&bytes)));
        match fs::read(&generation_path) {
            Ok(existing) if existing == bytes => {}
            Ok(_) => return fail("coordinator generation digest collision"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Err(error) = atomic_write(&generation_path, &bytes) {
                    return fail(format!(
                        "coordinator generation preservation failed: {error}"
                    ));
                }
            }
            Err(error) => return fail(format!("coordinator generation unavailable: {error}")),
        }
    }
    let write_state = |state: &CoordinatorState| -> Result<(), String> {
        atomic_write(
            &coordinator_path,
            &serde_json::to_vec_pretty(state).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())
    };
    if let Err(e) = write_state(&state) {
        return fail(format!("coordinator revision 0 persistence failed: {e}"));
    }

    let journal = root.join("projection-journal.json");
    if journal.exists() {
        if let Err(e) = pipeline::projection_journal::recover(&journal) {
            return fail(format!("pending projection journal recovery failed: {e}"));
        }
        let recovered_prompt = match fs::read(&canonical_prompt) {
            Ok(bytes) => bytes,
            Err(e) => return fail(format!("recovered prompt unavailable: {e}")),
        };
        prompt_hash = digest(&recovered_prompt);
        state.prompt_sha256 = prompt_hash.clone();
        if let Err(e) = write_state(&state) {
            return fail(format!("recovered revision 0 persistence failed: {e}"));
        }
    }

    // Self-bootstrap evidence is intentionally checked before any gate receipt is trusted.
    let pinned = std::env::var("GAL_PINNED_EXECUTABLE_SHA256").ok();
    let current_exe = bound_executable;
    let running_hash = state
        .execution_binding
        .as_ref()
        .map(|binding| binding.executable_sha256.clone())
        .unwrap_or_default();
    let installed = std::env::var_os("GAL_INSTALLED_EXECUTABLE")
        .map(PathBuf::from)
        .unwrap_or_else(|| current_exe.clone());
    let installed_hash = fs::read(&installed).ok().map(|b| digest(&b));
    if pinned.as_deref().is_none_or(|p| {
        !p.eq_ignore_ascii_case(&running_hash)
            || installed_hash
                .as_deref()
                .is_none_or(|h| !p.eq_ignore_ascii_case(h))
    }) {
        if let Err(e) = state
            .checkpoint("self-bootstrap".into(), CheckpointKind::TaskQuality, None)
            .and_then(|_| {
                write_state(&state).map_err(|_| {
                    pipeline::coordinator::CoordinatorError::InvalidTransition(
                        "checkpoint persistence failed",
                    )
                })
            })
        {
            return fail(format!("self-bootstrap checkpoint failed: {e}"));
        }
        println!(
            "gal pipeline: checkpoint=self-bootstrap revision={}",
            state.revision
        );
        return ExitCode::Success;
    }

    let segment = digest(
        format!(
            "{}:{}:{}",
            state.revision,
            prompt_hash,
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
        .as_bytes(),
    );
    let receipt_path = root.join(format!("entry-preflight-{segment}.receipt.md"));
    let before_snapshot = match repo_snapshot(&worktree) {
        Ok(v) => v,
        Err(e) => return fail(format!("entry binding unavailable: {e}")),
    };
    let before = SystemTime::now();
    let preflight_args = vec![
        "pipeline-preflight".into(),
        canonical_prompt.to_string_lossy().into_owned(),
        "--receipt".into(),
        receipt_path.to_string_lossy().into_owned(),
    ];
    let preflight = super::pipeline_preflight::cmd_pipeline_preflight(&preflight_args);
    if preflight != ExitCode::Success {
        return fail("entry-preflight-failed");
    }
    let receipt = match fs::read_to_string(&receipt_path) {
        Ok(s) => s,
        Err(e) => return fail(format!("entry-preflight-receipt-unavailable: {e}")),
    };
    let modified = fs::metadata(&receipt_path).and_then(|m| m.modified()).ok();
    if modified.is_none_or(|m| m + Duration::from_secs(2) < before)
        || !receipt.lines().any(|l| l.trim() == "overall: pass")
    {
        return fail("entry-preflight-receipt-stale-or-not-pass");
    }
    let after_prompt = match fs::read(&canonical_prompt) {
        Ok(b) => digest(&b),
        Err(e) => return fail(format!("prompt changed or unreadable after preflight: {e}")),
    };
    if after_prompt != prompt_hash {
        return fail("entry-preflight-prompt-binding-changed");
    }
    let after_snapshot = match repo_snapshot(&worktree) {
        Ok(v) => v,
        Err(e) => return fail(format!("entry-preflight-binding-unavailable: {e}")),
    };
    if before_snapshot != after_snapshot {
        return fail("entry-preflight-worktree-binding-changed");
    }
    let binding = json!({"prompt_sha256":prompt_hash,"worktree":worktree,"head":after_snapshot.0,"dirty_diff_sha256":after_snapshot.1,"receipt_sha256":digest(receipt.as_bytes()),"receipt_path":receipt_path.to_string_lossy(),"executable_sha256":running_hash,"installed_executable_sha256":installed_hash});
    if let Err(e) = atomic_write(
        &root.join("entry-binding.json"),
        &serde_json::to_vec_pretty(&binding).unwrap_or_default(),
    ) {
        return fail(format!("entry binding persistence failed: {e}"));
    }
    if let Ok(pin) = super::pipeline_preflight::read_pin(&canonical_prompt) {
        if pin.dispatch_mode == super::pipeline_preflight::DispatchMode::HostSubagent {
            state.pinned_mode = Some("host-subagent".into());
            state.pinned_host = pin.host;
        }
    }
    let commit = Some(after_snapshot.0);
    if let Err(e) = state.checkpoint(
        "task-quality-entry".into(),
        CheckpointKind::TaskQuality,
        commit,
    ) {
        return fail(format!("task-quality checkpoint failed: {e}"));
    }
    state.next_action = NextAction::AwaitOrchestrator {
        checkpoint_id: "task-quality-entry".into(),
    };
    if let Err(e) = write_state(&state) {
        return fail(format!("task-quality checkpoint persistence failed: {e}"));
    }
    println!(
        "gal pipeline: checkpoint=task-quality-entry revision={}",
        state.revision
    );
    ExitCode::Success
}

#[cfg(test)]
mod tests {
    fn fixture_task_id() -> String {
        format!("{}-{:02}", 'T', 1)
    }

    #[test]
    fn design_stop_counts_only_consecutive_authorized_same_gate_repairs() {
        let fixture = tempfile::tempdir().unwrap();
        let task = fixture_task_id();
        let write_attempt = |id: &str| {
            fs::write(
                fixture.path().join(format!("{id}.log")),
                format!(
                    "attempt_id:       {id}\ntask_id: {task}\nphase: {}\nterminal_state: completed\n",
                    if id.starts_with("fix") {
                        "implement"
                    } else {
                        "audit"
                    }
                ),
            )
            .unwrap();
        };
        let record = |round: u8,
                      fix: &str,
                      attempt: &str,
                      gate: &str,
                      premise: &str,
                      outcome: &str,
                      resolution: &str,
                      origin: &str| {
            format!(
                "Pipeline Evidence: {{\"schema_version\":1,\"task_id\":\"{task}\",\"repair_round\":{round},\"attempt_id\":\"{attempt}\",\"implement_fix_attempt_id\":\"{fix}\",\"prompt_sha256\":\"{}\",\"head_sha\":\"{}\",\"producer\":\"AUDITOR\",\"executor_terminal_state\":\"completed\",\"security_gate\":\"pass\",\"requires_out_of_allowlist_fix\":false,\"gates\":[{{\"gate_id\":\"{gate}\",\"premise_id\":\"{premise}\",\"premise_text\":\"reproduce premise\",\"outcome\":\"{outcome}\",\"findings\":[{{\"finding_id\":\"stable-finding\",\"severity\":\"medium\",\"origin\":\"{origin}\",\"resolution\":\"{resolution}\",\"evidence\":[\"src/lib.rs:10\"]}}]}}]}}",
                "a".repeat(64), "b".repeat(40)
            )
        };
        let two_failures = format!(
            "{}\n{}",
            record(
                1,
                "fix-1",
                "audit-1",
                "gate-a",
                "premise-a",
                "fail",
                "open",
                "fix-regression"
            ),
            record(
                2,
                "fix-2",
                "audit-2",
                "gate-a",
                "premise-a",
                "fail",
                "open",
                "missing-contract"
            )
        );
        for id in ["fix-1", "audit-1", "fix-2", "audit-2"] {
            write_attempt(id);
        }
        assert!(repeated_repair_failure(&two_failures, &task, fixture.path()).is_some());

        let different_gate = format!(
            "{}\n{}",
            record(
                1,
                "fix-1",
                "audit-1",
                "gate-a",
                "premise-a",
                "fail",
                "open",
                "fix-regression"
            ),
            record(
                2,
                "fix-2",
                "audit-2",
                "gate-b",
                "premise-a",
                "fail",
                "open",
                "fix-regression"
            )
        );
        assert!(repeated_repair_failure(&different_gate, &task, fixture.path()).is_none());
        let interrupted_by_pass = format!(
            "{}\n{}",
            record(
                1,
                "fix-1",
                "audit-1",
                "gate-a",
                "premise-a",
                "fail",
                "open",
                "fix-regression"
            ),
            record(
                2,
                "fix-2",
                "audit-2",
                "gate-a",
                "premise-a",
                "pass",
                "fixed",
                "fix-regression"
            )
        );
        assert!(repeated_repair_failure(&interrupted_by_pass, &task, fixture.path()).is_none());
        let initial_round = format!(
            "{}\n{}",
            record(
                0,
                "",
                "audit-0",
                "gate-a",
                "premise-a",
                "fail",
                "open",
                "initial"
            ),
            record(
                1,
                "fix-1",
                "audit-1",
                "gate-a",
                "premise-a",
                "fail",
                "open",
                "fix-regression"
            )
        );
        assert!(repeated_repair_failure(&initial_round, &task, fixture.path()).is_none());
    }

    #[test]
    fn structured_security_gate_requires_explicit_completed_evidence() {
        let task = fixture_task_id();
        let evidence = |security_gate: &str, terminal: &str| {
            format!(
                "### [{task}]\nPipeline Evidence: {{\"schema_version\":1,\"task_id\":\"{task}\",\"repair_round\":0,\"attempt_id\":\"attempt-1\",\"implement_fix_attempt_id\":null,\"prompt_sha256\":\"{}\",\"head_sha\":\"{}\",\"producer\":\"AUDITOR\",\"executor_terminal_state\":\"{terminal}\",\"security_gate\":\"{security_gate}\",\"requires_out_of_allowlist_fix\":false,\"gates\":[{{\"gate_id\":\"cli\",\"premise_id\":\"parse\",\"premise_text\":\"parse evidence\",\"outcome\":\"pass\",\"findings\":[]}}]}}",
                "a".repeat(64), "b".repeat(40)
            )
        };
        let safe = PipelineSafety {
            security_failed: false,
            requires_out_of_allowlist_fix: false,
            high_or_critical_finding_open: false,
        };
        let failed = PipelineSafety {
            security_failed: true,
            ..safe
        };
        assert_eq!(
            structured_security_gate(&evidence("fail", "completed"), "audit", &task),
            Some(failed)
        );
        assert_eq!(
            structured_security_gate(&evidence("pass", "completed"), "audit", &task),
            Some(safe)
        );
        assert_eq!(
            structured_security_gate(&evidence("pass", "started"), "audit", &task),
            None
        );
        assert_eq!(
            structured_security_gate("audit found a protected path", "audit", &task),
            None
        );
        assert_eq!(
            structured_security_gate(
                &format!(
                    "{}\n{}",
                    evidence("fail", "completed"),
                    evidence("pass", "completed")
                ),
                "audit",
                &task
            ),
            None
        );
    }

    use super::*;

    fn linked_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let primary = temp.path().join("primary");
        let worktree = temp.path().join("linked");
        fs::create_dir_all(primary.join(".dev/plans")).unwrap();
        let git = |cwd: &Path, args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(cwd)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&primary, &["init", "-q", "-b", "main"]);
        git(
            &primary,
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(&primary, &["config", "user.name", "Fixture"]);
        fs::write(primary.join(".dev/plans/entry.prompt.md"), "prompt\n").unwrap();
        git(&primary, &["add", "--all"]);
        git(&primary, &["commit", "-qm", "fixture"]);
        git(
            &primary,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "linked",
                worktree.to_str().unwrap(),
            ],
        );
        (
            temp,
            worktree.clone(),
            worktree.join(".dev/plans/entry.prompt.md"),
        )
    }

    #[test]
    fn recovery_refuses_retained_lock_and_projection_journal() {
        let (_temp, worktree, prompt) = linked_fixture();
        let primary = worktree.parent().unwrap().join("primary");
        let git = |cwd: &Path, args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(cwd)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        fs::write(primary.join("primary-advance.txt"), "primary\n").unwrap();
        git(&primary, &["add", "--all"]);
        git(&primary, &["commit", "-qm", "fixture primary advance"]);
        let before_head = git(&worktree, &["rev-parse", "HEAD"]);
        let root = worktree.join(".dev/pipeline/entry");
        fs::create_dir_all(&root).unwrap();
        fs::create_dir(root.join(".coordinator-revision-lock")).unwrap();
        let error = recover_entry_worktree(&prompt, &worktree).unwrap_err();
        assert!(error.contains("retained coordinator lock"), "{error}");
        assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), before_head);
        assert!(!worktree.join("primary-advance.txt").exists());
        fs::remove_dir(root.join(".coordinator-revision-lock")).unwrap();
        fs::write(root.join("projection-journal.json"), b"pending").unwrap();
        let error = recover_entry_worktree(&prompt, &worktree).unwrap_err();
        assert!(error.contains("projection journal"), "{error}");
        assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), before_head);
        fs::remove_file(root.join("projection-journal.json")).unwrap();

        let mut state = CoordinatorState::new_legacy("entry", "prompt-hash", "task", "implement");
        state.active_attempt = Some(pipeline::coordinator::ActiveAttempt {
            attempt_id: "attempt".into(),
            task_id: "task".into(),
            phase: "implement".into(),
            process_id: None,
            process_birth: None,
            dispatch_claimed: false,
        });
        fs::write(
            root.join("coordinator.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        assert!(!recover_entry_worktree(&prompt, &worktree).unwrap());
        assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), before_head);
        assert!(!worktree.join("primary-advance.txt").exists());

        state.active_attempt = None;
        state.checkpoint = Some(pipeline::coordinator::SemanticCheckpoint {
            checkpoint_id: "checkpoint".into(),
            revision: 1,
            task_id: "task".into(),
            phase: "implement".into(),
            prompt_sha256: "prompt-hash".into(),
            execution_binding_sha256: "binding-hash".into(),
            commit: None,
            kind: pipeline::coordinator::CheckpointKind::TaskQuality,
            consumed_receipt_id: None,
        });
        fs::write(
            root.join("coordinator.json"),
            serde_json::to_vec(&state).unwrap(),
        )
        .unwrap();
        assert!(!recover_entry_worktree(&prompt, &worktree).unwrap());
        assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), before_head);
        assert!(!worktree.join("primary-advance.txt").exists());
    }

    #[test]
    fn recovery_skips_non_git_and_primary_checkouts_without_creating_custody() {
        let temp = tempfile::tempdir().unwrap();
        let prompt = temp.path().join(".dev/plans/entry.prompt.md");
        fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        fs::write(&prompt, "prompt\n").unwrap();
        assert!(!recover_entry_worktree(&prompt, temp.path()).unwrap());
        assert!(!temp.path().join(".dev/pipeline").exists());

        let primary = temp.path().join("primary");
        fs::create_dir_all(&primary).unwrap();
        let output = std::process::Command::new("git")
            .current_dir(&primary)
            .args(["init", "-q", "-b", "main"])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(!recover_entry_worktree(&prompt, &primary).unwrap());
        assert!(!primary.join(".dev/pipeline").exists());
    }

    #[test]
    fn recovery_classifier_accepts_only_guarded_entry() {
        let prompt = format!(".dev/plans/recovery-entry-{}.prompt.md", std::process::id());
        let entry = vec!["pipeline".into(), prompt.clone()];
        assert_eq!(fresh_entry_prompt(&entry), Some(PathBuf::from(&prompt)));
        let task = format!("T-{:02}", 1);
        let resume = vec![
            "pipeline".into(),
            prompt.clone(),
            "--resume".into(),
            "--task".into(),
            task,
            "--phase".into(),
            "implement".into(),
        ];
        assert!(fresh_entry_prompt(&resume).is_none());
        let phase = vec![
            "pipeline".into(),
            prompt.clone(),
            "--phase".into(),
            "implement".into(),
        ];
        assert!(fresh_entry_prompt(&phase).is_none());
        for action in [
            "--status",
            "--submit-checkpoint-receipt",
            "--close",
            "--abandon",
        ] {
            assert!(
                fresh_entry_prompt(&["pipeline".into(), prompt.clone(), action.into()]).is_none()
            );
        }
        let raw_spec = vec!["pipeline".into(), "task-spec.json".into()];
        assert!(fresh_entry_prompt(&raw_spec).is_none());
    }

    #[test]
    fn recovery_rejects_target_escape_before_creating_custody_state() {
        let (_temp, worktree, _) = linked_fixture();
        let outside = worktree.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let target_prompt = outside.join("entry.prompt.md");
        fs::write(&target_prompt, "outside prompt\n").unwrap();
        let error = recover_entry_worktree(&target_prompt, &worktree).unwrap_err();
        assert!(error.contains("outside the linked worktree"), "{error}");
        assert!(!worktree.join(".dev/pipeline").exists());
    }

    #[cfg(windows)]
    #[test]
    fn recovery_rejects_junction_escape_before_creating_custody_state() {
        let (_temp, worktree, _) = linked_fixture();
        let outside = worktree.parent().unwrap().join("outside");
        fs::create_dir_all(&outside).unwrap();
        let junction = worktree.join("redirected");
        let status = std::process::Command::new("cmd.exe")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .status()
            .unwrap();
        assert!(status.success(), "could not create fixture junction");

        let error =
            recover_entry_worktree(&junction.join("entry.prompt.md"), &worktree).unwrap_err();
        assert!(error.contains("parent resolves outside"), "{error}");
        assert!(!worktree.join(".dev/pipeline").exists());
    }
    use pipeline::task_spec::{extract_open_retry_handoff, RetryHandoffError};
    use tempfile::TempDir;

    const DATE: &str = "2026-10-18";
    fn fixture_prompt() -> String {
        format!(
            "# Plan Prompt: Demo\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {t}\n\n## Test Results\n\n### [{t}]\n\nVerdict: FAIL\n\n## Review Results\n",
            t = tid(1)
        )
    }

    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    fn count(text: &str, needle: &str) -> usize {
        text.matches(needle).count()
    }

    #[test]
    fn resume_phase_handler_maps_only_phases_resume_handles() {
        assert_eq!(
            resume_phase_handler("implement"),
            Some(ResumePhaseHandler::Implement)
        );
        assert_eq!(resume_phase_handler("fix"), Some(ResumePhaseHandler::Fix));
        assert_eq!(
            resume_phase_handler("test"),
            Some(ResumePhaseHandler::TestOrAudit)
        );
        assert_eq!(
            resume_phase_handler("audit"),
            Some(ResumePhaseHandler::TestOrAudit)
        );
        assert_eq!(
            resume_phase_handler("converge"),
            Some(ResumePhaseHandler::Converge)
        );
        for phase in [
            "",
            "Test",
            "fix ",
            "review",
            "goal-backward",
            "boundary-widening",
        ] {
            assert_eq!(
                resume_phase_handler(phase),
                None,
                "{phase:?} must fail closed"
            );
        }
    }

    #[test]
    fn handoff_opens_status_notes_when_missing_and_points_at_test_results() {
        let text = write_retry_handoff(
            &fixture_prompt(),
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "abc123",
        );

        let open = extract_open_retry_handoff(&text, &tid(1)).expect("one OPEN handoff");
        assert_eq!(open.origin_phase, "TEST");
        assert!(open
            .problem
            .contains("TEST verdict failed (test-verdict-failed)"));
        assert!(open
            .next_human_step
            .contains(&format!("`## Test Results` > `### [{}]`", tid(1))));
        assert!(open.next_human_step.contains("allowlist"));
        assert!(text.contains(&format!(
            "  - Test Results: `## Test Results` > `### [{}]`",
            tid(1)
        )));
        assert!(text.contains("  - Review Results: not-applicable"));
        assert!(text.contains("     - Commit: abc123"));
        assert_eq!(count(&text, "### Handoff Notes"), 1);
        let notes = text.find("### Handoff Notes").unwrap();
        let results = text.find("## Test Results").unwrap();
        assert!(notes < results, "handoff must sit under ## Status");
    }

    #[test]
    fn handoff_opens_under_existing_notes_and_drops_none_placeholder() {
        let prompt = &format!(
            "## Status\n\nWorkflow: IMPLEMENT\n\n### Handoff Notes\n\nNone.\n\n## Review Results\n\n### [{}]\n\nVerdict: REJECT\n",
            tid(1)
        );
        let text = write_retry_handoff(
            prompt,
            &tid(1),
            "AUDIT",
            "audit-verdict-failed",
            DATE,
            "def456",
        );

        assert!(!text.lines().any(|line| line.trim() == "None."));
        let open = extract_open_retry_handoff(&text, &tid(1)).expect("one OPEN handoff");
        assert_eq!(open.origin_phase, "AUDIT");
        assert!(open
            .next_human_step
            .contains(&format!("`## Review Results` > `### [{}]`", tid(1))));
        assert!(text.contains("  - Test Results: not-applicable"));
        assert!(text.contains(&format!(
            "  - Review Results: `## Review Results` > `### [{}]`",
            tid(1)
        )));
    }

    #[test]
    fn repeated_failure_of_same_origin_appends_attempt_without_new_block() {
        let first = write_retry_handoff(
            &fixture_prompt(),
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "abc123",
        );
        let second = write_retry_handoff(
            &first,
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "def456",
        );

        assert_eq!(
            count(&second, &format!("#### Retry Handoff — {} / TEST", tid(1))),
            1
        );
        assert!(second.contains("  1. 2026-10-18 — TEST verdict failed (test-verdict-failed)."));
        assert!(second.contains("  2. 2026-10-18 — TEST verdict failed (test-verdict-failed)."));
        assert!(second.contains("     - Commit: def456"));
        let open = extract_open_retry_handoff(&second, &tid(1)).expect("still one OPEN handoff");
        assert_eq!(open.origin_phase, "TEST");
        let next = second.find("- Next human step:").unwrap();
        let attempt_two = second.find("  2. 2026-10-18").unwrap();
        assert!(attempt_two < next, "attempt must precede Next human step");
    }

    #[test]
    fn passing_verdict_resolves_open_block_and_extraction_reports_not_found() {
        let opened = write_retry_handoff(
            &fixture_prompt(),
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "abc123",
        );
        let resolved = resolve_retry_handoff(
            &opened,
            &tid(1),
            "TEST",
            "Cleared by a passing TEST verdict.",
        );

        assert_eq!(
            extract_open_retry_handoff(&resolved, &tid(1)),
            Err(RetryHandoffError::NotFound { task_id: tid(1) })
        );
        assert_eq!(count(&resolved, "- Status: RESOLVED"), 1);
        assert!(resolved.contains("- Next human step: Cleared by a passing TEST verdict."));
        assert_eq!(
            count(
                &resolved,
                &format!("#### Retry Handoff — {} / TEST", tid(1))
            ),
            1,
            "history is kept"
        );
    }

    #[test]
    fn resolve_leaves_other_origin_and_missing_block_unchanged() {
        let opened = write_retry_handoff(
            &fixture_prompt(),
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "abc123",
        );
        assert_eq!(
            resolve_retry_handoff(&opened, &tid(1), "AUDIT", "Cleared."),
            opened,
            "AUDIT resolve must not touch the TEST block"
        );
        assert_eq!(
            resolve_retry_handoff(&fixture_prompt(), &tid(1), "TEST", "Cleared."),
            fixture_prompt(),
            "resolve without a block is a no-op"
        );
    }

    #[test]
    fn failure_after_resolved_block_opens_a_fresh_open_block() {
        let opened = write_retry_handoff(
            &fixture_prompt(),
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "abc123",
        );
        let resolved = resolve_retry_handoff(&opened, &tid(1), "TEST", "Cleared.");
        let reopened = write_retry_handoff(
            &resolved,
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "fed789",
        );

        assert_eq!(
            count(
                &reopened,
                &format!("#### Retry Handoff — {} / TEST", tid(1))
            ),
            2
        );
        assert_eq!(count(&reopened, "- Status: OPEN"), 1);
        assert_eq!(count(&reopened, "- Status: RESOLVED"), 1);
        assert!(extract_open_retry_handoff(&reopened, &tid(1)).is_ok());
    }

    #[test]
    fn crlf_prompt_keeps_crlf_line_endings() {
        let prompt = fixture_prompt().replace('\n', "\r\n");
        let text = write_retry_handoff(
            &prompt,
            &tid(1),
            "TEST",
            "test-verdict-failed",
            DATE,
            "abc123",
        );

        assert!(extract_open_retry_handoff(&text, &tid(1)).is_ok());
        assert!(
            !text.replace("\r\n", "").contains('\n'),
            "no bare LF may remain"
        );
    }

    fn guarded_placement_manifest(root: &Path) -> dispatch::host_attempt_state::HostAttemptState {
        dispatch::host_attempt_state::HostAttemptState {
            attempt_id: "host_test".into(),
            worktree: root.to_path_buf(),
            prompt: root.join("prompt.md"),
            task: tid(1),
            phase: "test".into(),
            profile: "codex_stop_v1".into(),
            run_id: "run-1".into(),
            host: "codex".into(),
            mode: "host-subagent".into(),
            spec_path: root.join("attempt.spec.md"),
            spec_sha256: "0".repeat(64),
            receipt_path: Some(root.join("receipt.md")),
            log_path: root.join("attempt.log.md"),
            snapshot_path: root.join("snapshot.bak"),
            started_at: "2026-10-23T00:00:00.000Z".into(),
            model: String::new(),
            effort: None,
            tier: None,
            tier_source: None,
            override_reason: None,
            policy_applied: false,
            prerequisite_refs: Vec::new(),
            failure_history: Vec::new(),
            observed_model: None,
            observed_effort: None,
            prompt_sha256: None,
            receipt_sha256: None,
            log_sha256: None,
            snapshot_sha256: None,
            worktree_status_sha256: None,
            worktree_diff_sha256: None,
        }
    }

    #[test]
    fn guarded_placement_render_validation_is_rejected_before_write() {
        let root = TempDir::new().unwrap();
        let manifest = guarded_placement_manifest(root.path());
        let mut repair = None;
        let mut placement = guarded_host_placement(&manifest, b"not a task prompt", &mut repair);
        let error = placement(b"invalid receipt").unwrap_err();

        assert_eq!(
            error.outcome,
            dispatch::run::HostAttemptPlacementOutcome::RejectedBeforeWrite
        );
        assert!(!manifest.prompt.exists());
    }

    #[test]
    fn guarded_placement_prompt_write_failure_is_may_have_written() {
        let root = TempDir::new().unwrap();
        let manifest = guarded_placement_manifest(root.path());
        std::fs::create_dir_all(&manifest.prompt).unwrap();
        let snapshot = b"# Plan\n\n## Test Results\n\n## Review Results\n";
        let receipt = format!(
            "### [{}] 2026-10-23\n\n**Verdict:** PASS\n**Evidence:** test evidence\n",
            tid(1)
        );
        let mut repair = None;
        let mut placement = guarded_host_placement(&manifest, snapshot, &mut repair);
        let error = placement(receipt.as_bytes()).unwrap_err();

        assert_eq!(
            error.outcome,
            dispatch::run::HostAttemptPlacementOutcome::MayHaveWritten
        );
        assert!(!error.cause.is_empty());
        assert!(manifest.prompt.is_dir());
    }

    #[test]
    fn guarded_host_close_places_derived_payload_from_the_sealed_snapshot() {
        let root = TempDir::new().unwrap();
        let task = tid(11);
        let snapshot = format!(
            "# Plan\n\n## Tasks\n\n- [ ] {task} — guarded close probe\n\n## Test Results\n\n## Review Results\n"
        );
        let mut manifest = guarded_placement_manifest(root.path());
        manifest.task = task.clone();
        manifest.phase = "test".into();
        manifest.snapshot_sha256 = Some(digest(snapshot.as_bytes()));
        let original = format!(
            "\u{feff}### [{task}] 2026-10-27\n\nCommand: guarded tester\nResult: PASS\n**Verdict:** PASS\n"
        );
        let original_digest = digest(original.as_bytes());
        let mut repair = Some(super::super::format_repair::RepairInvocation {
            invocation_id: "guarded-probe".into(),
            next_round: 1,
        });
        let mut placement = guarded_host_placement(&manifest, snapshot.as_bytes(), &mut repair);
        let result = placement(original.as_bytes());
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(digest(original.as_bytes()), original_digest);
        let written = std::fs::read(&manifest.prompt).unwrap();
        assert!(String::from_utf8_lossy(&written).contains("Evidence:"));
        drop(placement);

        let mut missing_evidence =
            guarded_host_placement(&manifest, snapshot.as_bytes(), &mut repair);
        let error =
            missing_evidence(b"Command: guarded tester\nEvidence: result missing\n").unwrap_err();
        assert_eq!(
            error.outcome,
            dispatch::run::HostAttemptPlacementOutcome::RejectedBeforeWrite
        );
    }
}
