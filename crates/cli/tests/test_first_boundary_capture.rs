//! `cargo test -p gal-cli --test test_first_boundary_capture`
//!
//! Integration tests for safe snapshot capture, atomic publication,
//! boundary kinds, and R19 transition journal digest binding.

use sha2::{Digest, Sha256};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn git(repo: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn create_temp_repo() -> TempDir {
    let temp = tempdir().expect("fixture tempdir");
    git(temp.path(), &["init", "-q", "-b", "main"]);
    git(
        temp.path(),
        &["config", "user.email", "fixture@example.com"],
    );
    git(temp.path(), &["config", "user.name", "CLI Fixture"]);
    git(temp.path(), &["config", "commit.gpgsign", "false"]);
    fs::write(
        temp.path().join(".gitignore"),
        ".dev/pipeline/\n.dev/executor-smoke/\n",
    )
    .unwrap();
    fs::write(temp.path().join("README.md"), "init\n").unwrap();
    git(temp.path(), &["add", ".gitignore", "README.md"]);
    git(temp.path(), &["commit", "-q", "-m", "init"]);
    temp
}

fn create_marked_prompt(slug: &str, task: &str, file_path: &str) -> String {
    format!(
        "# Plan Prompt: {slug}\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {task} — Do work in `{file_path}`.\n  - Test-first: required\n  - Seam: `src/lib.rs`\n  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=dGVzdA — desc`\n  - Production Paths: `{file_path}`\n  - Test Paths: `tests/test.rs`\n  - Scaffold: not-required\n\n## Files to Create or Modify\n\n- `{file_path}`\n"
    )
}

fn create_markerless_prompt(slug: &str, task: &str, file_path: &str) -> String {
    format!(
        "# Plan Prompt: {slug}\n\n## Tasks\n\n- [ ] {task} — Do work in `{file_path}`.\n\n## Files to Create or Modify\n\n- `{file_path}`\n"
    )
}

fn write_journal_record(
    repo_root: &Path,
    slug: &str,
    state: &str,
    kind: &str,
    old_digest: &str,
    new_digest: &str,
) {
    let journal_dir = repo_root
        .join(".dev")
        .join("pipeline")
        .join("journal")
        .join(slug);
    fs::create_dir_all(&journal_dir).unwrap();
    let journal_file = journal_dir.join("transition.journal.tsv");
    let line = format!("{state}\t{kind}\t{old_digest}\t{new_digest}\n");
    fs::write(journal_file, line).unwrap();
}

fn run_boundary_check(
    repo: &Path,
    prompt_path: &Path,
    task: &str,
    receipt_path: &Path,
    extra_args: &[&str],
) -> (i32, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
    cmd.current_dir(repo)
        .arg("boundary-check")
        .arg(prompt_path)
        .arg("--task")
        .arg(task)
        .arg("--receipt")
        .arg(receipt_path);

    for arg in extra_args {
        cmd.arg(arg);
    }

    let output = cmd.output().expect("failed to execute gal binary");
    let code = output.status.code().expect("process terminated by signal");
    let receipt_text = fs::read_to_string(receipt_path).unwrap_or_default();
    if code != 0 {
        eprintln!(
            "Command failed (code {code}):\nSTDOUT:\n{}\nSTDERR:\n{}\nRECEIPT:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
            receipt_text
        );
    }
    (code, receipt_text)
}

#[test]
fn test_marked_prompt_valid_binding_passes() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "valid-marked-slug";
    let task = tid(1);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_marked_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let prompt_sha256 = format!("{:x}", Sha256::digest(prompt_content.as_bytes()));
    write_journal_record(repo, slug, "committed", "init", "-", &prompt_sha256);

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "implementation-commit"],
    );

    assert_eq!(code, 0, "valid marked prompt boundary-check should exit 0");
    assert!(receipt_text.contains("overall: pass"));
    assert!(receipt_text.contains("marker_state: marked"));
    assert!(receipt_text.contains(&prompt_sha256));
}

#[test]
fn test_marked_prompt_no_journal_fails() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "no-journal-slug";
    let task = tid(2);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_marked_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_ne!(code, 0, "marked prompt with no journal must fail");
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains("marked prompt has no committed transition journal record"));
}

#[test]
fn test_marked_prompt_prepared_record_fails() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "prepared-slug";
    let task = tid(3);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_marked_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let prompt_sha256 = format!("{:x}", Sha256::digest(prompt_content.as_bytes()));
    write_journal_record(repo, slug, "prepared", "init", "-", &prompt_sha256);

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_ne!(
        code, 0,
        "prepared transition record must fail boundary-check"
    );
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains("prepared transition record"));
}

