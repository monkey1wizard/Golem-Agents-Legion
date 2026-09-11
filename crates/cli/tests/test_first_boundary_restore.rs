//! `cargo test -p gal-cli --test test_first_boundary_restore`
//!
//! Integration probes for transactional snapshot restore and verified rollback.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git must be available");
    assert!(output.status.success(), "git {args:?} failed");
}

fn fixture() -> (TempDir, PathBuf, String) {
    let temp = tempdir().expect("fixture tempdir");
    let repo = temp.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "fixture@example.com"]);
    git(repo, &["config", "user.name", "CLI Fixture"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::create_dir_all(repo.join("tests")).unwrap();
    fs::write(repo.join(".gitignore"), ".dev/pipeline/\n").unwrap();
    fs::write(repo.join("README.md"), "init\n").unwrap();
    fs::write(repo.join("src/lib.rs"), "production-v1\n").unwrap();
    fs::write(repo.join("tests/test.rs"), "test-v1\n").unwrap();
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", "init"]);

    let slug = "restore-fixture";
    let task = format!("T-{:02}", 22u32);
    let prompt = format!(
        "# Plan Prompt: {slug}\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {task} — Restore snapshot.\n  - Test-first: required\n  - Seam: `src/lib.rs`\n  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=dGVzdA — restore failure`\n  - Production Paths: `src/lib.rs`\n  - Test Paths: `tests/test.rs`\n  - Scaffold: not-required\n\n## Files to Create or Modify\n\n- `src/lib.rs`\n- `tests/test.rs`\n"
    );
    let prompt_path = repo.join(".dev/plans").join(format!("{slug}.prompt.md"));
    fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
    fs::write(&prompt_path, prompt).unwrap();
    (temp, prompt_path, task)
}

fn capture(repo: &Path, prompt: &Path, task: &str) {
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("boundary-check")
        .arg(prompt)
        .args([
            "--task",
            task,
            "--capture",
            "--phase",
            "post-test",
            "--generation",
            "1",
        ])
        .output()
        .expect("capture must run");
    assert!(
        output.status.success(),
        "capture failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn restore(repo: &Path, prompt: &Path, task: &str, fault: Option<&str>) -> (i32, String) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
    command
        .current_dir(repo)
        .arg("boundary-check")
        .arg(prompt)
        .arg("--task")
        .arg(task)
        .arg("--receipt")
        .arg(repo.join("restore-receipt.md"))
        .args(["--restore", "--phase", "post-test", "--generation", "1"]);
    if let Some(fault) = fault {
        command.env("GAL_TEST_FIRST_RESTORE_FAIL_AT", fault);
    }
    let output = command.output().expect("restore must run");
    (
        output.status.code().unwrap_or(-1),
        fs::read_to_string(repo.join("restore-receipt.md")).unwrap(),
    )
}

fn assert_rollback(repo: &Path, prompt: &Path, task: &str, fault: &str) {
    let (code, receipt) = restore(repo, prompt, task, Some(fault));
    assert_ne!(code, 0, "fault {fault} must fail");
    assert!(receipt.contains("verdict=rolled-back"), "{receipt}");
    assert_eq!(
        fs::read(repo.join("src/lib.rs")).unwrap(),
        b"production-mutated\n"
    );
    assert_eq!(
        fs::read(repo.join("tests/test.rs")).unwrap(),
        b"test-mutated\n"
    );
    assert_eq!(
        fs::read(repo.join("README.md")).unwrap(),
        b"undeclared-mutated\n"
    );

    let (code, receipt) = restore(repo, prompt, task, None);
    assert_eq!(code, 0, "rerun must restore after {fault}: {receipt}");
    assert!(receipt.contains("verdict=restored"), "{receipt}");
    assert_eq!(
        fs::read(repo.join("src/lib.rs")).unwrap(),
        b"production-v1\n"
    );
    assert_eq!(fs::read(repo.join("tests/test.rs")).unwrap(), b"test-v1\n");
    assert_eq!(
        fs::read(repo.join("README.md")).unwrap(),
        b"undeclared-mutated\n"
    );
}

#[test]
fn restore_is_exact_idempotent_and_does_not_touch_undeclared_paths() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    capture(repo, &prompt, &task);
    fs::write(repo.join("src/lib.rs"), "production-mutated\n").unwrap();
    fs::remove_file(repo.join("tests/test.rs")).unwrap();
    fs::write(repo.join("README.md"), "undeclared-mutated\n").unwrap();

    let (code, receipt) = restore(repo, &prompt, &task, None);
    assert_eq!(code, 0, "{receipt}");
    assert!(receipt.contains("verdict=restored"), "{receipt}");
    assert_eq!(
        fs::read(repo.join("src/lib.rs")).unwrap(),
        b"production-v1\n"
    );
    assert_eq!(fs::read(repo.join("tests/test.rs")).unwrap(), b"test-v1\n");
    assert_eq!(
        fs::read(repo.join("README.md")).unwrap(),
        b"undeclared-mutated\n"
    );

    let (code, receipt) = restore(repo, &prompt, &task, None);
    assert_eq!(code, 0, "idempotent rerun failed: {receipt}");
    assert!(receipt.contains("verdict=restored"), "{receipt}");
}

#[test]
fn restore_rolls_back_replacement_at_every_replace_boundary() {
    for fault in [
        "before-apply",
        "after-delete",
        "after-replace",
        "after-recheck",
    ] {
        let (temp, prompt, task) = fixture();
        let repo = temp.path();
        capture(repo, &prompt, &task);
        fs::write(repo.join("src/lib.rs"), "production-mutated\n").unwrap();
        fs::write(repo.join("tests/test.rs"), "test-mutated\n").unwrap();
        fs::write(repo.join("README.md"), "undeclared-mutated\n").unwrap();
        assert_rollback(repo, &prompt, &task, fault);
    }
}

#[test]
fn restore_rolls_back_delete_and_create_boundaries() {
    for fault in ["after-delete", "1", "after-recheck"] {
        let (temp, prompt, task) = fixture();
        let repo = temp.path();
        fs::remove_file(repo.join("tests/test.rs")).unwrap();
        capture(repo, &prompt, &task);
        fs::write(repo.join("src/lib.rs"), "production-mutated\n").unwrap();
        fs::write(repo.join("tests/test.rs"), "test-mutated\n").unwrap();
        fs::write(repo.join("README.md"), "undeclared-mutated\n").unwrap();
        let (code, receipt) = restore(repo, &prompt, &task, Some(fault));
        assert_ne!(code, 0);
        assert!(receipt.contains("verdict=rolled-back"), "{receipt}");
        assert_eq!(
            fs::read(repo.join("src/lib.rs")).unwrap(),
            b"production-mutated\n"
        );
        assert_eq!(
            fs::read(repo.join("tests/test.rs")).unwrap(),
            b"test-mutated\n"
        );
        let (code, receipt) = restore(repo, &prompt, &task, None);
        assert_eq!(code, 0, "{receipt}");
        assert!(receipt.contains("verdict=restored"), "{receipt}");
        assert_eq!(
            fs::read(repo.join("src/lib.rs")).unwrap(),
            b"production-v1\n"
        );
        assert!(!repo.join("tests/test.rs").exists());
        assert_eq!(
            fs::read(repo.join("README.md")).unwrap(),
            b"undeclared-mutated\n"
        );
    }

    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    capture(repo, &prompt, &task);
    fs::write(repo.join("src/lib.rs"), "production-mutated\n").unwrap();
    fs::write(repo.join("tests/test.rs"), "test-mutated\n").unwrap();
    fs::write(repo.join("README.md"), "undeclared-mutated\n").unwrap();
    fs::remove_file(repo.join("tests/test.rs")).unwrap();
    let (code, receipt) = restore(repo, &prompt, &task, Some("after-create"));
    assert_ne!(code, 0);
    assert!(receipt.contains("verdict=rolled-back"), "{receipt}");
    assert_eq!(
        fs::read(repo.join("src/lib.rs")).unwrap(),
        b"production-mutated\n"
    );
    assert!(!repo.join("tests/test.rs").exists());
    let (code, receipt) = restore(repo, &prompt, &task, None);
    assert_eq!(code, 0, "{receipt}");
    assert!(receipt.contains("verdict=restored"), "{receipt}");
    assert_eq!(
        fs::read(repo.join("src/lib.rs")).unwrap(),
        b"production-v1\n"
    );
    assert_eq!(fs::read(repo.join("tests/test.rs")).unwrap(), b"test-v1\n");
    assert_eq!(
        fs::read(repo.join("README.md")).unwrap(),
        b"undeclared-mutated\n"
    );
}

#[test]
fn restore_rejects_tampered_snapshot_before_mutation() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    capture(repo, &prompt, &task);
    let generation = fs::read_dir(
        repo.join(".dev/pipeline/snapshots/restore-fixture")
            .join(&task),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap()
    .path();
    let manifest = generation.join("post-test.snapshot.tsv");
    let original = fs::read_to_string(&manifest).unwrap();
    fs::write(&manifest, original.replace("present", "missing")).unwrap();
    fs::write(repo.join("src/lib.rs"), "production-mutated\n").unwrap();
    fs::write(repo.join("tests/test.rs"), "test-mutated\n").unwrap();
    fs::write(repo.join("README.md"), "undeclared-mutated\n").unwrap();

    let (code, receipt) = restore(repo, &prompt, &task, None);
    assert_ne!(code, 0);
    assert!(receipt.contains("verdict=rolled-back"), "{receipt}");
    assert_eq!(
        fs::read(repo.join("src/lib.rs")).unwrap(),
        b"production-mutated\n"
    );
    assert_eq!(
        fs::read(repo.join("tests/test.rs")).unwrap(),
        b"test-mutated\n"
    );
    assert_eq!(
        fs::read(repo.join("README.md")).unwrap(),
        b"undeclared-mutated\n"
    );
}

#[test]
fn restore_reports_rollback_unconfirmed_when_prevalidation_cannot_read_prompt() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    capture(repo, &prompt, &task);
    fs::remove_file(&prompt).unwrap();
    fs::create_dir(&prompt).unwrap();

    let (code, receipt) = restore(repo, &prompt, &task, None);
    assert_ne!(code, 0);
    assert!(
        receipt.contains("verdict=rollback-unconfirmed"),
        "{receipt}"
    );
}
