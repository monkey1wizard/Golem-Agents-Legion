//! `cargo test -p gal-cli --test test_first_boundary_snapshot`
//!
//! Integration probes for transactional snapshot capture and read-only verify.

use sha2::{Digest, Sha256};
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
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture() -> (TempDir, PathBuf, String) {
    let temp = tempdir().expect("fixture tempdir");
    let repo = temp.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "fixture@example.com"]);
    git(repo, &["config", "user.name", "CLI Fixture"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    fs::create_dir_all(repo.join("src")).unwrap();
    fs::create_dir_all(repo.join("tests")).unwrap();
    let root_gitignore = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".gitignore");
    let gitignore_content = fs::read_to_string(&root_gitignore).expect("must read root .gitignore");
    fs::write(repo.join(".gitignore"), gitignore_content).unwrap();
    fs::write(repo.join("README.md"), "init\n").unwrap();
    fs::write(repo.join("src/lib.rs"), "production-v1\n").unwrap();
    fs::write(repo.join("tests/test.rs"), "test-v1\n").unwrap();
    git(repo, &["add", "."]);
    git(repo, &["commit", "-q", "-m", "init"]);

    let slug = "snapshot-fixture";
    let task = format!("T-{:02}", 45u32);
    let prompt = format!(
        "# Plan Prompt: {slug}\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {task} — Snapshot `{}`.\n  - Test-first: required\n  - Seam: `src/lib.rs`\n  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=dGVzdA — snapshot failure`\n  - Production Paths: `src/lib.rs`\n  - Test Paths: `tests/test.rs`\n  - Scaffold: not-required\n\n## Files to Create or Modify\n\n- `src/lib.rs`\n- `tests/test.rs`\n",
        "src/lib.rs"
    );
    let prompt_path = repo.join(".dev/plans").join(format!("{slug}.prompt.md"));
    fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
    fs::write(&prompt_path, prompt).unwrap();
    (temp, prompt_path, task)
}

fn run_boundary(repo: &Path, prompt: &Path, task: &str, args: &[&str]) -> (i32, String) {
    let receipt = repo.join("receipt.md");
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("boundary-check")
        .arg(prompt)
        .arg("--task")
        .arg(task)
        .arg("--receipt")
        .arg(&receipt)
        .args(args)
        .output()
        .expect("boundary-check must run");
    let text = fs::read_to_string(&receipt).unwrap_or_default();
    let _ = fs::remove_file(&receipt);
    (output.status.code().unwrap_or(-1), text)
}

fn snapshot_task_root(repo: &Path) -> PathBuf {
    repo.join(".dev/pipeline/snapshots/snapshot-fixture")
        .join(format!("T-{:02}", 45u32))
}

fn generation_dir(repo: &Path, generation: u32) -> PathBuf {
    let root = snapshot_task_root(repo);
    fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with(&format!("g{generation}-c")))
        })
        .expect("generation directory must exist")
}

fn capture(repo: &Path, prompt: &Path, task: &str, generation: u32, phase: &str) -> (i32, String) {
    run_boundary(
        repo,
        prompt,
        task,
        &[
            "--capture",
            "--phase",
            phase,
            "--generation",
            &generation.to_string(),
        ],
    )
}

fn restore(repo: &Path, prompt: &Path, task: &str, generation: u32, phase: &str) -> (i32, String) {
    run_boundary(
        repo,
        prompt,
        task,
        &[
            "--restore",
            "--phase",
            phase,
            "--generation",
            &generation.to_string(),
        ],
    )
}

#[test]
fn changed_capture_advances_and_identical_capture_is_idempotent() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();

    assert_eq!(capture(repo, &prompt, &task, 1, "pre-implement").0, 0);
    let first = generation_dir(repo, 1);
    let first_manifest = fs::read(first.join("pre-implement.snapshot.tsv")).unwrap();

    assert_eq!(capture(repo, &prompt, &task, 1, "pre-implement").0, 0);
    assert_eq!(
        fs::read(first.join("pre-implement.snapshot.tsv")).unwrap(),
        first_manifest
    );
    assert_eq!(
        fs::read_dir(snapshot_task_root(repo)).unwrap().count(),
        1,
        "identical capture must not replace or add a generation"
    );

    fs::write(repo.join("src/lib.rs"), "production-v2\n").unwrap();
    assert_eq!(capture(repo, &prompt, &task, 1, "pre-implement").0, 0);
    assert!(generation_dir(repo, 2).exists());
}

