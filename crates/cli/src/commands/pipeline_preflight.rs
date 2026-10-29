//! `gal pipeline-preflight` — pipeline-internal Step 1 prereq gate.
//!
//! Checks: `## Tasks` ≥1 well-formed T-NN, `## Test Plan` present, no root
//! BLOCKING OQ, `Workflow` ≠ DONE; reads resume cursor (`Current Task` /
//! open `Interrupted Phase`); wrong-plan guard against `.dev/state.md`.
//!
//! Path classification reuses `dispatch::resolve_pipeline_input` — no
//! reimplementation of the classification logic.
//!
//! Not a public `/gal` slash command (peer of `finalize-check`/`boundary-check`).

use super::dispatch::{resolve_pipeline_input, resolve_receipt_path, PipelineInput};
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use dispatch::host_session::{locate_claude_session, locate_codex, HostKind, HostSessionRef};
use dispatch::host_usage::HostRoots;
use gal_engine::ExitCode;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// --- arg parsing ---

#[derive(Debug)]
struct Args {
    prompt: PathBuf,
    receipt: PathBuf,
    terminal_reverify: bool,
    host: Option<String>,
    dispatch_mode: DispatchMode,
    host_session: Option<String>,
}

/// Orchestrator binding that a preflight receipt records. A bound record names the host
/// session, the window start, and the run identity that `gal usage collect` reads. An
/// unavailable record names the reason no session is bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OrchestratorRecord {
    Bound(OrchestratorBinding),
    Unavailable { reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrchestratorBinding {
    /// Host session reference such as `codex:<session>`.
    pub host_session: String,
    /// Window start as an epoch-second reference that the session verifier reads.
    pub window_started_at: String,
    pub canonical_worktree: String,
    pub plan_scope: String,
}

impl OrchestratorRecord {
    /// Trailing receipt section. It sits after the check table, so the pass or fail
    /// result above it never depends on the binding.
    fn render(&self) -> String {
        match self {
            Self::Bound(binding) => format!(
                "\n## Orchestrator\n\norchestrator: bound\norchestrator_session: {}\norchestrator_window_started_at: {}\norchestrator_worktree: {}\norchestrator_plan_scope: {}\n",
                binding.host_session,
                binding.window_started_at,
                binding.canonical_worktree,
                binding.plan_scope,
            ),
            Self::Unavailable { reason } => format!(
                "\n## Orchestrator\n\norchestrator: unavailable\norchestrator_reason: {}\n",
                reason.replace(['\r', '\n'], " "),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DispatchMode {
    Routed,
    HostSubagent,
}

impl DispatchMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Routed => "routed",
            Self::HostSubagent => "host-subagent",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PreflightPin {
    pub host: Option<String>,
    pub dispatch_mode: DispatchMode,
    pub run_id: String,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum PinError {
    Missing,
    Unparseable,
}

pub(crate) fn read_pin(prompt: &Path) -> Result<PreflightPin, PinError> {
    let path = resolve_receipt_path(Some(prompt), None, "preflight.receipt.md")
        .map_err(|_| PinError::Unparseable)?;
    let text = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            PinError::Missing
        } else {
            PinError::Unparseable
        }
    })?;
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(&format!("{name}: ")))
    };
    let host = match field("host") {
        Some("none") => None,
        Some(v @ ("claude" | "codex")) => Some(v.to_string()),
        _ => return Err(PinError::Unparseable),
    };
    let dispatch_mode = match field("dispatch_mode") {
        Some("routed") => DispatchMode::Routed,
        Some("host-subagent") => DispatchMode::HostSubagent,
        _ => return Err(PinError::Unparseable),
    };
    let run_id = field("run_id")
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .ok_or(PinError::Unparseable)?
        .to_string();
    if dispatch_mode == DispatchMode::HostSubagent && host.is_none() {
        return Err(PinError::Unparseable);
    }
    Ok(PreflightPin {
        host,
        dispatch_mode,
        run_id,
    })
}

fn new_run_id() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("{seconds}-{}", std::process::id())
}

fn unresolved_host_attempt(prompt: &Path) -> Result<Option<String>, String> {
    let scope = resolve_receipt_path(Some(prompt), None, "preflight.receipt.md")
        .map_err(|error| format!("receipt scope resolution: {error}"))?;
    let attempts = scope
        .parent()
        .unwrap_or(Path::new("."))
        .join("host-attempts");
    unresolved_host_attempt_in(&attempts)
}

fn unresolved_host_attempt_in(attempts: &Path) -> Result<Option<String>, String> {
    let entries = match std::fs::read_dir(&attempts) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read host attempts: {error}")),
    };
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read host attempt entry: {error}"))?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(&path)
                .map_err(|error| format!("cannot read host attempt manifest: {error}"))?,
        )
        .map_err(|error| format!("cannot parse host attempt manifest: {error}"))?;
        let attempt_id = manifest
            .get("attempt_id")
            .and_then(serde_json::Value::as_str)
            .or_else(|| path.file_stem().and_then(|stem| stem.to_str()))
            .unwrap_or("unknown");
        let log = manifest
            .get("log_path")
            .and_then(serde_json::Value::as_str)
            .map(|log| {
                let log = PathBuf::from(log);
                if log.is_absolute() {
                    log
                } else {
                    path.parent().unwrap_or(Path::new(".")).join(log)
                }
            })
            .unwrap_or_else(|| path.with_extension("log"));
        if std::fs::read_to_string(&log).is_ok_and(|text| {
            text.lines()
                .any(|line| line.trim() == "terminal_state:  started")
        }) {
            return Ok(Some(attempt_id.to_string()));
        }
    }
    Ok(None)
}

fn matching_resume_pin(prompt: &Path) -> Option<PreflightPin> {
    let pin = read_pin(prompt).ok()?;
    let receipt = resolve_receipt_path(Some(prompt), None, "preflight.receipt.md").ok()?;
    let coordinator = receipt.parent()?.join("coordinator.json");
    let coordinator: serde_json::Value =
        serde_json::from_slice(&std::fs::read(coordinator).ok()?).ok()?;
    (coordinator
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        == Some(pin.run_id.as_str()))
    .then_some(pin)
}

fn default_receipt_path(prompt: &Path) -> Result<PathBuf, String> {
    resolve_receipt_path(Some(prompt), None, "preflight.receipt.md")
        .map_err(|error| format!("receipt scope resolution: {error}"))
}

fn terminal_reverify_receipt_path(prompt: &Path) -> Result<PathBuf, String> {
    resolve_receipt_path(Some(prompt), None, "terminal-reverify.receipt.md")
        .map_err(|error| format!("receipt scope resolution: {error}"))
}

/// Binds the named orchestrator session at preflight time. Only the session the caller names is
/// checked, so preflight never selects a conversation by itself. Every failure becomes an
/// unavailable record with its reason.
///
/// Preflight has no attempt yet, so this runs the session's record check: the record exists, its
/// working directory matches, and it is unambiguous. The attempt-token and tool-use checks wait
/// for the attempt that the orchestrator later launches.
fn bind_orchestrator(
    host: Option<&str>,
    host_session: Option<&str>,
    repo_root: &Path,
    plan_scope: &str,
    started_seconds: u64,
) -> OrchestratorRecord {
    let unavailable = |reason: String| OrchestratorRecord::Unavailable { reason };
    let Some(session) = host_session else {
        return unavailable("no --host-session was given".to_owned());
    };
    let Some(host) = host else {
        return unavailable(
            "no host is pinned, so the host session has no host to verify against".to_owned(),
        );
    };
    let reference = match HostSessionRef::parse(session) {
        Ok(reference) => reference,
        Err(error) => return unavailable(format!("host session `{session}` is invalid: {error}")),
    };
    if reference.host.as_str() != host {
        return unavailable(format!(
            "host session `{session}` is a {} session, but the pinned host is {host}",
            reference.host.as_str()
        ));
    }
    let Some(home) = dirs::home_dir() else {
        return unavailable("the home directory is unknown".to_owned());
    };
    let window_started_at = format!("epoch+{started_seconds}s");
    let located = match reference.host {
        HostKind::Claude => {
            locate_claude_session(&home, repo_root, &reference.reference).map(|_| ())
        }
        HostKind::Codex => locate_codex(
            &HostRoots::from_env(&home).codex_root,
            repo_root,
            &reference.reference,
            &window_started_at,
        )
        .map(|_| ()),
    };
    if let Err(error) = located {
        return unavailable(format!("host session `{session}` did not verify: {error}"));
    }
    OrchestratorRecord::Bound(OrchestratorBinding {
        host_session: session.to_owned(),
        window_started_at,
        canonical_worktree: std::fs::canonicalize(repo_root)
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|_| repo_root.to_string_lossy().into_owned()),
        plan_scope: plan_scope.to_owned(),
    })
}

/// Plan scope of a prompt, matching the plan id that usage capture derives from the same path.
fn plan_scope_of(prompt: &Path) -> String {
    let name = prompt
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    name.strip_suffix(".prompt.md")
        .or_else(|| name.strip_suffix(".md"))
        .unwrap_or(name)
        .to_owned()
}

