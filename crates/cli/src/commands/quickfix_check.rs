//! `gal quickfix-check [--receipt <path>]` — scope and verification gate for the
//! quickfix lane.
//!
//! The gate judges a set of changed paths against the committed `.dev/project.md`
//! (read from `HEAD`, never the working tree, so an edit cannot loosen its own
//! gate): the diff must stay within the configured file and line limits, touch no
//! Protected Path and no dependency manifest, and every authoritative check
//! command must pass. An empty change set is `not-run`, never `pass`.
//!
//! Limits come from the `quickfix` key of `~/.gal/config/config.json`. A missing
//! file or key means the defaults. A malformed key is an error with no fallback.

use super::finalize_check::{
    locate_authoritative_commands, run_command_verbatim, CheckOutcome, CheckState, Receipt,
};
use gal_engine::ExitCode;
use regex::Regex;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_MAX_FILES: u64 = 3;
const DEFAULT_MAX_LINES: u64 = 50;
const DEFAULT_MAX_ATTEMPTS: u64 = 2;
const DEFAULT_RECEIPT: &str = ".dev/pipeline/quickfix/check.receipt.md";
const DEPENDENCY_FILE_NAMES: [&str; 3] = ["Cargo.toml", "Cargo.lock", "package-lock.json"];

/// Limits and switches of the quickfix lane.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuickfixConfig {
    pub(crate) max_files: u64,
    pub(crate) max_lines: u64,
    pub(crate) max_attempts: u64,
    pub(crate) hook: bool,
}

impl Default for QuickfixConfig {
    fn default() -> Self {
        Self {
            max_files: DEFAULT_MAX_FILES,
            max_lines: DEFAULT_MAX_LINES,
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            hook: false,
        }
    }
}

fn read_unsigned_field(
    object: &serde_json::Map<String, serde_json::Value>,
    key: &str,
    default: u64,
) -> Result<u64, String> {
    match object.get(key) {
        None => Ok(default),
        Some(value) => value
            .as_u64()
            .ok_or_else(|| format!("quickfix.{key} must be a non-negative integer")),
    }
}

/// Parse the `quickfix` key out of a parsed `config.json` root.
pub(crate) fn parse_quickfix_config(root: &serde_json::Value) -> Result<QuickfixConfig, String> {
    let Some(root_object) = root.as_object() else {
        return Err("config.json root must be a JSON object".to_string());
    };
    let Some(section) = root_object.get("quickfix") else {
        return Ok(QuickfixConfig::default());
    };
    let Some(object) = section.as_object() else {
        return Err("quickfix must be a JSON object".to_string());
    };
    for key in object.keys() {
        if !matches!(
            key.as_str(),
            "maxFiles" | "maxLines" | "maxAttempts" | "hook"
        ) {
            return Err(format!("quickfix.{key} is not a recognised field"));
        }
    }
    let hook = match object.get("hook") {
        None => false,
        Some(value) => value
            .as_bool()
            .ok_or("quickfix.hook must be true or false")?,
    };
    Ok(QuickfixConfig {
        max_files: read_unsigned_field(object, "maxFiles", DEFAULT_MAX_FILES)?,
        max_lines: read_unsigned_field(object, "maxLines", DEFAULT_MAX_LINES)?,
        max_attempts: read_unsigned_field(object, "maxAttempts", DEFAULT_MAX_ATTEMPTS)?,
        hook,
    })
}

/// Load the quickfix configuration from the machine config file.
pub(crate) fn load_quickfix_config() -> Result<QuickfixConfig, String> {
    let Some(path) = gal_foundation::paths::machine_config_path() else {
        return Ok(QuickfixConfig::default());
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(QuickfixConfig::default());
        }
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let root: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| format!("{} is not valid JSON: {error}", path.display()))?;
    parse_quickfix_config(&root).map_err(|error| format!("{}: {error}", path.display()))
}

/// Overall outcome of a gate evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum QuickfixVerdict {
    Pass,
    Fail,
    NotRun,
}

/// Result of evaluating one changed-path set.
#[derive(Debug, Clone)]
pub(crate) struct QuickfixEvaluation {
    pub(crate) receipt: Receipt,
    pub(crate) file_count: usize,
    pub(crate) line_count: u64,
    pub(crate) config: QuickfixConfig,
}

