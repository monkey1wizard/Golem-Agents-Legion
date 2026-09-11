//! `cargo test -p gal-cli --test test_first_preflight`
//!
//! Integration tests for dual schema preflight validation under `Pipeline Contract: test-first-v1`.

use gal_foundation::validated_repo_path::{
    ValidatedRepoPath, ValidatedRepoPathError, ValidatedRepoPathMode,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

// Compose plan-task IDs at runtime — never hardcode `T-NN` literals in
// source (naming-gate provenance rule: plan-task IDs belong only in .dev).
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
    temp
}

fn write_plan_and_state(repo: &Path, name: &str, prompt_text: &str) -> (PathBuf, PathBuf) {
    let plans_dir = repo.join(".dev").join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    let prompt_path = plans_dir.join(format!("{name}.prompt.md"));
    std::fs::write(&prompt_path, prompt_text).unwrap();

    let state_path = repo.join(".dev").join("state.md");
    let state_text = format!(
        "## Session Continuity\n\n| {name} | .dev/plans/{name}.prompt.md | in progress |\n"
    );
    std::fs::write(&state_path, state_text).unwrap();

    let receipt_path = repo.join(format!("{name}.preflight.receipt.md"));
    (prompt_path, receipt_path)
}

fn run_preflight(repo: &Path, prompt_path: &Path, receipt_path: &Path) -> (i32, String) {
    let output = run_preflight_output(repo, prompt_path, receipt_path);

    let code = output.status.code().expect("process terminated by signal");
    let receipt_text = std::fs::read_to_string(receipt_path).unwrap_or_default();
    (code, receipt_text)
}

fn run_preflight_output(
    repo: &Path,
    prompt_path: &Path,
    receipt_path: &Path,
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("pipeline-preflight")
        .arg(prompt_path)
        .arg("--receipt")
        .arg(receipt_path)
        .output()
        .expect("failed to execute gal binary")
}

fn assert_route_line(stdout: &str) {
    let mut lines = stdout.lines();
    let first = lines.next().unwrap_or_default();
    let segments: Vec<_> = first.split("; ").collect();
    assert!(
        first == "(no executorRouting configured)"
            || (segments.len() == 3
                && ["coder: ", "tester: ", "auditor: "]
                    .iter()
                    .zip(segments)
                    .all(|(prefix, segment)| {
                        segment.starts_with(prefix) && segment.len() > prefix.len()
                    })),
        "preflight stdout line 1 is the route line: {first:?}"
    );
    let second = lines.next().unwrap_or_default();
    assert!(
        second.starts_with("gal pipeline-preflight: "),
        "preflight stdout line 2 is the summary: {second:?}"
    );
}

fn assert_route_line_shape(stdout: &str) {
    let first = stdout.lines().next().unwrap_or_default();
    let segments: Vec<_> = first.split("; ").collect();
    assert!(
        first == "(no executorRouting configured)"
            || (segments.len() == 3
                && ["coder: ", "tester: ", "auditor: "]
                    .iter()
                    .zip(segments)
                    .all(|(prefix, segment)| {
                        segment.starts_with(prefix) && segment.len() > prefix.len()
                    })),
        "preflight stdout line 1 is the route line: {first:?}"
    );
}

#[test]
fn test_first_preflight_accepts_valid_dual_schemas() {
    let temp = create_temp_repo();
    let (t1, t2) = (tid(1), tid(2));
    let prompt_content = format!(
        r#"# Plan Prompt: Test Preflight

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Implement feature with test-first contract.
  - Test-first: required
  - Seam: `crates/cli/src/lib.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/src/lib.rs`
  - Test Paths: `crates/cli/tests/test_feat.rs`
  - Scaffold: required

- [ ] {t2} — Document feature changes.
  - Test-first: not-applicable — Documentation only change with no executable behavior
  - Non-red probe: `cargo test -p gal-cli`

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
| {t2} | test doc |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "valid-dual", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_eq!(code, 0, "valid dual schema preflight should exit 0");
    assert!(
        receipt.contains("overall: pass"),
        "receipt overall should be pass"
    );
    assert!(
        receipt.contains("test-first-contracts"),
        "receipt should include test-first-contracts check"
    );
}

#[test]
fn test_first_preflight_preserves_legacy_schema() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Legacy Preflight

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Legacy task implementation without marker.

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "legacy-schema", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_eq!(code, 0, "legacy schema preflight should exit 0");
    assert!(
        receipt.contains("overall: pass"),
        "receipt overall should be pass"
    );
    assert!(
        !receipt.contains("test-first-contracts"),
        "legacy preflight should not run test-first-contracts check"
    );
}

