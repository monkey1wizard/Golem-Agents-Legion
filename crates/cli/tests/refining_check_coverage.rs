//! Black-box fixtures for the `test-plan-coverage` receipt row in
//! `gal refining-check`.
//!
//! `task-test-pairing` only compares `T-NN`/`TP-NN` **suffixes** — it never
//! resolves a row's `Covers` cell to the task(s) it actually names. These
//! probes lock the stronger relation: every `## Test Plan` row's `Covers`
//! cell must resolve to a declared `T-NN`, and every declared blocking task
//! must have at least one covering row. Written error-first: the locked
//! EF-01 fixture first, then the "suffix pairing alone is not enough" case
//! the contract explicitly calls out, then the happy path.
//!
//! Production path (`crates/cli/src/commands/refining_check.rs`) is frozen
//! for this task — these probes drive the `gal` binary black-box and read
//! only the emitted receipt.

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

// Compose plan-task IDs at runtime — never hardcode `T-NN`/`TP-NN` literals
// in source (naming-gate provenance rule: plan-task IDs belong only in
// `.dev`), matching the convention already used in
// `refining_check.rs`'s own unit tests.
fn tid(n: u32) -> String {
    format!("T-{n:02}")
}
fn tpid(n: u32) -> String {
    format!("TP-{n:02}")
}

/// A temp repo shaped like `refining_check.rs`'s own `temp_repo()`: a git
/// root (naming-gate's `scan_tree` is git-proven and fails closed outside
/// one) plus the doc fixtures every sample plan below references.
fn repo() -> TempDir {
    let tmp = tempdir().expect("fixture tempdir");
    std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
    std::fs::write(tmp.path().join("docs").join("example.md"), "ok\n").unwrap();
    std::fs::write(
        tmp.path().join("docs").join("glossary.md"),
        "# RETIRED-TERMS:BEGIN\n# RETIRED-TERMS:END\n",
    )
    .unwrap();
    let status = Command::new("git")
        .arg("-C")
        .arg(tmp.path())
        .arg("init")
        .arg("--quiet")
        .status()
        .expect("git init spawns");
    assert!(status.success(), "git init must succeed");
    tmp
}

fn write_plan(repo: &Path, name: &str, text: &str) -> PathBuf {
    let path = repo.join(format!("{name}.md"));
    std::fs::write(&path, text).unwrap();
    path
}

/// Run `gal refining-check <target> --receipt <receipt>` and return the
/// process exit code plus the receipt's contents (empty string if the
/// receipt was never written).
fn run(repo: &Path, target: &Path, receipt: &Path) -> (i32, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .arg("refining-check")
        .arg(target)
        .arg("--receipt")
        .arg(receipt)
        .output()
        .expect("failed to spawn gal binary");
    let code = output
        .status
        .code()
        .expect("process was not terminated by signal");
    let receipt_text = std::fs::read_to_string(receipt).unwrap_or_default();
    (code, receipt_text)
}

/// Assemble a minimal, otherwise-clean plan around caller-supplied
/// `## Test Plan` rows and `## Tasks` bullets, so each probe below isolates
/// the coverage relation instead of tripping unrelated checks.
fn plan(test_plan_rows: &str, tasks: &str) -> String {
    format!(
        "# Plan\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n{test_plan_rows}\n## Review Results\n### Engineering Review\nCLEAR\n\n<!-- ENG_REVIEW: CLEAR -->\n\n## Tasks\n{tasks}\n"
    )
}

fn coverage_check(receipt: &str) -> Option<&str> {
    receipt
        .lines()
        .find(|line| line.starts_with("| test-plan-coverage |"))
}