impl QuickfixEvaluation {
    /// `Fail` when any check failed, `NotRun` when nothing failed but a check
    /// could not run, otherwise `Pass`.
    pub(crate) fn verdict_kind(&self) -> QuickfixVerdict {
        if self
            .receipt
            .checks
            .iter()
            .any(|check| check.state == CheckState::Fail)
        {
            QuickfixVerdict::Fail
        } else if self
            .receipt
            .checks
            .iter()
            .any(|check| check.state == CheckState::NotRun)
        {
            QuickfixVerdict::NotRun
        } else {
            QuickfixVerdict::Pass
        }
    }

    /// The verdict as the word printed in receipts.
    pub(crate) fn verdict(&self) -> &'static str {
        match self.verdict_kind() {
            QuickfixVerdict::Pass => "pass",
            QuickfixVerdict::Fail => "fail",
            QuickfixVerdict::NotRun => "not-run",
        }
    }
}

pub(crate) fn run_git(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    run_git_env(repo, args, &[])
}

/// Run `git` in `repo` with extra environment variables.
pub(crate) fn run_git_env(
    repo: &Path,
    args: &[&str],
    envs: &[(&str, &OsStr)],
) -> Result<Vec<u8>, String> {
    let mut command = Command::new("git");
    command.current_dir(repo).args(args);
    for (key, value) in envs {
        command.env(key, value);
    }
    let output = command
        .output()
        .map_err(|error| format!("cannot run git {}: {error}", args.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}

pub(crate) fn split_nul_paths(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .map(|part| String::from_utf8_lossy(part).replace('\\', "/"))
        .collect()
}

fn list_untracked_paths(repo: &Path) -> Result<BTreeSet<String>, String> {
    let bytes = run_git(repo, &["ls-files", "--others", "--exclude-standard", "-z"])?;
    Ok(split_nul_paths(&bytes).into_iter().collect())
}

/// Every path whose working tree differs from `HEAD`, including untracked files.
pub(crate) fn collect_working_tree_changes(repo: &Path) -> Result<BTreeSet<String>, String> {
    let tracked = run_git(repo, &["diff", "--name-only", "-z", "--no-renames", "HEAD"])?;
    let mut paths: BTreeSet<String> = split_nul_paths(&tracked).into_iter().collect();
    paths.extend(list_untracked_paths(repo)?);
    Ok(paths)
}

/// Count non-blank lines of an untracked file. A file holding a NUL byte is
/// binary and contributes no lines.
fn count_untracked_lines(path: &Path) -> Result<u64, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read untracked file {}: {error}", path.display()))?;
    if bytes.contains(&0) {
        return Ok(0);
    }
    Ok(String::from_utf8_lossy(&bytes)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count() as u64)
}

/// Lines changed across `changed`, ignoring whitespace-only and blank-line
/// edits. A binary tracked file (numstat `-`) counts as zero lines.
fn count_changed_lines(repo: &Path, changed: &BTreeSet<String>) -> Result<u64, String> {
    let numstat = run_git(
        repo,
        &[
            "diff",
            "--numstat",
            "-z",
            "--no-renames",
            "--ignore-all-space",
            "--ignore-blank-lines",
            "HEAD",
        ],
    )?;
    let mut total: u64 = 0;
    for record in numstat.split(|byte| *byte == 0).filter(|r| !r.is_empty()) {
        let text = String::from_utf8_lossy(record);
        let mut fields = text.splitn(3, '\t');
        let (Some(added), Some(deleted), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            return Err(format!("unreadable numstat record '{text}'"));
        };
        if !changed.contains(&path.replace('\\', "/")) {
            continue;
        }
        if let (Ok(added), Ok(deleted)) = (added.parse::<u64>(), deleted.parse::<u64>()) {
            total += added + deleted;
        }
    }
    for path in list_untracked_paths(repo)? {
        if changed.contains(&path) {
            total += count_untracked_lines(&repo.join(&path))?;
        }
    }
    Ok(total)
}

/// True when `project_md` has a `## Protected Paths` heading.
fn has_protected_paths_heading(project_md: &str) -> bool {
    project_md
        .lines()
        .any(|line| line.trim_end() == "## Protected Paths")
}

/// Backticked entries under the `## Protected Paths` heading.
fn parse_protected_paths(project_md: &str) -> Vec<String> {
    let mut in_section = false;
    let mut section = String::new();
    for line in project_md.lines() {
        if line.starts_with("## ") {
            in_section = line.trim_end() == "## Protected Paths";
            continue;
        }
        if in_section {
            section.push_str(line);
            section.push('\n');
        }
    }
    let ticks = Regex::new(r"`([^`\r\n]+)`").expect("static regex");
    ticks
        .captures_iter(&section)
        .map(|capture| capture[1].trim().replace('\\', "/"))
        .map(|entry| entry.trim_start_matches("./").to_string())
        .filter(|entry| !entry.is_empty() && !entry.contains(char::is_whitespace))
        .collect()
}

fn is_under_protected_path(path: &str, entry: &str) -> bool {
    let entry = entry.trim_end_matches('/');
    path == entry || path.starts_with(&format!("{entry}/"))
}

fn is_dependency_file(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    DEPENDENCY_FILE_NAMES.contains(&name)
}

fn check(name: &str, state: CheckState, summary: String) -> CheckOutcome {
    CheckOutcome {
        name: name.to_string(),
        command: None,
        state,
        summary,
    }
}

fn limit_check(name: &str, label: &str, actual: u64, limit: u64) -> CheckOutcome {
    if actual > limit {
        check(
            name,
            CheckState::Fail,
            format!("scope exceeded: {actual} {label} changed, limit {limit}"),
        )
    } else {
        check(
            name,
            CheckState::Pass,
            format!("{actual} {label} changed, limit {limit}"),
        )
    }
}

fn restricted_paths_check(name: &str, what: &str, offenders: Vec<String>) -> CheckOutcome {
    if offenders.is_empty() {
        check(name, CheckState::Pass, format!("no {what} touched"))
    } else {
        check(
            name,
            CheckState::Fail,
            format!("{what} touched: {}", offenders.join(", ")),
        )
    }
}

fn command_checks(project_md: Option<&str>) -> Vec<CheckOutcome> {
    let commands = project_md.and_then(locate_authoritative_commands);
    match commands {
        None => vec![check(
            "authoritative-command",
            CheckState::NotRun,
            "no <!-- gal:authoritative-check --> json fence in committed .dev/project.md"
                .to_string(),
        )],
        Some(commands) if commands.is_empty() => vec![check(
            "authoritative-command",
            CheckState::NotRun,
            "authoritative-check fence has an empty command array".to_string(),
        )],
        Some(commands) => commands
            .iter()
            .map(|command| {
                let (passed, summary) = run_command_verbatim(command);
                CheckOutcome {
                    name: "authoritative-command".to_string(),
                    command: Some(command.clone()),
                    state: if passed {
                        CheckState::Pass
                    } else {
                        CheckState::Fail
                    },
                    summary: if passed {
                        summary
                    } else {
                        format!("command failed: {command}; {summary}")
                    },
                }
            })
            .collect(),
    }
}

/// Evaluate `changed` (repo-relative, `/`-separated) against the quickfix gate.
///
/// Check commands run in the process working directory, which the caller keeps
/// at `repo`. Scope checks always run. The check commands run only when every
/// scope check passed, otherwise they are recorded as `not-run`.
pub(crate) fn evaluate_change_set(
    repo: &Path,
    config: &QuickfixConfig,
    changed: &BTreeSet<String>,
) -> QuickfixEvaluation {
    let mut checks = Vec::new();
    let file_count = changed.len();
    let mut line_count = 0;

    if changed.is_empty() {
        checks.push(check(
            "change-set",
            CheckState::NotRun,
            "no changed files to check".to_string(),
        ));
        return QuickfixEvaluation {
            receipt: Receipt { checks },
            file_count,
            line_count,
            config: config.clone(),
        };
    }

    let project_md = run_git(repo, &["show", "HEAD:.dev/project.md"])
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned());

    checks.push(limit_check(
        "scope-files",
        "file(s)",
        file_count as u64,
        config.max_files,
    ));
    match count_changed_lines(repo, changed) {
        Ok(lines) => {
            line_count = lines;
            checks.push(limit_check(
                "scope-lines",
                "line(s)",
                lines,
                config.max_lines,
            ));
        }
        Err(error) => checks.push(check(
            "scope-lines",
            CheckState::Fail,
            format!("cannot count changed lines: {error}"),
        )),
    }

    match &project_md {
        Ok(text) if !has_protected_paths_heading(text) => checks.push(check(
            "protected-paths",
            CheckState::NotRun,
            "committed .dev/project.md has no ## Protected Paths heading".to_string(),
        )),
        Ok(text) => {
            let protected = parse_protected_paths(text);
            let offenders = changed
                .iter()
                .filter(|path| {
                    protected
                        .iter()
                        .any(|entry| is_under_protected_path(path, entry))
                })
                .cloned()
                .collect();
            checks.push(restricted_paths_check(
                "protected-paths",
                "Protected Path(s)",
                offenders,
            ));
        }
        Err(error) => checks.push(check(
            "protected-paths",
            CheckState::Fail,
            format!("cannot read committed .dev/project.md: {error}"),
        )),
    }

    checks.push(restricted_paths_check(
        "dependency-files",
        "dependency file(s)",
        changed
            .iter()
            .filter(|path| is_dependency_file(path))
            .cloned()
            .collect(),
    ));

    if checks.iter().all(|c| c.state == CheckState::Pass) {
        checks.extend(command_checks(project_md.as_ref().ok().map(String::as_str)));
    } else {
        checks.push(check(
            "authoritative-command",
            CheckState::NotRun,
            "skipped because a scope check did not pass".to_string(),
        ));
    }

    QuickfixEvaluation {
        receipt: Receipt { checks },
        file_count,
        line_count,
        config: config.clone(),
    }
}