#[test]
fn test_first_preflight_rejects_missing_required_field() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Missing Seam

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task missing Seam.
  - Test-first: required
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/src/lib.rs`
  - Test Paths: `crates/cli/tests/test_feat.rs`
  - Scaffold: required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "missing-seam", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(code, 0, "missing required field must fail preflight");
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
    assert!(
        receipt.contains("Seam"),
        "receipt summary should mention missing Seam"
    );
}

#[test]
fn test_first_preflight_accepts_same_file_paths() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Overlapping Paths

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task with overlap.
  - Test-first: required
  - Seam: `crates/cli/src/lib.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/src/lib.rs`
  - Test Paths: `crates/cli/src/lib.rs`
  - Scaffold: required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "overlapping-paths", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_eq!(
        code, 0,
        "preflight accepts same-file production and test paths"
    );
    assert!(
        receipt.contains("overall: pass"),
        "receipt overall should be pass"
    );
    assert!(
        receipt.contains("test-first-contracts"),
        "receipt should include test-first-contracts check"
    );
}

#[test]
fn test_first_preflight_rejects_unsafe_paths() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Unsafe Path

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task with unsafe dotted path.
  - Test-first: required
  - Seam: `crates/cli/src/lib.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/../outside.rs`
  - Test Paths: `crates/cli/tests/test_feat.rs`
  - Scaffold: required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "unsafe-path", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(code, 0, "unsafe path must fail preflight");
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
    assert!(
        receipt.contains("invalid path"),
        "receipt summary should mention invalid path"
    );
}

#[test]
fn test_first_preflight_rejects_unsafe_seam() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Unsafe Seam

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task with unsafe dotted test path.
  - Test-first: required
  - Seam: `crates/cli/src/lib.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/src/lib.rs`
  - Test Paths: `crates/cli/../outside.rs`
  - Scaffold: required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "unsafe-seam", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(code, 0, "unsafe test path must fail preflight");
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
    assert!(
        receipt.contains("invalid path"),
        "receipt summary should mention invalid path"
    );
}

#[cfg(unix)]
#[test]
fn test_first_preflight_rejects_linked_seam() {
    let temp = create_temp_repo();
    let repo_root = temp.path();

    let real_dir = repo_root.join("real_dir");
    std::fs::create_dir_all(&real_dir).unwrap();
    let real_file = real_dir.join("file.rs");
    std::fs::write(&real_file, "// code\n").unwrap();

    let link_dir = repo_root.join("link_dir");
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Linked Seam

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task with symlinked test path.
  - Test-first: required
  - Seam: `real_dir/file.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `real_dir/file.rs`
  - Test Paths: `link_dir/file.rs`
  - Scaffold: required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "linked-seam", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(code, 0, "symlinked test path must fail preflight");
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
    assert!(
        receipt.contains("symlink, junction, or reparse point"),
        "receipt summary should mention symlink"
    );
}

#[test]
fn test_first_preflight_rejects_unjustified_not_applicable() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Unjustified Not Applicable

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Not applicable task missing non-red probe.
  - Test-first: not-applicable — Rationale is here but probe is missing

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test doc |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "unjustified-na", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(
        code, 0,
        "unjustified not-applicable task must fail preflight"
    );
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
    assert!(
        receipt.contains("missing reproducible non-red probe"),
        "receipt summary should mention missing non-red probe"
    );
}

#[test]
fn test_first_preflight_rejects_non_exact_parent() {
    let temp = create_temp_repo();
    let repo_root = temp.path();

    // ValidatedDirectChildDirectory requires rel_path.parent() == expected_parent
    let mode = ValidatedRepoPathMode::ValidatedDirectChildDirectory {
        expected_parent: PathBuf::from("crates/cli"),
    };
    let res = ValidatedRepoPath::new(repo_root, Path::new("crates/engine/file.rs"), mode);

    assert!(
        res.is_err(),
        "non-exact parent directory must be rejected by ValidatedRepoPath"
    );
    if let Err(ValidatedRepoPathError::InvalidPath { reason }) = res {
        assert!(
            reason.contains("does not match expected parent"),
            "expected non-exact parent error, got: {reason}"
        );
    } else {
        panic!("expected InvalidPath error for non-exact parent");
    }
}

