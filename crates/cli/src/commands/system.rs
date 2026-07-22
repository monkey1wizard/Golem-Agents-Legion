//! System/repo commands: filter-transform, init,
//! translation-freshness, commit-msg, naming-gate.

use gal_engine::ExitCode;
use std::path::Path;

pub(crate) fn cmd_filter_transform(args: &[String], smudge: bool) -> ExitCode {
    use std::io::{Read, Write};

    let _ = args;
    let repo_root = match std::env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("gal {}: {e}", if smudge { "smudge" } else { "clean" });
            return ExitCode::Error;
        }
    };

    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!(
            "gal {}: failed to read stdin: {e}",
            if smudge { "smudge" } else { "clean" }
        );
        return ExitCode::Error;
    }

    let result = if smudge {
        crate::gal::git_filters::run_smudge(&input, &repo_root)
    } else {
        crate::gal::git_filters::run_clean(&input, &repo_root)
    };

    match result {
        Ok(result) => {
            if let Err(e) = std::io::stdout().write_all(result.output.as_bytes()) {
                eprintln!(
                    "gal {}: failed to write stdout: {e}",
                    if smudge { "smudge" } else { "clean" }
                );
                return ExitCode::Error;
            }
            for warning in result.warnings {
                eprintln!("{warning}");
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::Error
        }
    }
}

/// Classify every fixed repo-adapter path (`REPO_ADAPTER_ROOTS` then
/// `REPO_ADAPTER_CONDITIONAL_LAYERS`, in that stable order) against the
/// renderer's `ProjectionReport`, returning one `"<Verb>: <path>"` row per
/// path that has something to report:
///
/// - a root always gets a row: `Written` if its full joined path is in
///   `written_files`, `Unchanged` otherwise (existing, byte-identical — the
///   renderer never removes a root).
/// - a conditional layer only gets a row when the report actually mentions
///   it: `Removed` if in `removed_paths`, `Written` if in `written_files`,
///   omitted entirely when absent from both — an absent conditional layer is
///   benign, not an outcome worth printing.
fn adapter_outcome_rows(
    report: &crate::gal::render::ProjectionReport,
    repo_root: &Path,
) -> Vec<String> {
    let mut rows = Vec::new();
    for root in crate::gal::render::REPO_ADAPTER_ROOTS {
        let full = repo_root.join(root);
        let verb = if report.written_files.contains(&full) {
            "Written"
        } else {
            "Unchanged"
        };
        rows.push(format!("{verb}: {root}"));
    }
    for layer in crate::gal::render::REPO_ADAPTER_CONDITIONAL_LAYERS {
        let full = repo_root.join(layer);
        if report.removed_paths.contains(&full) {
            rows.push(format!("Removed: {layer}"));
        } else if report.written_files.contains(&full) {
            rows.push(format!("Written: {layer}"));
        }
    }
    rows
}

/// Build the full set of `gal init` success-report lines — pure data
/// production, separated from the `println!` side effect in `cmd_init` so
/// the report content is directly unit-testable.
fn init_report_lines(report: &crate::init_repo::InitRepoReport) -> Vec<String> {
    let mut lines = vec![format!(
        "Initialized repo context in: {}",
        report.target_path.display()
    )];
    let adapter_verb = if report.regenerated_only {
        "Preserved (already initialized)"
    } else {
        "Created"
    };
    lines.push(format!("- {adapter_verb}: .dev/project.md"));
    lines.push(format!("- {adapter_verb}: .dev/state.md"));
    lines.push("- Ensured: .dev/plans/".to_string());

    for row in adapter_outcome_rows(&report.adapter_report, &report.target_path) {
        lines.push(format!("- {row}"));
    }

    if report.regenerated_only {
        lines.push(
            "- Next: re-run any time to refresh the repo-local adapters from the current .dev/project.md"
                .to_string(),
        );
    } else {
        lines.push(
            "- Next: review .dev/project.md, fill in summary fields, then run `gal doctor`"
                .to_string(),
        );
    }
    if !report.source_docs.is_empty() {
        lines.push(String::new());
        lines.push(format!(
            "Adopt-existing: found {} source document(s).",
            report.source_docs.len()
        ));
        lines
            .push("Review .dev/project.md and fill in summaries from discovered docs.".to_string());
    }
    if !report.tech_hints.is_empty() {
        lines.push(format!(
            "Detected tech stack: {}",
            report.tech_hints.join(", ")
        ));
    }
    lines
}

