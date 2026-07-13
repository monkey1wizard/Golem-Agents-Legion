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
            println!(
                "Initialized repo context in: {}",
                report.target_path.display()
            );
            let adapter_verb = if report.regenerated_only {
                "Preserved (already initialized)"
            } else {
                "Created"
            };
            println!("- {adapter_verb}: .dev/project.md");
            println!("- {adapter_verb}: .dev/state.md");
            println!("- Ensured: .dev/plans/");
            let render_verb = if report.regenerated_only {
                "Regenerated"
            } else {
                "Generated"
            };
            println!("- {render_verb}: .github/copilot-instructions.md");
            println!("- {render_verb}: GEMINI.md");
            println!("- {render_verb}: CLAUDE.md");
            println!("- {render_verb}: AGENTS.md");
            if report.regenerated_only {
                println!("- Next: re-run any time to refresh the 5 adapters from the current .dev/project.md");
            } else {
                println!(
                    "- Next: review .dev/project.md, fill in summary fields, then run `gal doctor`"
                );
            }
            if !report.source_docs.is_empty() {
                println!();
                println!(
                    "Adopt-existing: found {} source document(s).",
                    report.source_docs.len()
                );
                println!("Review .dev/project.md and fill in summaries from discovered docs.");
            }
            if !report.tech_hints.is_empty() {
                println!("Detected tech stack: {}", report.tech_hints.join(", "));
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
        &["README.md", "docs/manual.md"],
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
        let context = assemble_commit_context(&entries, resolve_staged_content, resolve_hunk_headers);
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

    let hits: Vec<FileViolations> = if staged {
        scan_staged(&gate)
    } else {
        gate.scan_tree(&root)
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
    eprintln!(
        "  plan-task IDs and retired terms belong only in .dev/** (see docs/naming.md)."
    );
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
fn scan_staged(
    gate: &crate::gal::naming_gate::NamingGate,
) -> Vec<crate::gal::naming_gate::FileViolations> {
    use crate::gal::naming_gate::FileViolations;
    use std::process::Command;

    let listing = match Command::new("git")
        .args(["diff", "--cached", "--diff-filter=ACMR", "--name-only"])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };
    let listing = String::from_utf8_lossy(&listing);
    let mut results = Vec::new();
    for rel in listing.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let show = Command::new("git")
            .arg("show")
            .arg(format!(":{rel}"))
            .output();
        let content = match show {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            _ => continue,
        };
        let violations = gate.scan_file(rel, &content);
        if !violations.is_empty() {
            results.push(FileViolations {
                path: rel.to_string(),
                violations,
            });
        }
    }
    results
}

#[cfg(test)]
mod commit_msg_mode_tests {
    use super::{commit_msg_mode, CommitMsgMode};

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn context_flag_selects_context_mode() {
        assert_eq!(commit_msg_mode(&args(&["--context"])), CommitMsgMode::Context);
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
        assert_eq!(commit_msg_mode(&args(&["--generate"])), CommitMsgMode::Print);
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
