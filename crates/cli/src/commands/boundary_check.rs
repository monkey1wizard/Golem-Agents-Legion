//! `gal boundary-check` — pipeline-internal 2c pre-commit allowlist check.
//!
//! Resolves the task's affected-files allowlist via the shared
//! `pipeline::task_spec` parser (the same set the dispatch spec shows the
//! executor: task-block backtick paths, falling back to
//! `## Files to Create or Modify`) and compares it against `git diff
//! --name-only` plus untracked files from `git status --porcelain`.
//!
//! ## Critical contract (architect blocking requirement)
//!
//! When the task names no affected files at all → `CheckState::NotRun` (exit
//! non-zero). This is an INVARIANT of the binary's judgment body, NOT just a
//! Verify note. Degrading to "no allowlist → allow everything" would silently
//! bypass the entire boundary gate for any task that forgets to declare its
//! allowlist.
//!
//! Not a public `/gal` slash command (peer of `finalize-check`/`converge-check`).

use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use gal_engine::ExitCode;
use std::collections::HashSet;
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
        .join(format!("{task}-boundary-check.receipt.md"))
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
        prompt: prompt.ok_or("boundary-check requires a <prompt> path")?,
        task,
        receipt,
    })
}

/// Resolve the nominated task's affected-files allowlist as a set of bare path
/// tokens, reusing the shared `pipeline::task_spec` parser so this allowlist
/// equals the one the dispatch spec shows the executor — by construction. No
/// second, drift-prone parser lives here (the prior `Affected:` /
/// `## Affected Files`-only parser was the source of the template `not-run`
/// drift).
///
/// Returns `None` when neither the task block nor the
/// `## Files to Create or Modify` section names any path — mapped to `NotRun`,
/// never allow-all.
fn parse_affected_files(task: &str, prompt_text: &str) -> Option<HashSet<String>> {
    let paths = pipeline::task_spec::extract_affected_file_paths(prompt_text, task);
    if paths.is_empty() {
        None
    } else {
        Some(paths.into_iter().collect())
    }
}
/// Get changed files from `git diff --name-only` (staged + unstaged) and
/// untracked files from `git status --porcelain` in `repo_root`.
fn changed_files(repo_root: &Path) -> Vec<String> {
    let mut files = std::collections::BTreeSet::new();
    // staged + unstaged diff
    if let Ok(out) = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("diff")
        .arg("--name-only")
        .arg("HEAD")
        .output()
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                let p = line.trim();
                if !p.is_empty() {
                    files.insert(p.to_string());
                }
            }
        }
    }
    // also include any unstaged/untracked. `--untracked-files=all` is
    // required: without it, a brand-new untracked directory collapses to one
    // line (`?? .claude/`) instead of listing the files inside it, so a task
    // that creates its first file in a new directory would never match its
    // own backtick-quoted file path in the allowlist.
    if let Ok(out) = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("status")
        .arg("--porcelain")
        .arg("--untracked-files=all")
        .output()
    {
        if out.status.success() {
            for line in String::from_utf8_lossy(&out.stdout).lines() {
                // porcelain: 2-char XY status + space + path
                let rest = if line.len() > 3 {
                    line[3..].trim()
                } else {
                    continue;
                };
                // handle renames: "R old -> new" or "R old\0new"
                let path = if let Some(arrow) = rest.find(" -> ") {
                    rest[arrow + 4..].trim()
                } else {
                    rest
                };
                if !path.is_empty() {
                    files.insert(path.to_string());
                }
                // untracked: status starts with `??`
                // modified: status has M/A/D/R etc — all captured above
            }
        }
    }
    files.into_iter().collect()
}

/// check: compare actual changed files against the `## Affected Files` allowlist.
///
/// Allowlist missing/empty → `NotRun` (non-zero, never allow-all).
/// Any changed file outside the allowlist → `Fail`.
/// All within allowlist → `Pass`.
fn check_boundary(task: &str, prompt_text: &str, repo_root: &Path) -> CheckOutcome {
    let name = "boundary-allowlist".to_string();

    let allowlist = match parse_affected_files(task, prompt_text) {
        None => {
            return CheckOutcome {
                name,
                command: None,
                state: CheckState::NotRun,
                summary: format!(
                    "{task}: no affected files named (no task-block backtick paths, no `## Files to Create or Modify` entries) — refusing to allow-all"
                ),
            };
        }
        Some(set) => set,
    };

    let changed = changed_files(repo_root);
    if changed.is_empty() {
        return CheckOutcome {
            name,
            command: Some("git diff --name-only HEAD + git status --porcelain".to_string()),
            state: CheckState::Pass,
            summary: "no changed files — nothing to check".to_string(),
        };
    }

    let mut violations: Vec<String> = changed
        .iter()
        .filter(|f| !allowlist_contains(&allowlist, f))
        .cloned()
        .collect();
    violations.sort();

    if violations.is_empty() {
        CheckOutcome {
            name,
            command: Some("git diff --name-only HEAD + git status --porcelain".to_string()),
            state: CheckState::Pass,
            summary: format!(
                "all {} changed file(s) are within the declared allowlist",
                changed.len()
            ),
        }
    } else {
        CheckOutcome {
            name,
            command: Some("git diff --name-only HEAD + git status --porcelain".to_string()),
            state: CheckState::Fail,
            summary: format!("out-of-allowlist: {}", violations.join(", ")),
        }
    }
}

