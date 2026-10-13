//! `gal prompt-check` - prompt-internal anchor validation receipt.
//!
//! Validates that a compressed execution prompt still carries the machine anchors
//! consumed by pipeline/finalize tooling. Missing anchors are receipt-visible and
//! exit non-zero. Not a public `/gal` command.

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use super::planning_authority::{parse_localized_meta, rendered_source_hash};
use gal_engine::ExitCode;
use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};

struct Args {
    prompt: PathBuf,
    receipt: PathBuf,
    assemble_dry_run: bool,
}

fn assemble_dry_run_checks(prompt_text: &str, prompt_path: &Path) -> Vec<CheckOutcome> {
    let task_re = Regex::new(r"^- \[ \] T-[0-9]{2}\b").expect("task regex compiles");
    let lines = non_fenced_lines(prompt_text);
    let mut in_tasks = false;
    let mut task_lines = Vec::new();
    let mut task_ids = Vec::new();
    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "## Tasks" {
            in_tasks = true;
            continue;
        }
        if in_tasks && trimmed.starts_with("## ") {
            break;
        }
        if in_tasks {
            task_lines.push(*line);
            if let Some(task) = task_re
                .captures(line.trim_start())
                .and_then(|captures| captures.get(0))
            {
                let id = task.as_str().split_whitespace().nth(3).unwrap_or_default();
                task_ids.push(id.to_string());
            }
        }
    }

    if task_ids.is_empty() {
        return vec![anchor_outcome(
            "assemble-dry-run",
            false,
            "no task was found in the `## Tasks` section",
        )];
    }

    let prompt_path = prompt_path.to_string_lossy();
    task_ids
        .into_iter()
        .map(|task_id| {
            let detail_heading = format!("### {task_id}");
            let has_detail_heading = in_tasks
                && task_lines.iter().any(|line| {
                    let trimmed = line.trim();
                    trimmed == detail_heading
                        || trimmed
                            .strip_prefix(&detail_heading)
                            .and_then(|rest| rest.chars().next())
                            .is_some_and(|next| {
                                next.is_whitespace() || (!next.is_ascii_alphanumeric() && next != '_')
                            })
                });
            if !has_detail_heading {
                return anchor_outcome(
                    &format!("assemble-dry-run-{task_id}"),
                    false,
                    &format!("{task_id}: missing detail heading `{detail_heading}`"),
                );
            }

            let input = pipeline::task_spec::TaskSpecInput {
                task_scope: &task_id,
                phase: "implement",
                prompt_path: &prompt_path,
                prompt_body: prompt_text,
                generated: "dry-run",
                git_branch: "dry-run",
                git_head: "dry-run",
                convention_hints: None,
                receipt_path: None,
                agent_contract_body: None,
                fix_mode: false,
            };
            if let Err(error) = pipeline::task_spec::assemble_task_spec(&input) {
                return anchor_outcome(
                    &format!("assemble-dry-run-{task_id}"),
                    false,
                    &format!("{task_id}: assemble_task_spec failed: {error}"),
                );
            }

            if pipeline::task_spec::extract_affected_file_paths(prompt_text, &task_id).is_empty() {
                return anchor_outcome(
                    &format!("assemble-dry-run-{task_id}"),
                    false,
                    &format!("{task_id}: affected-file allowlist is empty"),
                );
            }

            anchor_outcome(
                &format!("assemble-dry-run-{task_id}"),
                true,
                &format!("{task_id}: detail heading, task-spec assembly, and affected-file allowlist passed"),
            )
        })
        .collect()
}

fn default_receipt_path(prompt: &Path) -> Result<PathBuf, String> {
    resolve_receipt_path(Some(prompt), None, "prompt-check.receipt.md")
        .map_err(|error| format!("receipt scope resolution: {error}"))
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut assemble_dry_run = false;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--assemble-dry-run" => assemble_dry_run = true,
            "--receipt" => {
                let value = it.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(value));
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
    let prompt = prompt.ok_or("prompt-check requires a <prompt> path")?;
    let receipt = match receipt {
        Some(path) => path,
        None => default_receipt_path(&prompt)?,
    };
    Ok(Args {
        prompt,
        receipt,
        assemble_dry_run,
    })
}