#[test]
fn test_marked_prompt_direct_write_digest_mismatch_fails() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "direct-write-slug";
    let task = tid(4);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_marked_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    write_journal_record(
        repo,
        slug,
        "committed",
        "init",
        "-",
        "0000000000000000000000000000000000000000000000000000000000000000",
    );

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_ne!(code, 0, "digest mismatch must fail boundary-check");
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains("digest mismatch"));
}

#[test]
fn test_markerless_prompt_reaches_inherited_verdict() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "legacy-slug";
    let task = tid(5);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_eq!(
        code, 0,
        "markerless prompt must reach inherited pass verdict"
    );
    assert!(receipt_text.contains("overall: pass"));
    assert!(receipt_text.contains("marker_state: markerless"));
    assert!(receipt_text.contains("inherited verdict unchanged"));
}

#[test]
fn test_early_state_md_change_fails_at_implementation_boundary() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "state-early-slug";
    let task = tid(6);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::write(repo.join(".dev/state.md"), "dirty state\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "implementation-commit"],
    );

    assert_ne!(
        code, 0,
        "early .dev/state.md change must fail at implementation boundary"
    );
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains(".dev/state.md"));
}

#[test]
fn test_default_boundary_kind_is_strictest_boundary() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "default-kind-slug";
    let task = tid(14);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::write(repo.join(".dev/state.md"), "dirty state\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_ne!(
        code, 0,
        ".dev/state.md must fail under the strict default without --boundary-kind"
    );
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains("boundary_kind: implementation-commit"));
    assert!(receipt_text.contains(".dev/state.md (rejected)"));
}

#[test]
fn test_unknown_boundary_kind_is_rejected_before_checks_run() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "unknown-boundary-kind";
    let task = tid(20);
    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(
        &prompt_path,
        create_markerless_prompt(slug, &task, "src/lib.rs"),
    )
    .unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "unknown"],
    );

    assert_ne!(code, 0);
    assert!(receipt_text.is_empty(), "unknown kind must run no checks");
}

#[test]
fn test_boundary_kind_exemption_matrix() {
    let kinds = [
        "implementation-commit",
        "state-recording",
        "post-test",
        "post-audit",
    ];

    for (index, kind) in kinds.iter().enumerate() {
        let temp = create_temp_repo();
        let repo = temp.path();
        let slug = format!("matrix-{kind}");
        let task = tid(21 + index as u32);
        let plans_dir = repo.join(".dev/plans");
        fs::create_dir_all(&plans_dir).unwrap();
        fs::create_dir_all(repo.join("src")).unwrap();
        fs::write(repo.join("src/lib.rs"), "pub fn foo() {}\n").unwrap();

        let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
        fs::write(
            &prompt_path,
            create_markerless_prompt(&slug, &task, "src/lib.rs"),
        )
        .unwrap();
        fs::write(plans_dir.join(format!("{slug}.md")), "active plan\n").unwrap();
        if *kind == "state-recording" {
            fs::write(repo.join(".dev/state.md"), "state\n").unwrap();
        }

        let active_receipt = repo.join("active-receipt.md");
        let (active_code, active_receipt_text) = run_boundary_check(
            repo,
            &prompt_path,
            &task,
            &active_receipt,
            &["--boundary-kind", kind],
        );
        assert_eq!(active_code, 0, "active surfaces must pass at {kind}");
        assert!(active_receipt_text.contains(&format!("boundary_kind: {kind}")));
        assert!(active_receipt_text.contains(&format!(
            ".dev/plans/{slug}.md (accepted), .dev/plans/{slug}.prompt.md (accepted)"
        )));

        fs::write(plans_dir.join("sibling.md"), "sibling plan\n").unwrap();
        fs::write(
            plans_dir.join("sibling.prompt.md"),
            create_markerless_prompt("sibling", &task, "src/lib.rs"),
        )
        .unwrap();
        let sibling_path = plans_dir.join("sibling.prompt.md");
        let sibling_receipt = repo.join("sibling-receipt.md");
        let (sibling_code, sibling_receipt_text) = run_boundary_check(
            repo,
            &sibling_path,
            &task,
            &sibling_receipt,
            &["--boundary-kind", kind],
        );
        assert_ne!(sibling_code, 0, "sibling surfaces must fail at {kind}");
        assert!(sibling_receipt_text.contains("overall: fail"));
    }
}

#[test]
fn test_state_md_change_passes_at_state_recording_boundary() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "state-record-slug";
    let task = tid(7);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::write(repo.join(".dev/state.md"), "dirty state\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "state-recording"],
    );

    assert_eq!(
        code, 0,
        ".dev/state.md change must pass at state-recording boundary"
    );
    assert!(receipt_text.contains("overall: pass"));
}