fn unix_seconds_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Reads the orchestrator record of the one preflight receipt that names `run_id`. The search
/// covers every plan scope under the pipeline root. A run with no receipt returns `None`.
pub(crate) fn read_orchestrator_record(run_id: &str) -> Result<Option<OrchestratorRecord>, String> {
    let root = Path::new(dispatch::dispatch::PIPELINE_ROOT);
    let entries = match std::fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("cannot read {}: {error}", root.display())),
    };
    let wanted = format!("run_id: {run_id}");
    let mut matching = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| format!("cannot read pipeline entry: {error}"))?;
        let Ok(text) = std::fs::read_to_string(entry.path().join("preflight.receipt.md")) else {
            continue;
        };
        if text.lines().any(|line| line == wanted) {
            matching.push(text);
        }
    }
    match matching.as_slice() {
        [] => Ok(None),
        [text] => Ok(Some(parse_orchestrator_record(text))),
        _ => Err(format!("run `{run_id}` names several preflight receipts")),
    }
}

/// Parses the trailing orchestrator section of a receipt. A receipt written before binding
/// existed, or one with an incomplete section, reads as unavailable with its reason.
fn parse_orchestrator_record(text: &str) -> OrchestratorRecord {
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(&format!("{name}: ")))
    };
    match field("orchestrator") {
        Some("bound") => match (
            field("orchestrator_session"),
            field("orchestrator_window_started_at"),
            field("orchestrator_worktree"),
            field("orchestrator_plan_scope"),
        ) {
            (Some(session), Some(started), Some(worktree), Some(scope)) => {
                OrchestratorRecord::Bound(OrchestratorBinding {
                    host_session: session.to_owned(),
                    window_started_at: started.to_owned(),
                    canonical_worktree: worktree.to_owned(),
                    plan_scope: scope.to_owned(),
                })
            }
            _ => OrchestratorRecord::Unavailable {
                reason: "the recorded orchestrator binding is incomplete".to_owned(),
            },
        },
        Some("unavailable") => OrchestratorRecord::Unavailable {
            reason: field("orchestrator_reason")
                .unwrap_or("the recorded reason is missing")
                .to_owned(),
        },
        _ => OrchestratorRecord::Unavailable {
            reason: "the preflight receipt predates orchestrator binding".to_owned(),
        },
    }
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut terminal_reverify = false;
    let mut host = None;
    let mut dispatch_mode = DispatchMode::Routed;
    let mut host_session: Option<String> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--terminal-reverify" => {
                if terminal_reverify {
                    return Err("duplicate --terminal-reverify".to_string());
                }
                terminal_reverify = true;
            }
            "--receipt" => {
                let v = it.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(v));
            }
            "--host" => {
                let v = it.next().ok_or("--host requires claude or codex")?;
                if !["claude", "codex"].contains(&v.as_str()) {
                    return Err("--host must be claude or codex".into());
                }
                host = Some(v.clone());
            }
            "--dispatch" => {
                let v = it
                    .next()
                    .ok_or("--dispatch requires routed or host-subagent")?;
                dispatch_mode = match v.as_str() {
                    "routed" => DispatchMode::Routed,
                    "host-subagent" => DispatchMode::HostSubagent,
                    _ => return Err("--dispatch must be routed or host-subagent".into()),
                };
            }
            "--host-session" => {
                let v = it
                    .next()
                    .ok_or("--host-session requires a host session reference")?;
                host_session = Some(v.clone());
            }
            s if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            s => {
                if prompt.is_none() {
                    prompt = Some(PathBuf::from(s));
                } else {
                    return Err(format!("unexpected extra argument '{s}'"));
                }
            }
        }
    }
    let prompt = prompt.ok_or("pipeline-preflight requires a <prompt> path")?;
    let prompt_text = prompt.to_string_lossy();
    let prompt_text = prompt_text
        .strip_prefix("#file:")
        .or_else(|| prompt_text.strip_prefix('@'))
        .unwrap_or(&prompt_text);
    if prompt_text.is_empty() {
        return Err("pipeline-preflight requires a non-empty <prompt> path".to_string());
    }
    let prompt = PathBuf::from(prompt_text);
    if terminal_reverify && receipt.is_some() {
        return Err("--terminal-reverify is prompt-only and does not accept --receipt".to_string());
    }
    if terminal_reverify && host_session.is_some() {
        return Err("--host-session is not accepted with --terminal-reverify".to_string());
    }
    if dispatch_mode == DispatchMode::HostSubagent && host.is_none() {
        return Err("--dispatch host-subagent requires --host".into());
    }
    if host.is_some() && receipt.is_some() {
        return Err("host-pinned runs require the canonical preflight receipt path".into());
    }
    let receipt = match receipt {
        Some(path) => path,
        None if terminal_reverify => terminal_reverify_receipt_path(&prompt)?,
        None => default_receipt_path(&prompt)?,
    };
    Ok(Args {
        prompt,
        receipt,
        terminal_reverify,
        host,
        dispatch_mode,
        host_session,
    })
}

/// Return a prompt eligible for linked-worktree recovery. Receipt-pinned and
/// terminal reverify invocations keep their existing executable binding.
pub(crate) fn fresh_prompt_entry(args: &[String]) -> Option<PathBuf> {
    let parsed = parse_args(args.get(1..)?).ok()?;
    if parsed.terminal_reverify
        || parsed.host.is_some()
        || parsed.host_session.is_some()
        || preflight_pin_exists(&parsed.prompt)
    {
        return None;
    }
    (resolve_pipeline_input(&parsed.prompt) == PipelineInput::Prompt).then_some(parsed.prompt)
}

pub(crate) fn preflight_pin_exists(prompt: &Path) -> bool {
    read_pin(prompt).is_ok()
        || resolve_receipt_path(Some(prompt), None, "preflight.receipt.md")
            .map(|path| path.exists())
            .unwrap_or(true)
}

fn post_recovery_checks(repo_root: &Path, prompt: &Path) -> Vec<CheckOutcome> {
    let canonical_root = std::fs::canonicalize(repo_root).ok();
    let status = std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["status", "--porcelain", "--untracked-files=all"])
        .output();
    let clean = status
        .as_ref()
        .is_ok_and(|output| output.status.success() && output.stdout.is_empty());
    let status_summary = match status {
        Ok(output) if output.status.success() && output.stdout.is_empty() => {
            "worktree clean".to_owned()
        }
        Ok(output) if output.status.success() => format!(
            "worktree has changes: {}",
            String::from_utf8_lossy(&output.stdout).trim()
        ),
        Ok(output) => format!(
            "git status failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => format!("git status unavailable: {error}"),
    };
    let canonical_prompt = std::fs::canonicalize(prompt).ok();
    let tracked = canonical_prompt
        .as_deref()
        .and_then(|path| {
            canonical_root
                .as_deref()
                .and_then(|root| path.strip_prefix(root).ok())
        })
        .is_some_and(|relative| {
            let spec = format!("HEAD:{}", relative.to_string_lossy().replace('\\', "/"));
            std::process::Command::new("git")
                .current_dir(canonical_root.as_deref().unwrap_or(repo_root))
                .args(["cat-file", "-e", &spec])
                .status()
                .is_ok_and(|status| status.success())
        });
    vec![
        CheckOutcome {
            name: "entry-recovery-worktree-clean".into(),
            command: Some("git status --porcelain --untracked-files=all".into()),
            state: if clean {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            summary: status_summary,
        },
        CheckOutcome {
            name: "entry-recovery-prompt-tracked".into(),
            command: Some("git cat-file -e HEAD:<prompt>".into()),
            state: if tracked {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            summary: if tracked {
                "prompt is tracked at post-recovery HEAD".into()
            } else {
                "prompt is absent from post-recovery HEAD or path binding failed".into()
            },
        },
    ]
}

// --- checks ---

fn render_route_line(
    table: &dispatch::routing::RoutingTable,
    host: Option<&str>,
    dispatch_mode: DispatchMode,
) -> String {
    let suffix = if host.is_some() || dispatch_mode == DispatchMode::HostSubagent {
        format!(
            "; host: {}; dispatch_mode: {}",
            host.unwrap_or("none"),
            dispatch_mode.as_str()
        )
    } else {
        String::new()
    };
    if table.entries.is_empty() {
        return format!("(no executorRouting configured){suffix}");
    }

    ["CODER", "TESTER", "AUDITOR"]
        .iter()
        .map(|role| {
            let label = role.to_ascii_lowercase();
            match table.get(role) {
                Some(entry) => {
                    let effort = entry
                        .effort
                        .as_deref()
                        .filter(|effort| !effort.is_empty())
                        .unwrap_or("(default)");
                    let remote = if entry.is_remote() {
                        entry
                            .ssh_target
                            .as_deref()
                            .map(|target| format!(" ssh:{target}"))
                            .unwrap_or_default()
                    } else {
                        String::new()
                    };
                    format!(
                        "{label}: {}: {} {effort}{remote}",
                        entry.executor, entry.model
                    )
                }
                None => format!("{label}: (unrouted)"),
            }
        })
        .collect::<Vec<_>>()
        .join("; ")
        + &suffix
}

pub(crate) fn is_malformed_task_bullet(line: &str) -> bool {
    let trimmed = line.trim_start();
    ["- [ ] ", "- [x] ", "- [X] "].iter().any(|prefix| {
        trimmed
            .strip_prefix(prefix)
            .is_some_and(|task| task.starts_with("T-") || task.starts_with("**T-"))
    })
}

/// Return whether the `## Tasks` section contains a valid task and any malformed
/// task-looking bullets outside fenced blocks.
fn task_bullet_status(prompt_text: &str) -> (bool, Vec<String>) {
    let lines: Vec<_> = prompt_text.lines().collect();
    let Some(start) = lines.iter().position(|line| line.trim() == "## Tasks") else {
        return (false, Vec::new());
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("## "))
        .map(|offset| start + 1 + offset)
        .unwrap_or(lines.len());
    let mut fence: Option<(char, usize)> = None;
    let mut has_valid_task = false;
    let mut malformed = Vec::new();
    for line in &lines[start + 1..end] {
        let trimmed = line.trim_start();
        let marker = trimmed.chars().next();
        if let Some(ch) = marker.filter(|ch| *ch == '`' || *ch == '~') {
            let count = trimmed.chars().take_while(|current| current == &ch).count();
            if count >= 3 {
                match fence {
                    None => fence = Some((ch, count)),
                    Some((open, length)) if open == ch && count >= length => fence = None,
                    _ => {}
                }
                continue;
            }
        }
        if fence.is_none() {
            if pipeline::task_spec::parse_plan_task_bullet(line).is_some() {
                has_valid_task = true;
            } else if is_malformed_task_bullet(line) {
                malformed.push(trimmed.to_string());
            }
        }
    }
    (has_valid_task, malformed)
}

/// check: `## Tasks` section has ≥1 well-formed T-NN bullet.
pub(crate) fn check_tasks_well_formed(prompt_text: &str) -> CheckOutcome {
    let name = "tasks-well-formed".to_string();
    let (has_valid_task, malformed) = task_bullet_status(prompt_text);
    if !malformed.is_empty() {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("malformed task bullets: [{}]", malformed.join(" | ")),
        }
    } else if has_valid_task {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "## Tasks contains ≥1 well-formed T-NN bullet".to_string(),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "## Tasks has no well-formed `- [ ] T-NN` / `- [x] T-NN` bullet".to_string(),
        }
    }
}