/// Strip a leading `./` so a root-level file cited as `./CLAUDE.md` (the form
/// `backtick_paths` requires, since it demands a `/` in the token) still
/// compares equal to the bare `git diff` path `CLAUDE.md`.
fn strip_leading_dot_slash(p: &str) -> &str {
    p.strip_prefix("./").unwrap_or(p)
}

/// Check whether a changed file path is covered by any allowlist entry.
///
/// An allowlist entry covers a path when the path equals the entry OR the path
/// starts with the entry as a directory prefix (allowing wildcard-like coverage
/// of entire subtrees declared as `crates/cli/src/`). Both sides are
/// normalized to strip a leading `./` first.
fn allowlist_contains(allowlist: &HashSet<String>, file: &str) -> bool {
    let file = strip_leading_dot_slash(file);
    if allowlist.iter().any(|e| strip_leading_dot_slash(e) == file) {
        return true;
    }
    let file_name = file.rsplit('/').next().unwrap_or(file);
    for entry in allowlist {
        let entry = strip_leading_dot_slash(entry);
        if !entry.contains('/') && !entry.contains('\\') && entry == file_name {
            return true;
        }
        let prefix = entry.trim_end_matches('/');
        if file.starts_with(&format!("{prefix}/")) {
            return true;
        }
    }
    false
}