#[test]
fn test_empty_allowlist_remains_not_run() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "empty-allowlist-slug";
    let task = tid(8);

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = format!("# Plan Prompt: {slug}\n\n## Tasks\n\n- [ ] {task} — No files.\n");
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_ne!(code, 0, "empty allowlist must return NotRun non-pass");
    assert!(receipt_text.contains("overall: not-run"));
    assert!(receipt_text.contains("refusing to allow-all"));
}

#[test]
fn test_boundary_check_does_not_capture_snapshots() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "no-capture";
    let task = tid(9);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();
    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();
    let prompt_content = create_marked_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();
    let prompt_sha256 = format!("{:x}", Sha256::digest(prompt_content.as_bytes()));
    write_journal_record(repo, slug, "committed", "init", "-", &prompt_sha256);

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--phase", "pre-test"],
    );

    assert_eq!(code, 0);
    assert!(receipt_text.contains("base_head:"));
    assert!(!repo.join(".dev/pipeline/snapshots").exists());
}

#[test]
fn test_foreign_plan_rejection_inherited() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "active-plan";
    let other_slug = "other-plan";
    let task = tid(10);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::write(plans_dir.join(format!("{other_slug}.md")), "other plan\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_ne!(code, 0, "foreign plan modification must fail");
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains(other_slug));
}

#[test]
fn test_dirty_non_exempt_file_fails_boundary() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "dirty-non-exempt";
    let task = tid(11);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();
    git(repo, &["add", prod_file]);
    git(repo, &["commit", "-q", "-m", "add prod"]);

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::write(repo.join("src/other.rs"), "unexpected\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "implementation-commit"],
    );

    assert_ne!(code, 0, "dirty non-exempt file must fail boundary");
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains("src/other.rs"));
}

#[test]
fn test_git_failure_in_non_repo() {
    let temp = tempdir().expect("fixture tempdir");
    let repo = temp.path();
    let slug = "non-repo";
    let task = tid(12);

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = format!("# Plan Prompt: {slug}\n\n## Tasks\n\n- [ ] {task} — No files.\n");
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let receipt_path = repo.join("receipt.md");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
    cmd.current_dir(repo)
        .arg("boundary-check")
        .arg(&prompt_path)
        .arg("--task")
        .arg(&task)
        .arg("--receipt")
        .arg(&receipt_path);

    let output = cmd.output().expect("failed to execute gal binary");
    let code = output.status.code().expect("process terminated");
    let receipt_text = fs::read_to_string(&receipt_path).unwrap_or_default();

    assert_ne!(code, 0, "non-git directory must fail");
    assert!(
        receipt_text.contains("fail") || receipt_text.contains("not-run"),
        "receipt must indicate failure or not-run"
    );
}

#[test]
fn test_changed_files_failure_fails_closed_not_pass() {
    // A non-empty allowlist forces the boundary-allowlist check past the
    // NotRun short-circuit and into changed_files(), where a non-git
    // directory makes both `git diff` and `git status` fail. Before the
    // FINDING-007 fix, changed_files() silently swallowed that failure and
    // returned an empty Vec, which check_boundary_with_options treated as
    // "no changed files — pass". This asserts the fail-closed behavior.
    let temp = tempdir().expect("fixture tempdir");
    let repo = temp.path();
    let slug = "non-repo-nonempty-allowlist";
    let task = tid(13);

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content =
        format!("# Plan Prompt: {slug}\n\n## Tasks\n\n- [ ] {task} — Touch `src/lib.rs`.\n");
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let receipt_path = repo.join("receipt.md");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
    cmd.current_dir(repo)
        .arg("boundary-check")
        .arg(&prompt_path)
        .arg("--task")
        .arg(&task)
        .arg("--receipt")
        .arg(&receipt_path);

    let output = cmd.output().expect("failed to execute gal binary");
    let code = output.status.code().expect("process terminated");
    let receipt_text = fs::read_to_string(&receipt_path).unwrap_or_default();

    assert_ne!(
        code, 0,
        "a Git enumeration failure must not pass the boundary gate"
    );
    assert!(
        receipt_text.contains("overall: fail"),
        "receipt overall should be fail, got: {receipt_text}"
    );
    assert!(
        receipt_text.contains("git uncertainty"),
        "receipt summary should name the Git uncertainty, got: {receipt_text}"
    );
    assert!(
        !receipt_text.contains("no changed files"),
        "must not silently degrade to the empty-changeset pass path"
    );
}

#[test]
fn test_ambiguous_journal_record_fails() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "ambiguous-journal";
    let task = tid(13);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_marked_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let journal_dir = repo
        .join(".dev")
        .join("pipeline")
        .join("journal")
        .join(slug);
    fs::create_dir_all(&journal_dir).unwrap();
    let journal_file = journal_dir.join("transition.journal.tsv");
    fs::write(
        &journal_file,
        "badstate\tinit\t-\t0000000000000000000000000000000000000000000000000000000000000000\n",
    )
    .unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);

    assert_ne!(code, 0, "ambiguous journal record must fail");
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains("ambiguous"));
}

