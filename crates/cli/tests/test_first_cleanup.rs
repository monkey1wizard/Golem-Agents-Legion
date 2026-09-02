//! Real-binary coverage for finalized test-first cleanup.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn init_repo() -> TempDir {
    let temp = TempDir::new().unwrap();
    Command::new("git")
        .current_dir(temp.path())
        .args(["init", "-q", "-b", "main"])
        .status()
        .unwrap();
    temp
}

fn run(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

fn snapshot_plan_roots(repo: &Path, plan: &str) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        if !dir.exists() {
            return;
        }
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let bytes = fs::read(&path).unwrap();
                let digest = format!("{:x}", Sha256::digest(&bytes));
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, digest));
            }
        }
    }
    let mut out = Vec::new();
    let r_root = repo.join(format!(".dev/pipeline/receipts/{plan}"));
    let s_root = repo.join(format!(".dev/pipeline/snapshots/{plan}"));
    walk(&r_root, repo, &mut out);
    walk(&s_root, repo, &mut out);
    out.sort();
    out
}

fn fixture() -> TempDir {
    let temp = init_repo();
    let receipts_task = temp
        .path()
        .join(".dev/pipeline/receipts/fixture-plan/fixture-task");
    fs::create_dir_all(&receipts_task).unwrap();
    fs::write(receipts_task.join("fixture.receipt.md"), b"evidence\n").unwrap();

    let snapshots_task = temp
        .path()
        .join(format!(".dev/pipeline/snapshots/fixture-plan/{}/g1-c0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef", tid(15)));
    fs::create_dir_all(&snapshots_task).unwrap();
    fs::write(
        snapshots_task.join("pre-implement.snapshot.tsv"),
        b"manifest\n",
    )
    .unwrap();
    temp
}

fn cleanup(repo: &Path) -> Output {
    run(
        repo,
        &[
            "test-first-cleanup",
            "run",
            "--plan",
            "fixture-plan",
            "--finalized",
            "--receipt",
            ".dev/pipeline/cleanup/fixture-plan.receipt.md",
        ],
    )
}

#[test]
fn real_binary_removes_exact_plan_roots_and_publishes_receipt() {
    let temp = fixture();
    let output = cleanup(temp.path());
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!temp
        .path()
        .join(".dev/pipeline/receipts/fixture-plan")
        .exists());
    assert!(!temp
        .path()
        .join(".dev/pipeline/snapshots/fixture-plan")
        .exists());
    assert!(temp
        .path()
        .join(".dev/pipeline/cleanup/fixture-plan.receipt.md")
        .exists());
}

#[test]
fn real_binary_resumes_after_process_death_at_quarantine_boundary() {
    let temp = fixture();
    let interrupted = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(temp.path())
        .env("GAL_TEST_FIRST_CLEANUP_FAIL_AT", "before-quarantine")
        .args([
            "test-first-cleanup",
            "run",
            "--plan",
            "fixture-plan",
            "--finalized",
            "--receipt",
            ".dev/pipeline/cleanup/fixture-plan.receipt.md",
        ])
        .output()
        .unwrap();
    assert!(!interrupted.status.success());
    assert!(temp
        .path()
        .join(".dev/pipeline/receipts/fixture-plan")
        .exists());

    let resumed = cleanup(temp.path());
    assert!(
        resumed.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&resumed.stdout),
        String::from_utf8_lossy(&resumed.stderr)
    );
    assert!(!temp
        .path()
        .join(".dev/pipeline/receipts/fixture-plan")
        .exists());
    assert!(!temp
        .path()
        .join(".dev/pipeline/snapshots/fixture-plan")
        .exists());
}

