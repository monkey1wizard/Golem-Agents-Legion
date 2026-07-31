//! `gal restore` — revert GAL source repo to last `gal-last-good` marker.
//!
//! Internal binary subcommand; NOT on the public `/gal` surface.
//! Peer of finalize-check / refresh / release.

use std::path::{Path, PathBuf};
use std::process::Command;

use gal_engine::render::resolve_source_from_cwd;
use gal_engine::ExitCode;

use crate::commands::refresh::refresh_with;

// ── ENV_GUARD for tests that mutate HOME/USERPROFILE ─────────────────────────
// Shared process-global guard (crate::commands::ENV_GUARD): refresh and restore
// MUST share one mutex or their env/cwd-mutating tests race on global HOME.
#[cfg(test)]
use crate::commands::ENV_GUARD;

#[cfg(test)]
fn set_home(tmp: &Path) {
    #[cfg(windows)]
    std::env::set_var("USERPROFILE", tmp);
    #[cfg(not(windows))]
    std::env::set_var("HOME", tmp);
}

// ── Public API ────────────────────────────────────────────────────────────────

/// True when `path` looks like a GAL source repository.
///
/// Checks for `.git` plus the three canonical plugin-core subdirectories.
/// This guard prevents `gal restore` from accidentally running in a
/// downstream repo that merely has GAL installed.
pub(crate) fn is_gal_source_repo(path: &Path) -> bool {
    path.join(".git").exists()
        && path.join("plugins/gal-core/skills").exists()
        && path.join("plugins/gal-core/agents").exists()
        && path.join("plugins/gal-core/commands").exists()
}

/// Resolve the current working directory.
fn cwd() -> Option<PathBuf> {
    std::env::current_dir().ok()
}