#[test]
fn capture_restore_and_verify_round_trip_through_real_commands() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    let original = fs::read(repo.join("src/lib.rs")).unwrap();

    assert_eq!(capture(repo, &prompt, &task, 1, "post-test").0, 0);
    fs::write(repo.join("src/lib.rs"), b"changed-before-restore\n").unwrap();

    let (restore_code, restore_receipt) = restore(repo, &prompt, &task, 1, "post-test");
    assert_eq!(restore_code, 0, "{restore_receipt}");
    assert!(
        restore_receipt.contains("verdict=restored"),
        "{restore_receipt}"
    );
    assert_eq!(fs::read(repo.join("src/lib.rs")).unwrap(), original);

    let (verify_code, verify_receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--phase", "post-test", "--generation", "1"],
    );
    assert_ne!(verify_code, -1, "boundary-check must run: {verify_receipt}");
    assert!(
        verify_receipt.contains("| snapshot-verify | pass |"),
        "{verify_receipt}"
    );
}

#[test]
fn injected_publication_failure_leaves_no_generation_or_staging() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .env("GAL_TEST_FIRST_SNAPSHOT_FAIL_AFTER_BLOB", "1")
        .arg("boundary-check")
        .arg(&prompt)
        .args([
            "--task",
            &task,
            "--capture",
            "--phase",
            "pre-implement",
            "--generation",
            "1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(
        !snapshot_task_root(repo).exists()
            || fs::read_dir(snapshot_task_root(repo))
                .map(|entries| entries.count() == 0)
                .unwrap_or(true)
    );
    // The real staging directory name embeds the process id
    // (`.staging-g<generation>-<pid>`), so assert no `.staging-*` prefixed
    // entry survives rather than matching one fixed, never-real name.
    let no_staging_survives = fs::read_dir(snapshot_task_root(repo))
        .map(|entries| {
            !entries
                .filter_map(Result::ok)
                .any(|e| e.file_name().to_string_lossy().starts_with(".staging-"))
        })
        .unwrap_or(true);
    assert!(
        no_staging_survives,
        "no staging directory must survive a failed capture"
    );
}

#[test]
fn invalid_phase_is_rejected_before_write() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    let (code, _) = capture(repo, &prompt, &task, 1, "invalid-phase");
    assert_ne!(code, 0);
    assert!(!snapshot_task_root(repo).exists());
}

#[test]
fn final_capture_contains_production_and_test_paths() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    assert_eq!(capture(repo, &prompt, &task, 1, "post-test").0, 0);
    let manifest =
        fs::read_to_string(generation_dir(repo, 1).join("post-test.snapshot.tsv")).unwrap();
    assert!(manifest.contains("src/lib.rs"));
    assert!(manifest.contains("tests/test.rs"));
}

#[test]
fn markerless_capture_and_verify_write_nothing() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    let mut markerless = fs::read_to_string(&prompt).unwrap();
    markerless = markerless.replace("Pipeline Contract: test-first-v1\n\n", "");
    fs::write(&prompt, markerless).unwrap();

    let before = fs::read_dir(repo.join(".dev")).unwrap().count();
    assert_ne!(capture(repo, &prompt, &task, 1, "post-test").0, 0);
    let (code, receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--phase", "post-test", "--generation", "1"],
    );
    assert_eq!(code, 0);
    assert!(receipt.contains("snapshot verification not requested"));
    assert_eq!(fs::read_dir(repo.join(".dev")).unwrap().count(), before);
    assert!(!snapshot_task_root(repo).exists());
}

#[test]
fn verify_rejects_tampered_manifest_blob_and_identity() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    assert_eq!(capture(repo, &prompt, &task, 1, "post-test").0, 0);
    let generation = generation_dir(repo, 1);
    let manifest_path = generation.join("post-test.snapshot.tsv");
    let original_manifest = fs::read_to_string(&manifest_path).unwrap();

    fs::write(
        &manifest_path,
        original_manifest.replace("present", "missing"),
    )
    .unwrap();
    let (_, receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--phase", "post-test", "--generation", "1"],
    );
    assert!(receipt.contains("snapshot-verify"));
    assert!(receipt.contains("fail"));

    fs::write(&manifest_path, original_manifest).unwrap();
    let blob = fs::read_dir(generation.join("blobs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::write(&blob, b"tampered").unwrap();
    let (_, receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--phase", "post-test", "--generation", "1"],
    );
    assert!(receipt.contains("snapshot-verify"));
    assert!(receipt.contains("fail"));
}

#[test]
fn verify_rejects_generation_contract_and_phase_mismatch_without_mutation() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    assert_eq!(capture(repo, &prompt, &task, 1, "post-test").0, 0);
    let before = fs::read_dir(snapshot_task_root(repo)).unwrap().count();

    for args in [
        vec!["--phase", "post-test", "--generation", "2"],
        vec!["--phase", "post-audit", "--generation", "1"],
    ] {
        let (_, receipt) = run_boundary(repo, &prompt, &task, &args);
        assert!(receipt.contains("snapshot-verify"));
        assert!(receipt.contains("fail"));
    }
    assert_eq!(
        fs::read_dir(snapshot_task_root(repo)).unwrap().count(),
        before
    );
}