#[test]
fn test_post_test_boundary_rejects_state_md() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "post-test-state";
    let task = tid(14);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::write(repo.join(".dev/state.md"), "dirty state\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "post-test"],
    );

    assert_ne!(
        code, 0,
        ".dev/state.md change must fail at post-test boundary"
    );
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains(".dev/state.md"));
}

#[test]
fn test_post_audit_boundary_rejects_state_md() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "post-audit-state";
    let task = tid(15);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::write(repo.join(".dev/state.md"), "dirty state\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "post-audit"],
    );

    assert_ne!(
        code, 0,
        ".dev/state.md change must fail at post-audit boundary"
    );
    assert!(receipt_text.contains("overall: fail"));
    assert!(receipt_text.contains(".dev/state.md"));
}

#[test]
fn test_receipt_names_exempt_paths_and_boundary_kind() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "receipt-detail";
    let task = tid(18);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "implementation-commit"],
    );

    assert_eq!(code, 0);
    assert!(receipt_text.contains("boundary_kind: implementation-commit"));
    assert!(receipt_text.contains("marker_state: markerless"));
    assert!(receipt_text.contains("current_prompt_digest:"));
    assert!(receipt_text.contains("matched_committed_journal_digest:"));
    assert!(receipt_text.contains("exempt_paths:"));
    assert!(receipt_text.contains(".dev/state.md (rejected)"));
}

#[test]
fn test_state_recording_receipt_shows_accepted() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "receipt-state-accept";
    let task = tid(19);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_markerless_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "state-recording"],
    );

    assert_eq!(code, 0);
    assert!(receipt_text.contains(".dev/state.md (accepted)"));
}

#[test]
fn test_symlinked_transition_journal_path_fails_boundary() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "symlink-journal-slug";
    let task = tid(22);
    let prod_file = "src/lib.rs";

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join(prod_file), "pub fn foo() {}\n").unwrap();

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = create_marked_prompt(slug, &task, prod_file);
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    let journal_dir = repo
        .join(".dev")
        .join("pipeline")
        .join("journal")
        .join(slug);
    fs::create_dir_all(&journal_dir).unwrap();

    // Create target file outside journal
    let target_file = repo.join("target.tsv");
    let prompt_sha256 = format!("{:x}", Sha256::digest(prompt_content.as_bytes()));
    let line = format!("committed\tinit\t-\t{prompt_sha256}\n");
    fs::write(&target_file, &line).unwrap();

    let journal_file = journal_dir.join("transition.journal.tsv");

    #[cfg(unix)]
    let symlink_res = std::os::unix::fs::symlink(&target_file, &journal_file);
    #[cfg(windows)]
    let symlink_res = std::os::windows::fs::symlink_file(&target_file, &journal_file);

    if symlink_res.is_ok() {
        let receipt_path = repo.join("receipt.md");
        let (code, receipt_text) =
            run_boundary_check(repo, &prompt_path, &task, &receipt_path, &[]);
        assert_ne!(
            code, 0,
            "symlinked transition journal file must fail boundary check"
        );
        assert!(receipt_text.contains("ambiguous transition journal record"));
    }
}

#[test]
fn test_path_extractor_names_repository_root_dotfile() {
    let temp = create_temp_repo();
    let repo = temp.path();
    let slug = "dotfile-allowlist-slug";
    let task = tid(32);

    let plans_dir = repo.join(".dev/plans");
    fs::create_dir_all(&plans_dir).unwrap();

    let prompt_content = format!(
        "# Plan Prompt: {slug}\n\n## Tasks\n\n- [ ] {task} — Touch `src/lib.rs` and `.gitattributes`.\n\n## Files to Create or Modify\n\n- `src/lib.rs`\n- `.gitattributes`\n"
    );
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_content).unwrap();

    fs::create_dir_all(repo.join("src")).unwrap();
    fs::write(repo.join("src/lib.rs"), "pub fn foo() {}\n").unwrap();
    fs::write(repo.join(".gitattributes"), "*.rs text\n").unwrap();

    let receipt_path = repo.join("receipt.md");
    let (code, receipt_text) = run_boundary_check(
        repo,
        &prompt_path,
        &task,
        &receipt_path,
        &["--boundary-kind", "implementation-commit"],
    );

    assert_eq!(
        code, 0,
        "repository root dotfile should be accepted in task spec allowlist, receipt:\n{receipt_text}"
    );
    assert!(receipt_text.contains("overall: pass"));
}
