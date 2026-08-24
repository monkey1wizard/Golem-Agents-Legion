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
}

fn default_receipt_path(prompt: &Path) -> PathBuf {
    resolve_receipt_path(Some(prompt), "prompt-check.receipt.md")
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
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
    let receipt = receipt.unwrap_or_else(|| default_receipt_path(&prompt));
    Ok(Args { prompt, receipt })
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

fn run_checks(prompt_text: &str) -> Vec<CheckOutcome> {
    let task_re = Regex::new(r"^- \[ \] T-[0-9]{2}\b").expect("task regex compiles");
    let lines = non_fenced_lines(prompt_text);
    let has_task = lines.iter().any(|l| task_re.is_match(l.trim_start()));
    vec![
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
            return ExitCode::Usage;
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
    if let Err(err) = std::fs::write(&parsed.receipt, render_receipt(&receipt)) {
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
            default_receipt_path(prompt),
            resolve_receipt_path(Some(prompt), "prompt-check.receipt.md")
        );
        let parsed = parse_args(&[
            prompt.display().to_string(),
            "--receipt".to_string(),
            "custom/receipt.md".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.receipt, PathBuf::from("custom/receipt.md"));
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
    }

    #[test]
    fn full_prompt_passes() {
        let receipt = Receipt {
            checks: run_checks(&full_prompt()),
        };
        assert!(receipt.passed());
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
