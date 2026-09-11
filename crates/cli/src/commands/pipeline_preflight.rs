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

use super::boundary_check::{evaluate_marked_digest, MarkedDigestResult};
use super::dispatch::{resolve_pipeline_input, resolve_receipt_path, PipelineInput};
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use gal_engine::ExitCode;
use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use pipeline::task_spec::{
    extract_plan_slug, has_test_first_marker, parse_task_contract, Applicability,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

// --- arg parsing ---

struct Args {
    prompt: PathBuf,
    receipt: PathBuf,
    terminal_reverify: bool,
}

fn default_receipt_path(prompt: &Path) -> PathBuf {
    resolve_receipt_path(Some(prompt), "preflight.receipt.md")
}

fn terminal_reverify_receipt_path(prompt: &Path) -> PathBuf {
    resolve_receipt_path(Some(prompt), "terminal-reverify.receipt.md")
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut terminal_reverify = false;
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
    if terminal_reverify && receipt.is_some() {
        return Err("--terminal-reverify is prompt-only and does not accept --receipt".to_string());
    }
    let receipt = receipt.unwrap_or_else(|| {
        if terminal_reverify {
            terminal_reverify_receipt_path(&prompt)
        } else {
            default_receipt_path(&prompt)
        }
    });
    Ok(Args {
        prompt,
        receipt,
        terminal_reverify,
    })
}

// --- checks ---

fn render_route_line(table: &dispatch::routing::RoutingTable) -> String {
    if table.entries.is_empty() {
        return "(no executorRouting configured)".to_string();
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
}

/// Regex-free check: does the text contain a well-formed task bullet `- [ ] T-NN`
/// or `- [x] T-NN` (two-digit or more)?
fn has_well_formed_task(prompt_text: &str) -> bool {
    prompt_text.lines().any(|l| {
        let t = l.trim_start();
        let rest = t
            .strip_prefix("- [ ] ")
            .or_else(|| t.strip_prefix("- [x] "));
        rest.is_some_and(|r| {
            // First whitespace-delimited token must be T-NN (T + digits ≥2)
            let tok = r.split_whitespace().next().unwrap_or("");
            tok.len() >= 4
                && tok.starts_with('T')
                && tok[1..].starts_with('-')
                && tok[2..].chars().all(|c| c.is_ascii_digit())
                && tok[2..].len() >= 2
        })
    })
}

/// check: `## Tasks` section has ≥1 well-formed T-NN bullet.
fn check_tasks_well_formed(prompt_text: &str) -> CheckOutcome {
    let name = "tasks-well-formed".to_string();
    if has_well_formed_task(prompt_text) {
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
    for line in prompt_text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") {
            in_tasks = trimmed == "## Tasks";
            continue;
        }
        if !in_tasks || trimmed.starts_with("### ") {
            continue;
        }
        if trimmed.starts_with("- [x] ") || trimmed.starts_with("- [ ] ") {
            let task = trimmed[6..].split_whitespace().next().unwrap_or("");
            if task.starts_with("T-") {
                task_count += 1;
                if trimmed.starts_with("- [ ] ") {
                    unchecked += 1;
                }
            }
        }
    }
    if task_count == 0 {
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

/// Extract task IDs from `## Tasks` section.
fn extract_task_ids(prompt_text: &str) -> Vec<String> {
    let mut task_ids = Vec::new();
    let mut in_tasks = false;
    for line in prompt_text.lines() {
        let t = line.trim();
        if t.starts_with("## ") {
            in_tasks = t == "## Tasks";
            continue;
        }
        if !in_tasks {
            continue;
        }
        if t.starts_with("### ") {
            continue;
        }
        let rest = t
            .strip_prefix("- [ ] ")
            .or_else(|| t.strip_prefix("- [x] "))
            .or_else(|| t.strip_prefix("- [] "));
        if let Some(r) = rest {
            let r = r.trim_start().strip_prefix("**").unwrap_or(r.trim_start());
            let tok = r.split_whitespace().next().unwrap_or("");
            let tok = tok
                .strip_suffix("**")
                .unwrap_or(tok)
                .trim_end_matches(':')
                .trim_end_matches('—')
                .trim_end_matches('-')
                .trim();
            if tok.len() >= 4
                && tok.starts_with('T')
                && tok[1..].starts_with('-')
                && tok[2..].chars().all(|c| c.is_ascii_digit())
                && tok[2..].len() >= 2
                && !task_ids.contains(&tok.to_string())
            {
                task_ids.push(tok.to_string());
            }
        }
    }
    task_ids
}

/// check: for `test-first-v1` marked prompts, validate all task contracts.
fn check_test_first_contracts(
    prompt_path: &Path,
    prompt_text: &str,
    repo_root: &Path,
) -> CheckOutcome {
    let name = "test-first-contracts".to_string();
    let plan_slug = extract_plan_slug(&prompt_path.to_string_lossy());
    let task_ids = extract_task_ids(prompt_text);

    if task_ids.is_empty() {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "## Tasks has no well-formed task bullets for contract validation".to_string(),
        };
    }

    for task_id in &task_ids {
        let contract = match parse_task_contract(prompt_text, &plan_slug, task_id) {
            Ok(c) => c,
            Err(e) => {
                return CheckOutcome {
                    name,
                    command: None,
                    state: CheckState::Fail,
                    summary: format!("task {task_id} contract validation failed: {e}"),
                };
            }
        };

        match contract.applicability {
            Applicability::Required => {
                if contract.seam.trim().is_empty() {
                    return CheckOutcome {
                        name,
                        command: None,
                        state: CheckState::Fail,
                        summary: format!("task {task_id} missing required field: Seam"),
                    };
                }
                if contract.expected_failures.is_empty() {
                    return CheckOutcome {
                        name,
                        command: None,
                        state: CheckState::Fail,
                        summary: format!(
                            "task {task_id} missing required field: Expected failures"
                        ),
                    };
                }
                if contract.production_paths.is_empty() {
                    return CheckOutcome {
                        name,
                        command: None,
                        state: CheckState::Fail,
                        summary: format!("task {task_id} missing required field: Production Paths"),
                    };
                }
                if contract.test_paths.is_empty() {
                    return CheckOutcome {
                        name,
                        command: None,
                        state: CheckState::Fail,
                        summary: format!("task {task_id} missing required field: Test Paths"),
                    };
                }
                if contract.scaffold != "required" && contract.scaffold != "not-required" {
                    return CheckOutcome {
                        name,
                        command: None,
                        state: CheckState::Fail,
                        summary: format!(
                            "task {task_id} has invalid scaffold value: '{}'",
                            contract.scaffold
                        ),
                    };
                }

                // Path validation via ValidatedRepoPath (reusing the shared foundation validator)
                for path_str in contract
                    .production_paths
                    .iter()
                    .chain(contract.test_paths.iter())
                {
                    match ValidatedRepoPath::new(
                        repo_root,
                        Path::new(path_str),
                        ValidatedRepoPathMode::RegularFileOrMissing,
                    ) {
                        Ok(_) => {}
                        Err(e) => {
                            return CheckOutcome {
                                name,
                                command: None,
                                state: CheckState::Fail,
                                summary: format!(
                                    "task {task_id} has invalid path '{path_str}': {e}"
                                ),
                            };
                        }
                    }
                }
            }
            Applicability::NotApplicable => {
                if contract.not_applicable_rationale.trim().is_empty() {
                    return CheckOutcome {
                        name,
                        command: None,
                        state: CheckState::Fail,
                        summary: format!(
                            "task {task_id} is not-applicable but missing technical rationale"
                        ),
                    };
                }
                if contract.non_red_probe.trim().is_empty() {
                    return CheckOutcome {
                        name,
                        command: None,
                        state: CheckState::Fail,
                        summary: format!(
                            "task {task_id} is not-applicable but missing reproducible non-red probe"
                        ),
                    };
                }
            }
        }
    }

    CheckOutcome {
        name,
        command: None,
        state: CheckState::Pass,
        summary: format!(
            "validated dual schemas for {} task contract(s)",
            task_ids.len()
        ),
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

/// check: for marked prompts under terminal recovery, marked digest must match transition journal.
fn check_terminal_marked_digest(evaluation: &MarkedDigestResult) -> CheckOutcome {
    let name = "terminal-marked-digest".to_string();

    match evaluation {
        MarkedDigestResult::Match {
            current_prompt_digest,
            ..
        } => CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!(
                "marked prompt digest matches transition journal ({current_prompt_digest})"
            ),
        },
        MarkedDigestResult::DigestMismatch {
            current_prompt_digest,
            matched_committed_journal_digest,
        } => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "marked prompt digest mismatch: expected {matched_committed_journal_digest}, got {current_prompt_digest}"
            ),
        },
        MarkedDigestResult::Prepared { prepared_digest, .. } => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "marked prompt transition journal state is prepared (digest: {prepared_digest})"
            ),
        },
        MarkedDigestResult::Missing { .. } => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "marked prompt transition journal is missing".to_string(),
        },
        MarkedDigestResult::Ambiguous { error, .. } => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("marked prompt transition journal is ambiguous: {error}"),
        },
        // Unreachable by construction: this evaluator only runs when the
        // prompt is marked, and a marked prompt never evaluates to `Unbound`.
        // The arm exists for match exhaustiveness, not as a live pass path.
        MarkedDigestResult::Unbound { .. } => CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "prompt is not marked for test-first journal binding".to_string(),
        },
    }
}

