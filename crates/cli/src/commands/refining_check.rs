//! `gal refining-check` - source-plan structure receipt.

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{split_md_row, CheckOutcome, CheckState, Receipt};
use super::test_plan_table::{covered_task_ids, scan_test_plan_rows};
use gal_engine::ExitCode;
use regex::Regex;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

struct Args {
    target: PathBuf,
    receipt: PathBuf,
}

fn default_receipt_path(target: &Path) -> Result<PathBuf, String> {
    resolve_receipt_path(Some(target), None, "refining-check.receipt.md")
        .map_err(|error| format!("receipt scope resolution: {error}"))
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
    let receipt = match receipt {
        Some(path) => path,
        None => default_receipt_path(&target)?,
    };
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
    let mut ids = Vec::new();
    let mut malformed = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in tasks.lines() {
        let t = line.trim_start();
        if let Some(ch) = t.chars().next().filter(|ch| *ch == '`' || *ch == '~') {
            let count = t.chars().take_while(|current| current == &ch).count();
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
            ids.push(task.id.to_string());
        } else if super::pipeline_preflight::is_malformed_task_bullet(line) {
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
    token == "README.md" || token.contains('/') || token.contains('\\') || token.starts_with('.')
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

/// Canonical document key shared by a doc and its language variants:
/// `docs/<name>.md` and `docs/i18n/<lang>/<name>.<lang>.md` map to `<name>`,
/// root `README.md` and `docs/i18n/<lang>/README.<lang>.md` map to `README`.
/// Any other path has no key.
fn canonical_doc_key(path: &str) -> Option<String> {
    let p = path.trim_start_matches("./").replace('\\', "/");
    if p == "README.md" {
        return Some("README".to_string());
    }
    if let Some(rest) = p.strip_prefix("docs/i18n/") {
        let (lang, file) = rest.split_once('/')?;
        if file.contains('/') || lang.is_empty() {
            return None;
        }
        return file
            .strip_suffix(&format!(".{lang}.md"))
            .filter(|name| !name.is_empty())
            .map(str::to_string);
    }
    let name = p.strip_prefix("docs/")?.strip_suffix(".md")?;
    (!name.is_empty() && !name.contains('/')).then(|| name.to_string())
}

/// Split `## Tasks` text into `(task id, block text)` pairs. Continuation
/// lines are re-prefixed as list items so `referenced_paths` reads them too.
fn task_blocks(tasks: &str) -> Vec<(String, String)> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    let mut current = false;
    for line in tasks.lines() {
        let t = line.trim_start();
        if let Some(task) = pipeline::task_spec::parse_plan_task_bullet(line) {
            blocks.push((task.id.to_string(), format!("{t}\n")));
            current = true;
        } else if super::pipeline_preflight::is_malformed_task_bullet(line) {
            current = false;
        } else if current {
            let Some((_, text)) = blocks.last_mut() else {
                continue;
            };
            text.push_str(&format!("- [ ] {}\n", t.trim_start_matches("- ")));
        }
    }
    blocks
}

/// `translation-split` lint: one document and its language variants must be
/// edited by one task. Fails when a canonical document key is referenced by
/// two or more tasks, including when the tasks name identical paths.
fn translation_split_outcome(tasks: &str) -> CheckOutcome {
    use std::collections::BTreeMap;
    let mut by_key: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let blocks = task_blocks(tasks);
    for (id, block) in blocks {
        for path in referenced_paths(&block) {
            if let Some(key) = canonical_doc_key(&path) {
                by_key.entry(key).or_default().insert(id.clone());
            }
        }
    }
    let split: Vec<String> = by_key
        .iter()
        .filter(|(_, ids)| ids.len() >= 2)
        .map(|(key, ids)| {
            format!(
                "`{key}` is referenced by {}",
                ids.iter().cloned().collect::<Vec<_>>().join(", ")
            )
        })
        .collect();
    if split.is_empty() {
        outcome(
            "translation-split",
            CheckState::Pass,
            "no document and its language variants are split across tasks".to_string(),
        )
    } else {
        outcome("translation-split", CheckState::Fail, split.join("; "))
    }
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

fn is_crate_source_path(path: &str) -> bool {
    let p = path.trim_start_matches("./").replace('\\', "/");
    p.starts_with("crates/") && p.contains("/src/") && p.ends_with(".rs")
}

pub(crate) fn test_plan_coverage_outcome(
    plan_text: &str,
    declared_task_ids: &[String],
) -> CheckOutcome {
    let lines: Vec<&str> = plan_text.lines().collect();
    let Some(section_start) = lines.iter().position(|line| line.trim() == "## Test Plan") else {
        return outcome(
            "test-plan-coverage",
            CheckState::Fail,
            "missing ## Test Plan section".to_string(),
        );
    };
    let section_end = lines[section_start + 1..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map(|offset| section_start + 1 + offset)
        .unwrap_or(lines.len());
    let mut rows = lines[section_start + 1..section_end]
        .iter()
        .map(|line| line.trim())
        .filter(|line| line.starts_with('|'));
    let Some(header) = rows.next() else {
        return outcome(
            "test-plan-coverage",
            CheckState::Fail,
            "missing Test Plan table header".to_string(),
        );
    };
    let header_cells = split_md_row(header);
    let Some(id_col) = header_cells
        .iter()
        .position(|cell| cell.eq_ignore_ascii_case("id"))
    else {
        return outcome(
            "test-plan-coverage",
            CheckState::Fail,
            "Test Plan table has no ID column".to_string(),
        );
    };
    let Some(covers_col) = header_cells
        .iter()
        .position(|cell| cell.eq_ignore_ascii_case("covers"))
    else {
        return outcome(
            "test-plan-coverage",
            CheckState::Fail,
            "Test Plan table has no Covers column".to_string(),
        );
    };

    let declared: BTreeSet<String> = declared_task_ids.iter().cloned().collect();
    let mut covered = BTreeSet::new();
    let mut violations = Vec::new();
    for (row_number, row) in rows.enumerate() {
        let cells = split_md_row(row);
        if cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|ch| ch == '-' || ch == ':'))
            || cells.iter().all(String::is_empty)
        {
            continue;
        }
        let row_label = cells
            .get(id_col)
            .filter(|cell| !cell.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("row {}", row_number + 1));
        let covers_cell = cells
            .get(covers_col)
            .map(String::as_str)
            .unwrap_or_default();
        let row_tasks = covered_task_ids(covers_cell);
        let declared_row_tasks: Vec<String> = row_tasks
            .iter()
            .filter(|task_id| declared.contains(*task_id))
            .cloned()
            .collect();
        if row_tasks.is_empty() || declared_row_tasks.is_empty() {
            violations.push(format!("{row_label}: Covers names no declared T-NN"));
        } else {
            covered.extend(declared_row_tasks);
        }
    }

    for task_id in declared_task_ids {
        if !covered.contains(task_id.as_str()) {
            violations.push(format!("{task_id}: no covering Test Plan row"));
        }
    }

    if violations.is_empty() {
        outcome(
            "test-plan-coverage",
            CheckState::Pass,
            "every Test Plan row resolves to declared tasks and every task is covered".to_string(),
        )
    } else {
        outcome(
            "test-plan-coverage",
            CheckState::Fail,
            violations.join("; "),
        )
    }
}

/// `manual-limit` lint: a plan holds at most one `manual` Test Plan row, and
/// that row's Description must carry non-empty text after the literal label
/// `Why manual:`. Locates `Type` and `Description` by header cell name.
fn manual_limit_outcome(plan_text: &str) -> CheckOutcome {
    let section = section_text(plan_text, "## Test Plan").unwrap_or_default();
    let mut rows = section
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|'));
    let header_cells = rows.next().map(split_md_row).unwrap_or_default();
    let column = |name: &str| {
        header_cells
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
    };
    let (Some(type_col), Some(description_col)) = (column("type"), column("description")) else {
        return outcome(
            "manual-limit",
            CheckState::Pass,
            "no Test Plan Type/Description columns to inspect".to_string(),
        );
    };

    let mut manual_labels: Vec<String> = Vec::new();
    let mut unexplained: Vec<String> = Vec::new();
    for (row_number, row) in rows.enumerate() {
        let cells = split_md_row(row);
        if cells.get(type_col).map(|c| c.to_lowercase()).as_deref() != Some("manual") {
            continue;
        }
        let label = cells
            .first()
            .filter(|cell| !cell.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("row {}", row_number + 1));
        let description = cells
            .get(description_col)
            .map(String::as_str)
            .unwrap_or_default();
        let has_reason = description
            .split_once("Why manual:")
            .is_some_and(|(_, reason)| !reason.trim().is_empty());
        if !has_reason {
            unexplained.push(label.clone());
        }
        manual_labels.push(label);
    }

    let mut violations = Vec::new();
    if manual_labels.len() > 1 {
        violations.push(format!(
            "{} manual rows [{}], at most one allowed",
            manual_labels.len(),
            manual_labels.join(", ")
        ));
    }
    if !unexplained.is_empty() {
        violations.push(format!(
            "manual row(s) [{}] lack non-empty text after `Why manual:`",
            unexplained.join(", ")
        ));
    }
    if violations.is_empty() {
        outcome(
            "manual-limit",
            CheckState::Pass,
            "at most one manual row, and it states why".to_string(),
        )
    } else {
        outcome("manual-limit", CheckState::Fail, violations.join("; "))
    }
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

#[derive(Clone)]
struct CargoTargetRoot {
    kind: Vec<String>,
    src_path: PathBuf,
}

#[derive(Clone)]
struct CargoPackageRoot {
    name: String,
    manifest_path: PathBuf,
    targets: Vec<CargoTargetRoot>,
}

fn cargo_packages(repo_root: &Path) -> Result<Vec<CargoPackageRoot>, String> {
    let output = std::process::Command::new("cargo")
        .current_dir(repo_root)
        .args(["metadata", "--no-deps", "--format-version", "1"])
        .output()
        .map_err(|error| format!("cargo metadata could not start: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("cargo metadata returned invalid JSON: {error}"))?;
    let packages = metadata["packages"]
        .as_array()
        .ok_or_else(|| "cargo metadata has no packages array".to_string())?;
    packages
        .iter()
        .map(|package| {
            let name = package["name"]
                .as_str()
                .ok_or_else(|| "cargo metadata package has no name".to_string())?;
            let manifest_path = package["manifest_path"]
                .as_str()
                .ok_or_else(|| format!("cargo metadata package {name} has no manifest path"))?;
            let targets = package["targets"]
                .as_array()
                .ok_or_else(|| format!("cargo metadata package {name} has no targets"))?
                .iter()
                .map(|target| {
                    Ok(CargoTargetRoot {
                        kind: target["kind"]
                            .as_array()
                            .ok_or_else(|| format!("target in {name} has no kind"))?
                            .iter()
                            .filter_map(|kind| kind.as_str().map(str::to_string))
                            .collect(),
                        src_path: PathBuf::from(
                            target["src_path"]
                                .as_str()
                                .ok_or_else(|| format!("target in {name} has no source path"))?,
                        ),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            Ok(CargoPackageRoot {
                name: name.to_string(),
                manifest_path: PathBuf::from(manifest_path),
                targets,
            })
        })
        .collect()
}

fn normalized(path: &Path) -> PathBuf {
    if let Ok(path) = std::fs::canonicalize(path) {
        return path;
    }
    if let (Some(parent), Some(name)) = (path.parent(), path.file_name()) {
        if let Ok(parent) = std::fs::canonicalize(parent) {
            return parent.join(name);
        }
    }
    path.to_path_buf()
}

fn task_blocks_with_acceptance(tasks: &str) -> Vec<(String, Vec<String>, Vec<String>)> {
    let mut blocks: Vec<(String, Vec<String>, Vec<String>)> = Vec::new();
    let mut in_acceptance = false;
    let mut in_files = false;
    let path_spans = Regex::new(r"`([^`]+)`").unwrap();
    for line in tasks.lines() {
        let trimmed = line.trim_start();
        if let Some(task) = pipeline::task_spec::parse_plan_task_bullet(line) {
            blocks.push((task.id.to_string(), Vec::new(), Vec::new()));
            in_acceptance = false;
            in_files = false;
            continue;
        }
        if let Some((_, acceptance, files)) = blocks.last_mut() {
            if trimmed.starts_with("- **Files**:") || trimmed.starts_with("Files:") {
                in_files = true;
                in_acceptance = false;
                let value = trimmed
                    .strip_prefix("- **Files**:")
                    .or_else(|| trimmed.strip_prefix("Files:"))
                    .unwrap_or_default();
                files.extend(
                    path_spans
                        .captures_iter(value)
                        .map(|capture| capture[1].to_string()),
                );
                continue;
            }
            if trimmed.starts_with("Acceptance:") || trimmed.starts_with("- **Acceptance**:") {
                in_acceptance = true;
                in_files = false;
                acceptance.push(
                    trimmed
                        .strip_prefix("- **Acceptance**:")
                        .or_else(|| trimmed.strip_prefix("Acceptance:"))
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                );
            } else if trimmed.starts_with("- **") || trimmed.starts_with("- **Probe**:") {
                in_acceptance = false;
                in_files = false;
            } else if in_acceptance {
                acceptance.push(trimmed.to_string());
            } else if in_files {
                files.extend(
                    path_spans
                        .captures_iter(trimmed)
                        .map(|capture| capture[1].to_string()),
                );
            }
        }
    }
    blocks
}

fn literal_cargo_test_commands(acceptance: &[String]) -> Vec<Vec<String>> {
    let spans = Regex::new(r"`([^`]+)`").unwrap();
    acceptance
        .iter()
        .flat_map(|line| spans.captures_iter(line))
        .filter_map(|capture| {
            let command = capture[1].trim();
            let mut words = command.split_whitespace();
            let first = words.next()?;
            let words: Vec<String> = if first == "rtk" {
                words.map(str::to_string).collect()
            } else {
                std::iter::once(first.to_string())
                    .chain(words.map(str::to_string))
                    .collect()
            };
            (words.first().is_some_and(|word| word == "cargo")
                && words.get(1).is_some_and(|word| word == "test"))
            .then_some(words)
        })
        .chain(acceptance.iter().filter_map(|line| {
            let command = line.trim();
            let mut words: Vec<String> = command.split_whitespace().map(str::to_string).collect();
            if words.first().is_some_and(|word| word == "rtk") {
                words.remove(0);
            }
            (words.first().is_some_and(|word| word == "cargo")
                && words.get(1).is_some_and(|word| word == "test"))
            .then_some(words)
        }))
        .collect()
}

fn command_covers_package(
    command: &[String],
    package: &CargoPackageRoot,
    source: &Path,
    require_unit: bool,
) -> bool {
    let mut selected = false;
    let mut excluded = false;
    let mut selector = None::<String>;
    let mut index = 2;
    let mut runtime = false;
    let mut filtered = false;
    let mut all_targets = false;
    let mut consumes_target_name = false;
    while index < command.len() {
        let arg = command[index].as_str();
        if consumes_target_name {
            consumes_target_name = false;
            index += 1;
            continue;
        }
        if runtime {
            // Any runtime argument can narrow execution, including ignored and skip.
            filtered = true;
            break;
        }
        match arg {
            "--" => runtime = true,
            "-p" | "--package" => {
                index += 1;
                if command.get(index).is_some_and(|name| name == &package.name) {
                    selected = true;
                }
            }
            "--workspace" => selected = true,
            "--all" => {
                selected = true;
                all_targets = true;
            }
            "--all-targets" => all_targets = true,
            "--exclude" => {
                index += 1;
                if command.get(index).is_some_and(|name| name == &package.name) {
                    excluded = true;
                }
            }
            "--lib" => selector = Some("lib".to_string()),
            "--bins" => selector = Some("bin".to_string()),
            "--test" | "--example" | "--bench" => consumes_target_name = true,
            "--doc" => {}
            "--no-run" => filtered = true,
            "--no-fail-fast" => {}
            value if value.starts_with("--package=") => {
                selected |= value == format!("--package={}", package.name)
            }
            value if value.starts_with("--exclude=") => {
                excluded |= value == format!("--exclude={}", package.name)
            }
            value if value.starts_with('-') => {}
            _ => filtered = true,
        }
        index += 1;
    }
    if !selected || excluded || filtered {
        return false;
    }
    if require_unit
        && (command.iter().any(|arg| arg == "--test")
            || command.iter().any(|arg| arg == "--example")
            || command.iter().any(|arg| arg == "--bench")
            || command.iter().any(|arg| arg == "--doc"))
    {
        return false;
    }
    if require_unit && all_targets {
        return true;
    }
    if let Some(kind) = selector {
        let target = package
            .targets
            .iter()
            .find(|target| normalized(&target.src_path) == normalized(source));
        return target
            .is_some_and(|target| target.kind.iter().any(|target_kind| target_kind == &kind));
    }
    true
}

fn acceptance_scope_outcome(plan_text: &str, repo_root: &Path) -> CheckOutcome {
    let tasks = section_text(plan_text, "## Tasks").unwrap_or_default();
    let blocks = task_blocks_with_acceptance(&tasks);
    let rust_tasks: Vec<_> = blocks
        .iter()
        .filter(|(_, _, files)| files.iter().any(|path| is_crate_source_path(path)))
        .collect();
    if rust_tasks.is_empty() {
        return outcome(
            "acceptance-scope",
            CheckState::Pass,
            "no task declares Rust source paths".to_string(),
        );
    }
    let packages = match cargo_packages(repo_root) {
        Ok(packages) => packages,
        Err(error) => {
            let metadata_error = error.as_str();
            let affected: Vec<String> = rust_tasks
                .iter()
                .flat_map(|(task_id, _, files)| {
                    files
                        .iter()
                        .filter(|path| is_crate_source_path(path))
                        .map(move |path| {
                            format!(
                                "{task_id} {path}: package and unit-test scope unresolved ({metadata_error})"
                            )
                        })
                })
                .collect();
            return outcome("acceptance-scope", CheckState::Fail, affected.join("; "));
        }
    };
    let mut violations = Vec::new();
    for (task_id, acceptance, declared_files) in rust_tasks {
        let paths: Vec<String> = declared_files
            .iter()
            .filter(|path| is_crate_source_path(path))
            .cloned()
            .collect();
        let commands = literal_cargo_test_commands(acceptance);
        for path in paths {
            let file = repo_root.join(&path);
            let manifest = packages
                .iter()
                .filter(|package| {
                    normalized(&package.manifest_path)
                        .parent()
                        .is_some_and(|root| normalized(&file).starts_with(normalized(root)))
                })
                .max_by_key(|package| package.manifest_path.components().count());
            let Some(package) = manifest else {
                violations.push(format!(
                    "{task_id} {path}: no enclosing Cargo package/manifest"
                ));
                continue;
            };
            if !package.manifest_path.is_file() {
                violations.push(format!(
                    "{task_id} {path}: manifest is missing: {}",
                    package.manifest_path.display()
                ));
                continue;
            }
            let exists = file.is_file();
            let source = &file;
            let has_tests = if exists {
                match std::fs::read_to_string(&file) {
                    Ok(text) => text.contains("#[cfg(test)]"),
                    Err(error) => {
                        violations.push(format!("{task_id} {path}: cannot read source: {error}"));
                        continue;
                    }
                }
            } else {
                true
            };
            if commands.is_empty()
                || !commands
                    .iter()
                    .any(|command| command_covers_package(command, package, source, false))
            {
                let unit_scope = if has_tests {
                    "; missing its unit-test run for #[cfg(test)] source"
                } else {
                    ""
                };
                violations.push(format!("{task_id} {path}: Acceptance has no literal cargo test command selecting package {}{unit_scope}", package.name));
                continue;
            }
            if has_tests
                && !commands
                    .iter()
                    .any(|command| command_covers_package(command, package, source, true))
            {
                violations.push(format!("{task_id} {path}: {path} has #[cfg(test)] but Acceptance is missing its unit-test run"));
            }
        }
    }
    if violations.is_empty() {
        outcome(
            "acceptance-scope",
            CheckState::Pass,
            "Rust source files are covered by literal package cargo test commands".to_string(),
        )
    } else {
        outcome("acceptance-scope", CheckState::Fail, violations.join("; "))
    }
}

/// Longest allowed cell, in Unicode characters, after backticked spans are removed.
const MAX_CELL_PROSE_CHARS: usize = 200;

/// Cell texts that mark an unfinished table cell.
const PLACEHOLDER_CELLS: [&str; 6] = ["tbd", "todo", "pending", "n/a", "...", "…"];

/// Shared cell validator for the `## Owner Acceptance` and `## Preconditions`
/// tables. Returns the reason a cell is unacceptable, or `None` when it is.
/// A cell must be non-empty, not a placeholder, at most
/// `MAX_CELL_PROSE_CHARS` characters outside backticked spans, and, when
/// `needs_backtick` is set, carry at least one non-blank backticked span.
fn cell_violation(cell: &str, needs_backtick: bool) -> Result<Option<&'static str>, regex::Error> {
    let span = Regex::new(r"`([^`]*)`")?;
    let angle = Regex::new(r"<[^<>]*>")?;
    let cell = cell.trim();
    if cell.is_empty() {
        return Ok(Some("is empty"));
    }
    let outside = span.replace_all(cell, "");
    let outside = outside.trim();
    let is_placeholder_word =
        |text: &str| PLACEHOLDER_CELLS.contains(&text.to_lowercase().as_str());
    if is_placeholder_word(cell) || is_placeholder_word(outside) || angle.is_match(cell) {
        return Ok(Some("is a placeholder"));
    }
    if outside.chars().count() > MAX_CELL_PROSE_CHARS {
        return Ok(Some("exceeds 200 characters outside backticks"));
    }
    if needs_backtick && !span.captures_iter(cell).any(|c| !c[1].trim().is_empty()) {
        return Ok(Some("has no backticked path, command, or URL"));
    }
    Ok(None)
}

/// Shape of one optional ID-keyed table section.
pub(crate) struct IdTable {
    pub(crate) check: &'static str,
    pub(crate) heading: &'static str,
    pub(crate) id_pattern: &'static str,
    pub(crate) columns: &'static [&'static str],
    pub(crate) backtick_column: &'static str,
    pub(crate) max_rows: Option<usize>,
}

/// Validate one optional ID-keyed table section. A missing section passes.
/// A present section fails on a missing header or column, a malformed ID, a
/// cell the shared validator rejects, or rows beyond `max_rows`. Every
/// violation names the offending ID and, for a cell, the column.
pub(crate) fn id_table_outcome(plan_text: &str, spec: &IdTable) -> CheckOutcome {
    let Some(section) = section_text(plan_text, spec.heading) else {
        return outcome(
            spec.check,
            CheckState::Pass,
            format!("no {} section", spec.heading),
        );
    };
    let id_re = match Regex::new(spec.id_pattern) {
        Ok(regex) => regex,
        Err(error) => {
            return outcome(
                spec.check,
                CheckState::Fail,
                format!("internal ID validator construction failed: {error}"),
            );
        }
    };
    let mut rows = section
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|'));
    let Some(header) = rows.next() else {
        return outcome(
            spec.check,
            CheckState::Fail,
            format!("{} has no table header", spec.heading),
        );
    };
    let header_cells = split_md_row(header);
    let mut violations = Vec::new();
    let mut positions = Vec::new();
    for name in spec.columns {
        match header_cells
            .iter()
            .position(|c| c.eq_ignore_ascii_case(name))
        {
            Some(idx) => positions.push((*name, idx)),
            None => violations.push(format!("missing column `{name}`")),
        }
    }
    if !violations.is_empty() {
        return outcome(spec.check, CheckState::Fail, violations.join("; "));
    }

    let mut row_count = 0usize;
    for row in rows {
        let cells = split_md_row(row);
        if cells
            .iter()
            .all(|cell| !cell.is_empty() && cell.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        row_count += 1;
        let id_idx = positions[0].1;
        let id = cells.get(id_idx).map(String::as_str).unwrap_or_default();
        if !id_re.is_match(id) {
            violations.push(format!("row {row_count}: malformed ID `{id}`"));
            continue;
        }
        if spec.max_rows.is_some_and(|max| row_count > max) {
            if let Some(max) = spec.max_rows {
                violations.push(format!("{id}: exceeds the {max}-row cap"));
            }
        }
        for (name, idx) in positions.iter().skip(1) {
            let cell = cells.get(*idx).map(String::as_str).unwrap_or_default();
            match cell_violation(cell, *name == spec.backtick_column) {
                Ok(Some(reason)) => violations.push(format!("{id} `{name}` {reason}")),
                Ok(None) => {}
                Err(error) => violations.push(format!(
                    "{id} `{name}` internal cell validator construction failed: {error}"
                )),
            }
        }
    }
    if violations.is_empty() {
        outcome(
            spec.check,
            CheckState::Pass,
            format!("{row_count} well-formed row(s) in {}", spec.heading),
        )
    } else {
        outcome(spec.check, CheckState::Fail, violations.join("; "))
    }
}

pub(crate) fn owner_acceptance_outcome(plan_text: &str) -> CheckOutcome {
    id_table_outcome(
        plan_text,
        &IdTable {
            check: "owner-acceptance",
            heading: "## Owner Acceptance",
            id_pattern: r"^OA-[0-9]{2}$",
            columns: &[
                "ID",
                "What to check",
                "Expected result",
                "Pass/fail rule",
                "Why not automated",
            ],
            backtick_column: "What to check",
            max_rows: Some(3),
        },
    )
}

pub(crate) fn preconditions_outcome(plan_text: &str) -> CheckOutcome {
    id_table_outcome(
        plan_text,
        &IdTable {
            check: "preconditions",
            heading: "## Preconditions",
            id_pattern: r"^PC-[0-9]{2}$",
            columns: &[
                "ID",
                "Requirement",
                "Check command",
                "Expected result",
                "How to satisfy",
            ],
            backtick_column: "Check command",
            max_rows: None,
        },
    )
}

fn id_well_formed_from(malformed_tasks: &[String], malformed_tests: &[String]) -> CheckOutcome {
    let passed = malformed_tasks.is_empty() && malformed_tests.is_empty();
    outcome(
        "id-well-formed",
        if passed {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        if passed {
            "all T/TP ids are well-formed".to_string()
        } else {
            format!(
                "malformed task ids: [{}]; malformed test ids: [{}]",
                malformed_tasks.join(" | "),
                malformed_tests.join(" | ")
            )
        },
    )
}

/// The `id-well-formed` row over plan text alone, for repair revalidation.
pub(crate) fn id_well_formed_outcome(plan_text: &str) -> CheckOutcome {
    let tasks = section_text(plan_text, "## Tasks").unwrap_or_default();
    let test_plan = section_text(plan_text, "## Test Plan").unwrap_or_default();
    id_well_formed_from(&task_ids(&tasks).1, &tp_ids(&test_plan).1)
}

/// The `test-plan-coverage` row over plan text alone, for repair revalidation.
pub(crate) fn test_plan_coverage_for_plan(plan_text: &str) -> CheckOutcome {
    let tasks = section_text(plan_text, "## Tasks").unwrap_or_default();
    test_plan_coverage_outcome(plan_text, &task_ids(&tasks).0)
}

fn run_checks(plan_text: &str, repo_root: &Path) -> Vec<CheckOutcome> {
    let tasks = section_text(plan_text, "## Tasks").unwrap_or_default();
    let test_plan = section_text(plan_text, "## Test Plan").unwrap_or_default();
    let (tasks_ids, malformed_tasks) = task_ids(&tasks);
    let (tests_ids, malformed_tests) = tp_ids(&test_plan);
    let task_dupes = dupes(&tasks_ids);
    let test_dupes = dupes(&tests_ids);
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
        id_well_formed_from(&malformed_tasks, &malformed_tests),
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
        test_plan_coverage_outcome(plan_text, &tasks_ids),
        manual_limit_outcome(plan_text),
        owner_acceptance_outcome(plan_text),
        preconditions_outcome(plan_text),
        acceptance_scope_outcome(plan_text, repo_root),
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
    checks.push(translation_split_outcome(&tasks));
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
            return if err.starts_with("receipt scope resolution:") {
                ExitCode::Error
            } else {
                ExitCode::Usage
            };
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
    let raw = std::fs::read(&parsed.target).unwrap_or_default();
    let plan_text = String::from_utf8(raw.clone()).unwrap_or_default();
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
    let body = match super::finalize_check::receipt_envelope(None, Some(&parsed.target)) {
        Ok(envelope) => {
            let rendered = format!(
                "{}{}",
                render_receipt(&receipt),
                super::finalize_check::render_repair_dispositions_for_target(
                    &receipt.checks,
                    "refining",
                    "default",
                    &parsed.target,
                    &raw,
                    None
                )
            );
            match super::finalize_check::bind_receipt(rendered, &envelope) {
                Ok(body) => body,
                Err(err) => {
                    eprintln!("gal refining-check: invalid execution binding: {err}");
                    return ExitCode::Error;
                }
            }
        }
        Err(err) => {
            eprintln!("gal refining-check: cannot establish execution identity: {err}");
            return ExitCode::Error;
        }
    };
    if let Err(err) = std::fs::write(&parsed.receipt, body) {
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
            default_receipt_path(target).unwrap(),
            default_receipt_path(target).unwrap(),
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
            tmp.path().join("docs").join("glossary.md"),
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
        format!(
            "# Plan\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {} | manual | Why manual: ad-hoc probe | {} |\n\n## Review Results\n### Engineering Review\nCLEAR\n\n<!-- ENG_REVIEW: CLEAR -->\n\n## Tasks\n- [ ] {} - `docs/example.md` update.\n",
            tpid(1),
            tid(1),
            tid(1)
        )
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
            tmp.path().join("docs").join("glossary.md"),
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
    fn receipt_has_no_task_test_pairing_row() {
        let repo = temp_repo();
        let checks = run_checks(&sample_plan(), repo.path());
        assert!(!checks.iter().any(|c| c.name == "task-test-pairing"));
    }
    #[test]
    fn manual_without_reason_fails() {
        let repo = temp_repo();
        let plan = sample_plan().replace("Why manual: ad-hoc probe", "check");
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "manual-limit" && c.state == CheckState::Fail));
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
            plan.push_str(&format!("| TP-{n:02} | grep | check | T-{n:02} |\n"));
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