/// check: `## Test Plan` section is present.
fn check_test_plan_present(prompt_text: &str) -> CheckOutcome {
    let name = "test-plan-present".to_string();
    if prompt_text.contains("## Test Plan") {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "## Test Plan section present".to_string(),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "## Test Plan section missing".to_string(),
        }
    }
}

pub(crate) fn check_retry_handoff_shape(prompt_text: &str) -> CheckOutcome {
    use super::converge_check::{current_task_value, CurrentTaskValue};
    let task = match current_task_value(prompt_text) {
        CurrentTaskValue::Active(task) => task,
        _ => {
            return CheckOutcome {
                name: "retry-handoff-shape".into(),
                command: None,
                state: CheckState::Pass,
                summary: "no active task Retry Handoff authority to parse".into(),
            };
        }
    };
    match pipeline::task_spec::extract_open_retry_handoff(prompt_text, &task) {
        Ok(_) => CheckOutcome {
            name: "retry-handoff-shape".into(),
            command: None,
            state: CheckState::Pass,
            summary: "active Retry Handoff authority has the required shape".into(),
        },
        Err(pipeline::task_spec::RetryHandoffError::NotFound { .. }) => CheckOutcome {
            name: "retry-handoff-shape".into(),
            command: None,
            state: CheckState::Pass,
            summary: "no active Retry Handoff block".into(),
        },
        Err(error) => CheckOutcome {
            name: "retry-handoff-shape".into(),
            command: None,
            state: CheckState::Fail,
            summary: format!("Retry Handoff authority is malformed: {error}"),
        },
    }
}

/// check: prompt does not carry the retired test-first contract marker.
fn check_retired_marker(prompt_text: &str) -> CheckOutcome {
    let name = "retired-marker".to_string();
    if prompt_text
        .lines()
        .any(|line| line.trim() == "Pipeline Contract: test-first-v1")
    {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "prompt carries a retired contract marker, regenerate with /plan-to-prompt"
                .to_string(),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "prompt does not carry a retired contract marker".to_string(),
        }
    }
}

/// Returns whether `line` contains a non-negated `BLOCKING` marker.
///
/// Strips the contract-recognized negated forms (`no`/`not`/`zero`/`without`/
/// `non-`/`no-` immediately preceding `BLOCKING`, case-insensitive on the
/// negation word only — `BLOCKING` itself stays upper-case since it is the
/// marker token) before testing whether an upper-case `BLOCKING` token
/// survives. A bare `BLOCK` is never a trigger.
fn is_active_blocker(line: &str) -> bool {
    const NEGATIONS: &[&str] = &["no", "not", "zero", "without", "non-", "no-"];
    let mut residue = line.to_string();
    for neg in NEGATIONS {
        let pattern_upper = format!("{neg} BLOCKING");
        let pattern_hyphen = format!("{neg}BLOCKING");
        for pattern in [pattern_upper.as_str(), pattern_hyphen.as_str()] {
            while let Some(idx) = find_ascii_case_insensitive(&residue, pattern) {
                residue.replace_range(idx..idx + pattern.len(), "");
            }
        }
    }
    residue.contains("BLOCKING")
}

/// Byte offset of the first ASCII-case-insensitive match of `needle` in `haystack`.
///
/// Matching stays on `haystack`'s own byte indices instead of searching a
/// lowercased copy: `str::to_lowercase` can change a string's byte length
/// (`İ` U+0130 grows 2→3, `ẞ` U+1E9E shrinks 3→2), so an index taken from the
/// lowercased copy can miss the original's char boundaries. `needle` is ASCII,
/// and a multi-byte UTF-8 char has no ASCII byte in it, so every match found
/// here is char-boundary-aligned and safe to `replace_range`.
fn find_ascii_case_insensitive(haystack: &str, needle: &str) -> Option<usize> {
    let (h, n) = (haystack.as_bytes(), needle.as_bytes());
    if n.is_empty() || h.len() < n.len() {
        return None;
    }
    (0..=h.len() - n.len()).find(|&i| h[i..i + n.len()].eq_ignore_ascii_case(n))
}

/// check: no root-level BLOCKING open question.
///
/// Scans for lines that contain a non-negated `BLOCKING` marker outside of
/// completed-task blocks, within `## Open Questions` or `## Review Results`
/// sections only (`pipeline_preflight.rs` doc comment above is the contract;
/// other sections such as `## Tasks` or `### Handoff Notes` are never
/// scanned).
fn check_no_root_blocking(prompt_text: &str) -> CheckOutcome {
    let name = "no-root-blocking".to_string();
    let mut in_scanned_section = false;
    let blocking_found = prompt_text.lines().any(|l| {
        let t = l.trim();
        if t.starts_with("## ") {
            in_scanned_section = t == "## Open Questions" || t == "## Review Results";
            return false;
        }
        if !in_scanned_section {
            return false;
        }
        // Skip completed-task lines (- [x])
        if t.starts_with("- [x]") {
            return false;
        }
        is_active_blocker(t)
    });
    if blocking_found {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "prompt contains a root-level BLOCKING marker".to_string(),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "no root-level BLOCKING marker found".to_string(),
        }
    }
}

/// check: `Workflow:` in `## Status` is not `DONE`.
fn check_workflow_not_done(prompt_text: &str) -> CheckOutcome {
    let name = "workflow-not-done".to_string();
    let workflow_line = prompt_text
        .lines()
        .find(|l| l.trim_start().starts_with("Workflow:"))
        .map(|l| l.trim_start()["Workflow:".len()..].trim().to_string());
    match workflow_line {
        None => CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: "no `Workflow:` line found in ## Status".to_string(),
        },
        Some(ref w) if w.eq_ignore_ascii_case("DONE") => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "Workflow: DONE — plan is already complete".to_string(),
        },
        Some(w) => CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!("Workflow: {w} (≠ DONE)"),
        },
    }
}

/// check: terminal recovery requires `Workflow: DONE`.
fn check_workflow_done(prompt_text: &str) -> CheckOutcome {
    let name = "workflow-done".to_string();
    let workflow_line = prompt_text
        .lines()
        .find(|line| line.trim_start().starts_with("Workflow:"))
        .map(|line| line.trim_start()["Workflow:".len()..].trim().to_string());
    match workflow_line {
        Some(workflow) if workflow.eq_ignore_ascii_case("DONE") => CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "Workflow: DONE".to_string(),
        },
        Some(workflow) => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("Workflow: {workflow} — terminal-reverify requires DONE"),
        },
        None => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "Workflow: DONE line missing".to_string(),
        },
    }
}

