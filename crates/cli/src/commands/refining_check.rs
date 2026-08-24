//! `gal refining-check` - source-plan structure receipt.

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use super::test_plan_table::{covered_task_ids, scan_test_plan_rows};
use gal_engine::ExitCode;
use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

struct Args {
    target: PathBuf,
    receipt: PathBuf,
}

fn default_receipt_path(target: &Path) -> PathBuf {
    resolve_receipt_path(Some(target), "refining-check.receipt.md")
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut target = None;
    let mut receipt = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--receipt" => {
                receipt = Some(PathBuf::from(it.next().ok_or("--receipt requires a path")?))
            }
            s if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            s => {
                if target.replace(PathBuf::from(s)).is_some() {
                    return Err(format!("unexpected extra argument '{s}'"));
                }
            }
        }
    }
    let target = target.ok_or("refining-check requires a <source-plan> path")?;
    let receipt = receipt.unwrap_or_else(|| default_receipt_path(&target));
    Ok(Args { target, receipt })
}

fn section_text(text: &str, heading: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let idx = lines.iter().position(|line| line.trim() == heading)?;
    let start = idx + 1;
    let end = lines[start..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map(|offset| start + offset)
        .unwrap_or(lines.len());
    Some(lines[start..end].join("\n"))
}

fn outcome(name: &str, state: CheckState, summary: String) -> CheckOutcome {
    CheckOutcome {
        name: name.to_string(),
        command: None,
        state,
        summary,
    }
}

fn naming_gate_outcome(repo_root: &Path) -> CheckOutcome {
    use crate::gal::naming_gate::{load_retired_terms, NamingGate};
    let retired = load_retired_terms(repo_root);
    match NamingGate::new(&retired) {
        Ok(gate) => match gate.scan_tree(repo_root) {
            Ok(hits) => {
                let total: usize = hits.iter().map(|f| f.violations.len()).sum();
                outcome(
                    "naming-gate",
                    if total == 0 {
                        CheckState::Pass
                    } else {
                        CheckState::Fail
                    },
                    format!("{total} violation(s) across {} file(s)", hits.len()),
                )
            }
            Err(err) => outcome(
                "naming-gate",
                CheckState::Fail,
                format!("scan failed: {err}"),
            ),
        },
        Err(err) => outcome(
            "naming-gate",
            CheckState::Fail,
            format!("scanner build failed: {err}"),
        ),
    }
}

fn task_ids(tasks: &str) -> (Vec<String>, Vec<String>) {
    let exact = Regex::new(r"(?m)^- \[[ x]\] (T-[0-9]{2})\b").unwrap();
    let mut ids = Vec::new();
    let mut malformed = Vec::new();
    for line in tasks.lines() {
        let t = line.trim_start();
        if let Some(c) = exact.captures(t) {
            ids.push(c[1].to_string());
        } else if t.starts_with("- [ ] T-") || t.starts_with("- [x] T-") {
            malformed.push(t.to_string());
        }
    }
    (ids, malformed)
}

fn tp_ids(plan: &str) -> (Vec<String>, Vec<String>) {
    let exact = Regex::new(r"(?m)^\| (TP-[0-9]{2}) \|").unwrap();
    let mut ids = Vec::new();
    let mut malformed = Vec::new();
    for line in plan.lines() {
        let t = line.trim();
        if let Some(c) = exact.captures(t) {
            ids.push(c[1].to_string());
        } else if t.starts_with("| TP-") {
            malformed.push(t.to_string());
        }
    }
    (ids, malformed)
}

fn dupes(ids: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut dupes = BTreeSet::new();
    for id in ids {
        if !seen.insert(id.clone()) {
            dupes.insert(id.clone());
        }
    }
    dupes.into_iter().collect()
}

fn expand_brace_path(token: &str) -> Vec<String> {
    if let (Some(open), Some(close)) = (token.find('{'), token.find('}')) {
        if open < close {
            let prefix = &token[..open];
            let suffix = &token[close + 1..];
            return token[open + 1..close]
                .split(',')
                .map(|part| format!("{prefix}{}{suffix}", part.trim()))
                .collect();
        }
    }
    vec![token.to_string()]
}

fn repo_path_like(token: &str) -> bool {
    if token.starts_with("~/") || token.starts_with("/") || token == "config.json" {
        return false;
    }
    token.contains('/') || token.contains('\\') || token.starts_with('.')
}

fn referenced_paths(tasks: &str) -> Vec<String> {
    let backtick = Regex::new(r"`([^`]+)`").unwrap();
    let mut out = Vec::new();
    for line in tasks.lines() {
        let t = line.trim_start();
        if !t.starts_with("- [") {
            continue;
        }
        for c in backtick.captures_iter(t) {
            let token = c[1].trim();
            if repo_path_like(token) {
                out.extend(expand_brace_path(token));
            }
        }
    }
    out
}

/// Documentation-shaped path: `.md`/`.txt`, `docs/i18n/**`, or
/// `docs/structure/*.ndjson`. `Cargo.toml` and any path under `crates/` are
/// never documentation — both change build or test behavior.
fn is_documentation_path(path: &str) -> bool {
    let p = path.trim_start_matches("./").replace('\\', "/");
    if p == "Cargo.toml" || p.starts_with("crates/") {
        return false;
    }
    p.ends_with(".md")
        || p.ends_with(".txt")
        || p.starts_with("docs/i18n/")
        || (p.starts_with("docs/structure/") && p.ends_with(".ndjson"))
}

/// A Rust source file under `crates/`.
fn is_rust_crate_path(path: &str) -> bool {
    let p = path.trim_start_matches("./").replace('\\', "/");
    p.starts_with("crates/") && p.ends_with(".rs")
}

/// `declared-vs-touched` lint: compare each task's strictly attributed
/// declared paths against its covering `## Test Plan` row types (via the
/// shared `test_plan_table` module). Tasks without paths are skipped.
///
/// Fails when a task's declared paths are all documentation yet a covering
/// row is `unit` or `integration`, and when a task declares a path under
/// `crates/**/*.rs` yet every covering row is `grep`, `manual`, or
/// `documentation`.
fn declared_vs_touched_outcome(plan_text: &str) -> CheckOutcome {
    let tasks = section_text(plan_text, "## Tasks").unwrap_or_default();
    let (task_id_list, _) = task_ids(&tasks);
    let rows = scan_test_plan_rows(plan_text);

    let mut violations: Vec<String> = Vec::new();
    for task_id in &task_id_list {
        let declared = pipeline::task_spec::extract_declared_file_paths_strict(plan_text, task_id);
        if declared.is_empty() {
            continue;
        }
        let covering_types: Vec<String> = rows
            .iter()
            .filter(|row| {
                covered_task_ids(&row.covers_cell)
                    .iter()
                    .any(|id| id == task_id)
            })
            .map(|row| row.type_cell.to_lowercase())
            .collect();
        if covering_types.is_empty() {
            continue;
        }

        let all_documentation = declared.iter().all(|p| is_documentation_path(p));
        if all_documentation
            && covering_types
                .iter()
                .any(|t| t == "unit" || t == "integration")
        {
            violations.push(format!(
                "{task_id}: documentation-only declared paths covered by a unit/integration row"
            ));
            continue;
        }

        let has_rust_crate_path = declared.iter().any(|p| is_rust_crate_path(p));
        if has_rust_crate_path
            && covering_types
                .iter()
                .all(|t| t == "grep" || t == "manual" || t == "documentation")
        {
            violations.push(format!(
                "{task_id}: crates/**/*.rs declared paths covered only by grep/manual/documentation row(s)"
            ));
        }
    }

    if violations.is_empty() {
        outcome(
            "declared-vs-touched",
            CheckState::Pass,
            "declared paths and covering test-plan row types are consistent".to_string(),
        )
    } else {
        outcome(
            "declared-vs-touched",
            CheckState::Fail,
            violations.join("; "),
        )
    }
}

fn run_checks(plan_text: &str, repo_root: &Path) -> Vec<CheckOutcome> {
    let tasks = section_text(plan_text, "## Tasks").unwrap_or_default();
    let test_plan = section_text(plan_text, "## Test Plan").unwrap_or_default();
    let (tasks_ids, malformed_tasks) = task_ids(&tasks);
    let (tests_ids, malformed_tests) = tp_ids(&test_plan);
    let task_dupes = dupes(&tasks_ids);
    let test_dupes = dupes(&tests_ids);
    let task_suffixes: BTreeSet<String> = tasks_ids.iter().map(|id| id[2..].to_string()).collect();
    let test_suffixes: BTreeSet<String> = tests_ids.iter().map(|id| id[3..].to_string()).collect();
    let missing_tp: Vec<String> = task_suffixes
        .difference(&test_suffixes)
        .map(|s| format!("TP-{s}"))
        .collect();
    let missing_paths: Vec<String> = referenced_paths(&tasks)
        .into_iter()
        .filter(|p| !repo_root.join(p).exists())
        .collect();

    let task_count = tasks_ids.len() + malformed_tasks.len();
    let mut checks = vec![
        outcome(
            "task-count",
            if task_count == 0 {
                CheckState::Fail
            } else if task_count <= 99 {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            if task_count == 0 {
                "## Tasks has no blocking tasks — at least one T-NN task is required".to_string()
            } else {
                format!("{task_count} task(s) found")
            },
        ),
        outcome(
            "id-well-formed",
            if malformed_tasks.is_empty() && malformed_tests.is_empty() {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            if malformed_tasks.is_empty() && malformed_tests.is_empty() {
                "all T/TP ids are well-formed".to_string()
            } else {
                format!(
                    "malformed task ids: [{}]; malformed test ids: [{}]",
                    malformed_tasks.join(" | "),
                    malformed_tests.join(" | ")
                )
            },
        ),
        outcome(
            "id-unique",
            if task_dupes.is_empty() && test_dupes.is_empty() {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            if task_dupes.is_empty() && test_dupes.is_empty() {
                "no duplicate T/TP ids".to_string()
            } else {
                format!(
                    "duplicate task ids: [{}]; duplicate test ids: [{}]",
                    task_dupes.join(", "),
                    test_dupes.join(", ")
                )
            },
        ),
        outcome(
            "task-test-pairing",
            if missing_tp.is_empty() {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            if missing_tp.is_empty() {
                "every T-NN has a matching TP-NN".to_string()
            } else {
                format!("missing tests: [{}]", missing_tp.join(", "))
            },
        ),
        outcome(
            "referenced-paths",
            if missing_paths.is_empty() {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            if missing_paths.is_empty() {
                "all task-referenced file paths exist".to_string()
            } else {
                format!("missing path(s): {}", missing_paths.join(", "))
            },
        ),
    ];
    let structural_pass = checks.iter().all(|c| c.state == CheckState::Pass);
    let has_clear = plan_text.contains("<!-- ENG_REVIEW: CLEAR -->");
    checks.push(outcome(
        "eng-review-clear",
        if has_clear && structural_pass {
            CheckState::Pass
        } else if has_clear {
            CheckState::Fail
        } else {
            CheckState::NotRun
        },
        if has_clear && structural_pass {
            "ENG_REVIEW clear marker present on a structurally valid plan".to_string()
        } else if has_clear {
            "ENG_REVIEW clear marker present but structure checks failed".to_string()
        } else {
            "missing <!-- ENG_REVIEW: CLEAR --> marker".to_string()
        },
    ));
    checks.push(declared_vs_touched_outcome(plan_text));
    checks.push(naming_gate_outcome(repo_root));
    checks
}

fn render_receipt(receipt: &Receipt) -> String {
    let mut out = String::from("# refining-check receipt\n\n");
    out.push_str(&format!(
        "overall: {}\n\n",
        if receipt.passed() { "pass" } else { "fail" }
    ));
    out.push_str("| check | state | command | summary |\n| --- | --- | --- | --- |\n");
    for check in &receipt.checks {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            check.name,
            check.state.as_str(),
            check.command.as_deref().unwrap_or("-"),
            check.summary
        ));
    }
    out
}

pub(crate) fn cmd_refining_check(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("gal refining-check: {err}");
            return ExitCode::Usage;
        }
    };
    if !parsed.target.exists() {
        eprintln!(
            "gal refining-check: source plan not found: {}",
            parsed.target.display()
        );
        return ExitCode::Usage;
    }
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let plan_text = std::fs::read_to_string(&parsed.target).unwrap_or_default();
    let receipt = Receipt {
        checks: run_checks(&plan_text, &repo_root),
    };
    if let Some(parent) = parsed.receipt.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal refining-check: cannot create receipt dir {}: {err}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }
    if let Err(err) = std::fs::write(&parsed.receipt, render_receipt(&receipt)) {
        eprintln!(
            "gal refining-check: cannot write receipt {}: {err}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }
    println!(
        "gal refining-check: {} ({} check(s)) -> {}",
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
        let target = Path::new(".dev/plans/refining-scope.md");
        assert_eq!(
            default_receipt_path(target),
            resolve_receipt_path(Some(target), "refining-check.receipt.md")
        );
        let parsed = parse_args(&[
            target.display().to_string(),
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
    fn tpid(n: u32) -> String {
        format!("TP-{n:02}")
    }

    fn temp_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(tmp.path().join("docs").join("example.md"), "ok\n").unwrap();
        std::fs::write(
            tmp.path().join("docs").join("naming.md"),
            "# RETIRED-TERMS:BEGIN\n# RETIRED-TERMS:END\n",
        )
        .unwrap();
        // naming-gate's scan_tree is git-proven: it must be a git repo, or the
        // scan fails closed instead of reporting an empty-clean result.
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(tmp.path())
            .arg("init")
            .arg("--quiet")
            .status()
            .expect("git init spawns");
        assert!(status.success(), "git init must succeed");
        tmp
    }

    fn sample_plan() -> String {
        format!("# Plan\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {} | manual | check | {} |\n\n## Review Results\n### Engineering Review\nCLEAR\n\n<!-- ENG_REVIEW: CLEAR -->\n\n## Tasks\n- [ ] {} - `docs/example.md` update.\n", tpid(1), tid(1), tid(1))
    }

    #[test]
    fn clean_plan_passes() {
        let repo = temp_repo();
        let receipt = Receipt {
            checks: run_checks(&sample_plan(), repo.path()),
        };
        assert!(receipt.passed(), "{:#?}", receipt.checks);
    }

    fn declared_vs_touched_state(plan: &str) -> CheckState {
        let checks = run_checks(plan, temp_repo().path());
        checks
            .iter()
            .find(|c| c.name == "declared-vs-touched")
            .expect("declared-vs-touched check must be emitted")
            .state
            .clone()
    }

    // documentation-only declared paths covered by a `unit` row → Fail.
    #[test]
    fn declared_vs_touched_flags_documentation_only_paths_with_unit_row() {
        let plan = format!(
            "# Plan\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {} | unit | check | {} |\n\n## Tasks\n- [ ] {} - `docs/example.md` update.\n",
            tpid(1),
            tid(1),
            tid(1)
        );
        assert_eq!(declared_vs_touched_state(&plan), CheckState::Fail);
    }

    // `crates/**/*.rs` declared path covered only by grep/manual rows → Fail.
    #[test]
    fn declared_vs_touched_flags_rust_crate_path_with_no_dispatch_only_rows() {
        let plan = format!(
            "# Plan\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {} | grep | check | {} |\n| {} | manual | check | {} |\n\n## Tasks\n- [ ] {} - `crates/cli/src/lib.rs` update.\n",
            tpid(1),
            tid(1),
            tpid(2),
            tid(1),
            tid(1)
        );
        assert_eq!(declared_vs_touched_state(&plan), CheckState::Fail);
    }

    // `Cargo.toml`-touching task covered by a `unit` row is not documentation
    // and not a `.rs` crate path — neither branch fires.
    #[test]
    fn declared_vs_touched_does_not_flag_cargo_toml_with_unit_row() {
        let plan = format!(
            "# Plan\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {} | unit | check | {} |\n\n## Tasks\n- [ ] {} - `Cargo.toml` update.\n",
            tpid(1),
            tid(1),
            tid(1)
        );
        assert_eq!(declared_vs_touched_state(&plan), CheckState::Pass);
    }

    #[test]
    fn naming_gate_outcome_fails_closed_outside_git_repo() {
        // No `git init`: scan_tree cannot prove a clean result and must fail
        // the check, not report a false Pass.
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(
            tmp.path().join("docs").join("naming.md"),
            "# RETIRED-TERMS:BEGIN\n# RETIRED-TERMS:END\n",
        )
        .unwrap();
        let result = naming_gate_outcome(tmp.path());
        assert_eq!(result.state, CheckState::Fail);
        assert!(
            result.summary.contains("scan failed"),
            "{:?}",
            result.summary
        );
    }
    #[test]
    fn missing_tp_fails() {
        let repo = temp_repo();
        let plan = sample_plan().replace(
            &format!("| {} | manual | check | {} |\n", tpid(1), tid(1)),
            "",
        );
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "task-test-pairing" && c.state == CheckState::Fail));
    }
    #[test]
    fn duplicate_id_fails() {
        let repo = temp_repo();
        let plan = sample_plan() + &format!("- [ ] {} - duplicate.\n", tid(1));
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "id-unique" && c.state == CheckState::Fail));
    }
    #[test]
    fn nonexistent_path_fails() {
        let repo = temp_repo();
        let plan = sample_plan().replace("`docs/example.md`", "`docs/missing.md`");
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "referenced-paths" && c.state == CheckState::Fail));
    }
    #[test]
    fn task_count_over_limit_fails() {
        let repo = temp_repo();
        let mut plan = sample_plan().replace(
            &format!("- [ ] {} - `docs/example.md` update.\n", tid(1)),
            "",
        );
        for n in 1..=100 {
            plan.push_str(&format!("- [ ] T-{n:02} - `docs/example.md` update.\n"));
        }
        for n in 2..=100 {
            plan.push_str(&format!("| TP-{n:02} | manual | check | T-{n:02} |\n"));
        }
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "task-count" && c.state == CheckState::Fail));
    }

    // zero blocking tasks → task-count Fail (a plan with no T-NN bullets is incomplete)
    #[test]
    fn zero_tasks_fails() {
        let repo = temp_repo();
        let plan = "## Tasks\n\n(no tasks yet)\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n";
        let receipt = Receipt {
            checks: run_checks(plan, repo.path()),
        };
        assert!(
            receipt
                .checks
                .iter()
                .any(|c| c.name == "task-count" && c.state == CheckState::Fail),
            "expected task-count Fail for empty ## Tasks, got: {:#?}",
            receipt.checks
        );
    }
}
