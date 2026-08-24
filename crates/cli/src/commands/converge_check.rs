//! `gal pipeline-converge-check` — pipeline-internal 2g closeout receipt check.
//!
//! Three bookkeeping checks (v1), all must pass for exit 0:
//!   (a) Three-surface checkbox agreement + the nominated task is `[x]` in both.
//!   (b) The task line's recorded `*(hash)*` commit note resolves to a real commit.
//!   (c) `## Status` `Current Task:` is no longer set to this task (cursor cleared).
//!
//! Plus a bound v2 per-phase execution-evidence check, additive to the three above:
//! the R1 plan-scoped executor-log directory (`.dev/executor-logs/<slug>/`, the SAME
//! directory `gal pipeline` scopes dispatch logs under) is scanned for `.log` files
//! whose exact `task_id:` header matches the nominated task. Directory scoping alone
//! isolates a same-`task_id` foreign-plan log (it simply never appears in this plan's
//! own directory) — no cross-plan filtering is needed on top of that. Each required
//! phase (`implement`/`test`/`audit` — `verify` is orchestrator-only and never
//! dispatched) is classified from its LATEST attempt (by the collision-safe,
//! fixed-width, chronologically-sortable filename token) into exactly one of three
//! outcome literals: no attempt at all → `in-conversation`; latest `terminal_state:
//! completed` → `dispatch-offload`; latest terminal but non-completed, when the v1
//! convergence checks above otherwise all pass → `recovered-in-conversation`. An
//! unterminated `started` marker, or any attempt log in the directory whose header
//! is missing/malformed, fails the receipt closed — evidence gaps are never silently
//! skipped. No new CLI flag: the directory is derived automatically from the prompt
//! path, reusing `dispatch::resolve_log_dir_override` (the same R1 derivation `gal
//! pipeline` uses to scope dispatch logs).
//!
//! Receipt is the sole pass-basis; fail/not-run → non-zero.
//! Not a public `/gal` slash command (peer of `finalize-check`/`boundary-check`).

use super::dispatch::{resolve_log_dir_override, resolve_receipt_path};
use super::finalize_check::{
    check_three_surface, checked_task_ids, commit_note_hash, CheckOutcome, CheckState, Receipt,
};
use gal_engine::ExitCode;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Required dispatch phases a bound v2 receipt classifies. `verify` is
/// orchestrator-owned and always in-process (never dispatched — see
/// `docs/architecture.md`'s verify-independence policy), so it carries no
/// attempt-log evidence and is intentionally excluded here.
pub(crate) const REQUIRED_PHASES: [&str; 3] = ["implement", "test", "audit"];

/// The three R2 outcome literals a passing per-phase check may record. These are
/// the ONLY strings ever written as a phase outcome — no synonyms.
///
/// `pub(crate)`: reused by `finalize_check`'s check(g) so the per-task bound-evidence
/// classification is the SAME engine `gal pipeline-converge-check` uses, not a copy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhaseOutcome {
    /// No attempt log at all for this phase — it ran without any dispatch attempt.
    InConversation,
    /// The latest attempt's `terminal_state` is `completed`.
    DispatchOffload,
    /// The latest attempt is terminal but non-completed, and the v1 convergence
    /// checks otherwise all pass — a human/orchestrator recovered it.
    RecoveredInConversation,
}

impl PhaseOutcome {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            PhaseOutcome::InConversation => "in-conversation",
            PhaseOutcome::DispatchOffload => "dispatch-offload",
            PhaseOutcome::RecoveredInConversation => "recovered-in-conversation",
        }
    }
}

/// One attempt log's binary-established evidence (R4): parsed exclusively from the
/// log file's own header lines, never inferred from the filename or any
/// model-controlled field.
#[derive(Debug, Clone)]
pub(crate) struct AttemptEvidence {
    file_name: String,
    terminal_state: String,
    timestamp_start: String,
}

/// The 4 required header fields a well-formed attempt log must carry.
struct LogHeaderFields {
    task_id: String,
    phase: String,
    terminal_state: String,
    timestamp_start: String,
}

