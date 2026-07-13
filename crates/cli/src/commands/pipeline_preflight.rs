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

use super::dispatch::{resolve_pipeline_input, PipelineInput};
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use gal_engine::ExitCode;
use std::path::{Path, PathBuf};

// --- arg parsing ---

struct Args {
    prompt: PathBuf,
    receipt: PathBuf,
}

fn default_receipt_path(prompt: &Path) -> PathBuf {
    let stem = prompt
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_suffix(".prompt.md"))
        .unwrap_or("preflight");
    Path::new(".dev")
        .join("pipeline")
        .join("receipts")
        .join(format!("{stem}-preflight.receipt.md"))
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
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
    let receipt = receipt.unwrap_or_else(|| default_receipt_path(&prompt));
    Ok(Args { prompt, receipt })
}

// --- checks ---

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

/// check: no root-level BLOCKING open question.
///
/// Scans for lines that contain `BLOCKING` outside of completed-task blocks,
/// within `## Open Questions` or `## Review Results` sections.
fn check_no_root_blocking(prompt_text: &str) -> CheckOutcome {
    let name = "no-root-blocking".to_string();
    // Quick heuristic: a BLOCKING line that is NOT inside a `- [x]` task block.
    // We look for `BLOCKING` in a line that starts with `- [ ]` or is a bare paragraph.
    let blocking_found = prompt_text.lines().any(|l| {
        let t = l.trim();
        // Skip completed-task lines (- [x])
        if t.starts_with("- [x]") {
            return false;
        }
        t.contains("BLOCKING")
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

fn render_receipt(receipt: &Receipt, prompt: &Path, cursor: &str) -> String {
    let stem = prompt.file_name().and_then(|n| n.to_str()).unwrap_or("?");
    let mut out =
        format!("# pipeline-preflight receipt\n\nprompt: {stem}\nresume-cursor: {cursor}\n\n");
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

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let prompt_text = std::fs::read_to_string(&parsed.prompt).unwrap_or_default();

    let cursor = read_resume_cursor(&prompt_text);

    let checks = vec![
        check_tasks_well_formed(&prompt_text),
        check_test_plan_present(&prompt_text),
        check_no_root_blocking(&prompt_text),
        check_workflow_not_done(&prompt_text),
        check_wrong_plan(&parsed.prompt, &repo_root),
    ];
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

    let content = render_receipt(&receipt, &parsed.prompt, &cursor);
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
        let result = cmd_pipeline_preflight(&[
            "pipeline-preflight".to_string(),
            source_plan_path,
        ]);
        assert_eq!(
            result,
            ExitCode::Usage,
            "source-plan input must return Usage, not proceed to checks"
        );
    }
}