pub(crate) fn render_quickfix_receipt(evaluation: &QuickfixEvaluation) -> String {
    let config = &evaluation.config;
    let mut out = String::from("# quickfix-check receipt\n\n");
    out.push_str(&format!("overall: {}\n", evaluation.verdict()));
    out.push_str(&format!(
        "files: {}\nlines: {}\n",
        evaluation.file_count, evaluation.line_count
    ));
    out.push_str(&format!(
        "config: maxFiles={} maxLines={} maxAttempts={} hook={}\n\n",
        config.max_files, config.max_lines, config.max_attempts, config.hook
    ));
    out.push_str("| check | state | command | summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    for outcome in &evaluation.receipt.checks {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            outcome.name,
            outcome.state.as_str(),
            outcome.command.as_deref().unwrap_or("—"),
            outcome.summary,
        ));
    }
    out.push_str("\nrepair_context: unavailable\nrepair_context_authority: none\n\n| check | disposition | repair_id |\n| --- | --- | --- |\n");
    for outcome in &evaluation.receipt.checks {
        if matches!(&outcome.state, CheckState::Fail | CheckState::NotRun) {
            out.push_str(&format!("| {} | owner | — |\n", outcome.name));
        }
    }
    out
}

fn parse_receipt_argument(args: &[String]) -> Result<PathBuf, String> {
    let mut receipt: Option<PathBuf> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--receipt" => {
                let value = iter.next().ok_or("--receipt requires a path")?;
                receipt = Some(PathBuf::from(value));
            }
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    Ok(receipt.unwrap_or_else(|| PathBuf::from(DEFAULT_RECEIPT)))
}