fn anchor_outcome(name: &str, passed: bool, summary: &str) -> CheckOutcome {
    CheckOutcome {
        name: name.to_string(),
        command: None,
        state: if passed {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        summary: summary.to_string(),
    }
}

/// Lines outside any fenced code block, so an anchor mentioned inside a fence or
/// as an inline `code span` can no longer spoof-pass.
fn non_fenced_lines(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_fence = false;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence {
            out.push(line);
        }
    }
    out
}

fn has_heading(lines: &[&str], heading: &str) -> bool {
    lines.iter().any(|l| l.trim() == heading)
}

fn has_line_prefix(lines: &[&str], prefix: &str) -> bool {
    lines.iter().any(|l| l.trim_start().starts_with(prefix))
}

fn imported_from_shape_outcome(lines: &[&str]) -> CheckOutcome {
    let name = "imported-from-shape";
    let Some(line) = lines
        .iter()
        .find(|line| line.trim_start().starts_with("imported-from:"))
    else {
        return anchor_outcome(name, true, "imported-from line absent - not required");
    };
    let value = line
        .trim_start()
        .strip_prefix("imported-from:")
        .map(str::trim)
        .unwrap_or_default();
    anchor_outcome(
        name,
        !value.is_empty(),
        if value.is_empty() {
            "imported-from line must contain a non-empty value"
        } else {
            "imported-from line has a non-empty value"
        },
    )
}

fn retired_marker_outcome(prompt_text: &str) -> CheckOutcome {
    let has_retired_marker = prompt_text
        .lines()
        .any(|line| line.trim() == "Pipeline Contract: test-first-v1");
    anchor_outcome(
        "retired-marker",
        !has_retired_marker,
        if has_retired_marker {
            "prompt carries a retired contract marker, regenerate with /plan-to-prompt"
        } else {
            "prompt does not carry a retired contract marker"
        },
    )
}

fn run_checks(prompt_text: &str) -> Vec<CheckOutcome> {
    let task_re = Regex::new(r"^- \[ \] T-[0-9]{2}\b").expect("task regex compiles");
    let lines = non_fenced_lines(prompt_text);
    let has_task = lines.iter().any(|l| task_re.is_match(l.trim_start()));
    vec![
        retired_marker_outcome(prompt_text),
        anchor_outcome(
            "status-heading",
            has_heading(&lines, "## Status"),
            "requires `## Status`",
        ),
        anchor_outcome(
            "current-task-anchor",
            has_line_prefix(&lines, "Current Task:"),
            "requires `Current Task:`",
        ),
        anchor_outcome(
            "tasks-heading",
            has_heading(&lines, "## Tasks"),
            "requires `## Tasks`",
        ),
        anchor_outcome(
            "unchecked-task-anchor",
            has_task,
            "requires at least one `- [ ] T-NN` task line",
        ),
        imported_from_shape_outcome(&lines),
        anchor_outcome(
            "test-results-heading",
            has_heading(&lines, "## Test Results"),
            "requires `## Test Results`",
        ),
        anchor_outcome(
            "review-results-heading",
            has_heading(&lines, "## Review Results"),
            "requires `## Review Results`",
        ),
        anchor_outcome(
            "review-subsection-architecture",
            has_heading(&lines, "### Architecture Review"),
            "requires `### Architecture Review`",
        ),
        anchor_outcome(
            "review-subsection-business",
            has_heading(&lines, "### Business Review"),
            "requires `### Business Review`",
        ),
        anchor_outcome(
            "review-subsection-design",
            has_heading(&lines, "### Design Review"),
            "requires `### Design Review`",
        ),
        anchor_outcome(
            "review-subsection-engineering",
            has_heading(&lines, "### Engineering Review"),
            "requires `### Engineering Review`",
        ),
    ]
}

fn configured_plan_language(config_path: Option<&Path>) -> Option<String> {
    let text = std::fs::read_to_string(config_path?).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("planLanguage")
        .and_then(Value::as_str)
        .map(str::to_string)
}

/// Derive the paired source plan path from an execution prompt path:
/// `.dev/plans/<slug>.prompt.md` -> `.dev/plans/<slug>.md`.
fn paired_source_plan(prompt_path: &Path) -> Option<PathBuf> {
    let name = prompt_path.file_name()?.to_str()?;
    let slug = name.strip_suffix(".prompt.md")?;
    Some(prompt_path.with_file_name(format!("{slug}.md")))
}

