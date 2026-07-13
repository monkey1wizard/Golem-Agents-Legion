//! Git filter registration for the `gal-config` smudge/clean filter.
//!
//! Registration points at the Rust binary commands `gal clean` / `gal smudge`.
//! `.gitattributes` is tracked in the repo and already carries the
//! `filter=gal-config` annotations.
//!
//! Used by the doctor filter-registered check (`gal doctor`) and by
//! `register_git_filter` for manual or init-time registration.
//!
//! Registration uses `git config --replace-all`, which is idempotent and
//! also collapses historical duplicate entries for the same key.

use std::io::Write;
use std::path::Path;

fn filter_command(_repo_root: &Path, subcommand: &str) -> String {
    // The gal-config filter is backed by the self-contained `gal` binary
    // (`gal clean` / `gal smudge`). Requires `gal` on PATH, which
    // `gal setup` / `gal install` provides.
    format!("gal {subcommand}")
}

/// The exact (key, value) pairs registered for the repo at `repo_root`.
pub fn registration_entries(repo_root: &Path) -> Vec<(String, String)> {
    vec![
        (
            "filter.gal-config.clean".into(),
            filter_command(repo_root, "clean"),
        ),
        (
            "filter.gal-config.smudge".into(),
            filter_command(repo_root, "smudge"),
        ),
        ("filter.gal-config.required".into(), "true".into()),
    ]
}

/// Register the `gal-config` filter in the repo-local git config.
/// Idempotent; honors `dry_run` (zero writes).
pub fn register_git_filter(
    repo_root: &Path,
    dry_run: bool,
    out: &mut dyn Write,
) -> std::io::Result<bool> {
    let _ = writeln!(out);
    let _ = writeln!(out, "=== Git filter (gal-config) ===");

    if !repo_root.join(".git").exists() {
        let _ = writeln!(
            out,
            "  [SKIP] {} is not a git repository — filter registration skipped.",
            repo_root.display()
        );
        return Ok(false);
    }
    for (key, value) in registration_entries(repo_root) {
        if dry_run {
            let _ = writeln!(out, "  [DRY RUN] Would set git config {key} = {value}");
            continue;
        }
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["config", "--replace-all", &key, &value])
            .status()?;
        if !status.success() {
            let _ = writeln!(
                out,
                "  [WARN] git config --replace-all {key} failed (exit {}).",
                status.code().unwrap_or(1)
            );
            return Ok(false);
        }
        let _ = writeln!(out, "  [OK] git config {key} = {value}");
    }
    Ok(true)
}

/// Read the registered clean filter for a repo (used by the health check).
/// Returns `None` when unregistered or git is unavailable.
pub fn registered_clean_filter(repo_root: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(["config", "--get", "filter.gal-config.clean"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    fn init_repo() -> tempfile::TempDir {
        let temp = tempfile::tempdir().unwrap();
        assert!(Command::new("git")
            .arg("-C")
            .arg(temp.path())
            .args(["init", "-q"])
            .status()
            .unwrap()
            .success());
        temp
    }

    fn get_all(repo: &Path, key: &str) -> Vec<String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(["config", "--local", "--get-all", key])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn entries_point_at_the_gal_binary_with_required_true() {
        let entries = registration_entries(Path::new("/repo"));
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].1, "gal clean");
        assert_eq!(entries[1].1, "gal smudge");
        assert_eq!(
            entries[2],
            ("filter.gal-config.required".to_string(), "true".to_string())
        );
    }

    #[test]
    fn registration_is_idempotent_and_collapses_duplicates() {
        if !git_available() {
            return;
        }
        let temp = init_repo();
        let repo = temp.path();

        for value in ["bash old-a.sh", "bash old-b.sh"] {
            Command::new("git")
                .arg("-C")
                .arg(repo)
                .args(["config", "--add", "filter.gal-config.clean", value])
                .status()
                .unwrap();
        }
        assert_eq!(get_all(repo, "filter.gal-config.clean").len(), 2);

        let mut out = Vec::new();
        assert!(register_git_filter(repo, false, &mut out).unwrap());
        assert!(register_git_filter(repo, false, &mut out).unwrap());

        let clean = get_all(repo, "filter.gal-config.clean");
        assert_eq!(clean.len(), 1, "duplicates collapsed to one entry");
        assert_eq!(clean[0], "gal clean");
        assert_eq!(get_all(repo, "filter.gal-config.required"), vec!["true"]);
        assert_eq!(
            registered_clean_filter(repo).unwrap(),
            clean[0],
            "health-check reader sees the registered value"
        );
    }

    #[test]
    fn dry_run_writes_nothing() {
        if !git_available() {
            return;
        }
        let temp = init_repo();
        let repo = temp.path();
        let mut out = Vec::new();
        register_git_filter(repo, true, &mut out).unwrap();
        assert!(get_all(repo, "filter.gal-config.clean").is_empty());
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("[DRY RUN] Would set git config filter.gal-config.clean"));
    }

    #[test]
    fn missing_repo_skips_safely() {
        let temp = tempfile::tempdir().unwrap();
        let mut out = Vec::new();
        assert!(!register_git_filter(temp.path(), false, &mut out).unwrap());
        let printed = String::from_utf8(out).unwrap();
        assert!(printed.contains("not a git repository"));
    }
}
