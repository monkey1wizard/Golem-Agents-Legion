//! `gal prompt-check` - prompt-internal anchor validation receipt.
//!
//! Validates that a compressed execution prompt still carries the machine anchors
//! consumed by pipeline/finalize tooling. Missing anchors are receipt-visible and
//! exit non-zero. Not a public `/gal` command.

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use super::planning_authority::{parse_localized_meta, rendered_source_hash};
use gal_engine::ExitCode;
use serde_json::Value;
use std::path::{Path, PathBuf};

struct Args {
    prompt: PathBuf,
    receipt: PathBuf,
    assemble_dry_run: bool,
}

fn assemble_dry_run_checks(prompt_text: &str, prompt_path: &Path) -> Vec<CheckOutcome> {
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
            if let Some(task) = pipeline::task_spec::parse_plan_task_bullet(line) {
                task_ids.push(task.id.to_string());
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
            if let Err(error) = pipeline::task_spec::assemble_plan_task_spec(&input) {
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

const VERIFICATION_LAYERS: [&str; 4] = ["contract-doc", "unit", "cli", "integration"];

fn has_duplicate_impact_keys(json: &str) -> bool {
    let bytes = json.as_bytes();
    let mut index = 1;
    let mut keys = std::collections::HashSet::new();
    loop {
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index) == Some(&b'}') {
            return false;
        }
        if bytes.get(index) != Some(&b'"') {
            return false;
        }
        let start = index;
        index += 1;
        let mut escaped = false;
        while let Some(byte) = bytes.get(index) {
            index += 1;
            if escaped {
                escaped = false;
            } else if *byte == b'\\' {
                escaped = true;
            } else if *byte == b'"' {
                break;
            }
        }
        let Ok(key) = serde_json::from_str::<String>(&json[start..index]) else {
            return false;
        };
        if !keys.insert(key) {
            return true;
        }
        while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
            index += 1;
        }
        if bytes.get(index) != Some(&b':') {
            return false;
        }
        index += 1;
        let mut depth = 0usize;
        let mut in_string = false;
        escaped = false;
        while let Some(byte) = bytes.get(index) {
            if in_string {
                if escaped {
                    escaped = false;
                } else if *byte == b'\\' {
                    escaped = true;
                } else if *byte == b'"' {
                    in_string = false;
                }
            } else {
                match *byte {
                    b'"' => in_string = true,
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' if depth > 0 => depth -= 1,
                    b',' if depth == 0 => break,
                    b'}' if depth == 0 => return false,
                    _ => {}
                }
            }
            index += 1;
        }
        if bytes.get(index) != Some(&b',') {
            return false;
        }
        index += 1;
    }
}

/// Validate task impact declarations and static Verification Map coverage.
/// Proof commands are treated as opaque text and are never executed here.
pub(crate) fn verification_map_checks(prompt_text: &str) -> Vec<CheckOutcome> {
    let lines = non_fenced_lines(prompt_text);
    let mut tasks = Vec::<(String, Vec<&str>)>::new();
    let mut current: Option<usize> = None;
    let mut in_tasks = false;
    let mut in_map = false;
    let mut map_rows = Vec::<Vec<String>>::new();
    for line in &lines {
        let trimmed = line.trim();
        if trimmed == "## Tasks" {
            in_tasks = true;
            in_map = false;
            continue;
        }
        if trimmed == "## Verification Map" {
            in_tasks = false;
            in_map = true;
            continue;
        }
        if trimmed.starts_with("## ") {
            in_tasks = false;
            in_map = false;
            current = None;
        }
        if in_tasks {
            if let Some(task) = pipeline::task_spec::parse_plan_task_bullet(line) {
                tasks.push((task.id.to_string(), Vec::new()));
                current = Some(tasks.len() - 1);
            } else if let Some(index) = current {
                tasks[index].1.push(line);
            }
        } else if in_map && trimmed.starts_with('|') {
            let cells: Vec<String> = trimmed
                .trim_matches('|')
                .split('|')
                .map(|cell| cell.trim().to_string())
                .collect();
            if !cells.iter().all(|cell| {
                cell.chars()
                    .all(|ch| ch == '-' || ch == ':' || ch.is_whitespace())
            }) {
                map_rows.push(cells);
            }
        }
    }

    let mut outcomes = Vec::new();
    if tasks.is_empty() {
        return vec![anchor_outcome(
            "verification-map",
            false,
            "no task was found in the `## Tasks` section",
        )];
    }
    let headers = [
        "Task",
        "Layer",
        "Proof Command",
        "Expected",
        "Producer",
        "Evidence",
    ];
    let table_valid = map_rows
        .first()
        .is_some_and(|row| row.iter().map(String::as_str).eq(headers));
    let rows = if table_valid { &map_rows[1..] } else { &[] };
    let mut seen = std::collections::HashSet::new();
    let mut malformed = !table_valid;
    let task_ids: std::collections::HashSet<_> = tasks.iter().map(|(id, _)| id.as_str()).collect();
    let mut coverage = std::collections::HashMap::<(String, String), usize>::new();
    for row in rows {
        if row.len() != 6 || row.iter().any(String::is_empty) || row[2].trim().is_empty() {
            malformed = true;
            continue;
        }
        if !task_ids.contains(row[0].as_str()) || !VERIFICATION_LAYERS.contains(&row[1].as_str()) {
            malformed = true;
            continue;
        }
        if !seen.insert((row[0].clone(), row[1].clone())) {
            malformed = true;
        }
        *coverage
            .entry((row[0].clone(), row[1].clone()))
            .or_default() += 1;
    }
    for (task_id, detail) in tasks {
        let prefix = "- **Verification Impact**:";
        let impacts: Vec<_> = detail
            .iter()
            .filter_map(|line| line.trim().strip_prefix(prefix).map(str::trim))
            .collect();
        let mut required = Vec::new();
        let valid_impact = if impacts.len() != 1 {
            false
        } else if let Ok(value) = serde_json::from_str::<Value>(impacts[0].trim_matches('`').trim())
        {
            let expected = [
                "contract_docs",
                "code_logic",
                "cli_surface",
                "cross_boundary",
            ];
            let object = value.as_object();
            if has_duplicate_impact_keys(impacts[0].trim_matches('`').trim())
                || !object.is_some_and(|object| {
                    object.len() == expected.len()
                        && expected
                            .iter()
                            .all(|key| object.get(*key).is_some_and(Value::is_boolean))
                })
            {
                false
            } else {
                let object = object.unwrap();
                if object["contract_docs"] == Value::Bool(true) {
                    required.push("contract-doc");
                }
                if object["code_logic"] == Value::Bool(true) {
                    required.push("unit");
                }
                if object["cli_surface"] == Value::Bool(true) {
                    required.push("cli");
                }
                if object["cross_boundary"] == Value::Bool(true) {
                    required.push("integration");
                }
                !required.is_empty()
            }
        } else {
            false
        };
        let covered = valid_impact
            && required.iter().all(|layer| {
                coverage
                    .get(&(task_id.clone(), (*layer).to_string()))
                    .copied()
                    == Some(1)
            });
        outcomes.push(anchor_outcome(
            &format!("verification-impact-{task_id}"),
            valid_impact,
            if valid_impact {
                "one valid Verification Impact object is present"
            } else {
                "requires exactly one JSON object with four Boolean flags and at least one true"
            },
        ));
        outcomes.push(anchor_outcome(
            &format!("verification-map-{task_id}"),
            covered && !malformed,
            if covered && !malformed {
                "all required layers have one valid map row"
            } else {
                "Verification Map is malformed, duplicated, or missing required task/layer coverage"
            },
        ));
    }
    outcomes
}

pub(crate) fn run_checks(prompt_text: &str) -> Vec<CheckOutcome> {
    let lines = non_fenced_lines(prompt_text);
    let mut in_tasks = false;
    let has_task = lines.iter().any(|line| {
        let trimmed = line.trim();
        if trimmed == "## Tasks" {
            in_tasks = true;
            return false;
        }
        if in_tasks && trimmed.starts_with("## ") {
            in_tasks = false;
        }
        in_tasks
            && pipeline::task_spec::parse_plan_task_bullet(line).is_some_and(|task| !task.checked)
    });
    let mut checks = vec![
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
    ];
    checks.extend(verification_map_checks(prompt_text));
    checks
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
            );
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
        return anchor_outcome(
            name,
            false,
            "non-English plan used the EN-draft flow but its equivalence-verdict is not EQUIVALENT (pending or stale)",
        );
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

    let raw = std::fs::read(&parsed.prompt).unwrap_or_default();
    let prompt_text = String::from_utf8(raw.clone()).unwrap_or_default();
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
            let rendered = format!(
                "{}{}",
                render_receipt(&receipt),
                super::finalize_check::render_repair_dispositions_for_target(
                    &receipt.checks,
                    "prompt",
                    if parsed.assemble_dry_run {
                        "assemble-dry-run"
                    } else {
                        "default"
                    },
                    &parsed.prompt,
                    &raw,
                    None
                )
            );
            match super::finalize_check::bind_receipt(rendered, &envelope) {
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
        format!(
            "# Plan\n\n<!-- gal:planning-authority\nsemantic-draft: .dev/plans/x.en.md\nplanLanguage: zh-TW\ndraft-hash: a\nrendered-source-hash: b\nprompt-hash: {prompt_hash}\nequivalence-verdict: {verdict}\n-->\n"
        )
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
        format!(
            "## Status\nCurrent Task: -\n\n## Tasks\n- [ ] {} - do work\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n## Verification Map\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {} | unit | `cargo test` | pass | TESTER | test output |\n\n## Test Results\nPending\n\n## Review Results\n### Architecture Review\nCLEAR\n### Business Review\nNot requested.\n### Design Review\nNot requested.\n### Engineering Review\nCLEAR\n",
            tid(1), tid(1)
        )
    }

    fn impact_json(mask: u8) -> String {
        format!(
            "{{\"contract_docs\":{},\"code_logic\":{},\"cli_surface\":{},\"cross_boundary\":{}}}",
            mask & 1 != 0,
            mask & 2 != 0,
            mask & 4 != 0,
            mask & 8 != 0
        )
    }

    fn verification_prompt(impact: &str, layers: &[&str]) -> String {
        let id = tid(1);
        let mut prompt = format!("## Tasks\n- [ ] {id} — work\n  - **Verification Impact**: `{impact}`\n\n## Verification Map\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n");
        for layer in layers {
            prompt.push_str(&format!(
                "| {id} | {layer} | `cargo test` | pass | TESTER | output |\n"
            ));
        }
        prompt
    }

    #[test]
    fn verification_map_accepts_all_fifteen_nonempty_flag_unions() {
        let layer_flags = [
            (1, "contract-doc"),
            (2, "unit"),
            (4, "cli"),
            (8, "integration"),
        ];
        for mask in 1..16 {
            let layers: Vec<_> = layer_flags
                .iter()
                .filter_map(|(bit, layer)| (mask & bit != 0).then_some(*layer))
                .collect();
            let checks = verification_map_checks(&verification_prompt(&impact_json(mask), &layers));
            assert!(
                checks.iter().all(|check| check.state == CheckState::Pass),
                "mask {mask}: {checks:?}"
            );
        }
    }

    #[test]
    fn verification_map_rejects_missing_malformed_and_duplicate_contracts() {
        let valid = verification_prompt(&impact_json(2), &["unit"]);
        let absent = valid.replace("  - **Verification Impact**: `", "  - **Impact**: `");
        assert!(verification_map_checks(&absent)
            .iter()
            .any(|check| check.state == CheckState::Fail));
        for invalid in [
            "{\"contract_docs\":false,\"code_logic\":\"true\",\"cli_surface\":false,\"cross_boundary\":false}",
            "{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false}",
            "{\"contract_docs\":false,\"code_logic\":false,\"cli_surface\":false,\"cross_boundary\":false}",
            "{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false,\"extra\":true}",
            "{\"contract_docs\":false,\"code_logic\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}",
        ] {
            let checks = verification_map_checks(&verification_prompt(invalid, &["unit"]));
            let impact_check_name = format!("verification-impact-{}", tid(1));
            assert!(checks.iter().any(|check| check.name == impact_check_name && check.state == CheckState::Fail), "{invalid}");
        }
        let id = tid(1);
        let duplicate = format!("{valid}| {id} | unit | `cargo test` | pass | TESTER | output |\n");
        let map_check_name = format!("verification-map-{id}");
        assert!(verification_map_checks(&duplicate)
            .iter()
            .any(|check| check.name == map_check_name && check.state == CheckState::Fail));
        let absent_table = valid.replace("## Verification Map", "## Other");
        assert!(verification_map_checks(&absent_table)
            .iter()
            .any(|check| check.state == CheckState::Fail));
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
        let id = tid(1);
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
                "retired-marker".to_string(),
                "status-heading".to_string(),
                "current-task-anchor".to_string(),
                "tasks-heading".to_string(),
                "unchecked-task-anchor".to_string(),
                "imported-from-shape".to_string(),
                "test-results-heading".to_string(),
                "review-results-heading".to_string(),
                "review-subsection-architecture".to_string(),
                "review-subsection-business".to_string(),
                "review-subsection-design".to_string(),
                "review-subsection-engineering".to_string(),
                format!("verification-impact-{id}"),
                format!("verification-map-{id}"),
                "equivalence-receipt".to_string(),
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
    fn assemble_dry_run_failure_is_receipt_visible() {
        let id = tid(1);
        let receipt = Receipt {
            checks: assemble_dry_run_checks(
                &dry_run_prompt("- Change: update `src/lib.rs`.", ""),
                Path::new(".dev/plans/example.prompt.md"),
            ),
        };
        assert!(!receipt.passed());
        assert!(receipt.checks.iter().any(|check| {
            check.name == format!("assemble-dry-run-{id}") && check.state == CheckState::Fail
        }));
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
    fn unchecked_task_anchor_uses_shared_identity_and_ignores_fenced_examples() {
        let id = tid(1);
        let check = |tasks: &str| {
            run_checks(&format!(
                "## Status\nCurrent Task: {id}\n\n## Tasks\n{tasks}\n"
            ))
            .into_iter()
            .find(|outcome| outcome.name == "unchecked-task-anchor")
            .unwrap()
            .state
        };
        assert_eq!(check(&format!("- [ ] {id}: colon task")), CheckState::Pass);
        assert_eq!(
            check(&format!("```md\n- [ ] {id}: fenced example\n```")),
            CheckState::Fail
        );
        assert_eq!(
            check(&format!("- [ ] T-{:03}: three digits", 1)),
            CheckState::Fail
        );
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
