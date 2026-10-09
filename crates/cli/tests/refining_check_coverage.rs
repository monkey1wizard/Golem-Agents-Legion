//! Black-box fixtures for the `test-plan-coverage` receipt row in
//! `gal refining-check`.
//!
//! `manual-limit` receipt rows in `gal refining-check`.
//!
//! Every `## Test Plan` row's `Covers` cell must resolve to a declared
//! `T-NN`, and every declared blocking task must have at least one covering
//! row. A row may cover several tasks, and `TP-NN` numbers need not equal
//! task numbers. A plan holds at most one `manual` row, and that row's
//! Description carries text after `Why manual:`. These probes drive the
//! `gal` binary black-box and read only the emitted receipt.

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
// plan clean when nothing reads the shifted `Covers` cell.
// `test-plan-coverage` must resolve the cell, find it names no declared `T-NN`, and fail.
#[test]
fn ef01_unescaped_pipe_shifts_covers_cell_and_fails_coverage() {
    let fixture = repo();
    let row = format!("| {} | grep | check | broken | {} |\n", tpid(1), tid(1));
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
// simply never declared in `## Tasks`. The row covers nothing real, and the
// one declared task is left with no covering row at all.
#[test]
fn covers_cell_naming_undeclared_task_fails_coverage_and_leaves_declared_task_uncovered() {
    let fixture = repo();
    let row = format!("| {} | grep | check | {} |\n", tpid(1), tid(9));
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

// Two rows both cover the first task and none covers the second, so the
// second declared task has zero covering rows. Matching `TP-NN` identifiers
// alone must not satisfy coverage, and the receipt carries no
// `task-test-pairing` row.
#[test]
fn uncovered_task_fails_coverage_and_receipt_has_no_pairing_row() {
    let fixture = repo();
    let rows = format!(
        "| {} | grep | check | {} |\n| {} | grep | check | {} |\n",
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

    assert!(
        !receipt_text.contains("task-test-pairing"),
        "receipt must not carry a task-test-pairing row: {receipt_text}"
    );
    let row_line = coverage_check(&receipt_text)
        .expect("test-plan-coverage check must be present in the receipt");
    assert!(
        row_line.contains("| fail |"),
        "test-plan-coverage must fail when a declared task has no covering row: {row_line}"
    );
    assert!(
        row_line.contains(&tid(2)),
        "the uncovered declared task must be named: {row_line}"
    );
}

// One TP may cover several tasks, and its number need not equal a task number.
#[test]
fn one_row_covering_several_tasks_passes_coverage() {
    let fixture = repo();
    let rows = format!("| {} | grep | check | {}, {} |\n", tpid(7), tid(1), tid(2));
    let tasks = format!(
        "- [ ] {} - `docs/example.md` update.\n- [ ] {} - `docs/glossary.md` update.\n",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), "multi-cover", &plan(&rows, &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let row_line = coverage_check(&receipt_text)
        .expect("test-plan-coverage check must be present in the receipt");
    assert!(row_line.contains("| pass |"), "must pass: {row_line}");
    assert!(
        !receipt_text.contains("task-test-pairing"),
        "receipt must not carry a task-test-pairing row: {receipt_text}"
    );
    assert_eq!(code, 0, "{receipt_text}");
}

fn manual_limit_line(name: &str, rows: &str) -> String {
    let fixture = repo();
    let tasks = format!(
        "- [ ] {} - `docs/example.md` update.\n- [ ] {} - `docs/glossary.md` update.\n",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), name, &plan(rows, &tasks));
    let receipt_path = fixture.path().join("receipt.md");
    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);
    receipt_text
        .lines()
        .find(|line| line.starts_with("| manual-limit |"))
        .expect("manual-limit row must be present")
        .to_string()
}

#[test]
fn two_manual_rows_fail_manual_limit() {
    let rows = format!(
        "| {} | manual | Why manual: a | {} |\n| {} | manual | Why manual: b | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(2)
    );
    let line = manual_limit_line("two-manual", &rows);
    assert!(line.contains("| fail |"), "must fail: {line}");
}

#[test]
fn manual_row_without_reason_fails_manual_limit() {
    let rows = format!(
        "| {} | manual | check | {} |\n| {} | grep | check | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(2)
    );
    let line = manual_limit_line("no-reason", &rows);
    assert!(line.contains("| fail |"), "must fail: {line}");
    assert!(line.contains(&tpid(1)), "must name the row: {line}");
}

#[test]
fn manual_row_with_empty_reason_fails_manual_limit() {
    let rows = format!(
        "| {} | manual | Why manual:   | {} |\n| {} | grep | check | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(2)
    );
    let line = manual_limit_line("empty-reason", &rows);
    assert!(line.contains("| fail |"), "must fail: {line}");
}

#[test]
fn single_manual_row_with_reason_passes_manual_limit() {
    let rows = format!(
        "| {} | manual | Run probe. Why manual: no harness fits | {} |\n| {} | grep | check | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(2)
    );
    let line = manual_limit_line("one-manual", &rows);
    assert!(line.contains("| pass |"), "must pass: {line}");
}

// Happy path: every row's Covers cell resolves to a declared task, and
// every declared task has at least one covering row.
#[test]
fn every_row_and_task_resolved_passes_coverage() {
    let fixture = repo();
    let rows = format!(
        "| {} | grep | check | {} |\n| {} | grep | check | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(2)
    );
    let tasks = format!(
        "- [ ] {} - `docs/example.md` update.\n- [ ] {} - `docs/glossary.md` update.\n",
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

fn split_check(receipt: &str) -> Option<&str> {
    receipt
        .lines()
        .find(|line| line.starts_with("| translation-split |"))
}

fn split_rows() -> String {
    format!(
        "| {} | grep | check | {} |\n| {} | grep | check | {} |\n",
        tpid(1),
        tid(1),
        tpid(2),
        tid(2)
    )
}

// A document and one of its language variants edited by two different tasks
// is a translation split: the receipt must name the key and both task IDs.
#[test]
fn translation_split_fails_when_two_tasks_share_a_document_key() {
    let fixture = repo();
    let tasks = format!(
        "- [ ] {} - `docs/workflows.md` update.\n- [ ] {} - `docs/i18n/ja/workflows.ja.md` update.\n",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), "split", &plan(&split_rows(), &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let line = split_check(&receipt_text).expect("translation-split row must be present");
    assert!(line.contains("| fail |"), "must fail: {line}");
    assert!(line.contains("workflows"), "must name the key: {line}");
    assert!(
        line.contains(&tid(1)) && line.contains(&tid(2)),
        "must name both task IDs: {line}"
    );
}

// Identical canonical paths in separate tasks are still a split.
#[test]
fn translation_split_fails_when_two_tasks_name_the_identical_path() {
    let fixture = repo();
    let tasks = format!(
        "- [ ] {} - `docs/workflows.md` update.
- [ ] {} - `docs/workflows.md` update.
",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), "identical", &plan(&split_rows(), &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let line = split_check(&receipt_text).expect("translation-split row must be present");
    assert!(line.contains("| fail |"), "must fail: {line}");
    assert!(
        line.contains("workflows") && line.contains(&tid(1)) && line.contains(&tid(2)),
        "must name the key and both task IDs: {line}"
    );
}

#[test]
fn translation_split_passes_when_one_task_references_both_variants() {
    let fixture = repo();
    let rows = format!("| {} | grep | check | {} |\n", tpid(1), tid(1));
    let tasks = format!(
        "- [ ] {} - `docs/workflows.md` and `docs/i18n/ja/workflows.ja.md` update.\n",
        tid(1)
    );
    let target = write_plan(fixture.path(), "joined", &plan(&rows, &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let line = split_check(&receipt_text).expect("translation-split row must be present");
    assert!(line.contains("| pass |"), "must pass: {line}");
}

// Root README shares a key with its localized variants.
#[test]
fn translation_split_matches_root_readme_with_localized_readme() {
    let fixture = repo();
    let tasks = format!(
        "- [ ] {} - `README.md` update.\n- [ ] {} - `docs/i18n/zh-Hant/README.zh-Hant.md` update.\n",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), "readme", &plan(&split_rows(), &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let line = split_check(&receipt_text).expect("translation-split row must be present");
    assert!(
        line.contains("| fail |") && line.contains("README"),
        "{line}"
    );
}

// Paths outside the doc convention never match, so unrelated files in two
// tasks do not trip the check.
#[test]
fn translation_split_ignores_paths_outside_the_convention() {
    let fixture = repo();
    let tasks = format!(
        "- [ ] {} - `docs/example.md` update.\n- [ ] {} - `docs/other/example.md` update.\n",
        tid(1),
        tid(2)
    );
    let target = write_plan(fixture.path(), "outside", &plan(&split_rows(), &tasks));
    let receipt_path = fixture.path().join("receipt.md");

    let (_code, receipt_text) = run(fixture.path(), &target, &receipt_path);

    let line = split_check(&receipt_text).expect("translation-split row must be present");
    assert!(line.contains("| pass |"), "must pass: {line}");
}

// `owner-acceptance` and `preconditions` receipt rows. A missing section
// passes. A present section fails with the offending ID and column named.
fn section_row<'a>(receipt: &'a str, name: &str) -> &'a str {
    receipt
        .lines()
        .find(|line| line.starts_with(&format!("| {name} |")))
        .unwrap_or_else(|| panic!("{name} row must be present in the receipt"))
}

fn run_with_sections(sections: &str) -> String {
    let fixture = repo();
    let rows = format!("| {} | grep | check | {} |\n", tpid(1), tid(1));
    let tasks = format!("- [ ] {} - `docs/example.md` update.\n", tid(1));
    let text = format!("{}\n{sections}\n", plan(&rows, &tasks));
    let target = write_plan(fixture.path(), "sections", &text);
    let receipt_path = fixture.path().join("receipt.md");
    run(fixture.path(), &target, &receipt_path).1
}

const OA_HEADER: &str = "## Owner Acceptance\n| ID | What to check | Expected result | Pass/fail rule | Why not automated |\n| --- | --- | --- | --- | --- |\n";
const PC_HEADER: &str = "## Preconditions\n| ID | Requirement | Check command | Expected result | How to satisfy |\n| --- | --- | --- | --- | --- |\n";

fn oa_row(id: &str, what: &str, expected: &str) -> String {
    format!("| {id} | {what} | {expected} | Window shows it | Needs eyes |\n")
}

#[test]
fn owner_acceptance_missing_sections_pass() {
    let receipt = run_with_sections("");
    assert!(section_row(&receipt, "owner-acceptance").contains("| pass |"));
    assert!(section_row(&receipt, "preconditions").contains("| pass |"));
}

#[test]
fn owner_acceptance_valid_tables_pass() {
    let sections = format!(
        "{OA_HEADER}{}{PC_HEADER}| PC-01 | Tool installed | `tool --version` | prints 1.0 | Install the tool |\n",
        oa_row("OA-01", "Open `docs/example.md`", "Page renders")
    );
    let receipt = run_with_sections(&sections);
    assert!(
        section_row(&receipt, "owner-acceptance").contains("| pass |"),
        "{receipt}"
    );
    assert!(
        section_row(&receipt, "preconditions").contains("| pass |"),
        "{receipt}"
    );
}

#[test]
fn owner_acceptance_bad_cells_fail_with_offending_id() {
    let long = "x".repeat(201);
    let cases = [
        oa_row("OA-01", "Open `docs/example.md`", ""),
        oa_row("OA-01", "Open `docs/example.md`", "TBD"),
        oa_row("OA-01", "Open `docs/example.md`", "<path>"),
        oa_row("OA-01", "Open `docs/example.md`", &long),
        oa_row("OA-01", "Open the example page", "Page renders"),
    ];
    for row in cases {
        let receipt = run_with_sections(&format!("{OA_HEADER}{row}"));
        let line = section_row(&receipt, "owner-acceptance");
        assert!(
            line.contains("| fail |") && line.contains("OA-01"),
            "{row}: {line}"
        );
    }
}

#[test]
fn owner_acceptance_backticked_text_is_not_counted_toward_length() {
    let cell = format!("{} `{}`", "a".repeat(40), "c".repeat(260));
    let receipt = run_with_sections(&format!(
        "{OA_HEADER}{}",
        oa_row("OA-01", "Open `docs/example.md`", &cell)
    ));
    assert!(
        section_row(&receipt, "owner-acceptance").contains("| pass |"),
        "{receipt}"
    );
}

#[test]
fn owner_acceptance_fourth_item_fails() {
    let rows: String = (1..=4)
        .map(|n| {
            oa_row(
                &format!("OA-{n:02}"),
                "Open `docs/example.md`",
                "Page renders",
            )
        })
        .collect();
    let receipt = run_with_sections(&format!("{OA_HEADER}{rows}"));
    let line = section_row(&receipt, "owner-acceptance");
    assert!(
        line.contains("| fail |") && line.contains("OA-04"),
        "{line}"
    );
}

#[test]
fn owner_acceptance_missing_column_and_malformed_id_fail() {
    let no_column = "## Owner Acceptance\n| ID | What to check | Expected result | Pass/fail rule |\n| --- | --- | --- | --- |\n| OA-01 | `a` | b | c |\n";
    let receipt = run_with_sections(no_column);
    let line = section_row(&receipt, "owner-acceptance");
    assert!(
        line.contains("| fail |") && line.contains("Why not automated"),
        "{line}"
    );

    let receipt = run_with_sections(&format!(
        "{OA_HEADER}{}",
        oa_row("OA-1", "Open `docs/example.md`", "Page renders")
    ));
    let line = section_row(&receipt, "owner-acceptance");
    assert!(line.contains("| fail |") && line.contains("OA-1"), "{line}");
}

#[test]
fn owner_acceptance_precondition_check_command_needs_backticks() {
    let receipt = run_with_sections(&format!(
        "{PC_HEADER}| PC-01 | Tool installed | tool --version | prints 1.0 | Install it |\n"
    ));
    let line = section_row(&receipt, "preconditions");
    assert!(
        line.contains("| fail |") && line.contains("PC-01") && line.contains("Check command"),
        "{line}"
    );

    let receipt = run_with_sections(&format!(
        "{PC_HEADER}| PC-1 | Tool installed | `tool` | prints 1.0 | Install it |\n"
    ));
    assert!(
        section_row(&receipt, "preconditions").contains("| fail |"),
        "{receipt}"
    );
}

#[test]
fn owner_acceptance_and_preconditions_reject_angle_placeholders_inside_commands() {
    let receipt = run_with_sections(&format!(
        "{OA_HEADER}{}\n{PC_HEADER}| PC-01 | File exists | `cat <path>` | contents print | Create the file |\n",
        oa_row("OA-01", "Run `cat <path>`", "Contents print")
    ));
    let owner = section_row(&receipt, "owner-acceptance");
    assert!(
        owner.contains("| fail |") && owner.contains("OA-01") && owner.contains("`What to check`"),
        "owner-acceptance must reject the placeholder and name its cell: {owner}"
    );
    let precondition = section_row(&receipt, "preconditions");
    assert!(
        precondition.contains("| fail |")
            && precondition.contains("PC-01")
            && precondition.contains("`Check command`"),
        "preconditions must reject the placeholder and name its cell: {precondition}"
    );
}

#[test]
fn preconditions_concrete_backticked_command_passes() {
    let receipt = run_with_sections(&format!(
        "{PC_HEADER}| PC-01 | Tool installed | `tool --version` | prints 1.0 | Install the tool |\n"
    ));
    assert!(
        section_row(&receipt, "preconditions").contains("| pass |"),
        "{receipt}"
    );
}
