//! Real-binary coverage for `gal test-first-commit`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git must spawn");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn fixture() -> (TempDir, String, PathBuf) {
    fixture_with_production(b"implementation\n", false)
}

/// Builds the shared fixture with an explicit production payload and an
/// explicit `core.autocrlf` setting.
///
/// `core.autocrlf` is pinned rather than inherited so each test states the
/// normalization regime it exercises instead of depending on the machine's
/// global git config. `crlf_normalizing = true` reproduces the Windows
/// default, where `git add` rewrites CRLF to LF on the way into the index and
/// the staged blob therefore stops matching the worktree bytes.
fn fixture_with_production(
    production_bytes: &[u8],
    crlf_normalizing: bool,
) -> (TempDir, String, PathBuf) {
    let temp = tempdir().unwrap();
    let repo = temp.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "fixture@example.com"]);
    git(repo, &["config", "user.name", "CLI Fixture"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    git(repo, &["config", "core.hooksPath", ".git/hooks"]);
    git(
        repo,
        &[
            "config",
            "core.autocrlf",
            if crlf_normalizing { "true" } else { "false" },
        ],
    );
    fs::create_dir_all(repo.join(".dev/pipeline/receipts")).unwrap();
    fs::write(repo.join(".gitignore"), ".dev/\n").unwrap();
    fs::write(repo.join("README.md"), "base\n").unwrap();
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", "base"]);

    let base = git(repo, &["rev-parse", "HEAD"]);
    let production = repo.join("odd;[x] 東京.txt");
    fs::write(&production, production_bytes).unwrap();
    fs::write(
        repo.join(".dev/pipeline/receipts/correctness.md"),
        "overall: pass\n",
    )
    .unwrap();
    fs::write(
        repo.join(".dev/pipeline/receipts/audit.md"),
        "Verdict: APPROVE\n",
    )
    .unwrap();
    (temp, base, production)
}

fn run_commit(repo: &Path, base: &str, production: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args([
            "test-first-commit",
            "run",
            "--plan",
            "fixture-plan",
            "--task",
            "fixture-task",
            "--base",
            base,
            "--production",
            production.file_name().unwrap().to_str().unwrap(),
            "--correctness-receipt",
            ".dev/pipeline/receipts/correctness.md",
            "--auditor-receipt",
            ".dev/pipeline/receipts/audit.md",
            "--message",
            "feat: implementation commit",
            "--receipt",
            ".dev/pipeline/receipts/commit.md",
        ])
        .output()
        .unwrap()
}

#[test]
fn production_dispatch_commits_literal_paths_and_writes_receipt_shape() {
    let (temp, base, production) = fixture();
    let output = run_commit(temp.path(), &base, &production);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let receipt = fs::read_to_string(temp.path().join(".dev/pipeline/receipts/commit.md")).unwrap();
    assert!(receipt.contains("kind=test-first-commit"));
    assert!(receipt.contains("plan_slug=fixture-plan"));
    assert!(receipt.contains("task_id=fixture-task"));
    assert!(receipt.contains(&format!("base_commit={base}")));
    assert!(receipt.contains("new_commit="));
    assert!(receipt.contains("parent="));
    assert!(receipt.contains("range="));
    assert!(receipt.contains("dirty_set=odd;[x] 東京.txt"));
    assert!(receipt.contains("cached_hashes=odd;[x] 東京.txt="));
    assert!(receipt.contains("overall: pass"));
    assert_eq!(git(temp.path(), &["rev-parse", "HEAD^"]), base);
    assert!(git(temp.path(), &["status", "--porcelain"]).is_empty());
}

#[test]
fn production_dispatch_commits_crlf_payload_under_index_normalization() {
    // Regression: `cached_hashes` used to snapshot the staged blob
    // (`git show :path`) while `verify_cached_bytes` read the worktree, so any
    // clean filter that rewrote content on staging — `core.autocrlf` on
    // Windows being the common one — made the two digests differ and aborted
    // the commit with "cached bytes differ" for every real implementation.
    let (temp, base, production) = fixture_with_production(b"line one\r\nline two\r\n", true);
    let output = run_commit(temp.path(), &base, &production);
    assert!(
        output.status.success(),
        "CRLF payload must commit under core.autocrlf=true; stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(git(temp.path(), &["rev-parse", "HEAD^"]), base);
    assert!(git(temp.path(), &["status", "--porcelain"]).is_empty());
}

#[test]
fn production_dispatch_rejects_stale_base_without_receipt_or_commit() {
    let (temp, base, production) = fixture();
    let stale = "0".repeat(40);
    let output = run_commit(temp.path(), &stale, &production);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("stale base"));
    assert!(!temp
        .path()
        .join(".dev/pipeline/receipts/commit.md")
        .exists());
    assert_eq!(git(temp.path(), &["rev-parse", "HEAD"]), base);
}

fn run_commit_with_receipt(
    repo: &Path,
    base: &str,
    production: &Path,
    receipt: &str,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args([
            "test-first-commit",
            "run",
            "--plan",
            "fixture-plan",
            "--task",
            "fixture-task",
            "--base",
            base,
            "--production",
            production.file_name().unwrap().to_str().unwrap(),
            "--correctness-receipt",
            ".dev/pipeline/receipts/correctness.md",
            "--auditor-receipt",
            ".dev/pipeline/receipts/audit.md",
            "--message",
            "feat: implementation commit",
            "--receipt",
            receipt,
        ])
        .output()
        .unwrap()
}

/// The gate must accept the marker `golem-auditor.agent.md` actually writes.
/// It compares whole trimmed lines, so the three shapes it used to accept could
/// never match a real auditor receipt, and wiring the gate into the pipeline
/// would have rejected every commit.
#[test]
fn declared_auditor_marker_is_accepted() {
    let (temp, base, production) = fixture();
    fs::write(
        temp.path().join(".dev/pipeline/receipts/audit.md"),
        "# Audit\n\nSome prose.\n\n<!-- AUDIT_REVIEW: CLEAR -->\n",
    )
    .unwrap();

    let output = run_commit(temp.path(), &base, &production);
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn auditor_receipt_without_any_pass_marker_is_rejected() {
    let (temp, base, production) = fixture();
    fs::write(
        temp.path().join(".dev/pipeline/receipts/audit.md"),
        "# Audit\n\nFindings remain open.\n",
    )
    .unwrap();

    let output = run_commit(temp.path(), &base, &production);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("required pass marker"));
    assert_eq!(git(temp.path(), &["rev-parse", "HEAD"]), base);
}

/// `--receipt` used to be joined onto the repository root unvalidated, so an
/// absolute path wrote outside the tree. The destination is now bound before
/// the commit runs, so a bad path costs nothing.
#[test]
fn receipt_path_outside_the_repository_is_rejected_before_committing() {
    let (temp, base, production) = fixture();
    let outside = tempdir().unwrap();
    let escape = outside.path().join("stolen/commit.receipt.md");

    let output =
        run_commit_with_receipt(temp.path(), &base, &production, &escape.to_string_lossy());

    assert!(!output.status.success());
    assert!(!escape.exists(), "no file may be written outside the repo");
    assert!(
        !escape.parent().unwrap().exists(),
        "no directory may be created outside the repo"
    );
    assert_eq!(
        git(temp.path(), &["rev-parse", "HEAD"]),
        base,
        "a rejected receipt path must not leave a commit behind"
    );
}

#[test]
fn receipt_path_traversal_is_rejected_before_committing() {
    let (temp, base, production) = fixture();

    let output = run_commit_with_receipt(temp.path(), &base, &production, "../escaped/commit.md");

    assert!(!output.status.success());
    assert!(!temp.path().parent().unwrap().join("escaped").exists());
    assert_eq!(git(temp.path(), &["rev-parse", "HEAD"]), base);
}