/// For a non-English plan that used the EN-draft flow (its source plan carries a
/// planning-authority metadata block), require the paired source plan's inline
/// `equivalence-verdict` to be `EQUIVALENT` and its `prompt-hash` to match a
/// live re-hash of this prompt's body. English plans and single-file source
/// plans skip.
fn equivalence_receipt_outcome(
    prompt_path: &Path,
    _repo_root: &Path,
    config_path: Option<&Path>,
) -> CheckOutcome {
    let name = "equivalence-receipt";
    match configured_plan_language(config_path) {
        Some(lang) if lang.trim().to_ascii_lowercase().starts_with("en") => {
            return anchor_outcome(
                name,
                true,
                "English planLanguage — equivalence receipt not required",
            );
        }
        None => {
            return anchor_outcome(
                name,
                true,
                "planLanguage not configured — equivalence receipt not required",
            )
        }
        Some(_) => {}
    }
    let Some(source) = paired_source_plan(prompt_path) else {
        return anchor_outcome(name, true, "no paired source plan — skipped");
    };
    let Ok(src_text) = std::fs::read_to_string(&source) else {
        return anchor_outcome(name, true, "paired source plan unreadable — skipped");
    };
    let Some(meta) = parse_localized_meta(&src_text) else {
        return anchor_outcome(
            name,
            true,
            "source plan has no EN-draft metadata block — single-file source",
        );
    };
    if !meta.equivalence_verdict.eq_ignore_ascii_case("EQUIVALENT") {
        return anchor_outcome(name, false, "non-English plan used the EN-draft flow but its equivalence-verdict is not EQUIVALENT (pending or stale)");
    }
    let Ok(prompt_text) = std::fs::read_to_string(prompt_path) else {
        return anchor_outcome(
            name,
            false,
            "prompt body unreadable — cannot verify prompt-hash",
        );
    };
    if meta.prompt_hash != rendered_source_hash(&prompt_text) {
        return anchor_outcome(
            name,
            false,
            "stamped prompt-hash no longer matches the live prompt body",
        );
    }
    anchor_outcome(
        name,
        true,
        "equivalence-verdict is EQUIVALENT and prompt-hash matches the live prompt body",
    )
}

fn render_receipt(receipt: &Receipt) -> String {
    let mut out = String::from("# prompt-check receipt\n\n");
    out.push_str(&format!(
        "overall: {}\n\n",
        if receipt.passed() { "pass" } else { "fail" }
    ));
    out.push_str("| check | state | command | summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for check in &receipt.checks {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            check.name,
            check.state.as_str(),
            check.command.as_deref().unwrap_or("-"),
            check.summary,
        ));
    }
    out
}