#[cfg(unix)]
#[test]
fn test_first_preflight_rejects_linked_paths() {
    let temp = create_temp_repo();
    let repo_root = temp.path();

    // Create real dir and symlink to it
    let real_dir = repo_root.join("real_dir");
    std::fs::create_dir_all(&real_dir).unwrap();
    let real_file = real_dir.join("file.rs");
    std::fs::write(&real_file, "// code\n").unwrap();

    let link_dir = repo_root.join("link_dir");
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Linked Path

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task using symlinked path.
  - Test-first: required
  - Seam: `real_dir/file.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `link_dir/file.rs`
  - Test Paths: `real_dir/file.rs`
  - Scaffold: required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "linked-path", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(code, 0, "symlinked path must fail preflight");
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
    assert!(
        receipt.contains("symlink, junction, or reparse point"),
        "receipt summary should mention symlink"
    );
}

#[cfg(windows)]
#[test]
fn test_first_preflight_rejects_windows_junction() {
    let temp = create_temp_repo();
    let repo_root = temp.path();

    let real_dir = repo_root.join("real_dir");
    std::fs::create_dir_all(&real_dir).unwrap();
    let real_file = real_dir.join("file.rs");
    std::fs::write(&real_file, "// code\n").unwrap();

    let junction_dir = repo_root.join("junction_dir");
    let real_dir_str = real_dir.to_string_lossy();
    let junction_str = junction_dir.to_string_lossy();
    let output = Command::new("cmd")
        .args(["/c", "mklink", "/J", &junction_str, &real_dir_str])
        .output()
        .expect("cmd must be available on Windows");
    assert!(
        output.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: Junction Path

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task using junctioned path.
  - Test-first: required
  - Seam: `real_dir/file.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `junction_dir/file.rs`
  - Test Paths: `real_dir/file.rs`
  - Scaffold: required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "junction-path", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(code, 0, "junctioned path must fail preflight");
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
    assert!(
        receipt.contains("symlink, junction, or reparse point"),
        "receipt summary should mention junction/reparse"
    );
}

#[cfg(unix)]
#[test]
fn test_first_preflight_rejects_uncertain_identity() {
    use std::os::unix::fs::PermissionsExt;

    let temp = create_temp_repo();
    let repo_root = temp.path().canonicalize().unwrap();

    let restricted = repo_root.join("restricted");
    std::fs::create_dir_all(&restricted).unwrap();
    std::fs::write(restricted.join("file.txt"), "secret").unwrap();

    let mut perms = std::fs::metadata(&restricted).unwrap().permissions();
    perms.set_mode(0o000);
    std::fs::set_permissions(&restricted, perms).unwrap();

    let res = ValidatedRepoPath::new(
        &repo_root,
        Path::new("restricted/file.txt"),
        ValidatedRepoPathMode::RegularFileOrMissing,
    );

    let mut reset_perms = std::fs::metadata(&restricted).unwrap().permissions();
    reset_perms.set_mode(0o755);
    let _ = std::fs::set_permissions(&restricted, reset_perms);

    assert!(
        matches!(res, Err(ValidatedRepoPathError::UncertainIdentity { .. })),
        "expected UncertainIdentity for permission-denied path, got {:?}",
        res
    );
}

#[test]
fn test_first_preflight_accepts_cli_flag_seam() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_content = format!(
        r#"# Plan Prompt: CLI Flag Seam

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — Task with CLI flag seam.
  - Test-first: required
  - Seam: `--flag/../outside.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/src/lib.rs`
  - Test Paths: `crates/cli/tests/test_feat.rs`
  - Scaffold: not-required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    );

    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "cli-flag-seam", &prompt_content);
    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_eq!(
        code, 0,
        "CLI flag seam preflight should exit 0, receipt: {receipt}"
    );
    assert!(
        receipt.contains("overall: pass"),
        "receipt overall should be pass"
    );
}