pub(crate) fn cmd_init(args: &[String]) -> ExitCode {
    let options = match crate::init_repo::parse_init_repo_options(args) {
        Ok(options) => options,
        Err(e) => {
            eprintln!("gal init: {e}");
            return ExitCode::Usage;
        }
    };

    match crate::init_repo::run_init_repo(&options) {
        Ok(report) => {
            for line in init_report_lines(&report) {
                println!("{line}");
            }
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal init: {e}");
            ExitCode::Error
        }
    }
}

pub(crate) fn cmd_translation_freshness() -> ExitCode {
    let repo_root = match std::env::current_dir() {
        Ok(path) => path,
        Err(e) => {
            eprintln!("gal translation-freshness: {e}");
            return ExitCode::Error;
        }
    };

    let report = crate::gal::translation::run_translation_freshness(
        &repo_root,
        &["README.md", "docs/manual.md", "docs/integrations.md"],
    );
    if report.rows.is_empty() {
        println!("No docs/i18n/ tree found; nothing to check.");
        return ExitCode::Success;
    }

    println!("{:<34} {:<8} {:<9} NOTE", "DOC", "LANG", "STATUS");
    for row in report.rows {
        println!(
            "{:<34} {:<8} {:<9} {}",
            row.doc, row.lang, row.status, row.note
        );
    }
    println!(
        "Summary: current={}  stale={}  missing={}",
        report.current, report.stale, report.missing
    );
    ExitCode::Success
}

/// Which behavior `cmd_commit_msg` selects for a given argument list.
/// `--print` is checked first, so `--print` wins when both `--print` and
/// `--context` are present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommitMsgMode {
    Print,
    Context,
    Hook,
}

fn commit_msg_mode(args: &[String]) -> CommitMsgMode {
    if args.iter().any(|a| a == "--print" || a == "--generate") {
        return CommitMsgMode::Print;
    }
    if args.iter().any(|a| a == "--context") {
        return CommitMsgMode::Context;
    }
    CommitMsgMode::Hook
}

/// Run `gal commit-msg` — git commit-msg hook + message generator.
///
/// Modes:
/// - `gal commit-msg --print`  → write a generated message to stdout (for the
///   `git-commits` skill / `git-commit-msg` command). Replaces the retired
/// - `gal commit-msg <file>`   → git commit-msg hook. Non-destructive: an
///   existing author message is preserved; only a blank message is filled.
pub(crate) fn cmd_commit_msg(args: &[String]) -> ExitCode {
    use crate::gal::commit_msg::{
        assemble_commit_context, fill_commit_msg_file, generate_commit_message, CommitMsgResult,
    };

    let entries = get_staged_entries();

    // --print mode: emit the generated message (or a friendly note) to stdout.
    if commit_msg_mode(args) == CommitMsgMode::Print {
        match generate_commit_message(&entries) {
            Some(msg) => {
                println!("{msg}");
                return ExitCode::Success;
            }
            None => {
                println!("No changes staged for commit.");
                return ExitCode::Success;
            }
        }
    }

    // --context mode: emit the 5-block FILES/BASELINE/PLANS/PROMPTS/CHANGES
    // assembly to stdout — the single front-end call a message-drafting
    // consumer needs, instead of reading the raw diff itself.
    if commit_msg_mode(args) == CommitMsgMode::Context {
        let context =
            assemble_commit_context(&entries, resolve_staged_content, resolve_hunk_headers);
        println!("{context}");
        return ExitCode::Success;
    }

    // Hook mode: first positional arg is the commit message file path.
    let msg_path_str = match args.iter().skip(1).find(|a| !a.starts_with("--")) {
        Some(p) => p.clone(),
        None => {
            eprintln!("gal commit-msg: missing message file argument");
            eprintln!("usage: gal commit-msg <path-to-commit-message-file>");
            eprintln!("       gal commit-msg --print");
            return ExitCode::Usage;
        }
    };

    let msg_path = Path::new(&msg_path_str);
    if !msg_path.exists() {
        eprintln!("gal commit-msg: message file not found: {msg_path_str}");
        return ExitCode::Error;
    }

    match fill_commit_msg_file(msg_path, &entries) {
        Ok(CommitMsgResult::NoOp | CommitMsgResult::Updated) => ExitCode::Success,
        Err(e) => {
            eprintln!("gal commit-msg: {e}");
            ExitCode::Error
        }
    }
}

