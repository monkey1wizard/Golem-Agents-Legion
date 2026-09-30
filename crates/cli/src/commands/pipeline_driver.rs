//! Guarded Codex pipeline entry and preflight latch.

use gal_engine::ExitCode;
use pipeline::coordinator::{
    AttemptOwnership, CheckpointKind, CheckpointReceipt, ContinuationProfile, CoordinatorState,
    NextAction, OwnerObservation,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_RECORD: u64 = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContinuationAction {
    Entry,
    Status,
    SubmitCheckpointReceipt,
    Resume,
}

#[derive(Debug)]
struct EntryArgs {
    prompt: PathBuf,
    phase: String,
    task: Option<String>,
    action: ContinuationAction,
}

fn parse(args: &[String]) -> Result<EntryArgs, String> {
    let mut prompt = None;
    let mut phase = "implement".to_string();
    let mut phase_supplied = false;
    let mut task = None;
    let mut action = None;
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
    })
}

fn continuation(args: &EntryArgs, root: &Path) -> Option<ExitCode> {
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
            let prompt_path = root
                .parent()
                .and_then(|_| std::env::current_dir().ok())
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
                None => return Some(fail("task-quality receipt prompt unavailable")),
            };
            let convention = Path::new("plugins/gal-core/conventions/task-quality.md");
            if !convention.is_file()
                || pipeline::task_spec::extract_task_goal(&prompt_text, &state.task_id).is_err()
            {
                return Some(fail("task-quality receipt validation failed"));
            }
            let spec =
                match pipeline::task_spec::assemble_task_spec(&pipeline::task_spec::TaskSpecInput {
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
                    Err(error) => {
                        return Some(fail(format!("task-quality task spec invalid: {error}")));
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
            if state
                .checkpoint
                .as_ref()
                .is_none_or(|c| c.kind != CheckpointKind::TaskQuality)
                || receipt.commit.as_deref() != Some(head.as_str())
                || spec.markdown.is_empty()
            {
                return Some(fail(
                    "checkpoint receipt task-quality or commit binding mismatch",
                ));
            }
            if let Err(error) = state.consume_checkpoint_receipt(receipt) {
                return Some(fail(error.to_string()));
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
        ContinuationAction::Resume => {
            if state.active_attempt.is_some() {
                return Some(fail("active attempt exists; refusing provider start"));
            }
            let journal = root.join("projection-journal.json");
            if journal.exists() && pipeline::projection_journal::recover(&journal).is_err() {
                return Some(fail("pending projection journal recovery failed"));
            }
            let prompt = std::env::current_dir().ok().map(|cwd| {
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
            if args.phase == "implement" {
                let task = args.task.as_deref().unwrap_or_default();
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
                    return Some(fail("implement attempt claim failed"));
                }
                if let Err(error) = atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&state).unwrap_or_default(),
                ) {
                    return Some(fail(format!("implement claim persistence failed: {error}")));
                }
                if state
                    .bind_process(
                        std::process::id(),
                        dispatch::dispatch::process_birth_identity(std::process::id()),
                    )
                    .is_err()
                {
                    return Some(fail("implement attempt bind failed"));
                }
                if let Err(error) = atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&state).unwrap_or_default(),
                ) {
                    return Some(fail(format!("implement bind persistence failed: {error}")));
                }
                let dispatch_args = vec![
                    "pipeline".into(),
                    prompt.to_string_lossy().into_owned(),
                    "--phase".into(),
                    "implement".into(),
                    "--task".into(),
                    task.to_string(),
                ];
                let dispatched = super::dispatch::cmd_pipeline(&dispatch_args);
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
                            "implement-dispatch:{dispatched:?};boundary:{}",
                            if passed { "pass" } else { "fail" }
                        )),
                    )
                    .is_err()
                {
                    return Some(fail("implement attempt finish failed"));
                }
                if passed {
                    state.phase = "test".into();
                    state.next_action = NextAction::StartTask {
                        task_id: task.to_string(),
                        phase: "test".into(),
                    };
                } else {
                    if state
                        .checkpoint(
                            format!("boundary-widening-{attempt_id}"),
                            CheckpointKind::BoundaryWidening,
                            None,
                        )
                        .is_err()
                    {
                        return Some(fail("boundary widening checkpoint failed"));
                    }
                }
                if let Err(error) = atomic_write(
                    &path,
                    &serde_json::to_vec_pretty(&state).unwrap_or_default(),
                ) {
                    return Some(fail(format!(
                        "implement terminal persistence failed: {error}"
                    )));
                }
                println!("{}", state.bounded_status());
                return Some(if dispatched == ExitCode::Success {
                    ExitCode::Success
                } else {
                    dispatched
                });
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
                let repo = match std::env::current_dir() {
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
                state.retry.max_attempts = state.retry.max_attempts.max(3);
                if state.retry.max_attempts > 0 && state.retry.attempt >= state.retry.max_attempts {
                    state.next_action = NextAction::Blocked {
                        reason: "retry-limit-exhausted".into(),
                    };
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
                    let result =
                        match guarded_phase_dispatch(&mut state, &path, &prompt, task, phase) {
                            Ok(result) => result,
                            Err(error) => return Some(fail(error)),
                        };
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
            println!("{}", state.bounded_status());
            Some(ExitCode::Success)
        }
        ContinuationAction::Entry => None,
    }
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

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
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
    Some(lines[task_start..task_end].join("\n").to_ascii_lowercase())
}