/// Run a git command in `dir`, return stdout on success.
fn git_output(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .map_err(|e| format!("git spawn failed: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// Run a git command in `dir`, return success/failure.
fn git_run(dir: &Path, args: &[&str]) -> Result<(), String> {
    git_output(dir, args).map(|_| ())
}

/// Entry point for `gal restore [--yes]`.
pub(crate) fn cmd_restore(args: &[String]) -> ExitCode {
    let yes = args.iter().any(|a| a == "--yes");

    // 1. Verify cwd is a GAL source repo.
    let repo = match cwd() {
        Some(p) => p,
        None => {
            eprintln!("gal restore: cannot determine working directory.");
            return ExitCode::Error;
        }
    };
    if !is_gal_source_repo(&repo) {
        eprintln!(
            "gal restore: current directory does not appear to be a GAL source repository.\n\
             Expected: .git + plugins/gal-core/{{skills,agents,commands}}"
        );
        return ExitCode::Usage;
    }

    // 2. Resolve gal-last-good marker (fail-closed; never fall back to HEAD).
    let marker_commit =
        match git_output(&repo, &["rev-parse", "--verify", "gal-last-good^{commit}"]) {
            Ok(sha) if !sha.is_empty() => sha,
            _ => {
                eprintln!(
                    "gal restore: no 'gal-last-good' tag found.\n\
                 The marker is written automatically by the next '/gal finalize'.\n\
                 Run '/gal finalize' on a verified plan to establish the baseline."
                );
                return ExitCode::Error;
            }
        };

    // 3. List discard scope + confirmation guard.
    let uncommitted = git_output(&repo, &["status", "--porcelain"]).unwrap_or_default();
    let ahead_count: u32 = git_output(
        &repo,
        &["rev-list", "--count", &format!("{marker_commit}..HEAD")],
    )
    .unwrap_or_else(|_| "0".to_string())
    .parse()
    .unwrap_or(0);

    let uncommitted_count = uncommitted.lines().count();

    println!("gal restore — discard scope:");
    println!("  Uncommitted changes: {uncommitted_count} file(s)");
    println!("  Commits ahead of gal-last-good: {ahead_count}");
    println!("  Marker commit: {marker_commit}");
    println!();

    if !yes {
        eprintln!(
            "gal restore: requires --yes to proceed (destructive operation).\n\
             Re-run with: gal restore --yes"
        );
        return ExitCode::Usage;
    }

    // 4. Create backup branch (hard precondition — abort if it fails).
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let backup_branch = format!("gal-restore-backup-{ts}");

    if let Err(e) = git_run(&repo, &["branch", &backup_branch]) {
        eprintln!("gal restore: failed to create backup branch '{backup_branch}': {e}");
        eprintln!("Aborting — worktree unchanged.");
        return ExitCode::Error;
    }
    println!("Created backup branch: {backup_branch}");

    // 5a. Move current branch + worktree to marker commit (A-semantics).
    //     `git reset --hard <marker>` moves HEAD + index + worktree in one step;
    //     post-marker tracked files are removed by the reset.
    if let Err(e) = git_run(&repo, &["reset", "--hard", &marker_commit]) {
        eprintln!("gal restore: reset to marker failed: {e}");
        eprintln!("Backup branch '{backup_branch}' retains the original HEAD.");
        return ExitCode::Error;
    }

    // 5b. Remove untracked files + directories (A-semantics for untracked).
    if let Err(e) = git_run(&repo, &["clean", "-fd"]) {
        eprintln!("gal restore: git clean -fd failed: {e}");
        eprintln!("Tracked files restored; untracked cleanup incomplete.");
        return ExitCode::Error;
    }

    println!("Worktree restored to {marker_commit} (gal-last-good).");

    // 5c. Rebuild derived content (canonical root + runtime projections).
    //     `refresh_with` expects a source root whose `skills/`/`agents/`/`commands/`
    //     live directly under it; in a repo checkout that is `<repo>/plugins/gal-core`,
    //     not the repo root. Resolve it the same way `gal refresh` does. The entry
    //     guard `is_gal_source_repo` already proved that layout exists, so this
    //     normally succeeds; a `None` means the guard's contract was widened without
    //     updating here — fail with a clear message rather than passing a wrong root.
    let source = match resolve_source_from_cwd(&repo) {
        Some(s) => s,
        None => {
            eprintln!(
                "gal restore: could not resolve a GAL source root under {}",
                repo.display()
            );
            eprintln!(
                "Source layer is at the marker; run 'gal refresh' to rebuild the derived layer."
            );
            return ExitCode::Error;
        }
    };
    match refresh_with(&source) {
        Ok(()) => {
            println!("Derived layer rebuilt (gal refresh).");
        }
        Err(e) => {
            eprintln!("gal restore: refresh failed: {e}");
            eprintln!(
                "Source layer is at the marker; run 'gal refresh' to rebuild the derived layer."
            );
            return ExitCode::Error;
        }
    }

    println!();
    println!("--- RESTORE COMPLETE ---");
    println!("Restored to: {marker_commit} (gal-last-good)");
    println!("Backup:      {backup_branch}");

    ExitCode::Success
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Initialise a minimal git repo and return its path.
    fn init_repo(tmp: &TempDir) -> PathBuf {
        let r = tmp.path().to_path_buf();
        git_run(&r, &["init", "--initial-branch=main"]).unwrap_or_else(|_| {
            git_run(&r, &["init"]).expect("git init");
            let _ = git_run(&r, &["checkout", "-b", "main"]);
        });
        git_run(&r, &["config", "user.email", "test@example.com"]).unwrap();
        git_run(&r, &["config", "user.name", "Test"]).unwrap();
        r
    }

    /// Create a commit with a single file and return its short hash.
    fn commit_file(repo: &Path, name: &str, contents: &str) -> String {
        fs::write(repo.join(name), contents).unwrap();
        git_run(repo, &["add", name]).unwrap();
        git_run(repo, &["commit", "-m", name]).unwrap();
        git_output(repo, &["rev-parse", "--short", "HEAD"]).unwrap()
    }

    /// Create the plugin-core subdirectory skeleton required by `is_gal_source_repo`.
    fn scaffold_gal_source(repo: &Path) {
        for sub in &["skills", "agents", "commands"] {
            fs::create_dir_all(repo.join(format!("plugins/gal-core/{sub}"))).unwrap();
        }
    }

    /// Scaffold a minimal but render-valid gal source: the three dirs plus the two
    /// components `render_canonical_root_to` hard-requires — a `doc-sync` skill and
    /// a `golem-steward` agent. Mirrors `plugins/gal-core/skills/doc-sync/SKILL.md`
    /// and `plugins/gal-core/agents/golem-steward.agent.md` at minimal fidelity so
    /// `refresh_with` (render + projection) completes.
    fn scaffold_renderable_source(repo: &Path) {
        scaffold_gal_source(repo);
        fs::create_dir_all(repo.join("plugins/gal-core/skills/doc-sync")).unwrap();
        fs::write(
            repo.join("plugins/gal-core/skills/doc-sync/SKILL.md"),
            "---\nname: doc-sync\ndescription: test doc-sync skill\n---\n\nbody\n",
        )
        .unwrap();
        fs::write(
            repo.join("plugins/gal-core/agents/golem-steward.agent.md"),
            "---\nname: golem-steward\ndescription: test steward\ntools: ['read']\n---\n\nbody\n",
        )
        .unwrap();
        // git does not track empty dirs; keep `commands/` present after
        // `git reset --hard` so `looks_like_source_root` still sees all three.
        fs::write(repo.join("plugins/gal-core/commands/.gitkeep"), "").unwrap();
    }

    // ── CommandKind parse/as_str/ALL.len ─────────────────────────────────────

    #[test]
    fn parse_restore_roundtrips() {
        use gal_engine::CommandKind;
        assert_eq!(CommandKind::parse("restore"), Some(CommandKind::Restore));
        assert_eq!(CommandKind::Restore.as_str(), "restore");
        assert_eq!(CommandKind::ALL.len(), 26);
    }

    // ── is_gal_source_repo ───────────────────────────────────────────────────

    #[test]
    fn non_gal_repo_rejected() {
        let tmp = TempDir::new().unwrap();
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let repo = init_repo(&tmp);
        // No plugins/gal-core skeleton.
        assert!(!is_gal_source_repo(&repo));
    }

    #[test]
    fn gal_source_repo_accepted() {
        let tmp = TempDir::new().unwrap();
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let repo = init_repo(&tmp);
        scaffold_gal_source(&repo);
        assert!(is_gal_source_repo(&repo));
    }

    #[test]
    fn non_git_dir_rejected() {
        let tmp = TempDir::new().unwrap();
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        scaffold_gal_source(tmp.path());
        // No .git directory.
        assert!(!is_gal_source_repo(tmp.path()));
    }

    // ── marker resolution fail-closed ────────────────────────────────────────

    #[test]
    fn no_marker_fails_closed() {
        let tmp = TempDir::new().unwrap();
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let repo = init_repo(&tmp);
        scaffold_gal_source(&repo);
        commit_file(&repo, "README.md", "init");

        // No gal-last-good tag → rev-parse must fail.
        let result = git_output(&repo, &["rev-parse", "--verify", "gal-last-good^{commit}"]);
        assert!(result.is_err(), "should fail when tag absent");
    }

    #[test]
    fn marker_resolves_to_tagged_commit() {
        let tmp = TempDir::new().unwrap();
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let repo = init_repo(&tmp);
        scaffold_gal_source(&repo);
        let sha = commit_file(&repo, "README.md", "init");

        git_run(&repo, &["tag", "gal-last-good"]).unwrap();

        let resolved =
            git_output(&repo, &["rev-parse", "--verify", "gal-last-good^{commit}"]).unwrap();
        assert!(
            resolved.starts_with(&sha) || sha.starts_with(&resolved[..7]),
            "resolved {resolved} should match commit {sha}"
        );
    }

    // ── destructive guard (no --yes = abort, zero mutation) ──────────────────

    #[test]
    fn without_yes_aborts_with_usage() {
        let tmp = TempDir::new().unwrap();
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());
        let repo = init_repo(&tmp);
        scaffold_gal_source(&repo);
        commit_file(&repo, "README.md", "marker");
        git_run(&repo, &["tag", "gal-last-good"]).unwrap();
        commit_file(&repo, "extra.md", "ahead");

        // Save HEAD before calling cmd_restore.
        let head_before = git_output(&repo, &["rev-parse", "HEAD"]).unwrap();

        // Temporarily chdir to the repo so cwd() picks it up.
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo).unwrap();
        let result = cmd_restore(&[]);
        std::env::set_current_dir(orig).unwrap();

        assert_eq!(result, ExitCode::Usage);
        let head_after = git_output(&repo, &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(
            head_before, head_after,
            "HEAD must not change without --yes"
        );
    }

    // ── safe apply — worktree exactly equals marker + backup + refresh ───────

    #[test]
    fn restore_with_yes_produces_a_semantics() {
        let tmp = TempDir::new().unwrap();
        let _g = ENV_GUARD.lock().unwrap_or_else(|e| e.into_inner());

        // Set HOME to an isolated tmp dir so refresh_with doesn't write to real ~/.gal.
        let home_tmp = TempDir::new().unwrap();
        set_home(home_tmp.path());

        let repo = init_repo(&tmp);

        // Marker commit: a render-valid gal source (committed so it survives
        // `git reset --hard` + `git clean -fd`) so the refresh step can resolve
        // `<repo>/plugins/gal-core` and rebuild the derived layer.
        scaffold_renderable_source(&repo);
        fs::write(repo.join("base.md"), "base").unwrap();
        git_run(&repo, &["add", "-A"]).unwrap();
        git_run(&repo, &["commit", "-m", "marker"]).unwrap();
        let marker_sha = git_output(&repo, &["rev-parse", "--short", "HEAD"]).unwrap();
        git_run(&repo, &["tag", "gal-last-good"]).unwrap();

        // Post-marker: add a tracked file + an untracked file.
        commit_file(&repo, "post.md", "post");
        fs::write(repo.join("untracked.md"), "untracked").unwrap();

        let head_before = git_output(&repo, &["rev-parse", "HEAD"]).unwrap();
        assert_ne!(
            &head_before[..marker_sha.len().min(head_before.len())],
            &marker_sha[..marker_sha.len().min(head_before.len())]
        );

        // Run restore --yes from the repo dir.
        let orig = std::env::current_dir().unwrap();
        std::env::set_current_dir(&repo).unwrap();
        let result = cmd_restore(&["restore".to_string(), "--yes".to_string()]);
        std::env::set_current_dir(orig).unwrap();

        // BUG-A fix: refresh now resolves <repo>/plugins/gal-core and fully
        // succeeds. This must be Success, not the previously-masked Error.
        assert_eq!(
            result,
            ExitCode::Success,
            "restore --yes must fully succeed (refresh resolves plugins/gal-core)"
        );

        // (1) HEAD must equal marker commit.
        let head_after = git_output(&repo, &["rev-parse", "HEAD"]).unwrap();
        assert!(
            head_after.starts_with(&marker_sha)
                || marker_sha.starts_with(&head_after[..7.min(head_after.len())]),
            "HEAD after restore {head_after} should match marker {marker_sha}"
        );

        // (2) post-marker tracked file must be gone.
        assert!(
            !repo.join("post.md").exists(),
            "post.md (tracked, added after marker) must be removed"
        );

        // (3) untracked file must be gone (git clean -fd).
        assert!(
            !repo.join("untracked.md").exists(),
            "untracked.md must be removed by git clean -fd"
        );

        // (4) Backup branch must exist and point to the original HEAD.
        let branches = git_output(&repo, &["branch"]).unwrap_or_default();
        assert!(
            branches.contains("gal-restore-backup-"),
            "backup branch must exist: {branches}"
        );
        let backup_name = branches
            .lines()
            .filter_map(|l| {
                let l = l.trim();
                if l.starts_with("gal-restore-backup-") {
                    Some(l.to_string())
                } else {
                    None
                }
            })
            .next()
            .expect("backup branch should exist");
        let backup_sha = git_output(&repo, &["rev-parse", &backup_name]).unwrap();
        assert_eq!(
            backup_sha, head_before,
            "backup branch must point at original HEAD"
        );
    }
}