// --- closed terminal-reverify lane: prompt-only DONE recovery ---
//
// `gal pipeline-preflight --terminal-reverify <prompt>` is a closed mode:
// Workflow must be DONE, every task checked, cursor cleared, no OPEN
// Retry Handoff / Interrupted Phase block under `### Handoff Notes`, and a
// clean working tree. Its receipt carries its own identity
// (`terminal-reverify.receipt.md` under the plan's receipt scope) instead
// of the ordinary `preflight.receipt.md`. Ordinary (non-flagged) preflight
// is byte-unchanged and still rejects DONE.

fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

fn terminal_reverify_receipt_path(repo: &Path, slug: &str) -> PathBuf {
    repo.join(".dev")
        .join("pipeline")
        .join("receipts")
        .join(slug)
        .join("terminal-reverify.receipt.md")
}

fn run_terminal_reverify(repo: &Path, prompt_path: &Path) -> (i32, String) {
    let output = run_terminal_reverify_output(repo, prompt_path);
    let code = output.status.code().expect("process terminated by signal");
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    (code, combined)
}

fn run_terminal_reverify_output(repo: &Path, prompt_path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("pipeline-preflight")
        .arg("--terminal-reverify")
        .arg(prompt_path)
        .output()
        .expect("failed to execute gal binary")
}

/// A terminal-shaped prompt: `Workflow: DONE`, cleared cursor (`Current
/// Task: —`), and a caller-supplied task bullet / handoff block so each
/// fixture violates exactly one invariant.
fn terminal_prompt(t1: &str, task_line: &str, handoff: &str) -> String {
    format!(
        r#"# Plan Prompt: Terminal Reverify Fixture

## Status

Workflow: DONE
Current Task: —
Test Retry Count: 0
Review Retry Count: 0

### Handoff Notes
{handoff}

## Tasks

{task_line}

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    )
}

fn terminal_marked_prompt(t1: &str, task_line: &str, handoff: &str) -> String {
    format!(
        r#"# Plan Prompt: Marked Terminal Reverify Fixture

Pipeline Contract: test-first-v1

## Status

Workflow: DONE
Current Task: —
Test Retry Count: 0
Review Retry Count: 0

### Handoff Notes
{handoff}

## Tasks

{task_line}
  - Test-first: required
  - Seam: `crates/cli/src/lib.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/src/lib.rs`
  - Test Paths: `crates/cli/tests/test_feat.rs`
  - Scaffold: not-required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | test feature |
"#
    )
}

fn write_terminal_fixture(repo: &Path, name: &str, prompt_text: &str) -> PathBuf {
    let plans_dir = repo.join(".dev").join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    let prompt_path = plans_dir.join(format!("{name}.prompt.md"));
    std::fs::write(&prompt_path, prompt_text).unwrap();

    let state_path = repo.join(".dev").join("state.md");
    let state_text = format!(
        "## Session Continuity\n\n| {name} | .dev/plans/{name}.prompt.md | in progress |\n"
    );
    std::fs::write(&state_path, state_text).unwrap();
    prompt_path
}

#[test]
fn terminal_reverify_rejects_non_done_workflow() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n")
        .replace("Workflow: DONE", "Workflow: IMPLEMENT");
    let prompt_path = write_terminal_fixture(temp.path(), "terminal-non-done", &prompt_text);
    commit_all(temp.path(), "fixture: non-DONE workflow");

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    assert_ne!(
        code, 0,
        "non-DONE workflow must reject --terminal-reverify, output: {combined}"
    );
    assert!(
        combined.contains("terminal-reverify"),
        "output must name the terminal-reverify lane, got: {combined}"
    );
    assert!(
        combined.to_lowercase().contains("done"),
        "failure reason must name the missing DONE workflow, got: {combined}"
    );

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "terminal-non-done");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
    assert!(
        receipt.contains("overall: fail"),
        "terminal-reverify receipt must record a failing row, got: {receipt}"
    );
}