#[cfg(unix)]
#[test]
fn verify_rejects_mode_change_and_link_swap() {
    use std::os::unix::fs::PermissionsExt;

    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    assert_eq!(capture(repo, &prompt, &task, 1, "post-test").0, 0);
    let production = repo.join("src/lib.rs");
    let original_mode = fs::metadata(&production).unwrap().permissions().mode();

    let mut mode = fs::metadata(&production).unwrap().permissions();
    mode.set_mode(original_mode ^ 0o100);
    fs::set_permissions(&production, mode).unwrap();
    let (_, receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--phase", "post-test", "--generation", "1"],
    );
    assert!(receipt.contains("snapshot-verify"));
    assert!(receipt.contains("fail"));

    let mut restored = fs::metadata(&production).unwrap().permissions();
    restored.set_mode(original_mode);
    fs::set_permissions(&production, restored).unwrap();
    fs::remove_file(&production).unwrap();
    std::os::unix::fs::symlink(repo.join("tests/test.rs"), &production).unwrap();
    let (_, receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--phase", "post-test", "--generation", "1"],
    );
    assert!(receipt.contains("snapshot-verify"));
    assert!(receipt.contains("fail"));
}

#[test]
fn verify_rejects_contract_digest_mismatch_without_mutation() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();
    assert_eq!(capture(repo, &prompt, &task, 1, "post-test").0, 0);
    let before = fs::read_dir(snapshot_task_root(repo)).unwrap().count();
    let original = fs::read_to_string(&prompt).unwrap();
    fs::write(
        &prompt,
        original.replace("Seam: `src/lib.rs`", "Seam: `src/other.rs`"),
    )
    .unwrap();

    let (_, receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--phase", "post-test", "--generation", "1"],
    );
    assert!(receipt.contains("snapshot-verify"));
    assert!(receipt.contains("fail"));
    assert_eq!(
        fs::read_dir(snapshot_task_root(repo)).unwrap().count(),
        before
    );
}

#[test]
fn reusable_marked_digest_evaluator_exposes_structured_digest_mismatch() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();

    let journal_dir = repo.join(".dev/pipeline/journal/snapshot-fixture");
    fs::create_dir_all(&journal_dir).unwrap();
    let journal_file = journal_dir.join("transition.journal.tsv");
    fs::write(
        &journal_file,
        "committed\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000\n",
    )
    .unwrap();

    let receipt_path = repo.join("receipt.md");
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("boundary-check")
        .arg(&prompt)
        .arg("--task")
        .arg(&task)
        .arg("--receipt")
        .arg(&receipt_path)
        .output()
        .expect("boundary-check must run");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
    let combined = format!("{stdout}\n{stderr}\n{receipt}");

    assert!(
        combined.contains("digest_result: digest-mismatch")
            || combined.contains("marked_digest_status: digest-mismatch")
            || combined.contains("marked_digest_result: digest-mismatch"),
        "reusable marked digest evaluator must report structured digest-mismatch result: combined output was:\n{combined}"
    );
}