/// check: terminal recovery requires DONE and every task checked.
fn check_terminal_tasks_checked(prompt_text: &str) -> CheckOutcome {
    let name = "terminal-tasks-checked".to_string();
    let mut in_tasks = false;
    let mut task_count = 0usize;
    let mut unchecked = 0usize;
    let mut fence: Option<(char, usize)> = None;
    let mut malformed = Vec::new();
    for line in prompt_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            in_tasks = trimmed == "## Tasks";
            continue;
        }
        if !in_tasks || trimmed.starts_with("### ") {
            continue;
        }
        let fence_line = trimmed.trim_start();
        if let Some(ch) = fence_line
            .chars()
            .next()
            .filter(|ch| *ch == '`' || *ch == '~')
        {
            let count = fence_line
                .chars()
                .take_while(|current| current == &ch)
                .count();
            if count >= 3 {
                match fence {
                    None => fence = Some((ch, count)),
                    Some((open, length)) if open == ch && count >= length => fence = None,
                    _ => {}
                }
                continue;
            }
        }
        if fence.is_some() {
            continue;
        }
        if let Some(task) = pipeline::task_spec::parse_plan_task_bullet(line) {
            task_count += 1;
            if !task.checked {
                unchecked += 1;
            }
        } else if is_malformed_task_bullet(line) {
            malformed.push(trimmed.to_string());
        }
    }
    if !malformed.is_empty() {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("malformed task bullets: [{}]", malformed.join(" | ")),
        }
    } else if task_count == 0 {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "## Tasks has no well-formed task bullets".to_string(),
        }
    } else if unchecked > 0 {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("{unchecked} task(s) remain unchecked"),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!("all {task_count} task(s) are checked"),
        }
    }
}

/// check: terminal recovery has no active cursor or open handoff/interruption.
fn check_terminal_cursor_closed(prompt_text: &str) -> CheckOutcome {
    let name = "terminal-cursor-closed".to_string();
    let current_task = prompt_text
        .lines()
        .find(|line| line.trim_start().starts_with("Current Task:"))
        .map(|line| line.trim_start()["Current Task:".len()..].trim());
    let current_open = current_task.is_some_and(|value| !value.is_empty() && value != "—");
    let interrupted = prompt_text.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with("Interrupted Phase:")
            && !trimmed["Interrupted Phase:".len()..].trim().is_empty()
    });
    let open_handoff = prompt_text.lines().enumerate().any(|(index, line)| {
        let trimmed = line.trim();
        let heading = trimmed.starts_with("#### Retry Handoff —")
            || trimmed.starts_with("#### Human Handback —");
        heading
            && prompt_text
                .lines()
                .skip(index + 1)
                .take_while(|next| !next.trim_start().starts_with("#### "))
                .any(|next| {
                    next.trim().eq_ignore_ascii_case("Status: OPEN")
                        || next.trim().eq_ignore_ascii_case("- Status: OPEN")
                })
    });
    if current_open || interrupted || open_handoff {
        let mut reasons = Vec::new();
        if current_open {
            reasons.push("cursor is not cleared");
        }
        if interrupted {
            reasons.push("interrupted phase is open");
        }
        if open_handoff {
            reasons.push("retry or human handoff is open");
        }
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: reasons.join(", "),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "cursor cleared and no open retry, interruption, or human handoff".to_string(),
        }
    }
}

/// check: terminal recovery is authorized only from a clean Git tree.
fn check_terminal_working_tree_clean(repo_root: &Path) -> CheckOutcome {
    // The read and its fail-closed ladder are shared with finalize so the two
    // cannot drift. Only the row name and summary wording stay local, because
    // both receipts are contract-visible text.
    let name = "terminal-working-tree-clean".to_string();
    let command = "git status --porcelain=v1 --untracked-files=all".to_string();
    match super::finalize_check::read_working_tree_status(repo_root) {
        super::finalize_check::WorkingTreeStatus::SpawnError(error) => CheckOutcome {
            name,
            command: Some(command),
            state: CheckState::Fail,
            summary: format!("spawn_error={error}; dirty_entries=unknown"),
        },
        super::finalize_check::WorkingTreeStatus::Undecodable { status, error } => CheckOutcome {
            name,
            command: Some(command),
            state: CheckState::Fail,
            summary: format!(
                "exit_status={status}; undecodable_stdout={error}; dirty_entries=unknown"
            ),
        },
        super::finalize_check::WorkingTreeStatus::Read {
            success,
            status,
            dirty_entries,
            ..
        } => CheckOutcome {
            name,
            command: Some(command),
            state: if success && dirty_entries == 0 {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            summary: format!("exit_status={status}; dirty_entries={dirty_entries}"),
        },
    }
}

/// Read the resume cursor from `## Status`: `Current Task:` and any open
/// `Interrupted Phase:` block.
fn read_resume_cursor(prompt_text: &str) -> String {
    let current = prompt_text
        .lines()
        .find(|l| l.trim_start().starts_with("Current Task:"))
        .map(|l| l.trim_start()["Current Task:".len()..].trim().to_string());
    let interrupted = prompt_text
        .lines()
        .find(|l| l.trim_start().starts_with("Interrupted Phase:"))
        .map(|l| {
            l.trim_start()["Interrupted Phase:".len()..]
                .trim()
                .to_string()
        });
    match (current, interrupted) {
        (Some(c), Some(p)) if !c.is_empty() && c != "—" => {
            format!("resume at {c} (interrupted phase: {p})")
        }
        (Some(c), _) if !c.is_empty() && c != "—" => format!("resume at {c}"),
        (_, Some(p)) if !p.is_empty() => format!("interrupted phase: {p}"),
        _ => "—".to_string(),
    }
}

/// check: wrong-plan guard — the prompt path should match the active-plan row
/// in `.dev/state.md`. A mismatch → Fail; state.md missing → NotRun (non-fatal
/// for CI envs, but the warning is surfaced).
fn check_wrong_plan(prompt_path: &Path, repo_root: &Path) -> CheckOutcome {
    let name = "wrong-plan-guard".to_string();
    let state_path = repo_root.join(".dev").join("state.md");
    let state_text = match std::fs::read_to_string(&state_path) {
        Ok(t) => t,
        Err(_) => {
            return CheckOutcome {
                name,
                command: None,
                state: CheckState::NotRun,
                summary: ".dev/state.md not found — skipping wrong-plan guard".to_string(),
            };
        }
    };

    // Normalize the prompt path for comparison (forward slashes, relative)
    let prompt_norm = prompt_path
        .to_string_lossy()
        .replace('\\', "/")
        .trim_start_matches('/')
        .to_string();
    let prompt_stem = prompt_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");

    // Search state.md for a Session Continuity row that mentions this prompt.
    // Format: `| <plan-slug> | .dev/plans/<slug>.prompt.md | … |`
    let found = state_text.lines().any(|l| {
        let l = l.replace('\\', "/");
        l.contains(&prompt_norm) || (!prompt_stem.is_empty() && l.contains(prompt_stem))
    });

    if found {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!("state.md has an active-plan row for {}", prompt_stem),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "{} is not the active plan in .dev/state.md — wrong plan?",
                prompt_stem
            ),
        }
    }
}

fn get_head_sha(repo_root: &Path) -> String {
    std::process::Command::new("git")
        .current_dir(repo_root)
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn standalone_runtime_identity(prompt: &Path, _repo_root: &Path) -> Result<String, String> {
    let prompt_bytes = std::fs::read(prompt).map_err(|e| format!("prompt unreadable: {e}"))?;
    let prompt_sha256 = format!("{:x}", Sha256::digest(&prompt_bytes));
    let envelope = super::finalize_check::receipt_envelope(Some("standalone"), Some(prompt))?;
    let fields = [
        "standalone",
        envelope.executable_path.as_str(),
        envelope.executable_sha256.as_str(),
        prompt_sha256.as_str(),
    ];
    let mut digest_input = Vec::new();
    for field in fields {
        digest_input.extend_from_slice(&(field.len() as u64).to_be_bytes());
        digest_input.extend_from_slice(field.as_bytes());
    }
    let execution_digest = format!("{:x}", Sha256::digest(digest_input));
    let envelope_text = super::finalize_check::render_receipt_envelope(&envelope);
    let binding_scope = format!("binding_scope: {}\n", envelope.binding_scope);
    let envelope_fields = envelope_text
        .strip_prefix(&binding_scope)
        .ok_or("receipt envelope omitted binding scope")?;
    Ok(format!(
        "{binding_scope}execution_digest: {execution_digest}\n{envelope_fields}"
    ))
}

enum PreflightReceiptMode<'a> {
    Ordinary,
    TerminalReverify {
        repo_root: &'a Path,
        prompt_text: &'a str,
    },
}