#[test]
fn positive_fixtures_for_closed_ownership_set_are_removed_and_optional_roots_converge() {
    let temp = init_repo();
    let task_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let blob_hex = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";

    // 1. Receipts Root allowed shapes:
    let r_root = temp.path().join(".dev/pipeline/receipts/fixture-plan");
    fs::create_dir_all(r_root.join("deep/sub")).unwrap();
    fs::write(r_root.join("item.receipt.md"), b"ok").unwrap();
    fs::write(r_root.join("deep/sub/nested.receipt.md"), b"ok").unwrap();

    let r_task = r_root.join(format!("{}/g1-c{task_hex}", tid(15)));
    fs::create_dir_all(r_task.join("outputs")).unwrap();
    fs::write(
        r_task.join(format!("outputs/{blob_hex}.bin")),
        b"probe blob",
    )
    .unwrap();

    // 2. Snapshots Root allowed shapes:
    let s_root = temp.path().join(".dev/pipeline/snapshots/fixture-plan");
    let s_task = s_root.join(format!("{}/g1-c{task_hex}", tid(15)));
    fs::create_dir_all(s_task.join("blobs")).unwrap();
    fs::create_dir_all(s_task.join("restore/blobs")).unwrap();

    for phase in [
        "implementation-commit",
        "pre-implement",
        "state-recording",
        "post-test",
        "post-audit",
    ] {
        fs::write(s_task.join(format!("{phase}.snapshot.tsv")), b"tsv\n").unwrap();
    }
    fs::write(s_task.join(format!("blobs/{blob_hex}.bin")), b"blob\n").unwrap();
    fs::write(s_task.join("restore/restore.journal.tsv"), b"journal\n").unwrap();
    fs::write(
        s_task.join(format!("restore/blobs/{blob_hex}.bin")),
        b"blob\n",
    )
    .unwrap();

    let output = cleanup(temp.path());
    assert!(
        output.status.success(),
        "stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!r_root.exists());
    assert!(!s_root.exists());

    // Either root missing validly converges
    let temp2 = init_repo();
    let r_root2 = temp2.path().join(".dev/pipeline/receipts/fixture-plan");
    fs::create_dir_all(&r_root2).unwrap();
    fs::write(r_root2.join("only.receipt.md"), b"ok").unwrap();
    let output2 = cleanup(temp2.path());
    assert!(output2.status.success());
    assert!(!r_root2.exists());

    // Neither root existing validly converges as no-op and publishes receipt
    let temp3 = init_repo();
    let output3 = cleanup(temp3.path());
    assert!(output3.status.success());
    assert!(temp3
        .path()
        .join(".dev/pipeline/cleanup/fixture-plan.receipt.md")
        .exists());
    assert!(!temp3
        .path()
        .join(".dev/pipeline/receipts/fixture-plan")
        .exists());
    assert!(!temp3
        .path()
        .join(".dev/pipeline/snapshots/fixture-plan")
        .exists());
}

#[test]
fn prevalidates_ownership_and_rejects_second_root_defect_without_mutating_first_root() {
    let temp = init_repo();
    let r_root = temp.path().join(".dev/pipeline/receipts/fixture-plan");
    fs::create_dir_all(&r_root).unwrap();
    fs::write(r_root.join("valid.receipt.md"), b"valid receipt").unwrap();

    let s_root = temp.path().join(".dev/pipeline/snapshots/fixture-plan");
    fs::create_dir_all(&s_root).unwrap();
    // Non-owned file in second root
    fs::write(s_root.join("unowned.txt"), b"unowned file").unwrap();

    let before_tree = snapshot_plan_roots(temp.path(), "fixture-plan");
    let output = cleanup(temp.path());
    let after_tree = snapshot_plan_roots(temp.path(), "fixture-plan");
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    assert!(
        !output.status.success(),
        "cleanup must fail when closed ownership set is violated"
    );
    assert!(
        combined.contains("cleanup ownership"),
        "expected 'cleanup ownership' in output stream, got:\n{combined}"
    );

    // Atomic prevalidation rule: First root MUST NOT be mutated or quarantined!
    assert!(
        r_root.join("valid.receipt.md").exists(),
        "first root was mutated before second root validation failed!"
    );
    assert!(
        s_root.join("unowned.txt").exists(),
        "second root was mutated!"
    );
    assert!(
        !temp
            .path()
            .join(".dev/pipeline/cleanup/fixture-plan")
            .exists(),
        "cleanup journal was written despite validation failure!"
    );
    assert_eq!(
        before_tree, after_tree,
        "rejection path must leave both plan roots byte-identical"
    );
    let fail_receipt_path = temp
        .path()
        .join(".dev/pipeline/cleanup/fixture-plan.receipt.md");
    assert!(
        fail_receipt_path.exists(),
        "fail receipt must be written on validation failure"
    );
    let fail_receipt = fs::read_to_string(&fail_receipt_path).unwrap();
    assert!(
        fail_receipt.contains("overall: fail"),
        "validation failure receipt must contain 'overall: fail', got:\n{fail_receipt}"
    );
}

#[test]
fn rejects_near_misses_and_unowned_markdown_before_mutation() {
    let t15 = tid(15);
    let near_misses: &[(&str, String)] = &[
        // (subpath under receipts or snapshots, relative path of bad file)
        ("receipts", "evidence.md".to_string()),
        ("receipts", "plan.md".to_string()),
        (
            "receipts",
            format!("{t15}/g1-c0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/outputs/short.bin"),
        ),
        (
            "snapshots",
            format!("{t15}/g1-c0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/invalid-phase.snapshot.tsv"),
        ),
        (
            "snapshots",
            format!("{t15}/g0-c0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef/pre-implement.snapshot.tsv"),
        ),
        (
            "snapshots",
            format!("{t15}/g1-c0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF/pre-implement.snapshot.tsv"),
        ),
    ];

    for (root_kind, bad_rel) in near_misses {
        let temp = init_repo();
        // Ensure both root directories exist so missing-root error doesn't preempt ownership validation
        let r_root = temp.path().join(".dev/pipeline/receipts/fixture-plan");
        let s_root = temp.path().join(".dev/pipeline/snapshots/fixture-plan");
        fs::create_dir_all(&r_root).unwrap();
        fs::create_dir_all(&s_root).unwrap();

        let target_root = temp
            .path()
            .join(format!(".dev/pipeline/{root_kind}/fixture-plan"));
        let bad_path = target_root.join(bad_rel);
        fs::create_dir_all(bad_path.parent().unwrap()).unwrap();
        fs::write(&bad_path, b"bad\n").unwrap();

        let before_tree = snapshot_plan_roots(temp.path(), "fixture-plan");
        let output = cleanup(temp.path());
        let after_tree = snapshot_plan_roots(temp.path(), "fixture-plan");
        let combined = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(
            !output.status.success(),
            "expected failure for near-miss {root_kind}/{bad_rel}"
        );
        assert!(
            combined.contains("cleanup ownership"),
            "expected 'cleanup ownership' for near-miss {root_kind}/{bad_rel}, got:\n{combined}"
        );
        assert!(
            bad_path.exists(),
            "near-miss fixture {bad_path:?} was mutated or deleted!"
        );
        assert_eq!(
            before_tree, after_tree,
            "near-miss fixture {root_kind}/{bad_rel} must leave both plan roots byte-identical"
        );
        let fail_receipt_path = temp
            .path()
            .join(".dev/pipeline/cleanup/fixture-plan.receipt.md");
        assert!(
            fail_receipt_path.exists(),
            "fail receipt must be written for near-miss failure"
        );
        let fail_receipt = fs::read_to_string(&fail_receipt_path).unwrap();
        assert!(
            fail_receipt.contains("overall: fail"),
            "near-miss failure receipt must contain 'overall: fail', got:\n{fail_receipt}"
        );
    }
}

#[test]
fn injected_failure_writes_fail_receipt_with_overall_fail() {
    let labels = ["after-journal", "before-quarantine", "before-removal"];
    for label in labels {
        let temp = fixture();
        let interrupted = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(temp.path())
            .env("GAL_TEST_FIRST_CLEANUP_FAIL_AT", label)
            .args([
                "test-first-cleanup",
                "run",
                "--plan",
                "fixture-plan",
                "--finalized",
                "--receipt",
                ".dev/pipeline/cleanup/fixture-plan.receipt.md",
            ])
            .output()
            .unwrap();
        assert!(
            !interrupted.status.success(),
            "injected failure at '{label}' must exit non-zero"
        );
        let fail_receipt_path = temp
            .path()
            .join(".dev/pipeline/cleanup/fixture-plan.receipt.md");
        assert!(
            fail_receipt_path.exists(),
            "fail receipt must be written on injected failure at '{label}'"
        );
        let fail_receipt = fs::read_to_string(&fail_receipt_path).unwrap();
        assert!(
            fail_receipt.contains("overall: fail"),
            "injected failure receipt at '{label}' must contain 'overall: fail', got:\n{fail_receipt}"
        );
    }
}

/// `--receipt` is a path the operator supplies, and it used to be joined onto
/// the repository root with no validation. `Path::join` drops the base for an
/// absolute path, so that join wrote outside the repository and created the
/// parent directories on the way.
#[test]
fn receipt_path_outside_the_repository_is_rejected_without_writing() {
    let temp = fixture();
    let outside = TempDir::new().unwrap();
    let escape = outside.path().join("stolen/cleanup.receipt.md");
    let before = snapshot_plan_roots(temp.path(), "fixture-plan");

    let output = run(
        temp.path(),
        &[
            "test-first-cleanup",
            "run",
            "--plan",
            "fixture-plan",
            "--finalized",
            "--receipt",
            &escape.to_string_lossy(),
        ],
    );

    assert!(
        !output.status.success(),
        "an absolute receipt path must be rejected"
    );
    assert!(
        !escape.exists(),
        "no file may be written outside the repository"
    );
    assert!(
        !escape.parent().unwrap().exists(),
        "no directory may be created outside the repository"
    );
    assert_eq!(
        snapshot_plan_roots(temp.path(), "fixture-plan"),
        before,
        "a rejected receipt path must leave the plan roots untouched"
    );
}

/// The same hole is reachable relatively. A `..` component walks out of the
/// tree even though the string is not absolute.
#[test]
fn receipt_path_traversal_is_rejected_without_writing() {
    let temp = fixture();
    let before = snapshot_plan_roots(temp.path(), "fixture-plan");

    let output = run(
        temp.path(),
        &[
            "test-first-cleanup",
            "run",
            "--plan",
            "fixture-plan",
            "--finalized",
            "--receipt",
            "../escaped/cleanup.receipt.md",
        ],
    );

    assert!(
        !output.status.success(),
        "a traversing receipt path must be rejected"
    );
    assert!(
        !temp.path().parent().unwrap().join("escaped").exists(),
        "no directory may be created above the repository"
    );
    assert_eq!(
        snapshot_plan_roots(temp.path(), "fixture-plan"),
        before,
        "a rejected receipt path must leave the plan roots untouched"
    );
}