fn guarded_phase_dispatch(
    state: &mut CoordinatorState,
    path: &Path,
    prompt: &Path,
    task: &str,
    phase: &str,
) -> Result<ExitCode, String> {
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
    let dispatch_args = vec![
        "pipeline".into(),
        prompt.to_string_lossy().into_owned(),
        "--phase".into(),
        phase.to_string(),
        "--task".into(),
        task.to_string(),
    ];
    let dispatched = super::dispatch::cmd_pipeline(&dispatch_args);
    let result_section = if phase == "test" {
        "Test Results"
    } else {
        "Review Results"
    };
    let text = fs::read_to_string(prompt)
        .map_err(|e| format!("{phase} result prompt unavailable: {e}"))?;
    let result = task_result(&text, result_section, task).unwrap_or_default();
    let dangerous =
        phase == "audit" && (result.contains("security") || result.contains("protected path"));
    let passed = dispatched == ExitCode::Success
        && if phase == "test" {
            result.contains("pass") && !result.contains("fail")
        } else {
            (result.contains("approve") || result.contains("pass"))
                && !result.contains("blocking")
                && !result.contains("reject")
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
    if dangerous {
        state.next_action = NextAction::Blocked {
            reason: "security-or-protected-path-finding".into(),
        };
    } else if passed {
        state.last_verified_evidence = Some(format!("{phase}-verdict:pass"));
    } else {
        state.record_retry(format!("{phase}-verdict-failed"));
    }
    atomic_write(path, &serde_json::to_vec_pretty(state).unwrap_or_default())
        .map_err(|e| format!("{phase} terminal persistence failed: {e}"))?;
    Ok(dispatched)
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

pub(crate) fn cmd_guarded_pipeline_entry(args: &[String]) -> ExitCode {
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
    let worktree_canon = match std::env::current_dir().and_then(fs::canonicalize) {
        Ok(p) => p,
        Err(e) => return fail(format!("worktree unavailable: {e}")),
    };
    let worktree = worktree_canon.to_string_lossy().replace('\\', "/");
    let scope = match safe_scope(&canonical_prompt) {
        Ok(s) => s,
        Err(e) => return fail(e),
    };
    let root = worktree_canon.join(".dev/pipeline").join(&scope);
    if let Some(exit) = continuation(&parsed, &root) {
        return exit;
    }
    let coordinator_path = root.join("coordinator.json");
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
        }
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
    state.next_action = NextAction::Blocked {
        reason: "entry-latch-pending".into(),
    };
    let coordinator_path = root.join("coordinator.json");
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
    let current_exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return fail(format!("executable resolution failed: {e}")),
    };
    let running_hash = match fs::read(&current_exe) {
        Ok(b) => digest(&b),
        Err(e) => return fail(format!("executable hash failed: {e}")),
    };
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
    if modified.is_none_or(|m| m < before) || !receipt.lines().any(|l| l.trim() == "overall: pass")
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