/// Parse the `GAL-DISPATCH-LOG v1` header block (see `dispatch::dispatch`'s module
/// doc for the exact format). Returns `None` when the `GAL-DISPATCH-LOG` marker line
/// is absent or any of the 4 required fields is missing/empty — malformed evidence,
/// per R2, is never silently skipped by the caller.
fn parse_log_header(content: &str) -> Option<LogHeaderFields> {
    let mut task_id: Option<String> = None;
    let mut phase: Option<String> = None;
    let mut terminal_state: Option<String> = None;
    let mut timestamp_start: Option<String> = None;
    let mut saw_marker = false;
    for line in content.lines() {
        if line.trim_start().starts_with("---STDOUT---") {
            break;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("GAL-DISPATCH-LOG") {
            saw_marker = true;
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            // A duplicate key is never legitimate: the writer emits each header
            // field exactly once, so a second occurrence can only come from a
            // value that smuggled a line break into the header block. Fail closed
            // rather than pick an occurrence — either choice would let the forged
            // line decide (an early field like `executor` can inject *before* the
            // real `terminal_state`, a late one like `session_id` *after* it).
            // Defense in depth behind `dispatch`'s write-time header sanitizer,
            // and the only guard for logs this binary did not write.
            match key {
                "task_id" if task_id.is_some() => return None,
                "phase" if phase.is_some() => return None,
                "terminal_state" if terminal_state.is_some() => return None,
                "timestamp_start" if timestamp_start.is_some() => return None,
                "task_id" => task_id = Some(value.to_string()),
                "phase" => phase = Some(value.to_string()),
                "terminal_state" => terminal_state = Some(value.to_string()),
                "timestamp_start" => timestamp_start = Some(value.to_string()),
                _ => {}
            }
        }
    }
    if !saw_marker {
        return None;
    }
    Some(LogHeaderFields {
        task_id: task_id?,
        phase: phase?,
        terminal_state: terminal_state?,
        timestamp_start: timestamp_start?,
    })
}

/// Outcome of scanning the plan-scoped log directory for the nominated task.
///
/// `pub(crate)`: `finalize_check`'s check(g) reads these fields directly to
/// aggregate per-task evidence across all checked tasks (reused, not duplicated).
#[derive(Debug, Default)]
pub(crate) struct ScanOutcome {
    /// Every retained attempt for this task, grouped by phase, in chronological
    /// (filename-sorted) order — the full history is kept, not just the latest.
    pub(crate) attempts_by_phase: BTreeMap<String, Vec<AttemptEvidence>>,
    /// File names of every `.log` file in the directory whose header could not be
    /// parsed. Fail-closed evidence (R2) — a malformed log's real `task_id` cannot
    /// be trusted, so it is never excluded on the assumption it belongs to some
    /// other task.
    pub(crate) malformed_files: Vec<String>,
    /// The directory exists but could not be read for a reason other than
    /// "not found" (e.g. permission denied) — a real evidence gap, not the
    /// legitimate "no dispatch ever ran" zero-evidence state. Fail closed
    /// rather than silently treating it as `in-conversation` for every phase.
    pub(crate) directory_unreadable: Option<String>,
}

/// Scan `dir` (the R1 plan-scoped executor-log directory) for `.log` files whose
/// exact `task_id:` header equals `task`. A missing directory (no dispatch attempt
/// has ever run for this plan) is not an error — it yields an empty `ScanOutcome`,
/// which classifies every phase as `in-conversation`. Directory scoping alone is
/// what isolates a same-`task_id` foreign-plan log: this function only ever reads
/// the one directory it is given.
pub(crate) fn scan_plan_log_dir(dir: &Path, task: &str) -> ScanOutcome {
    let mut outcome = ScanOutcome::default();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return outcome; // no directory yet — legitimate zero-evidence state.
        }
        Err(e) => {
            // The directory exists but couldn't be read (permissions, etc.) — a
            // real evidence gap. Do not fold this into "no dispatch ever ran".
            outcome.directory_unreadable = Some(e.to_string());
            return outcome;
        }
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("log"))
        .collect();
    // Fixed-width, zero-padded filename prefixes (see `dispatch::dispatch::unique_attempt_token`)
    // make lexicographic sort equivalent to chronological order.
    files.sort();
    for path in files {
        let file_name = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("")
            .to_string();
        let Ok(content) = std::fs::read_to_string(&path) else {
            outcome.malformed_files.push(file_name);
            continue;
        };
        match parse_log_header(&content) {
            None => outcome.malformed_files.push(file_name),
            Some(fields) if fields.task_id == task => {
                outcome
                    .attempts_by_phase
                    .entry(fields.phase)
                    .or_default()
                    .push(AttemptEvidence {
                        file_name,
                        terminal_state: fields.terminal_state,
                        timestamp_start: fields.timestamp_start,
                    });
            }
            Some(_) => {} // well-formed header, different task_id — not this task's evidence.
        }
    }
    outcome
}

/// Classify one phase's evidence into a pass-with-outcome or a fail-closed reason.
pub(crate) enum PhaseVerdict {
    Outcome(PhaseOutcome),
    FailClosed(String),
}

/// Apply the R2 classification rule to one phase's retained attempts (latest wins).
/// `convergence_passed` gates ONLY the non-completed/non-started branch — an
/// `in-conversation` (no attempt) verdict never depends on convergence state.
pub(crate) fn classify_phase(
    attempts: &[AttemptEvidence],
    convergence_passed: bool,
) -> PhaseVerdict {
    let Some(latest) = attempts.last() else {
        return PhaseVerdict::Outcome(PhaseOutcome::InConversation);
    };
    match latest.terminal_state.as_str() {
        "started" => PhaseVerdict::FailClosed(format!(
            "latest attempt '{}' is an unterminated 'started' marker",
            latest.file_name
        )),
        "completed" => PhaseVerdict::Outcome(PhaseOutcome::DispatchOffload),
        other => {
            if convergence_passed {
                PhaseVerdict::Outcome(PhaseOutcome::RecoveredInConversation)
            } else {
                PhaseVerdict::FailClosed(format!(
                    "latest attempt '{}' terminal_state '{other}' is non-completed, and the \
                     three-surface/commit/cursor convergence checks did not all pass",
                    latest.file_name
                ))
            }
        }
    }
}