fn render_receipt(receipt: &Receipt, task: &str) -> String {
    let mut out = format!("# boundary-check receipt\n\ntask: {task}\n\n");
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

/// Run `gal boundary-check`. Parse args, run allowlist check, write receipt, exit.
pub(crate) fn cmd_boundary_check(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal boundary-check: {e}");
            return ExitCode::Usage;
        }
    };

    if !parsed.prompt.exists() {
        eprintln!(
            "gal boundary-check: prompt not found: {}",
            parsed.prompt.display()
        );
        return ExitCode::Usage;
    }

    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let prompt_text = std::fs::read_to_string(&parsed.prompt).unwrap_or_default();

    let checks = vec![check_boundary(&parsed.task, &prompt_text, &repo_root)];
    let receipt = Receipt { checks };

    if let Some(parent) = parsed.receipt.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal boundary-check: cannot create receipt dir {}: {e}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }

    let content = render_receipt(&receipt, &parsed.task);
    if let Err(e) = std::fs::write(&parsed.receipt, &content) {
        eprintln!(
            "gal boundary-check: cannot write receipt {}: {e}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }

    println!(
        "gal boundary-check: {} ({} check(s)) → {}",
        if receipt.passed() { "pass" } else { "fail" },
        receipt.checks.len(),
        parsed.receipt.display()
    );
    receipt.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // A template-shape execution prompt: task-block backtick paths + a
    // `## Files to Create or Modify` section, NO `Affected:`, NO
    // `## Affected Files`. Task id composed at runtime (never a literal).
    fn template_prompt(task: &str) -> String {
        format!(
            "## Files to Create or Modify\n\n- `crates/cli/src/commands/boundary_check.rs`\n\n## Tasks\n\n- [ ] {task} — Do the work in `crates/cli/src/commands/boundary_check.rs`. Verify: green.\n"
        )
    }

    // critical contract: missing `## Affected Files` → NotRun (never allow-all)
    #[test]
    fn missing_affected_files_section_is_not_run() {
        let task = format!("T-{}", 99usize);
        let prompt = "## Tasks\n\n- [ ] do something\n";
        let outcome = check_boundary(&task, prompt, std::path::Path::new("."));
        assert_eq!(
            outcome.state,
            CheckState::NotRun,
            "must be NotRun, not Pass"
        );
    }

    // critical contract: empty `## Affected Files` → NotRun (never allow-all)
    #[test]
    fn empty_affected_files_section_is_not_run() {
        let task = format!("T-{}", 99usize);
        let prompt = "## Affected Files\n\n## Tasks\n\n";
        let outcome = check_boundary(&task, prompt, std::path::Path::new("."));
        assert_eq!(
            outcome.state,
            CheckState::NotRun,
            "empty section must be NotRun"
        );
    }

    // regression: a template-shape prompt (NO `Affected:`) resolves a non-empty
    // allowlist via the shared parser, and the task-named file is in-allowlist
    // (Pass path — the exact drift this fix closes).
    #[test]
    fn template_shape_prompt_resolves_nonempty_allowlist() {
        let task = format!("T-{}", 42usize);
        let set = parse_affected_files(&task, &template_prompt(&task))
            .expect("template-shape prompt must resolve a non-empty allowlist");
        assert!(allowlist_contains(
            &set,
            "crates/cli/src/commands/boundary_check.rs"
        ));
    }

    // regression: the same allowlist rejects an out-of-list file (Fail path).
    #[test]
    fn template_shape_prompt_rejects_out_of_allowlist_file() {
        let task = format!("T-{}", 42usize);
        let set =
            parse_affected_files(&task, &template_prompt(&task)).expect("non-empty allowlist");
        assert!(!allowlist_contains(&set, "docs/manual.md"));
    }

    // invariant: a task that names no path anywhere (no backtick paths, no
    // `## Files to Create or Modify`) → NotRun, never allow-all.
    #[test]
    fn template_task_with_no_paths_is_not_run() {
        let task = format!("T-{}", 42usize);
        let prompt = format!("## Tasks\n\n- [ ] {task} — A task naming no paths.\n");
        let outcome = check_boundary(&task, &prompt, std::path::Path::new("."));
        assert_eq!(
            outcome.state,
            CheckState::NotRun,
            "empty allowlist must be NotRun"
        );
    }

    // regression: a brand-new untracked directory must not collapse into one
    // `?? dir/` line — every file inside it must be individually comparable
    // against the allowlist. Without `--untracked-files=all`, `git status
    // --porcelain` reports only the directory, so a task whose allowlist
    // names `newdir/file.md` would false-fail even though that exact file is
    // the only thing that changed.
    #[test]
    fn changed_files_lists_individual_paths_inside_a_new_untracked_directory() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        let run_git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .arg("-C")
                .arg(repo_root)
                .args(args)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        };
        run_git(&["init", "-q"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "test"]);
        std::fs::write(repo_root.join("README.md"), "hi\n").unwrap();
        run_git(&["add", "README.md"]);
        run_git(&["commit", "-q", "-m", "init"]);

        std::fs::create_dir_all(repo_root.join("newdir")).unwrap();
        std::fs::write(repo_root.join("newdir").join("file.md"), "content\n").unwrap();

        let files = changed_files(repo_root);
        assert!(
            files.contains(&"newdir/file.md".to_string()),
            "expected the individual file path, got {files:?}"
        );
        assert!(
            !files.iter().any(|f| f == "newdir/"),
            "must not collapse into the bare directory entry, got {files:?}"
        );
    }

    // allowlist_contains: prefix match covers subtree
    #[test]
    fn allowlist_prefix_covers_subtree() {
        let mut set = HashSet::new();
        set.insert("crates/cli/src/".to_string());
        assert!(allowlist_contains(&set, "crates/cli/src/main.rs"));
        assert!(!allowlist_contains(&set, "docs/manual.md"));
    }

    // regression: a root-level file cited with the leading `./` that
    // backtick_paths requires (it demands a `/` in the token) must still match
    // the bare `git diff` path with no leading `./`.
    #[test]
    fn allowlist_root_level_dot_slash_entry_matches_bare_diff_path() {
        let mut set = HashSet::new();
        set.insert("./CLAUDE.md".to_string());
        assert!(allowlist_contains(&set, "CLAUDE.md"));
        assert!(!allowlist_contains(&set, "AGENTS.md"));
    }

    // compact execution prompts often name a single touched file by basename.
    #[test]
    fn allowlist_basename_covers_matching_file_name() {
        let mut set = HashSet::new();
        set.insert("boundary_check.rs".to_string());
        assert!(allowlist_contains(
            &set,
            "crates/cli/src/commands/boundary_check.rs"
        ));
        assert!(!allowlist_contains(
            &set,
            "crates/cli/src/commands/converge_check.rs"
        ));
    }

    // cmd entry: Usage when prompt missing
    #[test]
    fn cmd_entry_returns_usage_for_missing_prompt() {
        let task = format!("T-{}", 99usize);
        let result = cmd_boundary_check(&[
            "boundary-check".to_string(),
            "nonexistent.prompt.md".to_string(),
            "--task".to_string(),
            task,
        ]);
        assert_eq!(result, ExitCode::Usage);
    }
}