#[test]
fn terminal_reverify_rejects_unchecked_task() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [ ] {t1} — pending."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "terminal-unchecked", &prompt_text);
    commit_all(temp.path(), "fixture: unchecked task under DONE");

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    assert_ne!(
        code, 0,
        "an unchecked task under Workflow: DONE is terminal corruption and must reject, output: {combined}"
    );
    assert!(
        combined.contains("terminal-reverify"),
        "output must name the terminal-reverify lane, got: {combined}"
    );

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "terminal-unchecked");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
    assert!(
        receipt.contains("overall: fail"),
        "terminal-reverify receipt must record a failing row, got: {receipt}"
    );
}

#[test]
fn terminal_reverify_rejects_open_retry_handoff() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let handoff = format!(
        "\n#### Retry Handoff — {t1} / TEST\n\n- Status: OPEN\n- Problem: flaky fixture\n\
         - Evidence:\n  - Test Results: not-applicable\n  - Review Results: not-applicable\n  \
         - Security Review: not-applicable\n- Attempts:\n  1. 2026-08-23 — retried once\n     \
         - Result: still failing\n     - Validation: not-run\n     - Commit: none\n\
         - Next human step: inspect fixture\n"
    );
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), &handoff);
    let prompt_path = write_terminal_fixture(temp.path(), "terminal-open-handoff", &prompt_text);
    commit_all(temp.path(), "fixture: open retry handoff");

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    assert_ne!(
        code, 0,
        "an OPEN handoff block must reject --terminal-reverify, output: {combined}"
    );
    assert!(
        combined.contains("terminal-reverify"),
        "output must name the terminal-reverify lane, got: {combined}"
    );

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "terminal-open-handoff");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
    assert!(
        receipt.contains("overall: fail"),
        "terminal-reverify receipt must record a failing row, got: {receipt}"
    );
}

#[test]
fn terminal_reverify_rejects_dirty_tree() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "terminal-dirty", &prompt_text);
    commit_all(temp.path(), "fixture: clean baseline");

    // Dirty the tree after the commit that made it otherwise terminal-shaped.
    std::fs::write(temp.path().join("uncommitted.txt"), "stray change\n").unwrap();

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    assert_ne!(
        code, 0,
        "a dirty tree must reject --terminal-reverify, output: {combined}"
    );
    assert!(
        combined.contains("terminal-reverify"),
        "output must name the terminal-reverify lane, got: {combined}"
    );

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "terminal-dirty");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
    assert!(
        receipt.contains("overall: fail"),
        "terminal-reverify receipt must record a failing row, got: {receipt}"
    );
}

#[test]
fn terminal_reverify_accepts_clean_all_checked_terminal_prompt() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "terminal-clean", &prompt_text);
    commit_all(temp.path(), "fixture: clean terminal prompt");

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "terminal-clean");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
    assert_eq!(
        code, 0,
        "a clean all-checked terminal prompt should pass --terminal-reverify, output: {combined}, receipt: {receipt}"
    );
    assert!(
        receipt.contains("overall: pass"),
        "terminal-reverify receipt should record pass, got: {receipt}"
    );
}

#[test]
fn terminal_reverify_reaches_no_dispatch_mutation_or_commit_surface() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "terminal-isolation", &prompt_text);
    commit_all(temp.path(), "fixture: isolation baseline");

    let head_before = git(temp.path(), &["rev-parse", "HEAD"]);
    let prompt_bytes_before = std::fs::read(&prompt_path).unwrap();

    let _ = run_terminal_reverify(temp.path(), &prompt_path);

    let head_after = git(temp.path(), &["rev-parse", "HEAD"]);
    let prompt_bytes_after = std::fs::read(&prompt_path).unwrap();
    assert_eq!(
        head_before, head_after,
        "terminal-reverify must never create a commit"
    );
    assert_eq!(
        prompt_bytes_before, prompt_bytes_after,
        "terminal-reverify must never mutate the prompt it reads"
    );

    // No dispatch (test/audit/commit) receipt may appear beside the closed
    // mode's own receipt — this mode authorizes only in-process
    // verification, never a dispatch, mutation, or commit surface.
    let scope_dir = temp
        .path()
        .join(".dev")
        .join("pipeline")
        .join("receipts")
        .join("terminal-isolation");
    if scope_dir.is_dir() {
        for entry in std::fs::read_dir(&scope_dir).unwrap() {
            let entry = entry.unwrap();
            let file_name = entry.file_name();
            let file_name = file_name.to_string_lossy().to_string();
            assert_eq!(
                file_name, "terminal-reverify.receipt.md",
                "terminal-reverify must not leave a dispatch, test, audit, or commit \
                 receipt behind — found: {file_name}"
            );
        }
    }
}

