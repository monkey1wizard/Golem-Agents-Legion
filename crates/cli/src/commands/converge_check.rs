//! `gal pipeline-converge-check` — pipeline-internal 2g closeout receipt check.
//!
//! Three checks — all must pass for exit 0:
//!   (a) Three-surface checkbox agreement + the nominated task is `[x]` in both.
//!   (b) The task line's recorded `*(hash)*` commit note resolves to a real commit.
//!   (c) `## Status` `Current Task:` is no longer set to this task (cursor cleared).
//!
//! Receipt is the sole pass-basis; fail/not-run → non-zero.
//! Not a public `/gal` slash command (peer of `finalize-check`/`boundary-check`).

use super::finalize_check::{
    check_three_surface, checked_task_ids, commit_note_hash, CheckOutcome, CheckState, Receipt,
};
use gal_engine::ExitCode;
use std::path::{Path, PathBuf};

struct Args {
    prompt: PathBuf,
    task: String,
    receipt: PathBuf,
}

fn default_receipt_path(task: &str) -> PathBuf {
    Path::new(".dev")
        .join("pipeline")
        .join("receipts")
        .join(format!("{task}-converge-check.receipt.md"))
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
    let receipt = receipt.unwrap_or_else(|| default_receipt_path(&task));
    Ok(Args {
        prompt: prompt.ok_or("pipeline-converge-check requires a <prompt> path")?,
        task,
        receipt,
    })
}

/// check(a): three-surface checkbox agreement AND the nominated task is `[x]` in
/// both the source plan and the execution prompt.
fn check_three_surface_and_task(
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
fn check_task_commit(task: &str, prompt_text: &str, repo_root: &Path) -> CheckOutcome {
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

/// check(c): `## Status` `Current Task:` must be cleared (not pointing at this task).
fn check_cursor_cleared(task: &str, prompt_text: &str) -> CheckOutcome {
    let name = "cursor-cleared".to_string();
    // Find the "Current Task:" line in ## Status.
    let current_task = prompt_text
        .lines()
        .find(|l| l.trim_start().starts_with("Current Task:"))
        .map(|l| l.trim_start()["Current Task:".len()..].trim().to_string());
    match current_task {
        None => CheckOutcome {
            name,
            command: None,
            state: CheckState::NotRun,
            summary: "no 'Current Task:' line found in ## Status".to_string(),
        },
        Some(ref ct) if ct == task || ct == &format!("T-{}", &task[2..]) => CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("Current Task is still '{ct}' — cursor not cleared"),
        },
        Some(ct) => {
            let display = if ct.is_empty() || ct == "—" {
                "cleared".to_string()
            } else {
                format!("'{ct}'")
            };
            CheckOutcome {
                name,
                command: None,
                state: CheckState::Pass,
                summary: format!("Current Task: {display} (≠ {})", task),
            }
        }
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

    let checks = vec![
        check_three_surface_and_task(&parsed.task, &parsed.prompt, &prompt_text, &repo_root),
        check_task_commit(&parsed.task, &prompt_text, &repo_root),
        check_cursor_cleared(&parsed.task, &prompt_text),
    ];
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
}