/// Build the `phase-<phase>` `CheckOutcome`: Pass carrying the R2 outcome literal
/// plus attempt-count/latest-evidence detail, or Fail carrying the fail-closed reason.
fn phase_check_outcome(
    phase: &str,
    attempts: &[AttemptEvidence],
    convergence_passed: bool,
) -> CheckOutcome {
    let name = format!("phase-{phase}");
    let attempt_count = attempts.len();
    let latest_detail = attempts
        .last()
        .map(|a| format!(", latest: {} @ {}", a.terminal_state, a.timestamp_start))
        .unwrap_or_default();
    match classify_phase(attempts, convergence_passed) {
        PhaseVerdict::Outcome(outcome) => CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!(
                "{} ({} attempt(s){})",
                outcome.as_str(),
                attempt_count,
                latest_detail
            ),
        },
        PhaseVerdict::FailClosed(reason) => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: reason,
        },
    }
}

/// Fail-closed integrity check: any malformed attempt log anywhere in the
/// plan-scoped directory fails the whole receipt, regardless of which task it
/// nominally claims to belong to (R2/R4 — unparseable evidence is never trusted).
fn attempt_log_integrity_check(malformed_files: &[String]) -> CheckOutcome {
    CheckOutcome {
        name: "attempt-log-integrity".to_string(),
        command: None,
        state: if malformed_files.is_empty() {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if malformed_files.is_empty() {
            "no malformed attempt logs in the plan-scoped directory".to_string()
        } else {
            format!(
                "malformed attempt log(s), header unparseable: {}",
                malformed_files.join(", ")
            )
        },
    }
}

struct Args {
    prompt: PathBuf,
    task: String,
    receipt: PathBuf,
}

fn default_receipt_path(prompt: &Path, task: &str) -> PathBuf {
    resolve_receipt_path(Some(prompt), format!("{task}-converge-check.receipt.md"))
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut task: Option<String> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--task" => {
                let v = it.next().ok_or("--task requires a task id")?;
                task = Some(v.clone());
            }
            "--receipt" => {
                let v = it.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(v));
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
    let task = task.ok_or("--task <id> is required")?;
    let prompt = prompt.ok_or("pipeline-converge-check requires a <prompt> path")?;
    let receipt = receipt.unwrap_or_else(|| default_receipt_path(&prompt, &task));
    Ok(Args {
        prompt,
        task,
        receipt,
    })
}

/// check(a): three-surface checkbox agreement AND the nominated task is `[x]` in
/// both the source plan and the execution prompt.
///
/// `pub(crate)`: reused by `finalize_check`'s check(g) to establish per-task
/// convergence, the same v1 bookkeeping gate `gal pipeline-converge-check` uses.
pub(crate) fn check_three_surface_and_task(
    task: &str,
    prompt_path: &Path,
    prompt_text: &str,
    repo_root: &Path,
) -> CheckOutcome {
    // Reuse the shared three-surface check from finalize_check.
    let surface_outcome = check_three_surface(prompt_path, prompt_text, repo_root);
    if surface_outcome.state != CheckState::Pass {
        return surface_outcome; // propagate: not-run or fail
    }

    // Additionally assert the specific task is checked on both surfaces.
    let prompt_ids = checked_task_ids(prompt_text);
    let prompt_has = prompt_ids.contains(task);

    // Find source plan text (same derivation as check_three_surface internally).
    let slug = prompt_path
        .file_name()
        .and_then(|f| f.to_str())
        .and_then(|f| f.strip_suffix(".prompt.md"));
    let source_has = if let Some(slug) = slug {
        let source_plan = repo_root
            .join(".dev")
            .join("plans")
            .join(format!("{slug}.md"));
        std::fs::read_to_string(&source_plan)
            .map(|text| checked_task_ids(&text).contains(task))
            .unwrap_or(false)
    } else {
        false
    };

    if prompt_has && source_has {
        CheckOutcome {
            name: "three-surface-checkbox".to_string(),
            command: None,
            state: CheckState::Pass,
            summary: format!(
                "source plan and prompt agree; {} is [x] on both surfaces",
                task
            ),
        }
    } else {
        CheckOutcome {
            name: "three-surface-checkbox".to_string(),
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "{} not [x] — prompt: {}, source: {}",
                task, prompt_has, source_has
            ),
        }
    }
}

/// check(b): the T-NN task line's `*(hash)*` commit note must resolve to a real commit.
///
/// `pub(crate)`: reused by `finalize_check`'s check(g) for per-task commit validation.
pub(crate) fn check_task_commit(task: &str, prompt_text: &str, repo_root: &Path) -> CheckOutcome {
    let name = "task-commit-existence".to_string();
    // Find the task line (the `- [x] T-NN …` line in the ## Tasks section).
    let task_line = prompt_text.lines().find(|l| {
        let t = l.trim_start();
        t.starts_with("- [x] ") && t[6..].split_whitespace().next() == Some(task)
    });
    let Some(line) = task_line else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: format!("{} has no [x] task line in this prompt", task),
        };
    };
    let Some(hash) = commit_note_hash(line) else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: format!("{} task line carries no *(hash)* commit note", task),
        };
    };
    let ok = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("cat-file")
        .arg("-e")
        .arg(format!("{hash}^{{commit}}"))
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    CheckOutcome {
        name,
        command: Some(format!("git cat-file -e {hash}")),
        state: if ok {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: if ok {
            format!("commit {hash} exists")
        } else {
            format!("commit {hash} not found")
        },
    }
}