fn render_receipt(
    receipt: &Receipt,
    prompt: &Path,
    cursor: &str,
    mode: PreflightReceiptMode<'_>,
    runtime_identity: &str,
    host: Option<&str>,
    dispatch_mode: DispatchMode,
    run_id: &str,
    raw: &[u8],
) -> String {
    let has_map_checks = receipt
        .checks
        .iter()
        .any(|check| check.name.starts_with("verification-"));
    let verification_map_passed = has_map_checks
        && receipt
            .checks
            .iter()
            .filter(|check| check.name.starts_with("verification-"))
            .all(|check| check.state == CheckState::Pass);
    let mut out = String::new();
    match &mode {
        PreflightReceiptMode::Ordinary => {
            let stem = prompt.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            if verification_map_passed {
                out.push_str(&format!(
                    "# pipeline-preflight receipt\n\nprompt: {stem}\nresume-cursor: {cursor}\nrun_id: {run_id}\nhost: {}\ndispatch_mode: {}\n\n",
                    host.unwrap_or("none"), dispatch_mode.as_str()
                ));
            } else {
                out.push_str(&format!(
                    "# pipeline-preflight receipt\n\nprompt: {stem}\nresume-cursor: {cursor}\n\n"
                ));
            }
        }
        PreflightReceiptMode::TerminalReverify {
            repo_root,
            prompt_text,
        } => {
            let prompt_path_str = prompt.to_string_lossy().replace('\\', "/");
            let prompt_sha256 = format!("{:x}", Sha256::digest(prompt_text.as_bytes()));
            let head = get_head_sha(repo_root);
            out.push_str(&format!(
                "# pipeline-preflight terminal-reverify receipt\n\n\
                 prompt_path: {prompt_path_str}\n\
                 prompt_sha256: {prompt_sha256}\n\
                 head: {head}\n\
                 mode: terminal-reverify\n\
                 resume-cursor: {cursor}\n\n"
            ));
        }
    }
    out.push_str(runtime_identity);
    out.push_str(&format!(
        "overall: {}\n\n",
        if receipt.passed() { "pass" } else { "fail" }
    ));
    out.push_str("| check | state | command | summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for c in &receipt.checks {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            c.name,
            c.state.as_str(),
            c.command.as_deref().unwrap_or("—"),
            c.summary,
        ));
    }
    // The gate name is shared with the repair helper. Terminal-reverify keeps
    // its own mode, which the catalog never nominates, so it stays read-only.
    let (gate_mode, active_run) = match &mode {
        PreflightReceiptMode::Ordinary => (
            super::format_repair::PREFLIGHT_ORDINARY_MODE,
            verification_map_passed.then_some(run_id),
        ),
        PreflightReceiptMode::TerminalReverify { .. } => ("terminal-reverify", None),
    };
    out.push_str(
        &super::finalize_check::render_repair_dispositions_for_target(
            &receipt.checks,
            super::format_repair::PREFLIGHT_GATE,
            gate_mode,
            prompt,
            raw,
            active_run,
        ),
    );
    out
}