#[test]
fn gitignore_tracks_only_transition_journals_under_pipeline_tree() {
    let temp = tempdir().expect("fixture tempdir");
    let repo = temp.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "fixture@example.com"]);
    git(repo, &["config", "user.name", "CLI Fixture"]);

    let root_gitignore = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join(".gitignore");
    let gitignore_content = fs::read_to_string(&root_gitignore).expect("must read root .gitignore");
    fs::write(repo.join(".gitignore"), gitignore_content).unwrap();

    let slug = "test-slug";
    let journal_dir = repo.join(".dev/pipeline/journal").join(slug);
    let locks_dir = repo.join(".dev/pipeline/locks");
    let backups_dir = repo.join(".dev/pipeline/backups");
    let receipts_dir = repo.join(".dev/pipeline/receipts");
    let snapshots_dir = repo.join(".dev/pipeline/snapshots");
    let cleanup_dir = repo.join(".dev/pipeline/cleanup");
    let quarantine_dir = repo.join(".dev/pipeline/quarantine");

    fs::create_dir_all(&journal_dir).unwrap();
    fs::create_dir_all(&locks_dir).unwrap();
    fs::create_dir_all(&backups_dir).unwrap();
    fs::create_dir_all(&receipts_dir).unwrap();
    fs::create_dir_all(&snapshots_dir).unwrap();
    fs::create_dir_all(&cleanup_dir).unwrap();
    fs::create_dir_all(&quarantine_dir).unwrap();

    let transition_journal = journal_dir.join("transition.journal.tsv");
    let cleanup_journal = journal_dir.join("cleanup.journal.tsv");
    let lock_file = locks_dir.join("test.lock");
    let backup_file = backups_dir.join("test.bak");
    let receipt_file = receipts_dir.join("test.receipt.md");
    let snapshot_file = snapshots_dir.join("test.snap");
    let cleanup_file = cleanup_dir.join("test.log");
    let quarantine_file = quarantine_dir.join("test.entry");

    fs::write(&transition_journal, "state\tinit\n").unwrap();
    fs::write(&cleanup_journal, "cleanup\n").unwrap();
    fs::write(&lock_file, "lock\n").unwrap();
    fs::write(&backup_file, "backup\n").unwrap();
    fs::write(&receipt_file, "receipt\n").unwrap();
    fs::write(&snapshot_file, "snapshot\n").unwrap();
    fs::write(&cleanup_file, "cleanup\n").unwrap();
    fs::write(&quarantine_file, "quarantine\n").unwrap();

    let check_ignore_output = Command::new("git")
        .current_dir(repo)
        .args([
            "check-ignore",
            ".dev/pipeline/journal/test-slug/transition.journal.tsv",
        ])
        .output()
        .expect("git check-ignore must run");

    assert!(
        !check_ignore_output.status.success(),
        "transition.journal.tsv must be trackable and not ignored by .gitignore, but git check-ignore succeeded: {}",
        String::from_utf8_lossy(&check_ignore_output.stdout)
    );

    git(repo, &["add", "."]);

    let status_output = Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain"])
        .output()
        .expect("git status must run");
    let status_text = String::from_utf8_lossy(&status_output.stdout);

    let pipeline_staged: Vec<&str> = status_text
        .lines()
        .map(|l| l.trim())
        .filter(|l| l.contains(".dev/pipeline/"))
        .collect();

    assert_eq!(
        pipeline_staged,
        vec!["A  .dev/pipeline/journal/test-slug/transition.journal.tsv"],
        "Only transition.journal.tsv under .dev/pipeline/ should be staged; found: {pipeline_staged:?}"
    );
}