/// The parsed value of the `## Status` `Current Task:` cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CurrentTaskValue {
    /// No `Current Task:` line was found.
    Missing,
    /// The cursor is explicitly cleared with an empty value or em dash.
    Cleared,
    /// The cursor names an active task.
    Active(String),
}

/// Parse the `Current Task:` cursor using the canonical convergence grammar.
pub(crate) fn current_task_value(prompt_text: &str) -> CurrentTaskValue {
    let Some(value) = prompt_text
        .lines()
        .find(|l| l.trim_start().starts_with("Current Task:"))
        .map(|l| l.trim_start()["Current Task:".len()..].trim())
    else {
        return CurrentTaskValue::Missing;
    };

    if value.is_empty() || value == "—" {
        CurrentTaskValue::Cleared
    } else {
        CurrentTaskValue::Active(value.to_string())
    }
}

/// check(c): `## Status` `Current Task:` must be cleared (not pointing at this task).
///
/// `pub(crate)`: reused by `finalize_check`'s check(g) for per-task convergence.
pub(crate) fn check_cursor_cleared(task: &str, prompt_text: &str) -> CheckOutcome {
    let name = "cursor-cleared".to_string();
    match current_task_value(prompt_text) {
        CurrentTaskValue::Missing => CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: "no 'Current Task:' line found in ## Status".to_string(),
        },
        CurrentTaskValue::Active(ct) if ct == task || ct == format!("T-{}", &task[2..]) => {
            CheckOutcome {
                name,
                command: None,
                state: CheckState::Fail,
                summary: format!("Current Task is still '{ct}' — cursor not cleared"),
            }
        }
        CurrentTaskValue::Cleared => CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!("Current Task: cleared (≠ {})", task),
        },
        CurrentTaskValue::Active(ct) => CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!("Current Task: '{ct}' (≠ {})", task),
        },
    }
}