// EF-01 (locked): a Test Plan row whose `Description` cell carries an
// unescaped `|` shifts every cell after it by one column, so the `Covers`
// column silently reads the wrong cell. Today `refining-check` passes this
// plan clean — `task-test-pairing` only regex-matches the `TP-NN` prefix of
// the row and never looks at the shifted `Covers` cell. `test-plan-coverage`
// must resolve the cell, find it names no declared `T-NN`, and fail.
#[test]
fn ef01_unescaped_pipe_shifts_covers_cell_and_fails_coverage() {
    let fixture = repo();
    let row = format!("| {} | manual | check | broken | {} |\n", tpid(1), tid(1));
    let tasks = format!("- [ ] {} - `docs/example.md` update.\n", tid(1));
    let target = write_plan(fixture.path(), "ef01", &plan(&row, &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let row_line = coverage_check(&receipt_text)
        .expect("test-plan-coverage check must be present in the receipt");
    assert!(
        row_line.contains("| fail |"),
        "test-plan-coverage must fail on a Covers cell shifted by an unescaped pipe: {row_line}"
    );
    assert!(
        row_line.contains(&tpid(1)),
        "test-plan-coverage must name the offending row: {row_line}"
    );
}

// A row's `Covers` cell can resolve to a well-formed `T-NN` token that is
// simply never declared in `## Tasks`. Suffix-based `task-test-pairing`
// still passes (the row id and the task id share a suffix by coincidence)
// even though the row actually covers nothing real, and the
// one declared task is left with no covering row at all.
#[test]
fn covers_cell_naming_undeclared_task_fails_coverage_and_leaves_declared_task_uncovered() {
    let fixture = repo();
    let row = format!("| {} | manual | check | {} |\n", tpid(1), tid(9));
    let tasks = format!("- [ ] {} - `docs/example.md` update.\n", tid(1));
    let target = write_plan(fixture.path(), "undeclared-covers", &plan(&row, &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let row_line = coverage_check(&receipt_text)
        .expect("test-plan-coverage check must be present in the receipt");
    assert!(
        row_line.contains("| fail |"),
        "a Covers cell naming an undeclared task must fail: {row_line}"
    );
    assert!(
        row_line.contains(&tid(1)),
        "the uncovered declared task must be named: {row_line}"
    );
}

// The contract is explicit: `test-plan-coverage` "must not be satisfied by
// matching TP-NN identifiers alone." Two rows both cover the first task and
// none covers the second — `task-test-pairing`'s suffix sets match, but the
// second task is a declared blocking task with zero covering rows.
#[test]
fn tp_suffix_pairing_alone_does_not_satisfy_coverage() {
    let fixture = repo();
    let rows = format!(
        "| {} | manual | check | {} |\n| {} | manual | check | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(1)
    );
    let tasks = format!(
        "- [ ] {} - `docs/example.md` update.\n- [ ] {} - `docs/example.md` update.\n",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), "suffix-only", &plan(&rows, &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let pairing_line = receipt_text
        .lines()
        .find(|line| line.starts_with("| task-test-pairing |"))
        .expect("task-test-pairing check must be present in the receipt");
    assert!(
        pairing_line.contains("| pass |"),
        "task-test-pairing (suffix-only) is expected to pass this fixture: {pairing_line}"
    );

    let row_line = coverage_check(&receipt_text)
        .expect("test-plan-coverage check must be present in the receipt");
    assert!(
        row_line.contains("| fail |"),
        "test-plan-coverage must fail when a declared task has no covering row, \
         even though TP-NN suffix pairing passes: {row_line}"
    );
    assert!(
        row_line.contains(&tid(2)),
        "the uncovered declared task must be named: {row_line}"
    );
}

// Happy path: every row's Covers cell resolves to a declared task, and
// every declared task has at least one covering row.
#[test]
fn every_row_and_task_resolved_passes_coverage() {
    let fixture = repo();
    let rows = format!(
        "| {} | manual | check | {} |\n| {} | manual | check | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(2)
    );
    let tasks = format!(
        "- [ ] {} - `docs/example.md` update.\n- [ ] {} - `docs/example.md` update.\n",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), "clean", &plan(&rows, &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let row_line = coverage_check(&receipt_text)
        .expect("test-plan-coverage check must be present in the receipt");
    assert!(
        row_line.contains("| pass |"),
        "fully resolved row/task coverage must pass: {row_line}"
    );
    assert_eq!(
        code, 0,
        "a plan with resolved coverage and no other violations should pass overall: {receipt_text}"
    );
}