#[test]
fn journal_exemption_is_narrowed_to_running_slug_across_all_boundary_kinds() {
    let (temp, prompt, task) = fixture();
    let repo = temp.path();

    let prompt_sha256 = format!("{:x}", Sha256::digest(fs::read(&prompt).unwrap()));
    let journal_line = format!("committed\tinit\t-\t{prompt_sha256}\n");

    // 1. Same-slug transition journal is allowed under state-recording boundary
    let same_slug_journal_dir = repo.join(".dev/pipeline/journal/snapshot-fixture");
    fs::create_dir_all(&same_slug_journal_dir).unwrap();
    let same_slug_journal = same_slug_journal_dir.join("transition.journal.tsv");
    fs::write(&same_slug_journal, &journal_line).unwrap();

    let (same_code, same_receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--boundary-kind", "state-recording"],
    );
    assert_eq!(
        same_code, 0,
        "running slug transition journal must be exempt at state-recording boundary, receipt:\n{same_receipt}"
    );

    // 2. Another slug's transition journal must be rejected at state-recording boundary
    let other_slug_journal_dir = repo.join(".dev/pipeline/journal/other-slug");
    fs::create_dir_all(&other_slug_journal_dir).unwrap();
    let other_slug_journal = other_slug_journal_dir.join("transition.journal.tsv");
    fs::write(&other_slug_journal, &journal_line).unwrap();

    let (other_code, other_receipt) = run_boundary(
        repo,
        &prompt,
        &task,
        &["--boundary-kind", "state-recording"],
    );
    assert_ne!(
        other_code, 0,
        "another slug transition journal must be rejected at state-recording boundary (same-slug required), receipt:\n{other_receipt}"
    );
    assert!(
        other_receipt.contains("same-slug") || other_receipt.contains("other-slug"),
        "receipt must identify rejected other-slug journal (same-slug required), receipt:\n{other_receipt}"
    );

    // Clean up other_slug_journal for subsequent checks
    fs::remove_file(&other_slug_journal).unwrap();

    // 3. Same-slug transition journal is exempt under every boundary kind, not just
    //    state-recording -- matching .prompt.md's existing unconditional exemption.
    let non_state_recording_kinds = ["implementation-commit", "post-test", "post-audit"];

    for kind in non_state_recording_kinds {
        let (kind_code, kind_receipt) =
            run_boundary(repo, &prompt, &task, &["--boundary-kind", kind]);
        assert_eq!(
            kind_code, 0,
            "running slug transition journal must be exempt at {kind} boundary, receipt:\n{kind_receipt}"
        );
    }

    // 4. Locks, backups, receipts, snapshots, cleanup, quarantine paths gain no exemption
    let forbidden_paths = [
        ".dev/pipeline/locks/test.lock",
        ".dev/pipeline/backups/test.bak",
        ".dev/pipeline/receipts/test.receipt.md",
        ".dev/pipeline/snapshots/test.snap",
        ".dev/pipeline/cleanup/test.log",
        ".dev/pipeline/quarantine/test.entry",
    ];

    // Each artifact is force-added so it genuinely enters the changed set. An
    // ignored file that only sits in the working tree can never be committed,
    // so making one visible is not the boundary's job. The real risk guarded
    // here is one of these paths being staged, and that is what is asserted.
    for path_str in forbidden_paths {
        let p = repo.join(path_str);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "test\n").unwrap();
        git(repo, &["add", "-f", path_str]);

        let (bad_code, bad_receipt) = run_boundary(
            repo,
            &prompt,
            &task,
            &["--boundary-kind", "state-recording"],
        );
        assert_ne!(
            bad_code, 0,
            "{path_str} must be rejected at state-recording boundary, receipt:\n{bad_receipt}"
        );

        git(repo, &["rm", "-f", "--quiet", "--cached", path_str]);
        fs::remove_file(&p).unwrap();
    }
}

fn run_boundary_raw(repo: &Path, prompt: &Path, task: &str, args: &[&str]) -> std::process::Output {
    let receipt = repo.join("receipt.md");
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("boundary-check")
        .arg(prompt)
        .arg("--task")
        .arg(task)
        .arg("--receipt")
        .arg(&receipt)
        .args(args)
        .output()
        .expect("boundary-check must run");
    let _ = fs::remove_file(&receipt);
    output
}

/// Recursively snapshots every regular file under `root` as
/// `(relative path, sha256 hex)`, sorted, so a before/after comparison
/// catches any filesystem side effect anywhere in the tree.
fn snapshot_tree(root: &Path) -> Vec<(String, String)> {
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
    walk(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn snapshot_producer_constrains_identity_grammar() {
    let (temp, prompt, valid_task) = fixture();
    let repo = temp.path();

    let unsafe_slug_prompt = repo.join(".dev/plans/unsafe..slug.prompt.md");
    fs::write(&unsafe_slug_prompt, fs::read_to_string(&prompt).unwrap()).unwrap();

    let invalid_cases = [
        (
            unsafe_slug_prompt.as_path(),
            valid_task.as_str(),
            "1",
            "pre-implement",
            "unsafe plan slug",
        ),
        (
            prompt.as_path(),
            "task-45",
            "1",
            "pre-implement",
            "noncanonical task",
        ),
        (
            prompt.as_path(),
            valid_task.as_str(),
            "0",
            "pre-implement",
            "zero generation",
        ),
        (
            prompt.as_path(),
            valid_task.as_str(),
            "1",
            "test",
            "invalid snapshot phase",
        ),
    ];

    for (prompt_path, task, gen, phase, desc) in invalid_cases {
        let before_tree = snapshot_tree(temp.path());
        let output = run_boundary_raw(
            repo,
            prompt_path,
            task,
            &["--capture", "--phase", phase, "--generation", gen],
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{stdout}\n{stderr}");

        assert!(
            !output.status.success(),
            "invalid identity ({desc}) must fail; output={combined}"
        );
        assert!(
            combined.contains("invalid artifact identity"),
            "invalid identity ({desc}) must report 'invalid artifact identity', got:\n{combined}"
        );

        let after_tree = snapshot_tree(temp.path());
        assert_eq!(
            before_tree, after_tree,
            "invalid identity ({desc}) must leave no staging directory or snapshot artifacts"
        );
    }
}