/// Run `gal pipeline-converge-check`. Parse args, run 3 checks, write receipt, exit.
pub(crate) fn cmd_pipeline_converge_check(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal pipeline-converge-check: {e}");
            return ExitCode::Usage;
        }
    };

    if !parsed.prompt.exists() {
        eprintln!(
            "gal pipeline-converge-check: prompt not found: {}",
            parsed.prompt.display()
        );
        return ExitCode::Usage;
    }

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let prompt_text = std::fs::read_to_string(&parsed.prompt).unwrap_or_default();

    let mut checks = vec![
        check_three_surface_and_task(&parsed.task, &parsed.prompt, &prompt_text, &repo_root),
        check_task_commit(&parsed.task, &prompt_text, &repo_root),
        check_cursor_cleared(&parsed.task, &prompt_text),
    ];
    // v1 convergence: all 3 bookkeeping checks above must pass before a
    // non-completed latest attempt is eligible to classify as recovered.
    let convergence_passed = checks.iter().all(|c| c.state == CheckState::Pass);

    // v2: bound per-phase execution evidence, scanned from the SAME R1 plan-scoped
    // log directory `gal pipeline` scopes dispatch logs under for this prompt.
    match resolve_log_dir_override(&repo_root, Some(parsed.prompt.as_path())) {
        Some(log_dir) => {
            let scan = scan_plan_log_dir(&log_dir, &parsed.task);
            if let Some(reason) = &scan.directory_unreadable {
                checks.push(CheckOutcome {
                    name: "attempt-log-directory".to_string(),
                    command: None,
                    state: CheckState::Fail,
                    summary: format!(
                        "plan-scoped log directory exists but could not be read: {reason}"
                    ),
                });
            }
            checks.push(attempt_log_integrity_check(&scan.malformed_files));
            for phase in REQUIRED_PHASES {
                let attempts = scan
                    .attempts_by_phase
                    .get(phase)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                checks.push(phase_check_outcome(phase, attempts, convergence_passed));
            }
        }
        None => {
            // Defensive: the prompt already exists (checked above), so this only
            // fires for a pathological filename with no derivable stem. Fail
            // closed rather than silently skipping per-phase evidence.
            checks.push(CheckOutcome {
                name: "attempt-log-directory".to_string(),
                command: None,
                state: CheckState::Fail,
                summary: "could not derive the R1 plan-scoped log directory from the prompt path"
                    .to_string(),
            });
        }
    }

    let receipt = Receipt { checks };

    if let Some(parent) = parsed.receipt.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal pipeline-converge-check: cannot create receipt dir {}: {e}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }

    let content = render_receipt(&receipt, &parsed.task);
    if let Err(e) = std::fs::write(&parsed.receipt, &content) {
        eprintln!(
            "gal pipeline-converge-check: cannot write receipt {}: {e}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }

    println!(
        "gal pipeline-converge-check: {} ({} check(s)) → {}",
        if receipt.passed() { "pass" } else { "fail" },
        receipt.checks.len(),
        parsed.receipt.display()
    );
    receipt.exit_code()
}

fn render_receipt(receipt: &Receipt, task: &str) -> String {
    let mut out = format!("# pipeline-converge-check receipt\n\ntask: {task}\n\n");
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
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_receipt_path_is_plan_scoped_and_explicit_override_wins() {
        let prompt = Path::new(".dev/plans/converge-scope.prompt.md");
        let task = format!("T-{:02}", 4usize);
        assert_eq!(
            default_receipt_path(prompt, &task),
            resolve_receipt_path(Some(prompt), format!("{task}-converge-check.receipt.md"))
        );
        let parsed = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task,
            "--receipt".to_string(),
            "custom/receipt.md".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.receipt, PathBuf::from("custom/receipt.md"));
    }
    use std::fs;
    use tempfile::TempDir;

    fn make_tmp_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        // init a bare git repo so git commands work
        std::process::Command::new("git")
            .arg("init")
            .arg(tmp.path())
            .output()
            .unwrap();
        tmp
    }

    // cursor-cleared check — task still set → Fail
    #[test]
    fn cursor_cleared_fails_when_task_still_set() {
        let task = format!("T-{}", 99usize);
        let text =
            format!("## Status\n\nCurrent Task: {task}\n\n## Tasks\n\n- [x] {task} — done\n");
        let outcome = check_cursor_cleared(&task, &text);
        assert_eq!(outcome.state, CheckState::Fail);
    }

    // cursor-cleared check — cleared (dash) → Pass
    #[test]
    fn cursor_cleared_passes_when_cleared() {
        let task = format!("T-{}", 99usize);
        let text = format!("## Status\n\nCurrent Task: —\n\n## Tasks\n\n- [x] {task} — done\n");
        let outcome = check_cursor_cleared(&task, &text);
        assert_eq!(outcome.state, CheckState::Pass);
    }

    #[test]
    fn current_task_value_distinguishes_missing_cleared_and_active_cursor() {
        assert_eq!(
            current_task_value("## Status\n\nWorkflow: IMPLEMENT\n"),
            CurrentTaskValue::Missing
        );
        assert_eq!(
            current_task_value("## Status\n\nCurrent Task: —\n"),
            CurrentTaskValue::Cleared
        );
        let task = tid(2);
        assert_eq!(
            current_task_value(&format!("## Status\n\nCurrent Task: {task}\n")),
            CurrentTaskValue::Active(task)
        );
    }

    // commit note absent → NotRun (never Fail)
    #[test]
    fn task_commit_not_run_when_no_hash() {
        let tmp = make_tmp_repo();
        let task = format!("T-{}", 99usize);
        let text = format!("## Tasks\n\n- [x] {task} — no commit note here\n");
        let outcome = check_task_commit(&task, &text, tmp.path());
        assert_eq!(outcome.state, CheckState::NotRun);
    }

    // bad hash → Fail
    #[test]
    fn task_commit_fails_for_nonexistent_hash() {
        let tmp = make_tmp_repo();
        let task = format!("T-{}", 99usize);
        let text = format!("## Tasks\n\n- [x] {task} — do stuff *(aaaaaaa)*\n");
        let outcome = check_task_commit(&task, &text, tmp.path());
        assert_eq!(outcome.state, CheckState::Fail);
    }

    // non-prompt file → three-surface check returns NotRun
    #[test]
    fn converge_check_non_prompt_file_is_not_run() {
        let tmp = make_tmp_repo();
        let not_prompt = tmp.path().join("not-a-prompt.txt");
        fs::write(&not_prompt, "some content").unwrap();
        let task = format!("T-{}", 99usize);
        let outcome = check_three_surface_and_task(&task, &not_prompt, "some content", tmp.path());
        assert_eq!(outcome.state, CheckState::NotRun);
    }

    // receipt written on clean stub call via cmd entry point (Usage on missing file)
    #[test]
    fn cmd_entry_returns_usage_for_missing_prompt() {
        let task = format!("T-{}", 99usize);
        let result = cmd_pipeline_converge_check(&[
            "pipeline-converge-check".to_string(),
            "nonexistent.prompt.md".to_string(),
            "--task".to_string(),
            task,
        ]);
        assert_eq!(result, ExitCode::Usage);
    }

    // ── v2 bound per-phase receipt ───────────────────────────────────────────────

    // Task ids are composed at runtime, never written as digit literals in source
    // (a stale `T-<digits>` literal is just dead data; the naming gate enforces this).
    fn tid(n: u32) -> String {
        format!("T-{:03}", n)
    }

    /// Write one well-formed `GAL-DISPATCH-LOG v1` attempt log, mirroring the exact
    /// header shape `dispatch::dispatch::write_log`/`write_started_marker` produce.
    fn write_attempt_log(
        dir: &Path,
        filename: &str,
        task_id: &str,
        phase: &str,
        terminal_state: &str,
        timestamp_start: &str,
    ) {
        fs::create_dir_all(dir).unwrap();
        let content = format!(
            "GAL-DISPATCH-LOG v1\n\
             timestamp_start: {timestamp_start}\n\
             timestamp_end:   {timestamp_start}\n\
             duration_ms:     1\n\
             executor:        claude\n\
             phase:           {phase}\n\
             task_id:         {task_id}\n\
             git_branch:      main\n\
             git_head:        abc1234\n\
             exit_code:       0\n\
             actual_model:    m\n\
             terminal_state:  {terminal_state}\n\
             session_id:      none\n\
             ---STDOUT---\n\
             \n\
             ---STDERR---\n\
             \n"
        );
        fs::write(dir.join(filename), content).unwrap();
    }

    /// Deterministic, chronologically-sortable filename mirroring
    /// `dispatch::dispatch::unique_attempt_token`'s fixed-width format, so tests can
    /// control attempt ordering explicitly.
    fn attempt_filename(secs: u64, seq: u32, task: &str, phase: &str) -> String {
        format!(
            "{:010}-000000001-{:06}-{}-{}-claude.log",
            secs, seq, task, phase
        )
    }

    // binding: classification reads from the SAME R1 plan-scoped directory
    // `resolve_log_dir_override` derives for this prompt (not a hand-rolled path).
    #[test]
    fn scan_uses_the_same_resolved_plan_scoped_directory_as_dispatch() {
        let tmp = make_tmp_repo();
        let task = tid(1);
        let prompt = tmp
            .path()
            .join(".dev")
            .join("plans")
            .join("sample-plan.prompt.md");
        fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        fs::write(&prompt, "## Tasks\n").unwrap();

        let log_dir = resolve_log_dir_override(tmp.path(), Some(prompt.as_path())).unwrap();
        assert_eq!(
            log_dir,
            tmp.path()
                .join(".dev")
                .join("executor-logs")
                .join("sample-plan")
        );

        write_attempt_log(
            &log_dir,
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:00:00Z",
        );

        let scan = scan_plan_log_dir(&log_dir, &task);
        assert_eq!(scan.attempts_by_phase.get("implement").unwrap().len(), 1);
        assert!(scan.malformed_files.is_empty());
    }

    // exact-header isolation: two plan-scoped directories each carry a log with the
    // identical task_id — scanning one directory never sees the other's attempt.
    #[test]
    fn exact_header_scan_isolates_same_task_id_foreign_plan_directory() {
        let task = tid(1);
        let dir_a = TempDir::new().unwrap();
        let dir_b = TempDir::new().unwrap();
        write_attempt_log(
            dir_a.path(),
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:00:00Z",
        );
        write_attempt_log(
            dir_b.path(),
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "timeout",
            "2026-07-16T00:00:00Z",
        );

        let scan_a = scan_plan_log_dir(dir_a.path(), &task);
        assert_eq!(
            scan_a.attempts_by_phase.get("implement").unwrap()[0].terminal_state,
            "completed"
        );
        // dir_a's scan must never surface dir_b's foreign attempt (or vice versa).
        assert_eq!(scan_a.attempts_by_phase.get("implement").unwrap().len(), 1);

        let scan_b = scan_plan_log_dir(dir_b.path(), &task);
        assert_eq!(
            scan_b.attempts_by_phase.get("implement").unwrap()[0].terminal_state,
            "timeout"
        );
    }

    // mixed phases: implement completed, test never attempted, audit unterminated
    // `started` — each phase classifies independently, and the unterminated audit
    // phase alone fails closed.
    #[test]
    fn mixed_phases_classify_independently_and_unterminated_audit_fails() {
        let task = tid(1);
        let dir = TempDir::new().unwrap();
        write_attempt_log(
            dir.path(),
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:00:00Z",
        );
        write_attempt_log(
            dir.path(),
            &attempt_filename(2, 0, &task, "audit"),
            &task,
            "audit",
            "started",
            "2026-07-16T00:05:00Z",
        );
        let scan = scan_plan_log_dir(dir.path(), &task);

        let implement = phase_check_outcome(
            "implement",
            scan.attempts_by_phase.get("implement").unwrap(),
            true,
        );
        assert_eq!(implement.state, CheckState::Pass);
        assert!(implement.summary.contains("dispatch-offload"));

        let test = phase_check_outcome("test", &[], true);
        assert_eq!(test.state, CheckState::Pass);
        assert!(test.summary.contains("in-conversation"));

        let audit =
            phase_check_outcome("audit", scan.attempts_by_phase.get("audit").unwrap(), true);
        assert_eq!(audit.state, CheckState::Fail);
        assert!(audit.summary.contains("unterminated"));
    }

    // retry ordering: an older disconnected-partial attempt is superseded by a
    // newer completed attempt — the latest wins the classification, but the older
    // failed attempt remains retained/visible in the scan, not discarded.
    #[test]
    fn retry_ordering_latest_wins_but_older_attempt_is_retained() {
        let task = tid(1);
        let dir = TempDir::new().unwrap();
        write_attempt_log(
            dir.path(),
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "disconnected-partial",
            "2026-07-16T00:00:00Z",
        );
        write_attempt_log(
            dir.path(),
            &attempt_filename(2, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:05:00Z",
        );
        let scan = scan_plan_log_dir(dir.path(), &task);
        let attempts = scan.attempts_by_phase.get("implement").unwrap();
        assert_eq!(
            attempts.len(),
            2,
            "older attempt must be retained, not discarded"
        );
        assert_eq!(attempts[0].terminal_state, "disconnected-partial");
        assert_eq!(attempts[1].terminal_state, "completed");

        let outcome = phase_check_outcome("implement", attempts, true);
        assert_eq!(outcome.state, CheckState::Pass);
        assert!(outcome.summary.contains("dispatch-offload"));
        assert!(outcome.summary.contains("2 attempt"));
    }

    // recovery: latest attempt terminal but non-completed, v1 convergence passes →
    // recovered-in-conversation, not a hard fail.
    #[test]
    fn recovered_in_conversation_when_convergence_passes() {
        let task = tid(1);
        let dir = TempDir::new().unwrap();
        write_attempt_log(
            dir.path(),
            &attempt_filename(1, 0, &task, "test"),
            &task,
            "test",
            "timeout-no-output",
            "2026-07-16T00:00:00Z",
        );
        let scan = scan_plan_log_dir(dir.path(), &task);
        let outcome =
            phase_check_outcome("test", scan.attempts_by_phase.get("test").unwrap(), true);
        assert_eq!(outcome.state, CheckState::Pass);
        assert!(outcome.summary.contains("recovered-in-conversation"));
    }

    // recovery denied: same non-completed latest attempt, but v1 convergence did
    // NOT all pass → fail closed instead of a silent recovered pass.
    #[test]
    fn non_completed_latest_without_passing_convergence_fails_closed() {
        let task = tid(1);
        let dir = TempDir::new().unwrap();
        write_attempt_log(
            dir.path(),
            &attempt_filename(1, 0, &task, "test"),
            &task,
            "test",
            "timeout-no-output",
            "2026-07-16T00:00:00Z",
        );
        let scan = scan_plan_log_dir(dir.path(), &task);
        let outcome =
            phase_check_outcome("test", scan.attempts_by_phase.get("test").unwrap(), false);
        assert_eq!(outcome.state, CheckState::Fail);
    }

    // malformed logs: a `.log` file missing the `task_id:` header line is treated as
    // fail-closed evidence — never silently ignored, never a panic.
    #[test]
    fn malformed_log_missing_task_id_header_fails_closed() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path()).unwrap();
        let garbled = "GAL-DISPATCH-LOG v1\nphase:           implement\nterminal_state:  completed\ntimestamp_start: 2026-07-16T00:00:00Z\n---STDOUT---\n\n---STDERR---\n";
        fs::write(dir.path().join("garbled.log"), garbled).unwrap();

        let scan = scan_plan_log_dir(dir.path(), &tid(1));
        assert_eq!(scan.malformed_files, vec!["garbled.log".to_string()]);
        assert!(scan.attempts_by_phase.is_empty());

        let integrity = attempt_log_integrity_check(&scan.malformed_files);
        assert_eq!(integrity.state, CheckState::Fail);
    }

    // malformed logs: content with no `GAL-DISPATCH-LOG` marker at all (garbled/
    // truncated) is also fail-closed, not silently skipped.
    #[test]
    fn malformed_log_without_dispatch_marker_fails_closed() {
        let dir = TempDir::new().unwrap();
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("truncated.log"), "not a real log at all").unwrap();

        let scan = scan_plan_log_dir(dir.path(), &tid(1));
        assert_eq!(scan.malformed_files, vec!["truncated.log".to_string()]);
    }

    // A log carrying a smuggled duplicate header line (the shape a `session_id`
    // line break would produce if it ever reached disk) is rejected as malformed
    // rather than letting the forged `completed` line win the classification.
    #[test]
    fn duplicate_terminal_state_header_fails_closed_instead_of_forging_completed() {
        let dir = TempDir::new().unwrap();
        let task = tid(1);
        let forged = format!(
            "GAL-DISPATCH-LOG v1\n\
             timestamp_start: epoch+1s\n\
             executor:        claude\n\
             phase:           implement\n\
             task_id:         {task}\n\
             terminal_state:  timeout-no-output\n\
             session_id:      abc\n\
             terminal_state:  completed\n\
             ---STDOUT---\n\
             \n\
             ---STDERR---\n\
             \n"
        );
        fs::create_dir_all(dir.path()).unwrap();
        fs::write(dir.path().join("forged.log"), forged).unwrap();

        let scan = scan_plan_log_dir(dir.path(), &task);
        assert_eq!(
            scan.malformed_files,
            vec!["forged.log".to_string()],
            "a duplicate terminal_state header must fail closed"
        );
        assert!(
            scan.attempts_by_phase.is_empty(),
            "a forged log must never become classifiable attempt evidence"
        );
    }

    // integrity check passes vacuously when the directory has no malformed logs.
    #[test]
    fn attempt_log_integrity_passes_with_no_malformed_files() {
        let outcome = attempt_log_integrity_check(&[]);
        assert_eq!(outcome.state, CheckState::Pass);
    }

    // no attempt log at all for a phase → in-conversation, Pass.
    #[test]
    fn phase_with_no_attempts_classifies_in_conversation() {
        let outcome = phase_check_outcome("implement", &[], true);
        assert_eq!(outcome.state, CheckState::Pass);
        assert!(outcome.summary.contains("in-conversation"));
        assert!(outcome.summary.contains("0 attempt"));
    }

    // a missing plan-scoped directory (no dispatch ever ran for this plan) is a
    // legitimate zero-evidence state, not an error — every phase reads as empty.
    #[test]
    fn scan_of_nonexistent_directory_yields_empty_scan() {
        let tmp = TempDir::new().unwrap();
        let missing_dir = tmp.path().join("never-created");
        let scan = scan_plan_log_dir(&missing_dir, &tid(1));
        assert!(scan.attempts_by_phase.is_empty());
        assert!(scan.malformed_files.is_empty());
        assert!(scan.directory_unreadable.is_none());
    }

    // A directory that exists but cannot be read (not "not found") must fail
    // closed, not silently fold into the legitimate "no dispatch ever ran" state.
    #[test]
    fn scan_of_unreadable_existing_path_fails_closed_not_silently_empty() {
        let tmp = TempDir::new().unwrap();
        // A plain FILE sitting where the scan expects a directory: read_dir on it
        // fails with a real (non-NotFound) I/O error, since the path exists.
        let not_a_dir = tmp.path().join("blocks-as-a-file");
        fs::write(&not_a_dir, "not a directory").unwrap();

        let scan = scan_plan_log_dir(&not_a_dir, &tid(1));
        assert!(
            scan.directory_unreadable.is_some(),
            "an existing-but-unreadable path must set directory_unreadable, not silently empty-scan"
        );
        assert!(scan.attempts_by_phase.is_empty());
    }

    // two DIFFERENT phases of the SAME task can carry two DIFFERENT non-trivial
    // outcomes simultaneously: implement's latest attempt completed
    // (dispatch-offload) while test's latest attempt is terminal-but-non-completed
    // under passing convergence (recovered-in-conversation). Each must surface its
    // own correct outcome — neither phase's classification may bleed into the other.
    #[test]
    fn dispatch_offload_and_recovered_in_conversation_coexist_across_phases() {
        let task = tid(1);
        let dir = TempDir::new().unwrap();
        write_attempt_log(
            dir.path(),
            &attempt_filename(1, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:00:00Z",
        );
        write_attempt_log(
            dir.path(),
            &attempt_filename(2, 0, &task, "test"),
            &task,
            "test",
            "timeout-no-output",
            "2026-07-16T00:05:00Z",
        );
        let scan = scan_plan_log_dir(dir.path(), &task);

        let implement = phase_check_outcome(
            "implement",
            scan.attempts_by_phase.get("implement").unwrap(),
            true,
        );
        assert_eq!(implement.state, CheckState::Pass);
        assert!(implement.summary.contains("dispatch-offload"));
        assert!(!implement.summary.contains("recovered-in-conversation"));

        let test = phase_check_outcome("test", scan.attempts_by_phase.get("test").unwrap(), true);
        assert_eq!(test.state, CheckState::Pass);
        assert!(test.summary.contains("recovered-in-conversation"));
        assert!(!test.summary.contains("dispatch-offload"));
    }

    // `verify` is orchestrator-only and never dispatched: it must not be a required
    // phase (no `phase-verify` check is emitted), even when an attempt log claiming
    // phase `verify` happens to sit in the plan-scoped directory.
    #[test]
    fn verify_phase_is_excluded_from_required_phases() {
        assert!(!REQUIRED_PHASES.contains(&"verify"));

        let task = tid(1);
        let dir = TempDir::new().unwrap();
        write_attempt_log(
            dir.path(),
            &attempt_filename(1, 0, &task, "verify"),
            &task,
            "verify",
            "completed",
            "2026-07-16T00:00:00Z",
        );
        let scan = scan_plan_log_dir(dir.path(), &task);
        // the log is still retained under its own phase key by the scan...
        assert_eq!(scan.attempts_by_phase.get("verify").unwrap().len(), 1);

        // ...but no `phase-verify` CheckOutcome is ever produced for REQUIRED_PHASES.
        let checks: Vec<CheckOutcome> = REQUIRED_PHASES
            .iter()
            .map(|phase| {
                let attempts = scan
                    .attempts_by_phase
                    .get(*phase)
                    .map(|v| v.as_slice())
                    .unwrap_or(&[]);
                phase_check_outcome(phase, attempts, true)
            })
            .collect();
        assert!(!checks.iter().any(|c| c.name == "phase-verify"));
    }

    // sort-order robustness: attempts written to disk in REVERSE chronological
    // order must still be retained and classified in chronological (oldest-first)
    // order by `scan_plan_log_dir`'s filename sort — directory-read order must not
    // leak into the retained attempt sequence or which attempt is "latest".
    #[test]
    fn scan_reorders_reverse_written_attempts_into_chronological_order() {
        let task = tid(1);
        let dir = TempDir::new().unwrap();
        // Write the LATER attempt to disk first, then the EARLIER attempt.
        write_attempt_log(
            dir.path(),
            &attempt_filename(9, 0, &task, "implement"),
            &task,
            "implement",
            "completed",
            "2026-07-16T00:09:00Z",
        );
        write_attempt_log(
            dir.path(),
            &attempt_filename(3, 0, &task, "implement"),
            &task,
            "implement",
            "disconnected-partial",
            "2026-07-16T00:03:00Z",
        );
        let scan = scan_plan_log_dir(dir.path(), &task);
        let attempts = scan.attempts_by_phase.get("implement").unwrap();
        assert_eq!(attempts.len(), 2);
        // regardless of write/read order, the fixed-width filename sort must place
        // the secs=3 attempt before the secs=9 attempt.
        assert_eq!(attempts[0].terminal_state, "disconnected-partial");
        assert_eq!(attempts[1].terminal_state, "completed");

        // and the LATEST (secs=9, completed) attempt wins the classification.
        let outcome = phase_check_outcome("implement", attempts, true);
        assert!(outcome.summary.contains("dispatch-offload"));
    }
}