/// Retrieve staged entries (status + path, rename-aware) from git.
/// Returns an empty list when git is unavailable or nothing is staged.
fn get_staged_entries() -> Vec<crate::gal::commit_msg::StagedEntry> {
    let output = std::process::Command::new("git")
        .args(["diff", "--cached", "--name-status", "--find-renames"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            crate::gal::commit_msg::parse_name_status(&text)
        }
        _ => Vec::new(), // git unavailable or not a repo → treat as empty staging
    }
}

/// Read a path's staged (index) content via `git show :<path>`, for
/// `assemble_commit_context`'s plan/prompt `resolve_staged_content` closure.
/// `None` on any read failure (including a deleted path, which the caller
/// never queries in the first place — see `assemble_commit_context`).
fn resolve_staged_content(path: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("show")
        .arg(format!(":{path}"))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Return the `@@ ... @@` hunk headers for the given staged code paths, for
/// `assemble_commit_context`'s `resolve_hunk_headers` closure. Empty string on
/// any git failure or when `code_paths` is empty.
fn resolve_hunk_headers(code_paths: &[String]) -> String {
    if code_paths.is_empty() {
        return String::new();
    }
    let mut cmd = std::process::Command::new("git");
    cmd.args(["diff", "--cached", "-U0", "--"]);
    cmd.args(code_paths);
    let output = match cmd.output() {
        Ok(o) if o.status.success() => o.stdout,
        _ => return String::new(),
    };
    String::from_utf8_lossy(&output)
        .lines()
        .filter(|l| l.starts_with("@@"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Run `gal naming-gate [--staged] [paths...]` — blocking naming-authority scan.
///
/// Scans durable surfaces for plan-task ID provenance and retired terms (see
/// `docs/naming.md`). **Blocking** (enforced by `infra-naming-authority`): returns
/// an error exit when any violation is found (or the scanner cannot be built — fail
/// closed), so pre-commit and `gal doctor` enforce the naming authority. Returns
/// `Success` only on a clean tree.
pub(crate) fn cmd_naming_gate(args: &[String]) -> ExitCode {
    use crate::gal::naming_gate::{load_retired_terms, FileViolations, NamingGate};

    let staged = args.iter().any(|a| a == "--staged");
    let root = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let retired = load_retired_terms(&root);
    let gate = match NamingGate::new(&retired) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("gal naming-gate: failed to build scanner: {e}");
            return ExitCode::Error; // blocking: cannot prove clean -> fail closed
        }
    };

    let scan_result: Result<Vec<FileViolations>, String> = if staged {
        scan_staged(&gate)
    } else {
        gate.scan_tree(&root)
    };
    let hits = match scan_result {
        Ok(hits) => hits,
        Err(e) => {
            eprintln!("naming-gate: scan failed: {e}");
            return ExitCode::Error; // fail closed: cannot prove clean
        }
    };

    let total: usize = hits.iter().map(|f| f.violations.len()).sum();
    if total == 0 {
        println!("naming-gate: clean (0 violations).");
        return ExitCode::Success;
    }

    eprintln!(
        "naming-gate: {total} naming-authority violation(s) across {} file(s).",
        hits.len()
    );
    eprintln!("  plan-task IDs and retired terms belong only in .dev/** (see docs/naming.md).");
    for f in hits.iter().take(10) {
        let first = &f.violations[0];
        eprintln!(
            "  {} — {} hit(s) (line {}: {})",
            f.path,
            f.violations.len(),
            first.line,
            first.matched
        );
    }
    if hits.len() > 10 {
        eprintln!("  … and {} more file(s).", hits.len() - 10);
    }
    eprintln!("Blocked — fix the violations above before committing (see docs/naming.md).");
    ExitCode::Error
}

/// Collect staged files and scan their **staged** content (`git show :path`),
/// so the check sees exactly what would be committed.
///
/// Fails closed: an inventory-spawn/nonzero-exit, invalid NUL-delimited path,
/// staged-blob-read failure, or invalid-UTF-8 blob aborts the whole scan with
/// `Err` instead of silently skipping the offending file.
fn scan_staged(
    gate: &crate::gal::naming_gate::NamingGate,
) -> Result<Vec<crate::gal::naming_gate::FileViolations>, String> {
    use crate::gal::naming_gate::FileViolations;
    use std::process::Command;

    let output = Command::new("git")
        .args([
            "diff",
            "--cached",
            "--diff-filter=ACMR",
            "--name-only",
            "-z",
        ])
        .output()
        .map_err(|e| format!("git diff --cached failed to spawn: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git diff --cached exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    let mut results = Vec::new();
    for rel_bytes in output.stdout.split(|&b| b == 0) {
        if rel_bytes.is_empty() {
            continue;
        }
        let rel = std::str::from_utf8(rel_bytes)
            .map_err(|e| format!("invalid UTF-8 staged path: {e}"))?;
        let show = Command::new("git")
            .arg("show")
            .arg(format!(":{rel}"))
            .output()
            .map_err(|e| format!("git show failed to spawn for {rel}: {e}"))?;
        if !show.status.success() {
            return Err(format!(
                "git show :{rel} exited with {}: {}",
                show.status,
                String::from_utf8_lossy(&show.stderr)
            ));
        }
        let content = String::from_utf8(show.stdout)
            .map_err(|e| format!("invalid UTF-8 staged blob for {rel}: {e}"))?;
        let violations = gate.scan_file(rel, &content);
        if !violations.is_empty() {
            results.push(FileViolations {
                path: rel.to_string(),
                violations,
            });
        }
    }
    Ok(results)
}

#[cfg(test)]
mod cmd_init_tests {
    use super::{adapter_outcome_rows, init_report_lines};
    use tempfile::TempDir;

    #[test]
    fn cmd_init_written_rows_cover_all_five_roots_on_fresh_bootstrap() {
        let temp = TempDir::new().unwrap();
        let opts = crate::init_repo::InitRepoOptions {
            target_path: temp.path().to_path_buf(),
            project_name: None,
            blank: true,
            force: false,
        };
        let report = crate::init_repo::run_init_repo(&opts).expect("fresh bootstrap must succeed");
        let lines = init_report_lines(&report);

        for root in crate::gal::render::REPO_ADAPTER_ROOTS {
            assert!(
                lines.iter().any(|l| l == &format!("- Written: {root}")),
                "expected a Written row for {root}, got {lines:?}"
            );
        }
    }

    #[test]
    fn cmd_init_unchanged_row_on_second_regen_with_no_content_change() {
        let temp = TempDir::new().unwrap();
        let opts = crate::init_repo::InitRepoOptions {
            target_path: temp.path().to_path_buf(),
            project_name: None,
            blank: true,
            force: false,
        };
        crate::init_repo::run_init_repo(&opts).expect("first bootstrap must succeed");

        // Second run against the same .dev/project.md content: every root
        // must resolve to Unchanged, not re-Written.
        let opts2 = crate::init_repo::InitRepoOptions {
            target_path: temp.path().to_path_buf(),
            project_name: None,
            blank: false,
            force: false,
        };
        let report2 = crate::init_repo::run_init_repo(&opts2).expect("regen must succeed");
        let lines = init_report_lines(&report2);
        for root in crate::gal::render::REPO_ADAPTER_ROOTS {
            assert!(
                lines.iter().any(|l| l == &format!("- Unchanged: {root}")),
                "expected an Unchanged row for {root} on the second identical run, got {lines:?}"
            );
        }
    }

    #[test]
    fn adapter_outcome_rows_reports_removed_conditional_layer() {
        // A real end-to-end Removed outcome needs a Rust→non-Rust selection
        // flip across two inits; the row-rendering logic itself is pure, so a
        // synthetic ProjectionReport fixture exercises it directly.
        let repo_root = std::path::Path::new("repo-root-placeholder");
        let mut report = crate::gal::render::ProjectionReport::default();
        report
            .removed_paths
            .push(repo_root.join(".claude/rules/gal-rust.md"));
        let rows = adapter_outcome_rows(&report, repo_root);
        assert!(
            rows.iter()
                .any(|r| r == "Removed: .claude/rules/gal-rust.md"),
            "expected a Removed row, got {rows:?}"
        );
        // The other conditional layer has no report entry at all -> omitted
        // entirely (an absent conditional layer is benign, not an outcome).
        assert!(
            !rows.iter().any(|r| r.contains("gal-rust.instructions.md")),
            "an absent conditional layer with no report entry must not appear, got {rows:?}"
        );
    }

    #[test]
    fn adapter_outcome_rows_handles_empty_report_fixture_without_panicking() {
        // Degenerate/edge fixture: a report with nothing in either list must
        // still produce exactly the five root rows (all Unchanged) and zero
        // conditional-layer rows, not panic or produce a malformed row.
        let repo_root = std::path::Path::new("repo-root-placeholder");
        let report = crate::gal::render::ProjectionReport::default();
        let rows = adapter_outcome_rows(&report, repo_root);
        assert_eq!(
            rows.len(),
            crate::gal::render::REPO_ADAPTER_ROOTS.len(),
            "an empty report must yield exactly one row per root and no layer rows, got {rows:?}"
        );
        assert!(rows.iter().all(|r| r.starts_with("Unchanged: ")));
    }

    #[test]
    fn cmd_init_report_omits_stale_five_adapters_wording() {
        let temp = TempDir::new().unwrap();
        let opts = crate::init_repo::InitRepoOptions {
            target_path: temp.path().to_path_buf(),
            project_name: None,
            blank: true,
            force: false,
        };
        let report = crate::init_repo::run_init_repo(&opts).expect("fresh bootstrap must succeed");
        let joined = init_report_lines(&report).join("\n");
        assert!(
            !joined.contains("5 adapters"),
            "the stale '5 adapters' wording must not appear, got: {joined:?}"
        );
    }
}

#[cfg(test)]
mod commit_msg_mode_tests {
    use super::{commit_msg_mode, CommitMsgMode};

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn context_flag_selects_context_mode() {
        assert_eq!(
            commit_msg_mode(&args(&["--context"])),
            CommitMsgMode::Context
        );
    }

    #[test]
    fn context_flag_with_stray_positional_still_context() {
        assert_eq!(
            commit_msg_mode(&args(&["--context", "some-stray-arg"])),
            CommitMsgMode::Context
        );
    }

    #[test]
    fn print_and_context_both_present_print_wins() {
        assert_eq!(
            commit_msg_mode(&args(&["--print", "--context"])),
            CommitMsgMode::Print
        );
        assert_eq!(
            commit_msg_mode(&args(&["--context", "--print"])),
            CommitMsgMode::Print
        );
    }

    #[test]
    fn print_flag_alone_selects_print_mode() {
        assert_eq!(commit_msg_mode(&args(&["--print"])), CommitMsgMode::Print);
        assert_eq!(
            commit_msg_mode(&args(&["--generate"])),
            CommitMsgMode::Print
        );
    }

    #[test]
    fn no_flags_selects_hook_mode() {
        assert_eq!(
            commit_msg_mode(&args(&[".git/COMMIT_EDITMSG"])),
            CommitMsgMode::Hook
        );
        assert_eq!(commit_msg_mode(&args(&[])), CommitMsgMode::Hook);
    }
}
