//! Hermetic production-dispatch probe for `gal release-notes`.
//!
//! Runs the real built `gal` binary (`env!("CARGO_BIN_EXE_gal")`) against a
//! throwaway temp git repository this test builds from scratch. It never reads
//! this repo's own tags or history — every commit and tag is created inside the
//! temp directory the test controls.

use std::path::Path;
use std::process::Command;

fn git_available() -> bool {
    Command::new("git").arg("--version").output().is_ok()
}

fn run_git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo)
        .args(args)
        .status()
        .expect("git spawn failed");
    assert!(status.success(), "git {args:?} failed in {repo:?}");
}

fn init_repo() -> tempfile::TempDir {
    let temp = tempfile::tempdir().unwrap();
    run_git(temp.path(), &["init", "-q", "-b", "main"]);
    run_git(temp.path(), &["config", "user.email", "test@example.com"]);
    run_git(temp.path(), &["config", "user.name", "Test User"]);
    // Override any machine-global core.hooksPath so a commit-msg hook from
    // this repo (or elsewhere) never rewrites the throwaway test commits.
    run_git(temp.path(), &["config", "core.hooksPath", ".git/hooks"]);
    temp
}

fn commit(repo: &Path, file: &str, message: &str) -> String {
    std::fs::write(repo.join(file), message).unwrap();
    run_git(repo, &["add", file]);
    run_git(repo, &["commit", "-q", "-m", message]);
    head(repo)
}

fn head(repo: &Path) -> String {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn tag(repo: &Path, name: &str) {
    run_git(repo, &["tag", name]);
}

fn run_release_notes(repo: &Path, extra: &[&str]) -> (bool, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("release-notes")
        .args(extra)
        .output()
        .expect("failed to spawn gal binary");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

#[test]
fn production_dispatch_selects_highest_stable_tag_as_default_range() {
    if !git_available() {
        return;
    }
    let temp = init_repo();
    let repo = temp.path();

    commit(repo, "a.txt", "feat: pre-tag change");
    tag(repo, "v0.1.1");
    commit(repo, "b.txt", "feat: tagged stable");
    tag(repo, "v0.1.2");
    commit(repo, "c.txt", "feat: after v0.1.2");

    let (success, stdout, stderr) = run_release_notes(repo, &[]);
    assert!(success, "gal release-notes failed: {stderr}");
    assert!(
        stdout.contains("feat: after v0.1.2"),
        "expected the post-v0.1.2 commit in stdout: {stdout}"
    );
    assert!(
        !stdout.contains("feat: pre-tag change"),
        "must not include commits before the selected v0.1.2 tag: {stdout}"
    );
    assert!(
        !stdout.contains("feat: tagged stable"),
        "the v0.1.2 tag itself is the range start, exclusive: {stdout}"
    );
}

#[test]
fn production_dispatch_explicit_range_prints_only_that_range() {
    if !git_available() {
        return;
    }
    let temp = init_repo();
    let repo = temp.path();

    let first = commit(repo, "a.txt", "feat: first");
    let second = commit(repo, "b.txt", "feat: second");
    let _ = first;

    let (success, stdout, stderr) = run_release_notes(repo, &[&format!("HEAD~1..{second}")]);
    assert!(success, "gal release-notes failed: {stderr}");
    assert!(stdout.contains("feat: second"));
    assert!(!stdout.contains("feat: first"));
}

#[test]
fn production_dispatch_excludes_merge_commit_but_keeps_landed_task_commit() {
    if !git_available() {
        return;
    }
    let temp = init_repo();
    let repo = temp.path();

    let base = commit(repo, "a.txt", "feat: base");
    run_git(repo, &["checkout", "-q", "-b", "side"]);
    commit(repo, "b.txt", "feat: side task");
    run_git(repo, &["checkout", "-q", "main"]);
    run_git(
        repo,
        &["merge", "--no-ff", "-q", "-m", "merge: land side", "side"],
    );
    let head_hash = head(repo);

    let (success, stdout, stderr) = run_release_notes(repo, &[&format!("{base}..{head_hash}")]);
    assert!(success, "gal release-notes failed: {stderr}");
    assert!(
        stdout.contains("feat: side task"),
        "expected the landed task commit in stdout: {stdout}"
    );
    assert!(
        !stdout.contains("merge: land side"),
        "the merge commit itself must not appear: {stdout}"
    );
}