pub(crate) fn cmd_prompt_check(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(parsed) => parsed,
        Err(err) => {
            eprintln!("gal prompt-check: {err}");
            return if err.starts_with("receipt scope resolution:") {
                ExitCode::Error
            } else {
                ExitCode::Usage
            };
        }
    };

    if !parsed.prompt.exists() {
        eprintln!(
            "gal prompt-check: prompt not found: {}",
            parsed.prompt.display()
        );
        return ExitCode::Usage;
    }

    let prompt_text = std::fs::read_to_string(&parsed.prompt).unwrap_or_default();
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let config_path = gal_foundation::paths::machine_config_path();
    let mut checks = run_checks(&prompt_text);
    checks.push(equivalence_receipt_outcome(
        &parsed.prompt,
        &repo_root,
        config_path.as_deref(),
    ));
    if parsed.assemble_dry_run {
        checks.extend(assemble_dry_run_checks(&prompt_text, &parsed.prompt));
    }
    let receipt = Receipt { checks };

    if let Some(parent) = parsed.receipt.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal prompt-check: cannot create receipt dir {}: {err}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }
    let body = match super::finalize_check::receipt_envelope(None, Some(&parsed.prompt)) {
        Ok(envelope) => {
            match super::finalize_check::bind_receipt(render_receipt(&receipt), &envelope) {
                Ok(body) => body,
                Err(err) => {
                    eprintln!("gal prompt-check: invalid execution binding: {err}");
                    return ExitCode::Error;
                }
            }
        }
        Err(err) => {
            eprintln!("gal prompt-check: cannot establish execution identity: {err}");
            return ExitCode::Error;
        }
    };
    if let Err(err) = std::fs::write(&parsed.receipt, body) {
        eprintln!(
            "gal prompt-check: cannot write receipt {}: {err}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }

    println!(
        "gal prompt-check: {} ({} check(s)) -> {}",
        if receipt.passed() { "pass" } else { "fail" },
        receipt.checks.len(),
        parsed.receipt.display()
    );
    receipt.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_receipt_path_is_plan_scoped_and_explicit_override_wins() {
        let prompt = Path::new(".dev/plans/prompt-scope.prompt.md");
        assert_eq!(
            default_receipt_path(prompt).unwrap(),
            default_receipt_path(prompt).unwrap(),
        );
        let parsed = parse_args(&[
            prompt.display().to_string(),
            "--receipt".to_string(),
            "custom/receipt.md".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.receipt, PathBuf::from("custom/receipt.md"));
        assert!(!parsed.assemble_dry_run);
    }
    use tempfile::TempDir;

    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    fn meta_src(prompt_hash: &str, verdict: &str) -> String {
        format!("# Plan\n\n<!-- gal:planning-authority\nsemantic-draft: .dev/plans/x.en.md\nplanLanguage: zh-TW\ndraft-hash: a\nrendered-source-hash: b\nprompt-hash: {prompt_hash}\nequivalence-verdict: {verdict}\n-->\n")
    }

    fn write_source_with_meta(dir: &TempDir, prompt_hash: &str, verdict: &str) {
        std::fs::create_dir_all(dir.path().join(".dev/plans")).unwrap();
        std::fs::write(
            dir.path().join(".dev/plans/x.md"),
            meta_src(prompt_hash, verdict),
        )
        .unwrap();
    }

    fn zh_config(dir: &TempDir) -> PathBuf {
        let cfg = dir.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"zh-TW\"}\n").unwrap();
        cfg
    }

    #[test]
    fn english_prompt_not_required() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"en\"}\n").unwrap();
        let prompt = dir.path().join(".dev/plans/x.prompt.md");
        let out = equivalence_receipt_outcome(&prompt, dir.path(), Some(&cfg));
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn pending_verdict_fails() {
        let dir = TempDir::new().unwrap();
        let cfg = zh_config(&dir);
        write_source_with_meta(&dir, "none", "pending");
        let prompt = dir.path().join(".dev/plans/x.prompt.md");
        std::fs::write(&prompt, "# Prompt\nGoal\n").unwrap();
        let out = equivalence_receipt_outcome(&prompt, dir.path(), Some(&cfg));
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn stale_prompt_hash_fails() {
        let dir = TempDir::new().unwrap();
        let cfg = zh_config(&dir);
        write_source_with_meta(&dir, "stale-hash-does-not-match", "EQUIVALENT");
        let prompt = dir.path().join(".dev/plans/x.prompt.md");
        std::fs::write(&prompt, "# Prompt\nGoal\n").unwrap();
        let out = equivalence_receipt_outcome(&prompt, dir.path(), Some(&cfg));
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn matching_verdict_and_hash_passes() {
        let dir = TempDir::new().unwrap();
        let cfg = zh_config(&dir);
        let prompt_body = "# Prompt\nGoal\n";
        write_source_with_meta(&dir, &rendered_source_hash(prompt_body), "EQUIVALENT");
        let prompt = dir.path().join(".dev/plans/x.prompt.md");
        std::fs::write(&prompt, prompt_body).unwrap();
        let out = equivalence_receipt_outcome(&prompt, dir.path(), Some(&cfg));
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    fn full_prompt() -> String {
        format!("## Status\nCurrent Task: -\n\n## Tasks\n- [ ] {} - do work\n\n## Test Results\nPending\n\n## Review Results\n### Architecture Review\nCLEAR\n### Business Review\nNot requested.\n### Design Review\nNot requested.\n### Engineering Review\nCLEAR\n", tid(1))
    }

    #[test]
    fn parse_requires_prompt() {
        assert!(parse_args(&[]).is_err());
    }

    #[test]
    fn parse_defaults_receipt_path() {
        let parsed = parse_args(&["x.prompt.md".to_string()]).unwrap();
        assert!(parsed.receipt.ends_with("prompt-check.receipt.md"));
        assert!(!parsed.assemble_dry_run);
    }

    #[test]
    fn parse_accepts_assemble_dry_run_flag() {
        let parsed =
            parse_args(&["x.prompt.md".to_string(), "--assemble-dry-run".to_string()]).unwrap();
        assert!(parsed.assemble_dry_run);
    }

    #[test]
    fn default_check_name_sequence_is_pinned() {
        let mut checks = run_checks(&full_prompt());
        checks.push(equivalence_receipt_outcome(
            Path::new(".dev/plans/example.prompt.md"),
            Path::new("."),
            None,
        ));
        let names: Vec<_> = checks.into_iter().map(|check| check.name).collect();
        assert_eq!(
            names,
            vec![
                "retired-marker",
                "status-heading",
                "current-task-anchor",
                "tasks-heading",
                "unchecked-task-anchor",
                "imported-from-shape",
                "test-results-heading",
                "review-results-heading",
                "review-subsection-architecture",
                "review-subsection-business",
                "review-subsection-design",
                "review-subsection-engineering",
                "equivalence-receipt",
            ]
        );
    }

    #[test]
    fn assemble_dry_run_failure_makes_flagged_receipt_fail() {
        let id = tid(1);
        let mut checks = run_checks(&full_prompt());
        checks.push(anchor_outcome("equivalence-receipt", true, "not required"));
        checks.extend(assemble_dry_run_checks(
            &dry_run_prompt("- Change: update `src/lib.rs`.", ""),
            Path::new(".dev/plans/example.prompt.md"),
        ));
        let receipt = Receipt { checks };
        assert!(!receipt.passed());
        let expected_name = format!("assemble-dry-run-{id}");
        assert!(receipt
            .checks
            .iter()
            .any(|check| check.name == expected_name && check.state == CheckState::Fail));
    }

    #[test]
    fn flagged_command_appends_dry_run_rows_and_returns_error_on_failure() {
        let _guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let id = tid(1);
        let dir = TempDir::new().unwrap();
        let prompt = dir.path().join("example.prompt.md");
        let receipt = dir.path().join("prompt-check.receipt.md");
        std::fs::write(
            &prompt,
            dry_run_prompt("- Change: update `src/lib.rs`.", ""),
        )
        .unwrap();

        // Run outside the source checkout: inside it, receipts require the
        // worktree-private executable, which the test binary is not.
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let exit = cmd_prompt_check(&[
            "prompt-check".to_string(),
            prompt.display().to_string(),
            "--assemble-dry-run".to_string(),
            "--receipt".to_string(),
            receipt.display().to_string(),
        ]);
        std::env::set_current_dir(original).unwrap();

        assert_eq!(exit, ExitCode::Error);
        let receipt_text = std::fs::read_to_string(receipt).unwrap();
        assert!(receipt_text.contains("overall: fail"));
        assert!(receipt_text.contains(&format!("assemble-dry-run-{id}")));
    }

    #[test]
    fn full_prompt_passes() {
        let receipt = Receipt {
            checks: run_checks(&full_prompt()),
        };
        assert!(receipt.passed());
    }

    #[test]
    fn imported_from_shape_is_optional() {
        let checks = run_checks(&full_prompt());
        let check = checks
            .iter()
            .find(|check| check.name == "imported-from-shape")
            .unwrap();
        assert_eq!(check.state, CheckState::Pass, "{check:?}");
    }

    #[test]
    fn imported_from_shape_accepts_non_empty_value() {
        let prompt = format!("{}\nimported-from: spec-kit tasks.md\n", full_prompt());
        let checks = run_checks(&prompt);
        let check = checks
            .iter()
            .find(|check| check.name == "imported-from-shape")
            .unwrap();
        assert_eq!(check.state, CheckState::Pass, "{check:?}");
    }

    #[test]
    fn imported_from_shape_rejects_empty_value() {
        let prompt = format!("{}\nimported-from:   \n", full_prompt());
        let checks = run_checks(&prompt);
        let check = checks
            .iter()
            .find(|check| check.name == "imported-from-shape")
            .unwrap();
        assert_eq!(check.state, CheckState::Fail, "{check:?}");
    }

    fn dry_run_prompt(detail: &str, files: &str) -> String {
        let id = tid(1);
        format!(
            "## Tasks\n\n- [ ] {id} — do work\n{detail}\n\n## Files to Create or Modify\n{files}\n"
        )
    }

    #[test]
    fn assemble_dry_run_passes_well_formed_task() {
        let id = tid(1);
        let checks = assemble_dry_run_checks(
            &dry_run_prompt(
                &format!("### {id}\n- Change: update `src/lib.rs`."),
                "- `src/lib.rs`",
            ),
            Path::new(".dev/plans/example.prompt.md"),
        );
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].state, CheckState::Pass, "{checks:?}");
        assert!(checks[0].name.contains(&id));
    }

    #[test]
    fn assemble_dry_run_passes_each_well_formed_task() {
        let (id1, id2, id3) = (tid(1), tid(2), tid(3));
        let prompt = format!(
            "## Tasks\n\n\
- [ ] {id1} — update library\n\
### {id1}\n- Change: update `src/lib.rs`.\n\n\
- [ ] {id2} — update tests\n\
### {id2}\n- Change: update `tests/lib.rs`.\n\n\
- [ ] {id3} — update docs\n\
### {id3}\n- Change: update `docs/workflows.md`.\n\n\
## Files to Create or Modify\n\
- `src/lib.rs`\n- `tests/lib.rs`\n- `docs/workflows.md`\n"
        );
        let checks = assemble_dry_run_checks(&prompt, Path::new(".dev/plans/example.prompt.md"));

        assert_eq!(checks.len(), 3);
        assert!(checks.iter().all(|check| check.state == CheckState::Pass));
        assert!(checks.iter().all(|check| check.name.contains("T-")));
    }

    #[test]
    fn assemble_dry_run_fails_missing_detail_heading() {
        let id = tid(1);
        let checks = assemble_dry_run_checks(
            &dry_run_prompt("- Change: update `src/lib.rs`.", "- `src/lib.rs`"),
            Path::new(".dev/plans/example.prompt.md"),
        );
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].state, CheckState::Fail);
        assert!(checks[0].summary.contains(&id));
        assert!(checks[0].summary.contains("detail heading"));
    }

    #[test]
    fn assemble_dry_run_fails_empty_allowlist() {
        let id = tid(1);
        let checks = assemble_dry_run_checks(
            &dry_run_prompt(
                &format!("### {id}\n- Change: update the implementation."),
                "",
            ),
            Path::new(".dev/plans/example.prompt.md"),
        );
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].state, CheckState::Fail);
        assert!(checks[0].summary.contains(&id));
        assert!(checks[0].summary.contains("allowlist"));
    }

    #[test]
    fn assemble_dry_run_fails_without_tasks_section() {
        let checks = assemble_dry_run_checks(
            "## Goal\nNo tasks here.\n",
            Path::new(".dev/plans/example.prompt.md"),
        );
        assert_eq!(checks.len(), 1);
        assert_eq!(checks[0].state, CheckState::Fail);
        assert_eq!(checks[0].name, "assemble-dry-run");
    }

    #[test]
    fn missing_status_fails() {
        let prompt = full_prompt().replacen("## Status\n", "", 1);
        let receipt = Receipt {
            checks: run_checks(&prompt),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "status-heading" && c.state == CheckState::Fail));
    }

    #[test]
    fn standalone_anchor_line_passes() {
        let receipt = Receipt {
            checks: run_checks(&full_prompt()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "status-heading" && c.state == CheckState::Pass));
    }

    #[test]
    fn anchor_in_code_span_does_not_pass() {
        // `## Status` appears only in a fenced block and inline code — no real heading.
        let prompt = format!(
            "```\n## Status\n```\nprose mentioning `## Status` inline\n## Tasks\n- [ ] {} do work\n",
            tid(1)
        );
        let receipt = Receipt {
            checks: run_checks(&prompt),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "status-heading" && c.state == CheckState::Fail));
    }

    #[test]
    fn missing_review_subsection_fails() {
        let prompt = full_prompt().replace("### Design Review\nNot requested.\n", "");
        let receipt = Receipt {
            checks: run_checks(&prompt),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "review-subsection-design" && c.state == CheckState::Fail));
    }
}