/// Run `gal pipeline-preflight`. Parse args, classify input, run checks, write receipt.
pub(crate) fn cmd_pipeline_preflight(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let mut parsed = match parse_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal pipeline-preflight: {e}");
            return if e.starts_with("receipt scope resolution:") {
                ExitCode::Error
            } else {
                ExitCode::Usage
            };
        }
    };

    // Classify via the shared classifier (not reimplemented).
    let kind = resolve_pipeline_input(&parsed.prompt);
    match kind {
        PipelineInput::SourcePlan => {
            eprintln!(
                "gal pipeline-preflight: '{}' is a source plan (.dev/plans/). Run on the execution prompt (.dev/plans/*.prompt.md) instead.",
                parsed.prompt.display()
            );
            return ExitCode::Usage;
        }
        PipelineInput::RawSpec => {
            if parsed.terminal_reverify {
                eprintln!(
                    "gal pipeline-preflight: --terminal-reverify requires an execution prompt input, not a raw task spec"
                );
                return ExitCode::Usage;
            }
            // Allow raw specs — they may lack some sections; checks will NotRun appropriately.
        }
        PipelineInput::Prompt => {}
    }

    if !parsed.prompt.exists() {
        eprintln!(
            "gal pipeline-preflight: prompt not found: {}",
            parsed.prompt.display()
        );
        return ExitCode::Usage;
    }

    let resume_pin = if parsed.host.is_some() {
        matching_resume_pin(&parsed.prompt)
    } else {
        None
    };
    if let Some(pin) = &resume_pin {
        parsed.host = pin.host.clone();
        parsed.dispatch_mode = pin.dispatch_mode;
    }

    let table = dispatch::routing::load_routing_default();
    println!(
        "{}",
        render_route_line(&table, parsed.host.as_deref(), parsed.dispatch_mode)
    );
    for w in &table.warnings {
        eprintln!("warning: {w}");
    }

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let raw = std::fs::read(&parsed.prompt).unwrap_or_default();
    let prompt_text = String::from_utf8(raw.clone()).unwrap_or_default();

    let retired_marker = check_retired_marker(&prompt_text);
    let cursor = read_resume_cursor(&prompt_text);

    let mut checks = if parsed.terminal_reverify {
        vec![
            retired_marker,
            check_workflow_done(&prompt_text),
            check_terminal_tasks_checked(&prompt_text),
            check_terminal_cursor_closed(&prompt_text),
            check_terminal_working_tree_clean(&repo_root),
            check_wrong_plan(&parsed.prompt, &repo_root),
        ]
    } else {
        vec![
            retired_marker,
            check_tasks_well_formed(&prompt_text),
            check_test_plan_present(&prompt_text),
            check_retry_handoff_shape(&prompt_text),
            check_no_root_blocking(&prompt_text),
            check_workflow_not_done(&prompt_text),
            check_wrong_plan(&parsed.prompt, &repo_root),
        ]
    };

    if !parsed.terminal_reverify {
        // Reuse prompt-check's evaluator instead of maintaining another map parser.
        checks.extend(super::prompt_check::verification_map_checks(&prompt_text));
    }

    if kind == PipelineInput::Prompt
        && !parsed.terminal_reverify
        && parsed.host.is_none()
        && parsed.host_session.is_none()
        && !preflight_pin_exists(&parsed.prompt)
        && std::env::var("GAL_ENTRY_RECOVERY").as_deref() == Ok("merged")
    {
        checks.extend(post_recovery_checks(&repo_root, &parsed.prompt));
    }

    let receipt = Receipt { checks };

    if let Some(parent) = parsed.receipt.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal pipeline-preflight: cannot create receipt dir {}: {e}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }

    let mode = if parsed.terminal_reverify {
        PreflightReceiptMode::TerminalReverify {
            repo_root: &repo_root,
            prompt_text: &prompt_text,
        }
    } else {
        PreflightReceiptMode::Ordinary
    };
    let runtime_identity = match standalone_runtime_identity(&parsed.prompt, &repo_root) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("gal pipeline-preflight: {error}");
            return ExitCode::Error;
        }
    };
    let run_id = resume_pin.map(|pin| pin.run_id).unwrap_or_else(new_run_id);
    if parsed.host.is_some() {
        match unresolved_host_attempt(&parsed.prompt) {
            Ok(Some(attempt_id)) => {
                eprintln!(
                    "gal pipeline-preflight: unresolved host attempt {attempt_id}; refusing to re-pin"
                );
                return ExitCode::Usage;
            }
            Err(error) => {
                eprintln!("gal pipeline-preflight: {error}");
                return ExitCode::Error;
            }
            Ok(None) => {}
        }
    }
    let mut content = render_receipt(
        &receipt,
        &parsed.prompt,
        &cursor,
        mode,
        &runtime_identity,
        parsed.host.as_deref(),
        parsed.dispatch_mode,
        &run_id,
        &raw,
    );
    if !parsed.terminal_reverify {
        let orchestrator = bind_orchestrator(
            parsed.host.as_deref(),
            parsed.host_session.as_deref(),
            &repo_root,
            &plan_scope_of(&parsed.prompt),
            unix_seconds_now(),
        );
        content.push_str(&orchestrator.render());
    }
    if let Err(e) = std::fs::write(&parsed.receipt, &content) {
        eprintln!(
            "gal pipeline-preflight: cannot write receipt {}: {e}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }

    println!(
        "gal pipeline-preflight: {} ({} check(s)) cursor={} → {}",
        if receipt.passed() { "pass" } else { "fail" },
        receipt.checks.len(),
        cursor,
        parsed.receipt.display()
    );
    receipt.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_verification_map_evaluator_accepts_complete_map_and_rejects_missing_layer() {
        let valid = minimal_prompt();
        assert!(super::super::prompt_check::verification_map_checks(&valid)
            .iter()
            .all(|check| check.state == CheckState::Pass));

        let undercovered = valid.replace("\"cli_surface\":false", "\"cli_surface\":true");
        let outcomes = super::super::prompt_check::verification_map_checks(&undercovered);
        assert!(outcomes
            .iter()
            .any(|check| check.name.starts_with("verification-map-")
                && check.state == CheckState::Fail));
    }

    #[test]
    fn failed_preflight_receipts_do_not_expose_a_usable_pin() {
        let task_id = format!("{}-{:02}", "T", 1usize);
        let failed = Receipt {
            checks: vec![CheckOutcome {
                name: format!("verification-map-{task_id}"),
                command: None,
                state: CheckState::Fail,
                summary: "Verification Map is malformed or missing required coverage".into(),
            }],
        };
        let text = render_receipt(
            &failed,
            Path::new(".dev/plans/example.prompt.md"),
            "—",
            PreflightReceiptMode::Ordinary,
            "",
            Some("codex"),
            DispatchMode::Routed,
            "run-id",
            b"prompt",
        );

        assert!(text.contains("overall: fail"));
        assert!(!text.lines().any(|line| line.starts_with("run_id: ")));
        assert!(!text.lines().any(|line| line.starts_with("host: ")));
        assert!(!text.lines().any(|line| line.starts_with("dispatch_mode: ")));
    }

    #[test]
    fn recovery_classifier_excludes_raw_and_terminal_preflight() {
        let temp = tempfile::tempdir().unwrap();
        let prompt = temp
            .path()
            .join(format!(".dev/plans/entry-{}.prompt.md", std::process::id()));
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        std::fs::write(&prompt, "prompt\n").unwrap();
        let prompt_text = prompt.to_string_lossy().into_owned();
        assert_eq!(
            fresh_prompt_entry(&["pipeline-preflight".into(), prompt_text.clone()]),
            Some(prompt.clone())
        );
        assert!(
            fresh_prompt_entry(&["pipeline-preflight".into(), "task-spec.md".into()]).is_none()
        );
        assert!(fresh_prompt_entry(&[
            "pipeline-preflight".into(),
            prompt_text.clone(),
            "--terminal-reverify".into()
        ])
        .is_none());
        assert!(fresh_prompt_entry(&[
            "pipeline-preflight".into(),
            prompt_text.clone(),
            "--host".into(),
            "codex".into(),
        ])
        .is_none());

        // The receipt path is relative to the current directory, so remove the
        // per-process scope directory even when an assertion fails.
        struct RemoveDir(PathBuf);
        impl Drop for RemoveDir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
                // `remove_dir` fails on a non-empty parent, so shared roots stay.
                for parent in self.0.ancestors().skip(1) {
                    if parent.as_os_str().is_empty() || std::fs::remove_dir(parent).is_err() {
                        break;
                    }
                }
            }
        }
        let pinned_receipt =
            resolve_receipt_path(Some(&prompt), None, "preflight.receipt.md").unwrap();
        let _cleanup = RemoveDir(pinned_receipt.parent().unwrap().to_path_buf());
        std::fs::create_dir_all(pinned_receipt.parent().unwrap()).unwrap();
        std::fs::write(&pinned_receipt, "run_id: pinned-run\n").unwrap();
        assert!(fresh_prompt_entry(&["pipeline-preflight".into(), prompt_text]).is_none());
    }

    #[test]
    fn host_pin_requires_host_for_subagent_dispatch() {
        let repo = tempfile::TempDir::new().unwrap();
        let parsed = parse_args(&[
            repo.path()
                .join("p.prompt.md")
                .to_string_lossy()
                .into_owned(),
            "--dispatch".into(),
            "host-subagent".into(),
        ]);
        assert!(parsed.unwrap_err().contains("requires --host"));
    }

    #[test]
    fn host_pin_rejects_custom_receipt_path() {
        let repo = tempfile::TempDir::new().unwrap();
        let parsed = parse_args(&[
            repo.path()
                .join("p.prompt.md")
                .to_string_lossy()
                .into_owned(),
            "--host".into(),
            "claude".into(),
            "--receipt".into(),
            "x".into(),
        ]);
        assert!(parsed
            .unwrap_err()
            .contains("canonical preflight receipt path"));
    }

    #[test]
    fn host_pin_reads_and_validates_canonical_receipt() {
        let dir = tempfile::TempDir::new().unwrap();
        let _cwd = TestCwd::enter(dir.path());
        let prompt = dir.path().join(".dev/plans/plan.prompt.md");
        let receipt = dir
            .path()
            .join(resolve_receipt_path(Some(&prompt), None, "preflight.receipt.md").unwrap());
        std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        std::fs::write(&receipt, "# pipeline-preflight receipt\nrun_id: run-1\nhost: codex\ndispatch_mode: host-subagent\n").unwrap();
        assert_eq!(
            read_pin(&prompt).unwrap(),
            PreflightPin {
                host: Some("codex".into()),
                dispatch_mode: DispatchMode::HostSubagent,
                run_id: "run-1".into()
            }
        );
        std::fs::write(&receipt, "host: codex\ndispatch_mode: host-subagent\n").unwrap();
        assert_eq!(read_pin(&prompt), Err(PinError::Unparseable));
    }

    #[test]
    fn host_pin_run_ids_are_nonempty_and_unique() {
        let first = new_run_id();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let second = new_run_id();
        assert!(!first.is_empty());
        assert_ne!(first, second);
    }

    #[test]
    fn host_pin_empty_run_id_is_unparseable() {
        let dir = tempfile::TempDir::new().unwrap();
        let _cwd = TestCwd::enter(dir.path());
        let prompt = dir.path().join(".dev/plans/plan.prompt.md");
        let receipt = dir
            .path()
            .join(resolve_receipt_path(Some(&prompt), None, "preflight.receipt.md").unwrap());
        std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        std::fs::write(&receipt, "run_id: \nhost: claude\ndispatch_mode: routed\n").unwrap();
        assert_eq!(read_pin(&prompt), Err(PinError::Unparseable));
    }

    #[test]
    fn host_pin_started_attempt_is_detected() {
        let dir = tempfile::TempDir::new().unwrap();
        let attempts = dir.path().join("host-attempts");
        std::fs::create_dir_all(&attempts).unwrap();
        let manifest = attempts.join("attempt-7.json");
        std::fs::write(
            &manifest,
            r#"{"attempt_id":"attempt-7","log_path":"attempt-7.log"}"#,
        )
        .unwrap();
        std::fs::write(attempts.join("attempt-7.log"), "terminal_state:  started\n").unwrap();
        assert_eq!(
            unresolved_host_attempt_in(&attempts).unwrap().as_deref(),
            Some("attempt-7")
        );
    }

    #[test]
    fn host_pin_matching_coordinator_preserves_pin() {
        let dir = tempfile::TempDir::new().unwrap();
        let _cwd = TestCwd::enter(dir.path());
        let prompt = dir.path().join(".dev/plans/plan.prompt.md");
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        std::fs::write(&prompt, "prompt").unwrap();
        let receipt = resolve_receipt_path(Some(&prompt), None, "preflight.receipt.md").unwrap();
        std::fs::create_dir_all(receipt.parent().unwrap()).unwrap();
        std::fs::write(
            &receipt,
            "run_id: 99-7\nhost: claude\ndispatch_mode: host-subagent\n",
        )
        .unwrap();
        std::fs::write(
            receipt.parent().unwrap().join("coordinator.json"),
            r#"{"run_id":"99-7"}"#,
        )
        .unwrap();
        assert_eq!(
            matching_resume_pin(&prompt).unwrap(),
            PreflightPin {
                host: Some("claude".into()),
                dispatch_mode: DispatchMode::HostSubagent,
                run_id: "99-7".into()
            }
        );
    }

    struct TestCwd {
        old: PathBuf,
        _guard: std::sync::MutexGuard<'static, ()>,
    }
    impl TestCwd {
        fn enter(path: &Path) -> Self {
            let lock = crate::commands::ENV_GUARD
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let old = std::env::current_dir().unwrap();
            std::env::set_current_dir(path).unwrap();
            Self { old, _guard: lock }
        }
    }
    impl Drop for TestCwd {
        fn drop(&mut self) {
            std::env::set_current_dir(&self.old).unwrap();
        }
    }

    #[test]
    fn default_receipt_path_matches_shared_plan_scope() {
        let prompt = Path::new(".dev/plans/receipt-scope.prompt.md");
        assert_eq!(
            default_receipt_path(prompt).unwrap(),
            default_receipt_path(prompt).unwrap(),
        );
    }

    fn route_entry(effort: Option<&str>) -> dispatch::routing::RouteEntry {
        dispatch::routing::RouteEntry {
            executor: "claude".to_string(),
            model: "claude-sonnet-5".to_string(),
            ssh_target: None,
            remote_workdir: None,
            effort: effort.map(str::to_string),
            timeout_secs: None,
        }
    }

    #[test]
    fn render_route_line_tp_01_renders_every_pipeline_role_in_fixed_order() {
        let table = dispatch::routing::RoutingTable {
            entries: std::collections::HashMap::from([
                ("CODER".to_string(), route_entry(Some("medium"))),
                ("TESTER".to_string(), route_entry(Some("medium"))),
                ("AUDITOR".to_string(), route_entry(Some("medium"))),
            ]),
            combinations: std::collections::HashMap::new(),
            executor_defaults: std::collections::HashMap::new(),
            warnings: Vec::new(),
        };

        assert_eq!(
            render_route_line(&table, None, DispatchMode::Routed),
            "coder: claude: claude-sonnet-5 medium; tester: claude: claude-sonnet-5 medium; auditor: claude: claude-sonnet-5 medium",
            "render_route_line renders every pipeline role in fixed order"
        );
    }

    #[test]
    fn render_route_line_tp_02_renders_defaults_unrouted_and_remote_routes() {
        let mut auditor = route_entry(Some("medium"));
        auditor.ssh_target = Some("dev@box".to_string());
        auditor.remote_workdir = Some("/srv/worktree".to_string());
        let table = dispatch::routing::RoutingTable {
            entries: std::collections::HashMap::from([
                ("CODER".to_string(), route_entry(None)),
                ("AUDITOR".to_string(), auditor),
            ]),
            combinations: std::collections::HashMap::new(),
            executor_defaults: std::collections::HashMap::new(),
            warnings: Vec::new(),
        };

        assert_eq!(
            render_route_line(&table, None, DispatchMode::Routed),
            "coder: claude: claude-sonnet-5 (default); tester: (unrouted); auditor: claude: claude-sonnet-5 medium ssh:dev@box"
        );
    }

    #[test]
    fn render_route_line_tp_03_handles_empty_and_non_pipeline_roles() {
        assert_eq!(
            render_route_line(
                &dispatch::routing::RoutingTable::default(),
                None,
                DispatchMode::Routed
            ),
            "(no executorRouting configured)"
        );

        let table = dispatch::routing::RoutingTable {
            entries: std::collections::HashMap::from([
                ("RESEARCHER#1".to_string(), route_entry(Some("high"))),
                ("ARCHITECT".to_string(), route_entry(Some("high"))),
            ]),
            combinations: std::collections::HashMap::new(),
            executor_defaults: std::collections::HashMap::new(),
            warnings: Vec::new(),
        };

        assert_eq!(
            render_route_line(&table, None, DispatchMode::Routed),
            "coder: (unrouted); tester: (unrouted); auditor: (unrouted)"
        );
    }

    #[test]
    fn host_pin_route_line_includes_pin_fields() {
        let table = dispatch::routing::RoutingTable::default();
        assert_eq!(
            render_route_line(&table, Some("claude"), DispatchMode::HostSubagent),
            "(no executorRouting configured); host: claude; dispatch_mode: host-subagent"
        );
    }

    fn minimal_prompt() -> String {
        let task = format!("T-{:02}", 1usize);
        format!(
            "## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {task}\n\n\
             ## Tasks\n\n- [ ] {task} — do something\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n\
             ## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {task} | unit | cargo test | pass | TESTER | test output |\n\n\
             ## Test Plan\n\n| ID | desc |\n| --- | --- |\n| preflight-pass | basic |\n"
        )
    }

    // tasks-well-formed: good prompt → pass
    #[test]
    fn tasks_well_formed_passes_for_valid_task() {
        assert_eq!(
            check_tasks_well_formed(&minimal_prompt()).state,
            CheckState::Pass
        );
    }

    #[test]
    fn tasks_well_formed_uses_shared_identity_and_ignores_fenced_examples() {
        let id = format!("T-{:02}", 1);
        let three_digit = format!("T-{:03}", 1);
        assert_eq!(
            check_tasks_well_formed(&format!("## Tasks\n- [ ] {id}: colon task\n")).state,
            CheckState::Pass
        );
        for prompt in [
            format!("## Tasks\n```md\n- [ ] {id}: fenced example\n```\n"),
            format!("## Tasks\n- [ ] {three_digit}: three digits\n"),
            format!("## Tasks\n- [X] {id}: uppercase checkbox\n"),
        ] {
            assert_eq!(check_tasks_well_formed(&prompt).state, CheckState::Fail);
        }
    }

    // tasks-well-formed: no tasks → fail
    #[test]
    fn tasks_well_formed_fails_when_no_tasks() {
        let prompt = "## Tasks\n\n- [ ] X-01 — not a T-NN\n";
        assert_eq!(check_tasks_well_formed(prompt).state, CheckState::Fail);
    }

    // test-plan-present: present → pass
    #[test]
    fn test_plan_present_passes_when_section_present() {
        assert_eq!(
            check_test_plan_present(&minimal_prompt()).state,
            CheckState::Pass
        );
    }

    // test-plan-present: absent → fail
    #[test]
    fn test_plan_present_fails_when_section_absent() {
        let task = format!("T-{:02}", 1usize);
        let prompt = format!("## Tasks\n\n- [ ] {task} — do something\n");
        let prompt = &prompt;
        assert_eq!(check_test_plan_present(prompt).state, CheckState::Fail);
    }

    #[test]
    fn retry_handoff_shape_absent_block_passes_without_granting_authority() {
        let task = format!("T-{:02}", 6usize);
        let prompt = format!("## Status\n\nCurrent Task: {task}\n\n## Tasks\n");
        let outcome = check_retry_handoff_shape(&prompt);
        assert_eq!(outcome.name, "retry-handoff-shape");
        assert_eq!(outcome.state, CheckState::Pass);
        assert!(outcome.summary.contains("no active Retry Handoff"));
    }

    #[test]
    fn malformed_retry_handoff_shape_fails_and_remains_owner() {
        let task = format!("T-{:02}", 6usize);
        let prompt = format!(
            "## Status\n\nCurrent Task: {task}\n\n### Handoff Notes\n\n#### Retry Handoff — {task} / IMPLEMENT\n\n## Tasks\n"
        );
        let outcome = check_retry_handoff_shape(&prompt);
        assert_eq!(outcome.state, CheckState::Fail);
        let rendered = super::super::finalize_check::render_repair_dispositions(&[outcome]);
        assert!(rendered.contains("retry-handoff-shape: owner"));
    }

    // workflow-not-done: DONE → fail
    #[test]
    fn workflow_done_fails() {
        let prompt = "## Status\n\nWorkflow: DONE\n\n## Tasks\n";
        assert_eq!(check_workflow_not_done(prompt).state, CheckState::Fail);
    }

    // workflow-not-done: IMPLEMENT → pass
    #[test]
    fn workflow_implement_passes() {
        assert_eq!(
            check_workflow_not_done(&minimal_prompt()).state,
            CheckState::Pass
        );
    }

    // no-root-blocking: blocking present → fail
    #[test]
    fn root_blocking_marker_fails() {
        let prompt = "## Open Questions\n\n- [ ] OQ-01 — something BLOCKING\n";
        assert_eq!(check_no_root_blocking(prompt).state, CheckState::Fail);
    }

    // no-root-blocking: BLOCKING only in completed task → pass
    #[test]
    fn blocking_in_completed_task_passes() {
        let task = format!("T-{:02}", 1usize);
        let prompt = format!("## Tasks\n\n- [x] {task} — was BLOCKING but resolved\n");
        assert_eq!(check_no_root_blocking(&prompt).state, CheckState::Pass);
    }

    // Class A: contract-mandated approval phrasing is not a blocker
    #[test]
    fn negated_approve_no_blocking_passes() {
        let prompt = "## Review Results\n\n**APPROVE (no BLOCKING).**\n";
        assert_eq!(check_no_root_blocking(prompt).state, CheckState::Pass);
    }

    // Class A: NON-BLOCKING nit is not a blocker
    #[test]
    fn negated_non_blocking_nit_passes() {
        let prompt = "## Review Results\n\na NON-BLOCKING nit: consider ...\n";
        assert_eq!(check_no_root_blocking(prompt).state, CheckState::Pass);
    }

    // A char whose lowercase changes byte length must not derail the strip.
    // `İ` U+0130 grows 2→3 bytes and `ẞ` U+1E9E shrinks 3→2 when lowercased.
    #[test]
    fn negation_strip_survives_length_changing_lowercase() {
        for lead in ["\u{0130}", "\u{1E9E}"] {
            let prompt = format!("## Review Results\n\n{lead} no BLOCKING\n");
            assert_eq!(check_no_root_blocking(&prompt).state, CheckState::Pass);
        }
        // The same lead char must not mask a genuine marker either.
        let prompt = "## Review Results\n\n\u{0130} BLOCKING: unresolved\n";
        assert_eq!(check_no_root_blocking(prompt).state, CheckState::Fail);
    }

    // Bare BLOCK (no BLOCKING token) must never trigger the check
    #[test]
    fn bare_block_without_blocking_fails_real_marker() {
        let prompt = "## Review Results\n\n**BLOCK** — BLOCKING: unsafe delete path\n";
        assert_eq!(check_no_root_blocking(prompt).state, CheckState::Fail);
    }

    // Class B: non-negated marker inside ## Tasks must not self-block
    #[test]
    fn marker_discussed_in_tasks_section_passes() {
        let task = format!("T-{:02}", 2usize);
        let prompt = format!(
            "## Tasks\n\n- [ ] {task} — treat non-negated upper-case BLOCKING tokens as blockers\n"
        );
        assert_eq!(check_no_root_blocking(&prompt).state, CheckState::Pass);
    }

    // An auditor ### subsection nested under ## Review Results still scans
    #[test]
    fn nested_h3_under_review_results_still_scanned() {
        let task = format!("T-{:02}", 3usize);
        let prompt =
            format!("## Review Results\n\n### [{task}] 2026-07-15\n\nBLOCKING: unresolved\n");
        assert_eq!(check_no_root_blocking(&prompt).state, CheckState::Fail);
    }

    // cmd entry: Usage for missing prompt
    #[test]
    fn cmd_entry_returns_usage_for_missing_prompt() {
        let result = cmd_pipeline_preflight(&[
            "pipeline-preflight".to_string(),
            "nonexistent.prompt.md".to_string(),
        ]);
        assert_eq!(result, ExitCode::Usage);
    }

    // wrong-plan-guard: prompt not mentioned in state.md → Fail
    #[test]
    fn wrong_plan_guard_fails_when_not_in_state_md() {
        let dir = tempfile::TempDir::new().unwrap();
        // state.md exists but mentions a *different* plan
        let state_path = dir.path().join(".dev").join("state.md");
        std::fs::create_dir_all(state_path.parent().unwrap()).unwrap();
        std::fs::write(
            &state_path,
            "## Session Continuity\n\n| other-plan | .dev/plans/other-plan.prompt.md | ... |\n",
        )
        .unwrap();
        // Prompt path is NOT in that state.md
        let prompt_path = Path::new(".dev/plans/my-plan.prompt.md");
        let outcome = check_wrong_plan(prompt_path, dir.path());
        assert_eq!(
            outcome.state,
            CheckState::Fail,
            "prompt absent from state.md must be Fail"
        );
    }

    // wrong-plan-guard: state.md missing → NotRun (non-fatal, surfaced as warning)
    #[test]
    fn wrong_plan_guard_not_run_when_state_md_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        // No state.md written — the dir is empty
        let prompt_path = Path::new(".dev/plans/my-plan.prompt.md");
        let outcome = check_wrong_plan(prompt_path, dir.path());
        assert_eq!(
            outcome.state,
            CheckState::NotRun,
            "missing state.md must be NotRun, not Fail"
        );
    }

    // source-plan input → Usage (must not proceed to checks)
    #[test]
    fn source_plan_input_returns_usage() {
        // A path that looks like a source plan (.dev/plans/foo.md, no .prompt.md suffix)
        // resolve_pipeline_input classifies it SourcePlan → cmd returns Usage before exists().
        let source_plan_path = ".dev/plans/fix-gal-codex-workflow-obedience.md".to_string();
        let result = cmd_pipeline_preflight(&["pipeline-preflight".to_string(), source_plan_path]);
        assert_eq!(
            result,
            ExitCode::Usage,
            "source-plan input must return Usage, not proceed to checks"
        );
    }

    #[test]
    fn prompt_argument_normalizes_file_references_once() {
        let args = |prompt: &str| vec![prompt.to_string()];
        for input in ["p.prompt.md", "#file:p.prompt.md", "@p.prompt.md"] {
            let parsed = parse_args(&args(input)).expect("prompt path parses");
            assert_eq!(parsed.prompt, PathBuf::from("p.prompt.md"));
        }
        assert!(parse_args(&args("#file:")).is_err());
        assert!(parse_args(&args("@")).is_err());
        assert_eq!(
            parse_args(&args("#file:#file:p.prompt.md")).unwrap().prompt,
            PathBuf::from("#file:p.prompt.md")
        );
    }

    #[test]
    fn host_session_argument_parses_and_rejects_terminal_reverify() {
        let args = |list: &[&str]| -> Vec<String> { list.iter().map(|s| s.to_string()).collect() };
        let parsed = parse_args(&args(&[
            "p.prompt.md",
            "--host",
            "codex",
            "--host-session",
            "codex:sess-1",
        ]))
        .expect("host session parses");
        assert_eq!(parsed.host_session.as_deref(), Some("codex:sess-1"));
        assert!(parse_args(&args(&["p.prompt.md", "--host-session"])).is_err());
        assert!(parse_args(&args(&[
            "p.prompt.md",
            "--terminal-reverify",
            "--host-session",
            "codex:sess-1",
        ]))
        .is_err());
    }

    #[test]
    fn orchestrator_record_round_trips_bound_and_unavailable_sections() {
        let bound = OrchestratorRecord::Bound(OrchestratorBinding {
            host_session: "codex:sess-1".to_owned(),
            window_started_at: "epoch+1792526584s".to_owned(),
            canonical_worktree: "C:/repo".to_owned(),
            plan_scope: "plan-a".to_owned(),
        });
        let receipt = format!(
            "# receipt\n\nrun_id: run-1\nhost: codex\n{}",
            bound.render()
        );
        assert_eq!(parse_orchestrator_record(&receipt), bound);

        let unavailable = OrchestratorRecord::Unavailable {
            reason: "no --host-session was given".to_owned(),
        };
        let receipt = format!("run_id: run-2\n{}", unavailable.render());
        assert_eq!(parse_orchestrator_record(&receipt), unavailable);

        assert_eq!(
            parse_orchestrator_record("run_id: run-3\nhost: none\n"),
            OrchestratorRecord::Unavailable {
                reason: "the preflight receipt predates orchestrator binding".to_owned(),
            }
        );
    }

    #[test]
    fn unbound_orchestrator_names_its_reason_without_a_session() {
        let record = bind_orchestrator(Some("codex"), None, Path::new("."), "plan-a", 1);
        assert_eq!(
            record,
            OrchestratorRecord::Unavailable {
                reason: "no --host-session was given".to_owned(),
            }
        );
        let mismatch = bind_orchestrator(
            Some("claude"),
            Some("codex:sess-1"),
            Path::new("."),
            "plan-a",
            1,
        );
        assert!(matches!(
            mismatch,
            OrchestratorRecord::Unavailable { reason } if reason.contains("is a codex session")
        ));
    }

    #[test]
    fn preflight_receipt_gate_name_matches_the_repair_helper() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let git = |args: &[&str]| {
            let output = std::process::Command::new("git")
                .current_dir(&root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "probe@example.invalid"]);
        git(&["config", "user.name", "Probe"]);
        git(&["config", "commit.gpgsign", "false"]);
        let prompt = root.join(".dev/plans/gate-name.prompt.md");
        std::fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        let short = format!("T-{}", 1);
        let text = format!(
            "## Status\n\nWorkflow: IMPLEMENT\n\n## Tasks\n\n- [ ] {short} probe task\n\n## Test Plan\n"
        );
        std::fs::write(&prompt, &text).unwrap();
        git(&["add", "--all"]);
        git(&["commit", "-qm", "fixture"]);
        let _cwd = TestCwd::enter(&root);

        let receipt = Receipt {
            checks: vec![check_tasks_well_formed(&text)],
        };
        assert_eq!(receipt.checks[0].state, CheckState::Fail);
        let ordinary = render_receipt(
            &receipt,
            &prompt,
            "none",
            PreflightReceiptMode::Ordinary,
            "",
            None,
            DispatchMode::Routed,
            "run-probe",
            text.as_bytes(),
        );
        let gate = super::super::format_repair::PREFLIGHT_GATE;
        let mode = super::super::format_repair::PREFLIGHT_ORDINARY_MODE;
        assert!(ordinary.lines().any(|line| line == format!("gate: {gate}")));
        assert!(ordinary.lines().any(|line| line == format!("mode: {mode}")));
        let target = ".dev/plans/gate-name.prompt.md";
        let declaration = super::super::format_repair::format_disposition_line(
            "tasks-well-formed",
            "bullet-shape",
            target,
        );
        assert!(
            ordinary.lines().any(|line| line == declaration),
            "{ordinary}"
        );
        let attempt = ordinary
            .lines()
            .find_map(|line| line.strip_prefix("gate_attempt_id: "))
            .unwrap();
        let repaired = super::super::format_repair::helper_accepts(
            &ordinary,
            "bullet-shape",
            gate,
            mode,
            attempt,
            target,
            text.as_bytes(),
            &root,
        )
        .unwrap();
        assert!(String::from_utf8(repaired)
            .unwrap()
            .contains(&format!("- [ ] T-{:02} probe task", 1)));

        let terminal = render_receipt(
            &receipt,
            &prompt,
            "none",
            PreflightReceiptMode::TerminalReverify {
                repo_root: &root,
                prompt_text: &text,
            },
            "",
            None,
            DispatchMode::Routed,
            "run-probe",
            text.as_bytes(),
        );
        assert!(terminal.lines().any(|line| line == format!("gate: {gate}")));
        assert!(terminal.contains("- tasks-well-formed: owner (repair_id: none)"));
    }
}