#[test]
fn ordinary_preflight_still_rejects_done_workflow() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let (prompt_path, receipt_path) =
        write_plan_and_state(temp.path(), "ordinary-done", &prompt_text);
    commit_all(temp.path(), "fixture: ordinary DONE prompt");

    let (code, receipt) = run_preflight(temp.path(), &prompt_path, &receipt_path);

    assert_ne!(
        code, 0,
        "ordinary (non-terminal-reverify) preflight must still reject Workflow: DONE"
    );
    assert!(
        receipt.contains("overall: fail"),
        "receipt overall should be fail"
    );
}

#[test]
fn terminal_reverify_rejects_marked_prompt_digest_mismatch() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_marked_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "marked-digest-mismatch", &prompt_text);

    // Write a transition journal with a mismatched marked digest
    let journal_dir = temp
        .path()
        .join(".dev")
        .join("pipeline")
        .join("journal")
        .join("marked-digest-mismatch");
    std::fs::create_dir_all(&journal_dir).unwrap();
    let journal_path = journal_dir.join("transition.journal.tsv");
    std::fs::write(
        &journal_path,
        format!(
            "1\tmarked_digest_0000000000000000000000000000000000000000000000000000000000000000\t2026-08-24T00:00:00Z\t{}\ttest\tpass\n",
            tid(1)
        ),
    )
    .unwrap();
    commit_all(
        temp.path(),
        "fixture: marked prompt with mismatched journal digest",
    );

    let head_before = git(temp.path(), &["rev-parse", "HEAD"]);
    let prompt_bytes_before = std::fs::read(&prompt_path).unwrap();

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    assert_ne!(
        code, 0,
        "marked prompt with transition journal digest mismatch must reject --terminal-reverify, output: {combined}"
    );

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "marked-digest-mismatch");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
    assert!(
        receipt.contains("overall: fail"),
        "terminal-reverify receipt must record overall: fail on marked digest mismatch, got: {receipt}"
    );

    let head_after = git(temp.path(), &["rev-parse", "HEAD"]);
    let prompt_bytes_after = std::fs::read(&prompt_path).unwrap();
    assert_eq!(head_before, head_after, "terminal-reverify must not commit");
    assert_eq!(
        prompt_bytes_before, prompt_bytes_after,
        "terminal-reverify must not mutate prompt"
    );

    let scope_dir = temp
        .path()
        .join(".dev")
        .join("pipeline")
        .join("receipts")
        .join("marked-digest-mismatch");
    if scope_dir.is_dir() {
        for entry in std::fs::read_dir(&scope_dir).unwrap() {
            let entry = entry.unwrap();
            let file_name = entry.file_name().to_string_lossy().to_string();
            assert_eq!(file_name, "terminal-reverify.receipt.md");
        }
    }
}

#[test]
fn terminal_reverify_rejects_marked_prompt_missing_journal() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_marked_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "marked-missing-journal", &prompt_text);
    commit_all(temp.path(), "fixture: marked prompt without journal");

    let head_before = git(temp.path(), &["rev-parse", "HEAD"]);
    let prompt_bytes_before = std::fs::read(&prompt_path).unwrap();

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    assert_ne!(
        code, 0,
        "marked prompt without transition journal must reject --terminal-reverify, output: {combined}"
    );

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "marked-missing-journal");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
    assert!(
        receipt.contains("overall: fail"),
        "terminal-reverify receipt must record overall: fail on missing journal, got: {receipt}"
    );

    let head_after = git(temp.path(), &["rev-parse", "HEAD"]);
    let prompt_bytes_after = std::fs::read(&prompt_path).unwrap();
    assert_eq!(head_before, head_after);
    assert_eq!(prompt_bytes_before, prompt_bytes_after);
}