fn write_receipt(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("cannot create receipt dir {}: {error}", parent.display()))?;
    }
    std::fs::write(path, content)
        .map_err(|error| format!("cannot write receipt {}: {error}", path.display()))
}

fn render_config_failure(error: &str) -> String {
    let mut out = String::from("# quickfix-check receipt\n\noverall: fail\n\n");
    out.push_str("| check | state | command | summary |\n");
    out.push_str("| --- | --- | --- | --- |\n");
    out.push_str(&format!(
        "| quickfix-config | fail | — | {} |\n",
        error.replace('\n', " ").replace('|', "\\|")
    ));
    out.push_str("\nrepair_context: unavailable\nrepair_context_authority: none\n\n| check | disposition | repair_id |\n| --- | --- | --- |\n| quickfix-config | owner | — |\n");
    out
}

/// Run `gal quickfix-check`: judge the whole working-tree diff against `HEAD`.
pub(crate) fn cmd_quickfix_check(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let receipt_path = match parse_receipt_argument(&rest) {
        Ok(path) => path,
        Err(error) => {
            eprintln!("gal quickfix-check: {error}");
            return ExitCode::Usage;
        }
    };
    let repo = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let config = match load_quickfix_config() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("gal quickfix-check: {error}");
            if let Err(write_error) = write_receipt(&receipt_path, &render_config_failure(&error)) {
                eprintln!("gal quickfix-check: {write_error}");
            }
            return ExitCode::Error;
        }
    };

    let mut changed = match collect_working_tree_changes(&repo) {
        Ok(changed) => changed,
        Err(error) => {
            eprintln!("gal quickfix-check: {error}");
            return ExitCode::Error;
        }
    };
    // A receipt written inside the repository is gate output, not part of the
    // change under review.
    let absolute_receipt = if receipt_path.is_absolute() {
        receipt_path.clone()
    } else {
        repo.join(&receipt_path)
    };
    if let (Ok(receipt_real), Ok(repo_real)) =
        (absolute_receipt.canonicalize(), repo.canonicalize())
    {
        if let Ok(relative) = receipt_real.strip_prefix(&repo_real) {
            changed.remove(&relative.to_string_lossy().replace('\\', "/"));
        }
    }

    let evaluation = evaluate_change_set(&repo, &config, &changed);
    if let Err(error) = write_receipt(&receipt_path, &render_quickfix_receipt(&evaluation)) {
        eprintln!("gal quickfix-check: {error}");
        return ExitCode::Error;
    }
    println!(
        "gal quickfix-check: {} ({} file(s), {} line(s)) → {}",
        evaluation.verdict(),
        evaluation.file_count,
        evaluation.line_count,
        receipt_path.display()
    );
    if evaluation.verdict() == "pass" {
        ExitCode::Success
    } else {
        ExitCode::Error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> Result<QuickfixConfig, String> {
        parse_quickfix_config(&serde_json::from_str(text).unwrap())
    }

    #[test]
    fn quickfix_failure_stays_owner_without_repair_helper_authority() {
        let evaluation = QuickfixEvaluation {
            receipt: Receipt {
                checks: vec![CheckOutcome {
                    name: "changed-paths".into(),
                    command: None,
                    state: CheckState::Fail,
                    summary: "policy check failed".into(),
                }],
            },
            file_count: 1,
            line_count: 1,
            config: QuickfixConfig::default(),
        };
        let rendered = render_quickfix_receipt(&evaluation);
        assert!(rendered.contains("repair_context: unavailable"));
        assert!(rendered.contains("| changed-paths | owner | — |"));
        assert!(!rendered.contains("gate_attempt_id:"));
    }

    #[test]
    fn missing_key_gives_defaults() {
        assert_eq!(parse("{}").unwrap(), QuickfixConfig::default());
    }

    #[test]
    fn explicit_fields_override_defaults() {
        let config =
            parse(r#"{"quickfix":{"maxFiles":5,"maxLines":9,"maxAttempts":1,"hook":true}}"#)
                .unwrap();
        assert_eq!(
            config,
            QuickfixConfig {
                max_files: 5,
                max_lines: 9,
                max_attempts: 1,
                hook: true
            }
        );
    }

    #[test]
    fn malformed_fields_are_errors() {
        assert!(parse(r#"{"quickfix":{"maxFiles":"three"}}"#).is_err());
        assert!(parse(r#"{"quickfix":{"maxLines":-1}}"#).is_err());
        assert!(parse(r#"{"quickfix":{"hook":"yes"}}"#).is_err());
        assert!(parse(r#"{"quickfix":[]}"#).is_err());
        assert!(parse(r#"{"quickfix":{"maxFile":3}}"#).is_err());
    }

    #[test]
    fn protected_paths_read_only_their_section() {
        let md = "## Other\n`not/this`\n\n## Protected Paths\n\n- `a/b/` — why\n- `c.txt`\n\n## Next\n`nor/this`\n";
        assert_eq!(parse_protected_paths(md), vec!["a/b/", "c.txt"]);
    }

    #[test]
    fn protected_paths_heading_is_detected() {
        assert!(has_protected_paths_heading(
            "## A

## Protected Paths
- `x`
"
        ));
        assert!(!has_protected_paths_heading(
            "## A
Protected Paths
### Protected Paths
"
        ));
    }

    #[test]
    fn protected_prefix_matches_whole_components() {
        assert!(is_under_protected_path("a/b/c.rs", "a/b/"));
        assert!(is_under_protected_path("a/b", "a/b/"));
        assert!(!is_under_protected_path("a/bc/d.rs", "a/b/"));
    }

    #[test]
    fn dependency_files_match_at_any_depth() {
        assert!(is_dependency_file("Cargo.lock"));
        assert!(is_dependency_file("web/app/package-lock.json"));
        assert!(!is_dependency_file("docs/Cargo.toml.md"));
    }
}
