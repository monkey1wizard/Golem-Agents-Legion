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

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use gal_engine::ExitCode;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Boundary-kind selects which allowlist a check enforces. The default
/// (`None`) is the task's own affected-files allowlist (`check_boundary`).
/// `StateRecording` replaces that allowlist entirely with the three
/// state-tracking surfaces the 2g convergence step owns — see
/// `check_state_recording_boundary`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoundaryKind {
    StateRecording,
}

impl BoundaryKind {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "state-recording" => Ok(BoundaryKind::StateRecording),
            other => Err(format!(
                "unknown --boundary-kind '{other}' (expected: state-recording)"
            )),
        }
    }
}

struct Args {
    prompt: PathBuf,
    task: String,
    receipt: PathBuf,
    range: Option<String>,
    boundary_kind: Option<BoundaryKind>,
}

fn default_receipt_path(prompt: &Path, task: &str) -> Result<PathBuf, String> {
    resolve_receipt_path(
        Some(prompt),
        Some(task),
        format!("{task}-boundary-check.receipt.md"),
    )
    .map_err(|error| format!("receipt scope resolution: {error}"))
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt: Option<PathBuf> = None;
    let mut task: Option<String> = None;
    let mut receipt: Option<PathBuf> = None;
    let mut range: Option<String> = None;
    let mut boundary_kind: Option<BoundaryKind> = None;
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
            "--range" => {
                let v = it.next().ok_or("--range requires a base..head range")?;
                if range.is_some() {
                    return Err("--range/--base may be specified only once".to_string());
                }
                range = Some(v.clone());
            }
            "--base" => {
                let v = it.next().ok_or("--base requires a commit")?;
                if range.is_some() {
                    return Err("--range/--base may be specified only once".to_string());
                }
                range = Some(format!("{v}..HEAD"));
            }
            "--boundary-kind" => {
                let v = it.next().ok_or("--boundary-kind requires a value")?;
                if boundary_kind.is_some() {
                    return Err("--boundary-kind may be specified only once".to_string());
                }
                boundary_kind = Some(BoundaryKind::parse(v)?);
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
    let prompt = prompt.ok_or("boundary-check requires a <prompt> path")?;
    let receipt = match receipt {
        Some(path) => path,
        None => default_receipt_path(&prompt, &task)?,
    };
    Ok(Args {
        prompt,
        task,
        receipt,
        range,
        boundary_kind,
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
fn changed_files(repo_root: &Path, range: Option<&str>) -> Result<Vec<String>, String> {
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
    if let Some(range) = range {
        // A range the caller asked for must resolve: silently skipping a failed
        // `git diff` would drop every committed path and fail open.
        let out = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .arg("diff")
            .arg("--name-only")
            .arg(range)
            .output()
            .map_err(|e| format!("git diff --name-only {range} could not run: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "git diff --name-only {range} failed ({}): {}",
                out.status,
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let p = line.trim();
            if !p.is_empty() {
                files.insert(p.to_string());
            }
        }
    }
    Ok(files.into_iter().collect())
}

/// check: compare actual changed files against the `## Affected Files` allowlist.
///
/// Allowlist missing/empty → `NotRun` (non-zero, never allow-all).
/// Any changed file outside the allowlist → `Fail`.
/// All within allowlist → `Pass`.
fn check_boundary(
    task: &str,
    prompt_text: &str,
    repo_root: &Path,
    prompt_path: &Path,
    range: Option<&str>,
) -> CheckOutcome {
    let name = "boundary-allowlist".to_string();
    let slug = derive_plan_slug(prompt_path);

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

    let command = || match range {
        Some(range) => format!(
            "git diff --name-only HEAD + git status --porcelain + git diff --name-only {range}"
        ),
        None => "git diff --name-only HEAD + git status --porcelain".to_string(),
    };
    let changed = match changed_files(repo_root, range) {
        Ok(changed) => changed,
        Err(message) => {
            return CheckOutcome {
                name,
                command: Some(command()),
                state: CheckState::Fail,
                summary: message,
            };
        }
    };
    if changed.is_empty() {
        return CheckOutcome {
            name,
            command: Some(command()),
            state: CheckState::Pass,
            summary: "no changed files — nothing to check".to_string(),
        };
    }

    let mut violations: Vec<String> = changed
        .iter()
        .filter(|f| !is_workflow_state_path(&slug, f) && !allowlist_contains(&allowlist, f))
        .cloned()
        .collect();
    violations.sort();

    if violations.is_empty() {
        CheckOutcome {
            name,
            command: Some(command()),
            state: CheckState::Pass,
            summary: format!(
                "all {} changed file(s) are within the declared allowlist",
                changed.len()
            ),
        }
    } else {
        CheckOutcome {
            name,
            command: Some(command()),
            state: CheckState::Fail,
            summary: format!("out-of-allowlist: {}", violations.join(", ")),
        }
    }
}

/// Normalize a path for comparison against `git diff`/`git status` output:
/// strip a leading `./` and convert Windows separators to `/`.
fn normalize_relpath(p: &Path) -> String {
    strip_leading_dot_slash(&p.display().to_string())
        .replace('\\', "/")
        .to_string()
}

/// `--boundary-kind state-recording` check: the running plan's convergence
/// commit (2g) is only ever allowed to touch the three state-tracking
/// surfaces it owns — this call's execution prompt, the paired source plan
/// `.dev/plans/<slug>.md`, and `.dev/state.md`. Unlike `check_boundary`, this
/// allowlist is fixed by the boundary kind itself, never by the task's own
/// `## Files to Create or Modify` / backtick-path declarations — a
/// state-recording commit has no task-declared allowlist of its own.
fn check_state_recording_boundary(
    repo_root: &Path,
    prompt_path: &Path,
    range: Option<&str>,
) -> CheckOutcome {
    let name = "boundary-state-recording".to_string();
    let slug = derive_plan_slug(prompt_path);
    let allowed: HashSet<String> = [
        normalize_relpath(prompt_path),
        format!(".dev/plans/{slug}.md"),
        ".dev/state.md".to_string(),
    ]
    .into_iter()
    .collect();

    let command = || match range {
        Some(range) => format!(
            "git diff --name-only HEAD + git status --porcelain + git diff --name-only {range}"
        ),
        None => "git diff --name-only HEAD + git status --porcelain".to_string(),
    };
    let changed = match changed_files(repo_root, range) {
        Ok(changed) => changed,
        Err(message) => {
            return CheckOutcome {
                name,
                command: Some(command()),
                state: CheckState::Fail,
                summary: message,
            };
        }
    };
    if changed.is_empty() {
        return CheckOutcome {
            name,
            command: Some(command()),
            state: CheckState::Pass,
            summary: "no changed files — nothing to check".to_string(),
        };
    }

    let mut violations: Vec<String> = changed
        .iter()
        .filter(|f| !allowed.contains(&strip_leading_dot_slash(f).replace('\\', "/")))
        .cloned()
        .collect();
    violations.sort();

    if violations.is_empty() {
        CheckOutcome {
            name,
            command: Some(command()),
            state: CheckState::Pass,
            summary: format!(
                "all {} changed file(s) are state-recording surfaces (prompt, .dev/plans/{slug}.md, .dev/state.md)",
                changed.len()
            ),
        }
    } else {
        CheckOutcome {
            name,
            command: Some(command()),
            state: CheckState::Fail,
            summary: format!(
                "out-of-allowlist (state-recording): {}",
                violations.join(", ")
            ),
        }
    }
}

/// Derive the running plan's slug from its execution prompt path.
///
/// `<slug>.prompt.md` yields `<slug>`. The `.prompt.md` suffix is stripped
/// before a bare `.md` suffix is ever considered, so `<slug>.prompt.md` never
/// degrades to `<slug>.prompt` (which a naive `.md`-only strip would produce).
/// A path with neither suffix returns its bare file name unchanged.
fn derive_plan_slug(prompt_path: &Path) -> String {
    let name = prompt_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("");
    if let Some(stripped) = name.strip_suffix(".prompt.md") {
        stripped.to_string()
    } else if let Some(stripped) = name.strip_suffix(".md") {
        stripped.to_string()
    } else {
        name.to_string()
    }
}

/// Workflow-state surfaces the task loop writes on every task, which are
/// therefore never evidence of an executor straying outside its files.
///
/// Two authorities force these to be dirty at gate time. The implementer
/// contract requires `Task Base Commit` to be written into the execution
/// prompt *before* implementing, and the pipeline's commit boundary forbids
/// the dispatched executor from committing. Without this exemption the gate
/// can never pass in dispatched mode, because the prompt it is checking is
/// guaranteed to be modified.
///
/// The exemption is scoped to the running plan, not to `.dev/plans/` at
/// large. It covers `.dev/state.md` plus exactly the two files that back
/// *this* plan's slug — `.dev/plans/<slug>.md` and
/// `.dev/plans/<slug>.prompt.md`. A sibling plan's files under `.dev/plans/`
/// stay in scope: the task loop never writes another plan's surfaces, so a
/// change there is a real stray edit, not workflow-state churn.
/// `.dev/project.md` and `.dev/research/` stay in scope for the same reason.
/// `.dev/pipeline/` never appears here at all —
/// they are gitignored, so `git status` never reports them.
fn is_workflow_state_path(slug: &str, file: &str) -> bool {
    let f = strip_leading_dot_slash(file).replace('\\', "/");
    f == ".dev/state.md"
        || f == format!(".dev/plans/{slug}.md")
        || f == format!(".dev/plans/{slug}.prompt.md")
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
    let file_name = file.rsplit('/').next().unwrap_or(file);
    for entry in allowlist {
        for expanded in expand_brace_alternatives(entry) {
            let expanded = strip_leading_dot_slash(&expanded);
            if expanded == file {
                return true;
            }
            if !expanded.contains('/') && !expanded.contains('\\') && expanded == file_name {
                return true;
            }
            let prefix = expanded.trim_end_matches('/');
            if file.starts_with(&format!("{prefix}/")) {
                return true;
            }
        }
    }
    false
}

/// Expand one `{a,b,c}` brace-alternation group in `s` into its literal
/// variants. Defense in depth alongside `task_spec::backtick_paths`'s own
/// expansion: an allowlist entry that reaches this comparison still carrying
/// raw braces (a future caller that builds the set without going through
/// `backtick_paths`) is expanded here too, rather than silently rejecting
/// every real file it was meant to cover. Multiple non-nested groups expand
/// via cross product through recursion. A malformed group (unbalanced,
/// empty, or nested braces) is left as one literal candidate, unchanged.
fn expand_brace_alternatives(s: &str) -> Vec<String> {
    let Some(open) = s.find('{') else {
        return vec![s.to_string()];
    };
    let after = &s[open + 1..];
    match after.find(['{', '}']) {
        Some(rel) if after.as_bytes()[rel] == b'}' => {
            let close = open + 1 + rel;
            let inner = &s[open + 1..close];
            if inner.is_empty() {
                return vec![s.to_string()];
            }
            let prefix = &s[..open];
            let suffix = &s[close + 1..];
            inner
                .split(',')
                .flat_map(|alt| expand_brace_alternatives(&format!("{prefix}{alt}{suffix}")))
                .collect()
        }
        _ => vec![s.to_string()],
    }
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
            return if e.starts_with("receipt scope resolution:") {
                ExitCode::Error
            } else {
                ExitCode::Usage
            };
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

    let checks = vec![match parsed.boundary_kind {
        Some(BoundaryKind::StateRecording) => {
            check_state_recording_boundary(&repo_root, &parsed.prompt, parsed.range.as_deref())
        }
        None => check_boundary(
            &parsed.task,
            &prompt_text,
            &repo_root,
            &parsed.prompt,
            parsed.range.as_deref(),
        ),
    }];
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

    #[test]
    fn default_receipt_path_is_plan_scoped_and_explicit_override_wins() {
        let prompt = Path::new(".dev/plans/boundary-scope.prompt.md");
        let task = format!("T-{:02}", 3usize);
        assert_eq!(
            default_receipt_path(prompt, &task).unwrap(),
            default_receipt_path(prompt, &task).unwrap(),
        );
        let parsed = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task.clone(),
            "--receipt".to_string(),
            "custom/receipt.md".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.receipt, PathBuf::from("custom/receipt.md"));
        assert_eq!(parsed.range, None);

        let ranged = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task.clone(),
            "--range".to_string(),
            "abc..def".to_string(),
        ])
        .unwrap();
        assert_eq!(ranged.range.as_deref(), Some("abc..def"));

        let based = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task.clone(),
            "--base".to_string(),
            "abc".to_string(),
        ])
        .unwrap();
        assert_eq!(based.range.as_deref(), Some("abc..HEAD"));
    }
    use tempfile::TempDir;

    // A template-shape execution prompt: task-block backtick paths + a
    // `## Files to Create or Modify` section, NO `Affected:`, NO
    // `## Affected Files`. Task id composed at runtime (never a literal).
    fn template_prompt(task: &str) -> String {
        format!(
            "## Files to Create or Modify\n\n- `crates/cli/src/commands/boundary_check.rs`\n\n## Tasks\n\n- [ ] {task} — Do the work in `crates/cli/src/commands/boundary_check.rs`. Verify: green.\n"
        )
    }

    // The workflow-state exemption exists so the gate can pass in dispatched
    // mode at all. It must cover exactly the surfaces the convergence step
    // owns for the running plan, and nothing else.
    #[test]
    fn workflow_state_surfaces_are_exempt() {
        let slug = "some-plan";
        assert!(is_workflow_state_path(slug, ".dev/state.md"));
        assert!(is_workflow_state_path(slug, ".dev/plans/some-plan.md"));
        assert!(is_workflow_state_path(
            slug,
            ".dev/plans/some-plan.prompt.md"
        ));
        assert!(
            is_workflow_state_path(slug, "./.dev/state.md"),
            "leading ./"
        );
        assert!(
            is_workflow_state_path(slug, ".dev\\plans\\some-plan.prompt.md"),
            "windows separators"
        );
    }

    // Regression guard: the exemption must not become a hole. Anything the
    // task loop does not write stays checkable, including the rest of `.dev/`
    // and a sibling plan's files under `.dev/plans/`.
    #[test]
    fn exemption_does_not_cover_unrelated_paths() {
        let slug = "some-plan";
        assert!(!is_workflow_state_path(slug, ".dev/project.md"));
        assert!(!is_workflow_state_path(slug, ".dev/research/note.md"));
        assert!(!is_workflow_state_path(slug, "crates/cli/src/main.rs"));
        assert!(!is_workflow_state_path(slug, "docs/workflows.md"));
        assert!(
            !is_workflow_state_path(slug, "plugins/gal-core/agents/golem-releaser.agent.md"),
            "a Protected Path must never be silently exempt"
        );
        assert!(
            !is_workflow_state_path(slug, "evil/.dev/plans/x.md"),
            "must anchor at the repo root, not match a nested lookalike"
        );
        assert!(
            !is_workflow_state_path(slug, ".dev/plans/other-plan.md"),
            "a sibling plan's files must not be exempt"
        );
    }

    // derive_plan_slug: `.prompt.md` must be stripped before a bare `.md`
    // check ever runs, or `<slug>.prompt.md` would degrade to `<slug>.prompt`.
    #[test]
    fn derive_plan_slug_strips_prompt_md_before_md() {
        assert_eq!(
            derive_plan_slug(Path::new(".dev/plans/some-plan.prompt.md")),
            "some-plan"
        );
        assert_eq!(
            derive_plan_slug(Path::new(".dev/plans/some-plan.md")),
            "some-plan"
        );
    }

    // critical contract: missing `## Affected Files` → NotRun (never allow-all)
    #[test]
    fn missing_affected_files_section_is_not_run() {
        let task = format!("T-{}", 99usize);
        let prompt = "## Tasks\n\n- [ ] do something\n";
        let outcome = check_boundary(
            &task,
            prompt,
            std::path::Path::new("."),
            std::path::Path::new("plan.prompt.md"),
            None,
        );
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
        let outcome = check_boundary(
            &task,
            prompt,
            std::path::Path::new("."),
            std::path::Path::new("plan.prompt.md"),
            None,
        );
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
        assert!(!allowlist_contains(&set, "docs/workflows.md"));
    }

    // invariant: a task that names no path anywhere (no backtick paths, no
    // `## Files to Create or Modify`) → NotRun, never allow-all.
    #[test]
    fn template_task_with_no_paths_is_not_run() {
        let task = format!("T-{}", 42usize);
        let prompt = format!("## Tasks\n\n- [ ] {task} — A task naming no paths.\n");
        let outcome = check_boundary(
            &task,
            &prompt,
            std::path::Path::new("."),
            std::path::Path::new("plan.prompt.md"),
            None,
        );
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
        run_git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(repo_root.join("README.md"), "hi\n").unwrap();
        run_git(&["add", "README.md"]);
        run_git(&["commit", "-q", "-m", "init"]);

        std::fs::create_dir_all(repo_root.join("newdir")).unwrap();
        std::fs::write(repo_root.join("newdir").join("file.md"), "content\n").unwrap();

        let files = changed_files(repo_root, None).unwrap();
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
        assert!(!allowlist_contains(&set, "docs/workflows.md"));
    }

    // regression: an entry carrying a `{a,b}` brace group must expand and
    // match each variant, not the raw literal path with braces in it.
    #[test]
    fn allowlist_contains_expands_brace_group_entry() {
        let mut set = HashSet::new();
        set.insert("crates/cli/src/commands/{boundary_check,converge_check}.rs".to_string());
        assert!(allowlist_contains(
            &set,
            "crates/cli/src/commands/boundary_check.rs"
        ));
        assert!(allowlist_contains(
            &set,
            "crates/cli/src/commands/converge_check.rs"
        ));
        assert!(!allowlist_contains(
            &set,
            "crates/cli/src/commands/finalize_check.rs"
        ));
    }

    // a malformed brace group (unbalanced) must not panic and must not match
    // anything beyond its own literal string.
    #[test]
    fn allowlist_contains_unbalanced_brace_entry_matches_nothing_useful() {
        let mut set = HashSet::new();
        set.insert("crates/cli/src/commands/{boundary_check.rs".to_string());
        assert!(!allowlist_contains(
            &set,
            "crates/cli/src/commands/boundary_check.rs"
        ));
    }

    // brace-free entries keep their exact prior behavior.
    #[test]
    fn allowlist_contains_brace_free_entry_still_matches_exactly() {
        let mut set = HashSet::new();
        set.insert("crates/cli/src/commands/dispatch.rs".to_string());
        assert!(allowlist_contains(
            &set,
            "crates/cli/src/commands/dispatch.rs"
        ));
        assert!(!allowlist_contains(
            &set,
            "crates/cli/src/commands/boundary_check.rs"
        ));
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

    fn init_temp_git_repo(repo_root: &Path) {
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
        run_git(&["config", "commit.gpgsign", "false"]);
        std::fs::write(repo_root.join("README.md"), "hi\n").unwrap();
        run_git(&["add", "README.md"]);
        run_git(&["commit", "-q", "-m", "init"]);
    }

    // Regression (2026-08-06): a changed path under `.dev/plans/` that belongs
    // to a plan other than the one being checked must stay a violation. The
    // workflow-state exemption is scoped to the running plan's own slug, not
    // to `.dev/plans/` at large.
    #[test]
    fn changed_path_under_other_plan_is_violation() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 3usize);
        let other_slug = format!("plan-{}", 4usize);
        let task = format!("T-{}", 3usize);

        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root
                .join(".dev/plans")
                .join(format!("{other_slug}.md")),
            "other plan\n",
        )
        .unwrap();

        let prompt_text = template_prompt(&task);
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_boundary(&task, &prompt_text, repo_root, &prompt_path, None);
        assert_eq!(outcome.state, CheckState::Fail, "{}", outcome.summary);
        assert!(
            outcome
                .summary
                .contains(&format!(".dev/plans/{other_slug}.md")),
            "expected sibling plan path in violation summary, got {}",
            outcome.summary
        );
    }

    #[test]
    fn range_fails_when_git_diff_rejects_range() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let task = format!("T-{}", 21usize);
        let prompt_path = PathBuf::from("plan.prompt.md");
        let outcome = check_boundary(
            &task,
            &template_prompt(&task),
            repo_root,
            &prompt_path,
            Some("missing..HEAD"),
        );
        assert_eq!(outcome.state, CheckState::Fail, "{}", outcome.summary);
        assert!(
            outcome.summary.contains("missing..HEAD"),
            "expected the rejected range in the summary, got {}",
            outcome.summary
        );
    }

    #[test]
    fn range_rejects_committed_path_outside_allowlist() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let base = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(base.status.success());
        let base = String::from_utf8(base.stdout).unwrap().trim().to_string();

        std::fs::create_dir_all(repo_root.join("docs")).unwrap();
        std::fs::write(repo_root.join("docs/workflows.md"), "manual\n").unwrap();
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["add", "docs/workflows.md"])
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["commit", "-q", "-m", "outside allowlist"])
            .status()
            .unwrap();
        assert!(status.success());

        let task = format!("T-{}", 21usize);
        let prompt_path = PathBuf::from("plan.prompt.md");
        let outcome = check_boundary(
            &task,
            &template_prompt(&task),
            repo_root,
            &prompt_path,
            Some(&format!("{base}..HEAD")),
        );
        assert_eq!(outcome.state, CheckState::Fail, "{}", outcome.summary);
        assert!(outcome.summary.contains("docs/workflows.md"));
    }

    #[test]
    fn range_passes_when_committed_paths_are_within_allowlist() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let base = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap();
        assert!(base.status.success());
        let base = String::from_utf8(base.stdout).unwrap().trim().to_string();

        let allowed_path = repo_root.join("crates/cli/src/commands");
        std::fs::create_dir_all(&allowed_path).unwrap();
        std::fs::write(allowed_path.join("boundary_check.rs"), "change\n").unwrap();
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["add", "crates/cli/src/commands/boundary_check.rs"])
            .status()
            .unwrap();
        assert!(status.success());
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["commit", "-q", "-m", "inside allowlist"])
            .status()
            .unwrap();
        assert!(status.success());

        let task = format!("T-{}", 21usize);
        let prompt_path = PathBuf::from("plan.prompt.md");
        let outcome = check_boundary(
            &task,
            &template_prompt(&task),
            repo_root,
            &prompt_path,
            Some(&format!("{base}..HEAD")),
        );
        assert_eq!(outcome.state, CheckState::Pass, "{}", outcome.summary);
    }

    // The exemption boundary, exercised end to end through `check_boundary`:
    // the active plan's `.md`, the active plan's `.prompt.md`, and
    // `.dev/state.md` must all be exempt even though none is in the
    // allowlist.
    #[test]
    fn active_plan_files_and_state_are_exempt() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 7usize);
        let task = format!("T-{}", 7usize);

        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root.join(".dev/plans").join(format!("{slug}.md")),
            "plan\n",
        )
        .unwrap();
        std::fs::write(
            repo_root
                .join(".dev/plans")
                .join(format!("{slug}.prompt.md")),
            "prompt\n",
        )
        .unwrap();
        std::fs::write(repo_root.join(".dev/state.md"), "state\n").unwrap();

        let prompt_text = template_prompt(&task);
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_boundary(&task, &prompt_text, repo_root, &prompt_path, None);
        assert_eq!(outcome.state, CheckState::Pass, "{}", outcome.summary);
    }

    // The other direction of the same boundary: a lookalike `.en.md`
    // translation of the active plan is NOT one of the exempted surfaces, so
    // it must still be checked against the allowlist.
    #[test]
    fn active_plan_en_md_is_not_exempt() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 8usize);
        let task = format!("T-{}", 8usize);

        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root.join(".dev/plans").join(format!("{slug}.en.md")),
            "translation\n",
        )
        .unwrap();

        let prompt_text = template_prompt(&task);
        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_boundary(&task, &prompt_text, repo_root, &prompt_path, None);
        assert_eq!(outcome.state, CheckState::Fail, "{}", outcome.summary);
        assert!(
            outcome.summary.contains(&format!("{slug}.en.md")),
            "expected the .en.md translation in violation summary, got {}",
            outcome.summary
        );
    }

    // derive_plan_slug: pin both directions — `<slug>.prompt.md` derives to
    // exactly `<slug>`, and never degrades to `<slug>.prompt`.
    #[test]
    fn derive_plan_slug_prompt_md_equals_slug_not_slug_prompt() {
        let slug = format!("plan-{}", 11usize);
        let path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let derived = derive_plan_slug(&path);
        assert_eq!(derived, slug);
        assert_ne!(derived, format!("{slug}.prompt"));
    }

    // --boundary-kind: valid value parses, unknown value fails closed, and
    // it composes with --task/--range like the other options.
    #[test]
    fn boundary_kind_state_recording_parses_and_unknown_value_fails_closed() {
        let prompt = Path::new(".dev/plans/boundary-scope.prompt.md");
        let task = format!("T-{:02}", 6usize);
        let parsed = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task.clone(),
            "--boundary-kind".to_string(),
            "state-recording".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.boundary_kind, Some(BoundaryKind::StateRecording));

        let err = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task.clone(),
            "--boundary-kind".to_string(),
            "nonsense".to_string(),
        ])
        .map(|_| ())
        .expect_err("unknown boundary-kind value must be rejected");
        assert!(
            err.contains("unknown --boundary-kind"),
            "expected fail-closed rejection, got: {err}"
        );

        let dup = parse_args(&[
            prompt.display().to_string(),
            "--task".to_string(),
            task,
            "--boundary-kind".to_string(),
            "state-recording".to_string(),
            "--boundary-kind".to_string(),
            "state-recording".to_string(),
        ])
        .map(|_| ())
        .expect_err("repeated --boundary-kind must be rejected");
        assert!(dup.contains("only once"));
    }

    // state-recording: changing exactly the prompt, its paired source plan,
    // and .dev/state.md must Pass even though none of them is in any
    // task-declared allowlist — the state-recording kind ignores the task's
    // own allowlist entirely.
    #[test]
    fn state_recording_passes_for_exactly_the_three_state_surfaces() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 20usize);
        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root.join(".dev/plans").join(format!("{slug}.md")),
            "plan\n",
        )
        .unwrap();
        std::fs::write(
            repo_root
                .join(".dev/plans")
                .join(format!("{slug}.prompt.md")),
            "prompt\n",
        )
        .unwrap();
        std::fs::write(repo_root.join(".dev/state.md"), "state\n").unwrap();

        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_state_recording_boundary(repo_root, &prompt_path, None);
        assert_eq!(outcome.state, CheckState::Pass, "{}", outcome.summary);
    }

    // state-recording: a changed file outside the three state surfaces must
    // Fail even though it is a normal implementation file that would be
    // in-scope for a task's own allowlist — state-recording never widens.
    #[test]
    fn state_recording_fails_when_other_files_are_mixed_in() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path();
        init_temp_git_repo(repo_root);

        let slug = format!("plan-{}", 21usize);
        std::fs::create_dir_all(repo_root.join(".dev/plans")).unwrap();
        std::fs::write(
            repo_root.join(".dev/plans").join(format!("{slug}.md")),
            "plan\n",
        )
        .unwrap();
        std::fs::write(
            repo_root
                .join(".dev/plans")
                .join(format!("{slug}.prompt.md")),
            "prompt\n",
        )
        .unwrap();
        std::fs::write(repo_root.join(".dev/state.md"), "state\n").unwrap();
        std::fs::create_dir_all(repo_root.join("docs")).unwrap();
        std::fs::write(repo_root.join("docs/workflows.md"), "manual\n").unwrap();

        let prompt_path = PathBuf::from(format!(".dev/plans/{slug}.prompt.md"));
        let outcome = check_state_recording_boundary(repo_root, &prompt_path, None);
        assert_eq!(outcome.state, CheckState::Fail, "{}", outcome.summary);
        assert!(
            outcome.summary.contains("docs/workflows.md"),
            "expected the out-of-scope file in the violation summary, got {}",
            outcome.summary
        );
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