#[test]
fn terminal_reverify_emits_deterministic_immutable_receipt_fields() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "terminal-receipt-fields", &prompt_text);
    commit_all(temp.path(), "fixture: terminal prompt for receipt fields");

    let (_code, _combined) = run_terminal_reverify(temp.path(), &prompt_path);

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "terminal-receipt-fields");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();

    assert!(
        receipt.contains("prompt_path:")
            || receipt.contains("Prompt Path:")
            || receipt.contains("prompt_path"),
        "receipt must contain prompt path, got: {receipt}"
    );
    assert!(
        receipt.contains("prompt_sha256:")
            || receipt.contains("prompt_hash:")
            || receipt.contains("prompt_sha256"),
        "receipt must contain prompt sha256, got: {receipt}"
    );
    assert!(
        receipt.contains("head:") || receipt.contains("HEAD:") || receipt.contains("head"),
        "receipt must contain HEAD commit sha, got: {receipt}"
    );
    assert!(
        receipt.contains("mode: terminal-reverify") || receipt.contains("terminal-reverify"),
        "receipt must contain mode, got: {receipt}"
    );
    assert!(
        receipt.contains("digest_result:")
            || receipt.contains("digest_status:")
            || receipt.contains("marked_digest:"),
        "receipt must contain digest result field, got: {receipt}"
    );
    assert!(
        receipt.contains("overall:"),
        "receipt must contain overall verdict field, got: {receipt}"
    );
}

#[test]
fn terminal_reverify_markerless_prompt_no_journal_passes() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt_text = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(temp.path(), "markerless-no-journal", &prompt_text);
    commit_all(temp.path(), "fixture: clean markerless prompt");

    let (code, combined) = run_terminal_reverify(temp.path(), &prompt_path);

    let receipt_path = terminal_reverify_receipt_path(temp.path(), "markerless-no-journal");
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();

    assert_eq!(
        code, 0,
        "markerless prompt with clean tree and all checked tasks must pass --terminal-reverify, output: {combined}, receipt: {receipt}"
    );
    assert!(
        receipt.contains("overall: pass"),
        "receipt overall should be pass"
    );
}

#[test]
fn preflight_prints_route_line_before_summary_in_both_modes() {
    let passing = create_temp_repo();
    let t1 = tid(1);
    let passing_prompt = format!(
        r#"# Plan Prompt: Route Line Passing Fixture

Pipeline Contract: test-first-v1

## Status

Workflow: IMPLEMENT
Current Task: {t1}

## Tasks

- [ ] {t1} — route line fixture.
  - Test-first: required
  - Seam: `crates/cli/src/lib.rs`
  - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`
  - Production Paths: `crates/cli/src/lib.rs`
  - Test Paths: `crates/cli/tests/test_feat.rs`
  - Scaffold: not-required

## Test Plan

| ID | desc |
| --- | --- |
| {t1} | route line |
"#
    );
    let (passing_prompt, passing_receipt) =
        write_plan_and_state(passing.path(), "route-line-pass", &passing_prompt);
    let passing_output = run_preflight_output(passing.path(), &passing_prompt, &passing_receipt);
    assert_eq!(
        passing_output.status.code(),
        Some(0),
        "passing fixture exit code changed"
    );
    let passing_stdout = String::from_utf8_lossy(&passing_output.stdout);
    assert_route_line(&passing_stdout);

    let failing = create_temp_repo();
    let failing_prompt = passing_prompt
        .to_string_lossy()
        .replace("Seam: `crates/cli/src/lib.rs`\n", "");
    let (failing_prompt, failing_receipt) =
        write_plan_and_state(failing.path(), "route-line-fail", &failing_prompt);
    let failing_output = run_preflight_output(failing.path(), &failing_prompt, &failing_receipt);
    assert_ne!(
        failing_output.status.code(),
        Some(0),
        "failing fixture exit code changed"
    );
    let failing_stdout = String::from_utf8_lossy(&failing_output.stdout);
    assert_route_line(&failing_stdout);

    let terminal = create_temp_repo();
    let terminal_prompt = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let terminal_prompt =
        write_terminal_fixture(terminal.path(), "route-line-terminal", &terminal_prompt);
    commit_all(terminal.path(), "fixture: route line terminal");
    let terminal_output = run_terminal_reverify_output(terminal.path(), &terminal_prompt);
    assert_route_line_shape(&String::from_utf8_lossy(&terminal_output.stdout));
}