enum PreflightReceiptMode<'a> {
    Ordinary,
    TerminalReverify {
        repo_root: &'a Path,
        is_marked: bool,
        marked_digest_eval: Option<&'a MarkedDigestResult>,
        prompt_text: &'a str,
    },
}

fn render_receipt(
    receipt: &Receipt,
    prompt: &Path,
    cursor: &str,
    mode: PreflightReceiptMode<'_>,
) -> String {
    let mut out = String::new();
    match mode {
        PreflightReceiptMode::Ordinary => {
            let stem = prompt.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            out.push_str(&format!(
                "# pipeline-preflight receipt\n\nprompt: {stem}\nresume-cursor: {cursor}\n\n"
            ));
        }
        PreflightReceiptMode::TerminalReverify {
            repo_root,
            is_marked,
            marked_digest_eval,
            prompt_text,
        } => {
            let prompt_path_str = prompt.to_string_lossy().replace('\\', "/");
            let prompt_sha256 = format!("{:x}", Sha256::digest(prompt_text.as_bytes()));
            let head = get_head_sha(repo_root);
            let digest_result = if !is_marked {
                "not-applicable"
            } else {
                match marked_digest_eval {
                    Some(MarkedDigestResult::Match { .. }) => "pass",
                    Some(MarkedDigestResult::DigestMismatch { .. }) => "digest-mismatch",
                    Some(MarkedDigestResult::Prepared { .. }) => "prepared",
                    Some(MarkedDigestResult::Missing { .. }) => "missing",
                    Some(MarkedDigestResult::Ambiguous { .. }) => "ambiguous",
                    Some(MarkedDigestResult::Unbound { .. }) | None => "not-applicable",
                }
            };
            out.push_str(&format!(
                "# pipeline-preflight terminal-reverify receipt\n\n\
                 prompt_path: {prompt_path_str}\n\
                 prompt_sha256: {prompt_sha256}\n\
                 head: {head}\n\
                 mode: terminal-reverify\n\
                 digest_result: {digest_result}\n\
                 resume-cursor: {cursor}\n\n"
            ));
        }
    }
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

/// Run `gal pipeline-preflight`. Parse args, classify input, run checks, write receipt.
pub(crate) fn cmd_pipeline_preflight(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal pipeline-preflight: {e}");
            return ExitCode::Usage;
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

    let table = dispatch::routing::load_routing_default();
    println!("{}", render_route_line(&table));
    for w in &table.warnings {
        eprintln!("warning: {w}");
    }

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let prompt_text = std::fs::read_to_string(&parsed.prompt).unwrap_or_default();

    let cursor = read_resume_cursor(&prompt_text);

    let is_marked = has_test_first_marker(&prompt_text);
    let marked_digest_eval = if is_marked {
        Some(evaluate_marked_digest(
            &prompt_text,
            &repo_root,
            &parsed.prompt,
        ))
    } else {
        None
    };

    let mut checks = if parsed.terminal_reverify {
        let mut list = vec![
            check_workflow_done(&prompt_text),
            check_terminal_tasks_checked(&prompt_text),
            check_terminal_cursor_closed(&prompt_text),
            check_terminal_working_tree_clean(&repo_root),
            check_wrong_plan(&parsed.prompt, &repo_root),
        ];
        if let Some(ref eval) = marked_digest_eval {
            list.push(check_terminal_marked_digest(eval));
        }
        list
    } else {
        vec![
            check_tasks_well_formed(&prompt_text),
            check_test_plan_present(&prompt_text),
            check_no_root_blocking(&prompt_text),
            check_workflow_not_done(&prompt_text),
            check_wrong_plan(&parsed.prompt, &repo_root),
        ]
    };

    if is_marked {
        checks.push(check_test_first_contracts(
            &parsed.prompt,
            &prompt_text,
            &repo_root,
        ));
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
            is_marked,
            marked_digest_eval: marked_digest_eval.as_ref(),
            prompt_text: &prompt_text,
        }
    } else {
        PreflightReceiptMode::Ordinary
    };
    let content = render_receipt(&receipt, &parsed.prompt, &cursor, mode);
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
    fn default_receipt_path_matches_shared_plan_scope() {
        let prompt = Path::new(".dev/plans/receipt-scope.prompt.md");
        assert_eq!(
            default_receipt_path(prompt),
            resolve_receipt_path(Some(prompt), "preflight.receipt.md")
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
            render_route_line(&table),
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
            render_route_line(&table),
            "coder: claude: claude-sonnet-5 (default); tester: (unrouted); auditor: claude: claude-sonnet-5 medium ssh:dev@box"
        );
    }

    #[test]
    fn render_route_line_tp_03_handles_empty_and_non_pipeline_roles() {
        assert_eq!(
            render_route_line(&dispatch::routing::RoutingTable::default()),
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
            render_route_line(&table),
            "coder: (unrouted); tester: (unrouted); auditor: (unrouted)"
        );
    }

    fn minimal_prompt() -> String {
        let task = format!("T-{:02}", 1usize);
        format!(
            "## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {task}\n\n\
             ## Tasks\n\n- [ ] {task} — do something\n\n\
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
}
