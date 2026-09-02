//! `cargo test -p gal-cli --test test_first_finalize`
//!
//! Real-binary coverage for the `TestFirstEvaluation` reuse `gal finalize-check`
//! (check(g)) gained in this task. Scoped to what this task actually changes:
//!
//! 1. `cmd_finalize_check` now reads its target through `read_validated_bytes`
//!    instead of a naive `std::fs::read_to_string(..).unwrap_or_default()` — a
//!    target path routed through a linked (symlinked/junctioned) directory
//!    component is rejected before any bytes are read, and non-UTF-8 target
//!    content is rejected explicitly instead of silently treated as empty.
//! 2. `check_executor_logs_with_evaluations` now runs the shared
//!    `evaluate_test_first` / `evaluation_receipt` pair per checked task when
//!    the prompt carries the test-first marker, surfacing a dedicated
//!    `test-first-evaluation` receipt row with the evaluator's real field
//!    values (not just a pass/fail exit code).
//!
//! The pure `TestFirstEvaluation` evaluator logic itself — digest computation,
//! probe-receipt parsing, dispute handling — is already covered exhaustively by
//! `test_first_converge.rs`; this file does not re-prove it. Likewise the
//! bound-convergence prerequisites (three-surface, task-commit, cursor-cleared)
//! are covered by `finalize_check.rs`'s own in-module `end_to_end_*` tests; this
//! file reuses the same minimal fixture shape rather than re-testing them.

use pipeline::task_spec::{extract_plan_slug, parse_task_contract};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn run_gal(repo: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn make_git_repo() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    git(tmp.path(), &["init", "-q", "-b", "main"]);
    git(tmp.path(), &["config", "user.email", "t@t"]);
    git(tmp.path(), &["config", "user.name", "t"]);
    git(tmp.path(), &["config", "commit.gpgsign", "false"]);
    tmp
}

fn write_project_fixture(repo: &Path) {
    let dev = repo.join(".dev");
    fs::create_dir_all(&dev).unwrap();
    fs::write(
        dev.join("project.md"),
        include_str!("../../../.dev/project.md"),
    )
    .unwrap();
}

fn commit_all(repo: &Path, msg: &str) -> String {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", msg]);
    let out = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn full_head(repo: &Path) -> String {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    String::from_utf8(out.stdout).unwrap().trim().to_string()
}

fn sha256_hex(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

fn finalize_check(repo: &Path, target: &Path) -> (std::process::Output, String) {
    let receipt_path = repo.join("finalize.receipt.md");
    let output = run_gal(
        repo,
        &[
            "finalize-check",
            target.to_str().unwrap(),
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );
    let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
    (output, receipt)
}

fn finalize_check_hygiene(repo: &Path, target: &Path) -> (std::process::Output, String) {
    let receipt_path = repo.join("finalize-hygiene.receipt.md");
    let durable_commit = full_head(repo);
    let output = run_gal(
        repo,
        &[
            "finalize-check",
            target.to_str().unwrap(),
            "--hygiene-only",
            "--durable-commit",
            &durable_commit,
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );
    let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
    (output, receipt)
}

fn receipt_check_names(receipt: &str) -> Vec<&str> {
    receipt
        .lines()
        .filter_map(|line| {
            let cells = line.split('|').map(str::trim).collect::<Vec<_>>();
            (cells.len() >= 4 && matches!(cells[2], "pass" | "fail" | "not-run")).then(|| cells[1])
        })
        .collect()
}

fn extract_receipt_line<'a>(receipt: &'a str, check_name: &str) -> Option<&'a str> {
    receipt
        .lines()
        .find(|line| line.starts_with(&format!("| {check_name} |")))
}

fn failed_goal_predicates(receipt: &str) -> Vec<&str> {
    receipt
        .lines()
        .find_map(|line| line.strip_prefix("goal_binding_failed_predicates: "))
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|predicate| !predicate.is_empty() && *predicate != "none")
        .collect()
}

/// Locked task-contract block matching the exact `/refining-plan` grammar
/// `parse_task_contract` expects, with a `*(hash)*` commit note on the
/// checked task line (required for the bound-convergence prerequisite).
fn contract_block(task: &str, hash: &str) -> String {
    format!(
        "- [x] {task} — placeholder. *({hash})*\n  \
         - Test-first: required\n  \
         - Seam: `crates/cli/src/commands/finalize_check.rs`\n  \
         - Expected failures:\n    \
         - EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64=bmVlZGxl — locked\n  \
         - Production Paths: `crates/cli/src/commands/finalize_check.rs`\n  \
         - Test Paths: `crates/cli/tests/test_first_finalize.rs`\n  \
         - Scaffold: not-required\n"
    )
}

/// Writes matching source plan + execution prompt bodies under
/// `.dev/plans/<slug>.{md,prompt.md}`: `marked` controls the test-first
/// marker, cursor is always cleared so only the marker and evidence vary
/// between tests. Both surfaces get byte-identical bodies so three-surface
/// agreement holds trivially.
fn write_plan_and_prompt(repo: &Path, slug: &str, marked: bool, tasks_body: &str) -> PathBuf {
    write_plan_and_prompt_with_status(repo, slug, marked, "", tasks_body)
}

fn write_plan_and_prompt_with_status(
    repo: &Path,
    slug: &str,
    marked: bool,
    extra_status: &str,
    tasks_body: &str,
) -> PathBuf {
    let marker = if marked {
        "\n\nPipeline Contract: test-first-v1"
    } else {
        ""
    };
    let body = format!(
        "# Plan Prompt: {slug}{marker}\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n{extra_status}\n## Tasks\n\n{tasks_body}\n"
    );
    let plans_dir = repo.join(".dev").join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    fs::write(plans_dir.join(format!("{slug}.md")), &body).unwrap();
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &body).unwrap();
    prompt_path
}

// ── (1) exact evaluation-field content, not exit-code comparison ──────────

#[test]
fn marked_task_missing_evidence_fails_with_exact_evaluation_fields() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-marked-plan-1";
    let tasks_body = contract_block(&task, &hash);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, true, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let line = extract_receipt_line(&receipt, "test-first-evaluation")
        .unwrap_or_else(|| panic!("no test-first-evaluation row in receipt: {receipt}"));
    assert!(line.contains("| fail |"), "{line}");
    // Exact field values, not a bare pass/fail read: verdict, plan_slug and
    // task_id must each name this run's real evaluator output.
    assert!(
        line.contains("verdict:Fail") || line.contains("verdict=Fail"),
        "{line}"
    );
    assert!(
        line.contains(&format!("plan_slug:{slug}")) || line.contains(&format!("plan_slug={slug}")),
        "{line}"
    );
    assert!(
        line.contains(&format!("task_id:{task}")) || line.contains(&format!("task_id={task}")),
        "{line}"
    );
    // The executor-log-scan row must also report this exact task as the
    // reason the whole check(g) failed — proving the evaluation is wired
    // into the failure path, not merely computed and discarded.
    let scan_line = extract_receipt_line(&receipt, "executor-log-scan").unwrap();
    assert!(scan_line.contains(&task), "{scan_line}");
    assert!(
        scan_line.contains("test-first evaluation failed"),
        "{scan_line}"
    );
}

// ── (2) markerless checked task never gets a test-first-evaluation row ────

#[test]
fn markerless_checked_task_produces_no_evaluation_row() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-markerless-plan-1";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    assert!(
        extract_receipt_line(&receipt, "test-first-evaluation").is_none(),
        "a markerless plan must never gain a test-first-evaluation row: {receipt}"
    );
    let scan_line = extract_receipt_line(&receipt, "executor-log-scan").unwrap();
    assert!(scan_line.contains("| pass |"), "{scan_line}");
}

// ── (2b) R2: the `test-results-structure` gate must be decided ────────────
// ── before citation harvesting. Zero or two unfenced `## Test Results`  ──
// ── headings fail the new structure row with the observed count and    ──
// ── force `cited-test-existence` to `not-run`; a fenced-only heading    ──
// ── counts as zero; exactly one heading passes structure and lets       ──
// ── citation harvesting proceed as before.                              ──
//
// This gate does not exist yet — `cited_test_names` still opens its window
// on the *first* line matching `## Test Results` and never counts headings,
// so every assertion below on the `test-results-structure` row is expected
// red: the row is absent from the receipt entirely.

/// Locked EF-01 acceptance probe. A dual-heading fixture (the exact defect
/// `## Evidence Baseline` and R2 describe) must produce a failing
/// `test-results-structure` row naming count 2, and `cited-test-existence`
/// must read `not-run` rather than falling through to the old first-window
/// scan and reporting false missing names.
#[test]
fn finalize_structure_gate_dual_heading_fails_before_citation_harvest() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-structure-dual-heading";
    let tasks_body = format!(
        "- [x] {task} — legacy placeholder. *({hash})*\n\n\
         ## Test Results\n\n`this_test_does_not_exist_anywhere_xyz` PASS\n\n\
         ## Test Results\n\n`another_missing_probe_zzz` PASS\n"
    );
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let structure_line = extract_receipt_line(&receipt, "test-results-structure")
        .unwrap_or_else(|| panic!("no test-results-structure row in receipt: {receipt}"));
    assert!(structure_line.contains("| fail |"), "{structure_line}");
    assert!(
        structure_line.contains('2'),
        "structure row must name the observed count of 2: {structure_line}"
    );

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(
        citation_line.contains("| not-run |"),
        "a structure failure must gate citation evaluation to not-run: {citation_line}"
    );
    assert!(
        !citation_line.to_ascii_lowercase().contains("missing"),
        "structure failure must never fall through to a false missing-name failure: {citation_line}"
    );
}

#[test]
fn finalize_structure_gate_zero_headings_fails_with_count_zero() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-structure-zero-headings";
    let tasks_body = format!(
        "- [x] {task} — legacy placeholder. *({hash})*\n\n\
         No results section exists in this plan at all.\n"
    );
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let structure_line = extract_receipt_line(&receipt, "test-results-structure")
        .unwrap_or_else(|| panic!("no test-results-structure row in receipt: {receipt}"));
    assert!(structure_line.contains("| fail |"), "{structure_line}");
    assert!(
        structure_line.contains('0'),
        "structure row must name the observed count of 0: {structure_line}"
    );

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(
        citation_line.contains("| not-run |"),
        "zero headings must gate citation evaluation to not-run: {citation_line}"
    );
}

#[test]
fn finalize_structure_gate_fenced_only_heading_counts_as_zero() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-structure-fenced-only";
    let tasks_body = format!(
        "- [x] {task} — legacy placeholder. *({hash})*\n\n\
         ```text\n## Test Results\n```\n\n\
         A fenced mention of the heading must never open the real section.\n"
    );
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let structure_line = extract_receipt_line(&receipt, "test-results-structure")
        .unwrap_or_else(|| panic!("no test-results-structure row in receipt: {receipt}"));
    assert!(structure_line.contains("| fail |"), "{structure_line}");
    assert!(
        structure_line.contains('0'),
        "a fenced-only heading must count as zero unfenced headings: {structure_line}"
    );

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(
        citation_line.contains("| not-run |"),
        "a fenced-only heading must gate citation evaluation to not-run: {citation_line}"
    );
}

#[test]
fn finalize_structure_gate_exactly_one_heading_passes_and_citation_proceeds() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-structure-exactly-one";
    let tasks_body = format!(
        "- [x] {task} — legacy placeholder. *({hash})*\n\n\
         ## Test Results\n\n`this_fn_absolutely_does_not_exist_zzz` PASS\n"
    );
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let structure_line = extract_receipt_line(&receipt, "test-results-structure")
        .unwrap_or_else(|| panic!("no test-results-structure row in receipt: {receipt}"));
    assert!(structure_line.contains("| pass |"), "{structure_line}");
    assert!(
        structure_line.contains('1'),
        "structure row must name the observed count of 1: {structure_line}"
    );

    // Structure passed, so citation evaluation must actually run rather than
    // being gated to not-run — proven by real harvest output (the fabricated
    // identifier reported missing), not a bare pass/fail read.
    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(
        !citation_line.contains("| not-run |"),
        "exactly one heading must let citation evaluation proceed: {citation_line}"
    );
    assert!(
        citation_line.contains("this_fn_absolutely_does_not_exist_zzz"),
        "citation evaluation must have actually harvested and resolved the cited \
         identifier, not merely passed through: {citation_line}"
    );

    // R2: the structure gate is wired before citation harvesting — the
    // rendered receipt must list the structure row ahead of the citation row.
    let structure_pos = receipt.find(structure_line).unwrap();
    let citation_pos = receipt.find(citation_line).unwrap();
    assert!(
        structure_pos < citation_pos,
        "test-results-structure must be wired before cited-test-existence in the receipt"
    );
}

// ── (/ R4) authoritative-command rows must carry bounded stdout/stderr ─
// ── plus original `stderr_bytes=` counts, not a bare `exit N` summary.     ─

#[test]
fn finalize_authoritative_command_failure_reports_bounded_stderr_bytes() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-authoritative-stderr-bytes";
    let tasks_body = format!(
        "- [x] {task} — legacy placeholder. *({hash})*\n\n\
         ## Test Results\n\n`this_fn_absolutely_does_not_exist_zzz` PASS\n"
    );
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    // Oracle: capture the exact bytes the failing command itself writes to
    // stderr, invoked the same verbatim whitespace-split-argv way
    // `run_command_verbatim` invokes it (no shell). This keeps the expected
    // byte count environment-derived rather than a hardcoded literal that
    // would drift with the installed `git` version's message text.
    let failing_command = "git this-is-not-a-real-git-subcommand-zzz";
    let oracle = Command::new("git")
        .arg("this-is-not-a-real-git-subcommand-zzz")
        .output()
        .unwrap();
    assert!(!oracle.status.success(), "fixture command must fail");
    let expected_stderr_bytes = oracle.stderr.len();
    assert!(
        expected_stderr_bytes > 0,
        "fixture command must emit stderr"
    );

    let project_md = format!(
        "<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"{failing_command}\"]\n}}\n```\n"
    );
    let dev_dir = tmp.path().join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    fs::write(dev_dir.join("project.md"), project_md).unwrap();

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let row = extract_receipt_line(&receipt, "authoritative-command")
        .unwrap_or_else(|| panic!("no authoritative-command row in receipt: {receipt}"));
    assert!(
        row.contains("| fail |"),
        "a nonzero exit must fail the row: {row}"
    );
    assert!(
        row.contains(&format!("stderr_bytes={expected_stderr_bytes}")),
        "row must carry stderr_bytes={expected_stderr_bytes} (the original count), \
         not a bare exit code: {row}"
    );
    assert!(
        row.contains("this-is-not-a-real-git-subcommand-zzz"),
        "the bounded stream must name the real failing command, not just exit N: {row}"
    );
}

// ── (3) target read is validated: a linked path component is rejected ─────
// ── before any bytes are read, and no receipt is written.               ──

#[cfg(windows)]
#[test]
fn finalize_check_target_read_rejects_junctioned_path() {
    let tmp = make_git_repo();
    let repo_root = tmp.path();

    let real_dir = repo_root.join("real_plans");
    fs::create_dir_all(&real_dir).unwrap();
    let real_prompt = real_dir.join("junctioned.prompt.md");
    fs::write(
        &real_prompt,
        "# Plan Prompt: junctioned\n\n## Status\n\nCurrent Task: —\n\n## Tasks\n\n",
    )
    .unwrap();

    let dev_dir = repo_root.join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    let junction_dir = dev_dir.join("plans");
    let output = Command::new("cmd")
        .args([
            "/c",
            "mklink",
            "/J",
            &junction_dir.to_string_lossy(),
            &real_dir.to_string_lossy(),
        ])
        .output()
        .expect("cmd must be available on Windows");
    assert!(
        output.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let receipt_path = repo_root.join("finalize.receipt.md");
    let target = junction_dir.join("junctioned.prompt.md");
    let out = run_gal(
        repo_root,
        &[
            "finalize-check",
            target.to_str().unwrap(),
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(64),
        "a junctioned target must fail closed with ExitCode::Usage, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("target validation failed"),
        "stderr must name the validated-read rejection, not a generic error: {stderr}"
    );
    assert!(
        stderr.contains("symlink, junction, or reparse point"),
        "stderr must name the actual reason: {stderr}"
    );
    assert!(
        !receipt_path.exists(),
        "a rejected target must never reach receipt creation"
    );
}

#[cfg(unix)]
#[test]
fn finalize_check_target_read_rejects_symlinked_path() {
    let tmp = make_git_repo();
    let repo_root = tmp.path();

    let real_dir = repo_root.join("real_plans");
    fs::create_dir_all(&real_dir).unwrap();
    let real_prompt = real_dir.join("linked.prompt.md");
    fs::write(
        &real_prompt,
        "# Plan Prompt: linked\n\n## Status\n\nCurrent Task: —\n\n## Tasks\n\n",
    )
    .unwrap();

    let dev_dir = repo_root.join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    let link_dir = dev_dir.join("plans");
    std::os::unix::fs::symlink(&real_dir, &link_dir).unwrap();

    let receipt_path = repo_root.join("finalize.receipt.md");
    let target = link_dir.join("linked.prompt.md");
    let out = run_gal(
        repo_root,
        &[
            "finalize-check",
            target.to_str().unwrap(),
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(64),
        "a symlinked target must fail closed with ExitCode::Usage, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("target validation failed"),
        "stderr must name the validated-read rejection, not a generic error: {stderr}"
    );
    assert!(
        !receipt_path.exists(),
        "a rejected target must never reach receipt creation"
    );
}

// ── (4) non-UTF-8 target content is rejected explicitly, not silently ─────
// ── swallowed into an empty string (the old `unwrap_or_default()` path). ──

#[test]
fn finalize_check_rejects_non_utf8_target_instead_of_silently_reading_empty() {
    let tmp = make_git_repo();
    let repo_root = tmp.path();
    let plans_dir = repo_root.join(".dev").join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    let prompt_path = plans_dir.join("binary-garbage.prompt.md");
    // Invalid UTF-8: a lone continuation byte.
    fs::write(&prompt_path, [0x23, 0x20, 0xFF, 0xFE, 0x00]).unwrap();

    let receipt_path = repo_root.join("finalize.receipt.md");
    let out = run_gal(
        repo_root,
        &[
            "finalize-check",
            prompt_path.to_str().unwrap(),
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );

    assert_eq!(
        out.status.code(),
        Some(64),
        "non-UTF-8 target must fail closed with ExitCode::Usage, got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("target is not UTF-8"),
        "stderr must name the exact rejection reason, not silently treat the file as \
         empty text: {stderr}"
    );
    assert!(
        !receipt_path.exists(),
        "a rejected target must never reach receipt creation"
    );
}

// ── (5) full mode is restricted to canonical `*.prompt.md` execution      ─
// ── prompts; a source-plan target must fail before any authoritative      ─
// ── command runs, a canonical prompt target must still proceed, and       ─
// ── hygiene-only mode's existing lifecycle-target contract is unaffected. ─

#[test]
fn finalize_full_mode_rejects_source_plan_before_authoritative_commands() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    commit_all(tmp.path(), "init");

    // Side-effect sentinel: an authoritative command that creates a real,
    // externally observable Git tag. If full mode ever spawns the
    // authoritative-command list for a rejected target, the tag exists.
    let sentinel_tag = "canonical-target-sentinel-source-plan";
    let project_md = format!(
        "<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"git tag {sentinel_tag}\"]\n}}\n```\n"
    );
    let dev_dir = tmp.path().join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    fs::write(dev_dir.join("project.md"), project_md).unwrap();

    // A source plan (`.dev/plans/<slug>.md`), not an execution prompt.
    let slug = "finalize-canonical-target-source-plan";
    let plans_dir = dev_dir.join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    let source_plan_path = plans_dir.join(format!("{slug}.md"));
    fs::write(
        &source_plan_path,
        "# Plan: finalize-canonical-target-source-plan\n\n## Status\n\nCurrent Task: —\n\n## Tasks\n\n",
    )
    .unwrap();

    let receipt_path = tmp.path().join("finalize.receipt.md");
    let out = run_gal(
        tmp.path(),
        &[
            "finalize-check",
            source_plan_path.to_str().unwrap(),
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );

    // Error-first: the locked-first assertion is the diagnostic text itself
    // (EF-04, stream=stderr, matcher "execution prompt"), checked before the
    // exit-code and receipt-absence assertions below.
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("execution prompt"),
        "stderr must name the execution prompt requirement, not a generic error: {stderr}"
    );
    assert_eq!(
        out.status.code(),
        Some(64),
        "a source-plan target in full mode must fail closed with ExitCode::Usage before any \
         command runs, got: stdout={}, stderr={stderr}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert!(
        !receipt_path.exists(),
        "a rejected target must never reach receipt creation"
    );

    let tags = Command::new("git")
        .current_dir(tmp.path())
        .args(["tag", "--list"])
        .output()
        .unwrap();
    let tag_list = String::from_utf8_lossy(&tags.stdout);
    assert!(
        !tag_list.contains(sentinel_tag),
        "authoritative commands must never run for a rejected source-plan target: {tag_list}"
    );
}

#[test]
fn finalize_full_mode_accepts_canonical_prompt_and_runs_authoritative_commands() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    commit_all(tmp.path(), "init");

    let sentinel_tag = "canonical-target-sentinel-canonical-prompt";
    let project_md = format!(
        "<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"git tag {sentinel_tag}\"]\n}}\n```\n"
    );
    let dev_dir = tmp.path().join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    fs::write(dev_dir.join("project.md"), project_md).unwrap();

    let slug = "finalize-canonical-target-canonical-prompt";
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, "");

    let (_output, _receipt) = finalize_check(tmp.path(), &prompt_path);

    let tags = Command::new("git")
        .current_dir(tmp.path())
        .args(["tag", "--list"])
        .output()
        .unwrap();
    let tag_list = String::from_utf8_lossy(&tags.stdout);
    assert!(
        tag_list.contains(sentinel_tag),
        "a canonical execution prompt (.dev/plans/<slug>.prompt.md) must still reach \
         authoritative-command execution: {tag_list}"
    );
}

#[test]
fn finalize_full_mode_rejects_non_canonical_prompt_path_before_authoritative_commands() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    commit_all(tmp.path(), "init");

    let sentinel_tag = "canonical-target-sentinel-non-canonical-prompt";
    let project_md = format!(
        "<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"git tag {sentinel_tag}\"]\n}}\n```\n"
    );
    let dev_dir = tmp.path().join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    fs::write(dev_dir.join("project.md"), project_md).unwrap();

    let docs_dir = tmp.path().join("docs");
    fs::create_dir_all(&docs_dir).unwrap();
    let prompt_path = docs_dir.join("notes.prompt.md");
    fs::write(
        &prompt_path,
        "# Plan Prompt: notes\n\n## Status\n\nCurrent Task: —\n\n## Tasks\n\n",
    )
    .unwrap();

    let receipt_path = tmp.path().join("finalize.receipt.md");
    let out = run_gal(
        tmp.path(),
        &[
            "finalize-check",
            prompt_path.to_str().unwrap(),
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );

    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("execution prompt"),
        "stderr must name the execution prompt requirement, not a generic error: {stderr}"
    );
    assert_eq!(
        out.status.code(),
        Some(64),
        "a non-canonical prompt path target in full mode must fail closed with ExitCode::Usage before any command runs"
    );
    assert!(
        !receipt_path.exists(),
        "a rejected target must never reach receipt creation"
    );

    let tags = Command::new("git")
        .current_dir(tmp.path())
        .args(["tag", "--list"])
        .output()
        .unwrap();
    let tag_list = String::from_utf8_lossy(&tags.stdout);
    assert!(
        !tag_list.contains(sentinel_tag),
        "authoritative commands must never run for a non-canonical prompt path target: {tag_list}"
    );
}

#[test]
fn finalize_hygiene_mode_still_accepts_non_canonical_target() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    commit_all(tmp.path(), "init");

    // A source plan (`.dev/plans/<slug>.md`), the same non-canonical shape
    // full mode rejects. Hygiene-only mode's lifecycle-target contract
    // predates that restriction and must not be narrowed by it.
    let slug = "finalize-canonical-target-hygiene-non-canonical";
    let plans_dir = tmp.path().join(".dev").join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    let source_plan_path = plans_dir.join(format!("{slug}.md"));
    fs::write(
        &source_plan_path,
        "# Plan: finalize-canonical-target-hygiene-non-canonical\n\n## Status\n\nCurrent Task: —\n\n## Tasks\n\n",
    )
    .unwrap();
    commit_all(tmp.path(), "add source plan");

    let (_output, receipt) = finalize_check_hygiene(tmp.path(), &source_plan_path);

    assert!(
        !receipt.is_empty(),
        "hygiene-only mode must still evaluate a non-canonical target and write a receipt, \
         not adopt full mode's new rejection"
    );
    assert!(
        !receipt.contains("execution prompt"),
        "the new full-mode-only execution-prompt rejection must not leak into hygiene-only \
         receipts: {receipt}"
    );
    assert!(
        receipt_check_names(&receipt).contains(&"state-bound"),
        "hygiene-only evaluation of a non-canonical target must still reach its normal rows: \
         {receipt}"
    );
}

// `extract_plan_slug` is exercised indirectly through the `plan_slug:` field
// assertion above; keep the import used explicitly so a future refactor that
// stops needing it here is caught by an unused-import warning rather than
// silently going stale.
#[test]
fn plan_slug_helper_matches_the_slug_this_file_writes_plans_under() {
    let slug = extract_plan_slug(".dev/plans/finalize-marked-plan-1.prompt.md");
    assert_eq!(slug, "finalize-marked-plan-1");
}

fn encode_argv(args: &[&str]) -> String {
    pipeline::task_spec::probe_evidence::encode_argv_b64(
        &args.iter().map(|a| (*a).to_string()).collect::<Vec<_>>(),
    )
    .unwrap()
}

fn write_attempt_log(dir: &Path, task: &str, phase: &str, terminal_state: &str, seq: u32) {
    fs::create_dir_all(dir).unwrap();
    let filename = format!(
        "{:010}-000000001-{seq:06}-{task}-{phase}-claude.log",
        1700000000 + seq as u64
    );
    let content = format!(
        "GAL-DISPATCH-LOG v1\n\
         timestamp_start: 2026-08-21T00:00:00Z\n\
         timestamp_end:   2026-08-21T00:00:01Z\n\
         duration_ms:     1\n\
         executor:        claude\n\
         phase:           {phase}\n\
         task_id:         {task}\n\
         git_branch:      main\n\
         git_head:        abc1234\n\
         exit_code:       0\n\
         actual_model:    m\n\
         terminal_state:  {terminal_state}\n\
         session_id:      none\n\
         ---STDOUT---\n\
         \n\
         ---STDERR---\n\
         \n"
    );
    fs::write(dir.join(filename), content).unwrap();
}

fn converge_check(repo: &Path, prompt: &Path, task: &str) -> (std::process::Output, String) {
    let receipt_path = repo.join("converge.receipt.md");
    let output = run_gal(
        repo,
        &[
            "pipeline-converge-check",
            prompt.to_str().unwrap(),
            "--task",
            task,
            "--receipt",
            receipt_path.to_str().unwrap(),
        ],
    );
    let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
    (output, receipt)
}

// ── (5) exact evaluation equality between converge-check and finalize-check ──

#[test]
fn finalize_check_evaluation_matches_converge_check_exact_fields() {
    let tmp = make_git_repo();
    let repo = tmp.path().to_path_buf();
    fs::write(repo.join("README.md"), "x").unwrap();
    let hash = commit_all(&repo, "init");
    let task = tid(4);
    let matcher = b"needle";
    let ef = format!(
        "EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64={} — locked",
        pipeline::task_spec::encode_base64url_unpadded(matcher)
    );
    let tasks_body = contract_block(&task, &hash);
    let slug = "finalize-eval-parity";

    let prompt = write_plan_and_prompt(&repo, slug, true, &tasks_body);
    let prompt_text = fs::read_to_string(&prompt).unwrap();
    let plan_slug = extract_plan_slug(&prompt.to_string_lossy());
    let contract = parse_task_contract(&prompt_text, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let prompt_path =
        write_plan_and_prompt_with_status(&repo, slug, true, &extra_status, &tasks_body);

    let boundary_path = repo
        .join(".dev/pipeline/receipts")
        .join(slug)
        .join(format!("{task}-boundary-check.receipt.md"));
    fs::create_dir_all(boundary_path.parent().unwrap()).unwrap();
    fs::write(
        &boundary_path,
        "# boundary-check receipt\n\noverall: pass\n",
    )
    .unwrap();

    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let script = "test -f fixed.marker && exit 0 || { printf needle; exit 1; }";
    let argv = encode_argv(&["sh", "-c", script]);

    let red = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "test",
            "red",
            "P-01",
            "acceptance",
            &argv,
            "2000",
            "--expected-failure",
            &ef,
        ],
    );
    assert!(
        red.status.success(),
        "red probe failed: {}",
        String::from_utf8_lossy(&red.stderr)
    );

    fs::write(repo.join("fixed.marker"), b"").unwrap();
    let green = run_gal(
        &repo,
        &[
            "test-first-probe",
            "run",
            &plan_slug,
            &task,
            "1",
            &digest,
            "green-rerun",
            "green",
            "P-01",
            "acceptance",
            &argv,
            "2000",
            "--expected-failure",
            &ef,
        ],
    );
    assert!(
        green.status.success(),
        "green probe failed: {}",
        String::from_utf8_lossy(&green.stderr)
    );

    let (_converge_out, converge_receipt) = converge_check(&repo, &prompt_path, &task);
    let (_finalize_out, finalize_receipt) = finalize_check(&repo, &prompt_path);

    let converge_line = extract_receipt_line(&converge_receipt, "test-first-evaluation")
        .expect("converge receipt must contain test-first-evaluation row");
    let finalize_line = extract_receipt_line(&finalize_receipt, "test-first-evaluation")
        .expect("finalize receipt must contain test-first-evaluation row");

    assert_eq!(
        converge_line, finalize_line,
        "exact evaluation equality required between converge-check and finalize-check"
    );
    assert!(finalize_line.contains("| pass |"), "{finalize_line}");
}

// ── (6) reject linked evidence artifact paths before byte read ────────────────

#[cfg(windows)]
#[test]
fn finalize_check_rejects_linked_probe_receipt_before_reading_bytes() {
    let tmp = make_git_repo();
    let repo = tmp.path().to_path_buf();
    fs::write(repo.join("README.md"), "x").unwrap();
    let hash = commit_all(&repo, "init");
    let task = tid(5);
    let slug = "finalize-linked-receipt";

    let tasks_body = contract_block(&task, &hash);

    let prompt_text_raw = format!(
        "# Plan Prompt: {slug}\n\nPipeline Contract: test-first-v1\n\n## Status\n\nCurrent Task: —\n\n## Tasks\n\n{tasks_body}\n"
    );
    let plans_dir = repo.join(".dev").join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    fs::write(plans_dir.join(format!("{slug}.md")), &prompt_text_raw).unwrap();
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_text_raw).unwrap();

    let plan_slug = extract_plan_slug(&prompt_path.to_string_lossy());
    let contract = parse_task_contract(&prompt_text_raw, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let prompt_path =
        write_plan_and_prompt_with_status(&repo, slug, true, &extra_status, &tasks_body);

    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    // Create real receipt dir and file in an external location, then junction the receipt dir
    let external_dir = repo.join("external_receipts");
    fs::create_dir_all(&external_dir).unwrap();
    fs::write(
        external_dir.join("probe-red.receipt.md"),
        format!("plan={slug}\ntask={task}\ngeneration=1\ncontract_digest={digest}\nphase=test\nexpectation=red\nrunner_contract=test-first-probe-v1\nprobe_set_digest=00\nverdict=pass\n\n"),
    ).unwrap();

    let receipt_target_dir = repo
        .join(".dev")
        .join("pipeline")
        .join("receipts")
        .join(slug)
        .join(&task)
        .join(format!("g1-c{digest}"));
    fs::create_dir_all(receipt_target_dir.parent().unwrap()).unwrap();

    let link_arg = receipt_target_dir.to_string_lossy().replace('/', "\\");
    let target_arg = external_dir.to_string_lossy().replace('/', "\\");

    let output = Command::new("cmd")
        .args(["/c", "mklink", "/J", &link_arg, &target_arg])
        .output()
        .expect("cmd available");
    assert!(
        output.status.success(),
        "mklink /J failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let (_out, receipt) = finalize_check(&repo, &prompt_path);

    let eval_line = extract_receipt_line(&receipt, "test-first-evaluation")
        .expect("receipt must contain test-first-evaluation row");
    assert!(eval_line.contains("| fail |"), "{eval_line}");
    assert!(
        eval_line.contains("symlink, junction, or reparse point")
            || eval_line.contains("invalid probe receipt"),
        "linked receipt path must be rejected before byte read: {eval_line}"
    );
}

#[cfg(unix)]
#[test]
fn finalize_check_rejects_linked_probe_receipt_before_reading_bytes() {
    let tmp = make_git_repo();
    let repo = tmp.path().to_path_buf();
    fs::write(repo.join("README.md"), "x").unwrap();
    let hash = commit_all(&repo, "init");
    let task = tid(5);
    let slug = "finalize-linked-receipt";

    let tasks_body = contract_block(&task, &hash);
    let prompt_text_raw = format!(
        "# Plan Prompt: {slug}\n\nPipeline Contract: test-first-v1\n\n## Status\n\nCurrent Task: —\n\n## Tasks\n\n{tasks_body}\n"
    );
    let plans_dir = repo.join(".dev").join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    fs::write(plans_dir.join(format!("{slug}.md")), &prompt_text_raw).unwrap();
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_text_raw).unwrap();

    let plan_slug = extract_plan_slug(&prompt_path.to_string_lossy());
    let contract = parse_task_contract(&prompt_text_raw, &plan_slug, &task).unwrap();
    let digest = contract.compute_digest();

    let extra_status = format!(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n| {task} | 1 | {digest} | init |\n\n### Deviations\n\n| Date | Task | What | Why |\n| --- | --- | --- | --- |\n"
    );
    let prompt_path =
        write_plan_and_prompt(&repo, slug, true, &format!("{extra_status}\n{tasks_body}"));

    let log_dir = repo.join(".dev/executor-logs").join(slug);
    write_attempt_log(&log_dir, &task, "implement", "completed", 1);
    write_attempt_log(&log_dir, &task, "test", "completed", 2);
    write_attempt_log(&log_dir, &task, "audit", "completed", 3);

    let external_dir = repo.join("external_receipts");
    fs::create_dir_all(&external_dir).unwrap();
    fs::write(
        external_dir.join("probe-red.receipt.md"),
        format!("plan={slug}\ntask={task}\ngeneration=1\ncontract_digest={digest}\nphase=test\nexpectation=red\nrunner_contract=test-first-probe-v1\nprobe_set_digest=00\nverdict=pass\n\n"),
    ).unwrap();

    let receipt_target_dir = repo
        .join(".dev/pipeline/receipts")
        .join(slug)
        .join(&task)
        .join(format!("g1-c{digest}"));
    fs::create_dir_all(receipt_target_dir.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&external_dir, &receipt_target_dir).unwrap();

    let (_out, receipt) = finalize_check(&repo, &prompt_path);

    let eval_line = extract_receipt_line(&receipt, "test-first-evaluation")
        .expect("receipt must contain test-first-evaluation row");
    assert!(eval_line.contains("| fail |"), "{eval_line}");
    assert!(
        eval_line.contains("symlink, junction, or reparse point")
            || eval_line.contains("invalid probe receipt"),
        "linked receipt path must be rejected before byte read: {eval_line}"
    );
}

// ── (7) R3: the strict Citation Reconciliation exception ledger. This ──────
// ── table does not exist yet — `cited_test_names` harvests every backtick ──
// ── token inside the whole results section, including reconciliation-     ──
// ── table cells, as an ordinary citation, and `check_cited_test_names` has ──
// ── no concept of a disposition ledger at all. So every assertion below is ──
// ── expected red: a validly-discharged token still reports as `missing`,   ──
// ── and every defect row never produces a `citation-reconciliation` marker ──
// ── anywhere in the failing `cited-test-existence` summary.

/// Builds a `## Test Results` section for one checked task: `cited_lines`
/// simulates the task's own real-evidence prose citing zero or more legacy
/// tokens, and `table_rows` are the `### Citation Reconciliation` table's
/// data rows (already `| ... |`-formatted, one per line, no trailing
/// newline).
fn reconciliation_fixture(task: &str, hash: &str, cited_lines: &str, table_rows: &str) -> String {
    format!(
        "- [x] {task} — legacy placeholder. *({hash})*\n\n\
         ## Test Results\n\n\
         {cited_lines}\n\n\
         ### Citation Reconciliation\n\n\
         | Citation | Disposition | Replacement | Rationale |\n\
         | --- | --- | --- | --- |\n\
         {table_rows}\n"
    )
}

#[test]
fn finalize_citation_reconciliation_valid_renamed_row_discharges_token() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    // A real fn under crates/ — the resolvable replacement `renamed` demands.
    fs::create_dir_all(tmp.path().join("crates")).unwrap();
    fs::write(
        tmp.path().join("crates").join("replacement.rs"),
        "fn replacement_probe_fn_zzz() {}\n",
    )
    .unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-valid-renamed";
    let cited = "The legacy suite cited `renamed_from_this_old_name_zzz` before the rename.";
    let row = "| `renamed_from_this_old_name_zzz` | renamed | `replacement_probe_fn_zzz` | \
               moved into the fn/mod/file/whole-word resolver rung |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, row);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(
        citation_line.contains("| pass |"),
        "a valid `renamed` row with a resolvable replacement must discharge the \
         unresolved token and let citation evaluation pass: {citation_line}"
    );
    assert!(
        !citation_line.to_ascii_lowercase().contains("missing"),
        "a discharged token must never be reported missing: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_valid_retired_row_discharges_token() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-valid-retired";
    let cited = "The old suite cited `retired_probe_from_history_zzz`.";
    let row = "| `retired_probe_from_history_zzz` | retired | — | the probe was deleted \
               when the legacy harness was replaced |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, row);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(
        citation_line.contains("| pass |"),
        "a valid `retired` row with a non-placeholder rationale must discharge the \
         unresolved token: {citation_line}"
    );
    assert!(
        !citation_line.to_ascii_lowercase().contains("missing"),
        "a discharged token must never be reported missing: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_valid_non_test_row_discharges_token() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-valid-non-test";
    let cited = "Historical notes mention `config_flag_non_test_zzz`.";
    let row = "| `config_flag_non_test_zzz` | non-test | — | this token names a config \
               flag, not a test |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, row);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(
        citation_line.contains("| pass |"),
        "a valid `non-test` row with a non-placeholder rationale must discharge the \
         unresolved token: {citation_line}"
    );
    assert!(
        !citation_line.to_ascii_lowercase().contains("missing"),
        "a discharged token must never be reported missing: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_duplicate_row_fails_naming_defect() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-duplicate";
    let cited = "The old suite cited `dup_legacy_token_zzz`.";
    let rows = "| `dup_legacy_token_zzz` | retired | — | deleted with the legacy harness |\n\
                | `dup_legacy_token_zzz` | retired | — | deleted with the legacy harness |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, rows);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(citation_line.contains("| fail |"), "{citation_line}");
    assert!(
        citation_line
            .to_ascii_lowercase()
            .contains("citation-reconciliation"),
        "a duplicate reconciliation row must fail naming the citation-reconciliation \
         defect, not a generic missing-name failure: {citation_line}"
    );
    assert!(
        citation_line.contains("dup_legacy_token_zzz"),
        "the failure must name the offending citation: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_extra_row_fails_naming_defect() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-extra";
    let cited = "Nothing legacy is cited in this task's own evidence prose.";
    let rows =
        "| `never_actually_cited_anywhere_zzz` | retired | — | deleted with the legacy harness |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, rows);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(citation_line.contains("| fail |"), "{citation_line}");
    assert!(
        citation_line
            .to_ascii_lowercase()
            .contains("citation-reconciliation"),
        "a reconciliation row for a token that was never harvested as unresolved must \
         fail naming the citation-reconciliation defect, not a generic missing-name \
         failure: {citation_line}"
    );
    assert!(
        citation_line.contains("never_actually_cited_anywhere_zzz"),
        "the failure must name the offending extra row: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_missing_row_fails_naming_defect() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-missing";
    let cited = "The suite cited `discharged_token_zzz` and `orphaned_token_zzz`.";
    let rows = "| `discharged_token_zzz` | retired | — | deleted with the legacy harness |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, rows);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(citation_line.contains("| fail |"), "{citation_line}");
    assert!(
        citation_line
            .to_ascii_lowercase()
            .contains("citation-reconciliation"),
        "an unresolved token with no covering reconciliation row must fail naming the \
         citation-reconciliation defect, not the ordinary missing-name failure: \
         {citation_line}"
    );
    assert!(
        citation_line.contains("orphaned_token_zzz"),
        "the failure must name the uncovered token: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_malformed_row_fails_naming_defect() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-malformed";
    let cited = "The suite cited `malformed_row_token_zzz`.";
    // Missing the Rationale column entirely — three cells instead of four.
    let rows = "| `malformed_row_token_zzz` | retired | — |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, rows);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(citation_line.contains("| fail |"), "{citation_line}");
    assert!(
        citation_line
            .to_ascii_lowercase()
            .contains("citation-reconciliation"),
        "a malformed (wrong column count) reconciliation row must fail naming the \
         citation-reconciliation defect: {citation_line}"
    );
    assert!(
        citation_line.contains("malformed_row_token_zzz"),
        "the failure must name the offending row: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_placeholder_rationale_fails_naming_defect() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-placeholder";
    let cited = "The suite cited `placeholder_rationale_token_zzz`.";
    let rows = "| `placeholder_rationale_token_zzz` | retired | — | TBD |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, rows);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(citation_line.contains("| fail |"), "{citation_line}");
    assert!(
        citation_line
            .to_ascii_lowercase()
            .contains("citation-reconciliation"),
        "a placeholder rationale (`TBD`) must fail naming the citation-reconciliation \
         defect: {citation_line}"
    );
    assert!(
        citation_line.contains("placeholder_rationale_token_zzz"),
        "the failure must name the offending row: {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_unresolvable_replacement_fails_naming_defect() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-unresolvable-replacement";
    let cited = "The suite cited `unresolvable_replacement_token_zzz`.";
    let rows = "| `unresolvable_replacement_token_zzz` | renamed | \
                `totally_fake_replacement_fn_xyz` | moved during the rewrite |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, rows);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(citation_line.contains("| fail |"), "{citation_line}");
    assert!(
        citation_line
            .to_ascii_lowercase()
            .contains("citation-reconciliation"),
        "a `renamed` row whose replacement does not resolve must fail naming the \
         citation-reconciliation defect: {citation_line}"
    );
    assert!(
        citation_line.contains("unresolvable_replacement_token_zzz")
            || citation_line.contains("totally_fake_replacement_fn_xyz"),
        "the failure must name the offending citation or its unresolvable replacement: \
         {citation_line}"
    );
}

#[test]
fn finalize_citation_reconciliation_already_resolvable_token_fails_naming_defect() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    // A real fn under crates/ — this citation already resolves through the
    // ordinary ladder and never needed a reconciliation row at all.
    fs::create_dir_all(tmp.path().join("crates")).unwrap();
    fs::write(
        tmp.path().join("crates").join("already_resolvable.rs"),
        "fn already_resolvable_probe_fn_zzz() {}\n",
    )
    .unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-reconciliation-already-resolvable";
    let cited = "The suite cites `already_resolvable_probe_fn_zzz` directly.";
    let rows = "| `already_resolvable_probe_fn_zzz` | retired | — | no longer needed |";
    let tasks_body = reconciliation_fixture(&task, &hash, cited, rows);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let citation_line = extract_receipt_line(&receipt, "cited-test-existence")
        .unwrap_or_else(|| panic!("no cited-test-existence row in receipt: {receipt}"));
    assert!(citation_line.contains("| fail |"), "{citation_line}");
    assert!(
        citation_line
            .to_ascii_lowercase()
            .contains("citation-reconciliation"),
        "a reconciliation row targeting an already-resolvable token must fail naming \
         the citation-reconciliation defect rather than silently passing: {citation_line}"
    );
    assert!(
        citation_line.contains("already_resolvable_probe_fn_zzz"),
        "the failure must name the offending row: {citation_line}"
    );
}

// ── `working-tree-clean` must be a fail-closed full-mode row,          ─
// ── decided from `git status --porcelain=v1 --untracked-files=all` run   ─
// ── after the authoritative commands. Dirty output, a nonzero exit from  ─
// ── `git`, and a spawn failure must all fail the row rather than pass or ─
// ── panic. `gal finalize-hygiene` (hygiene-only mode) is untouched by    ─
// ── this task and is not exercised here.                                ─

/// Oracle helper: the real dirty-entry count from the exact porcelain
/// invocation the contract names, taken over the same tree finalize-check
/// inspects — never a hardcoded literal that could drift from what the
/// fixture actually produces.
fn count_porcelain_entries(repo: &Path) -> usize {
    let out = Command::new("git")
        .current_dir(repo)
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "oracle `git status` must succeed on a real, valid repo"
    );
    String::from_utf8(out.stdout)
        .unwrap()
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn finalize_check_with_env(
    repo: &Path,
    target: &Path,
    env: &[(&str, &str)],
) -> (std::process::Output, String) {
    let receipt_path = repo.join("finalize.receipt.md");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_gal"));
    cmd.current_dir(repo).args([
        "finalize-check",
        target.to_str().unwrap(),
        "--receipt",
        receipt_path.to_str().unwrap(),
    ]);
    for (key, value) in env {
        cmd.env(key, value);
    }
    let output = cmd.output().unwrap();
    let receipt = fs::read_to_string(&receipt_path).unwrap_or_default();
    (output, receipt)
}

/// Locked EF-01 acceptance probe. Writing the plan+prompt pair (never
/// committed) leaves exactly two new untracked files in the working tree —
/// a deterministic, environment-derived dirty fixture rather than a
/// hardcoded entry count.
#[test]
fn finalize_working_tree_clean_dirty_fixture_fails_naming_entry_count() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-working-tree-dirty";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let expected_dirty = count_porcelain_entries(tmp.path());
    assert!(
        expected_dirty > 0,
        "fixture must leave a genuinely dirty tree (the unwritten plan/prompt pair)"
    );

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let row = extract_receipt_line(&receipt, "working-tree-clean")
        .unwrap_or_else(|| panic!("no working-tree-clean row in receipt: {receipt}"));
    assert!(
        row.contains("| fail |"),
        "a dirty tree must fail the row: {row}"
    );
    assert!(
        row.contains(&expected_dirty.to_string()),
        "row must report the real dirty entry count ({expected_dirty}): {row}"
    );
}

/// Happy path: once the plan/prompt pair is itself committed, the tree the
/// row inspects is genuinely clean and the row must pass.
#[test]
fn finalize_working_tree_clean_clean_fixture_passes() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-working-tree-clean";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);
    commit_all(tmp.path(), "add plan");
    assert_eq!(
        count_porcelain_entries(tmp.path()),
        0,
        "fixture setup must leave a clean tree before the real assertion"
    );

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let row = extract_receipt_line(&receipt, "working-tree-clean")
        .unwrap_or_else(|| panic!("no working-tree-clean row in receipt: {receipt}"));
    assert!(
        row.contains("| pass |"),
        "a clean tree must pass the row: {row}"
    );
}

#[test]
fn finalize_working_tree_clean_is_the_terminal_row_in_both_modes() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    write_project_fixture(tmp.path());
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-working-tree-terminal";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);
    commit_all(tmp.path(), "add plan");

    let (_, full_receipt) = finalize_check(tmp.path(), &prompt_path);
    let rendered_dirty = {
        let _ = fs::remove_file(tmp.path().join("finalize.receipt.md"));
        count_porcelain_entries(tmp.path())
    };
    assert_eq!(
        rendered_dirty, 0,
        "full finalize must not dirty the tree (no adapter files written)"
    );
    let (_, hygiene_receipt) = finalize_check_hygiene(tmp.path(), &prompt_path);
    let hygiene_dirty = {
        let _ = fs::remove_file(tmp.path().join("finalize-hygiene.receipt.md"));
        count_porcelain_entries(tmp.path())
    };
    assert_eq!(hygiene_dirty, 0, "hygiene finalize must not dirty the tree");

    for (mode, receipt) in [("full", full_receipt), ("hygiene-only", hygiene_receipt)] {
        let names = receipt_check_names(&receipt);
        assert_eq!(
            names.last().copied(),
            Some("working-tree-clean"),
            "{mode} must evaluate working-tree-clean after every preceding in-process operation: {receipt}"
        );
        let row = extract_receipt_line(&receipt, "working-tree-clean").unwrap();
        assert!(
            row.contains("| pass |"),
            "{mode} must pass when working tree is clean: {row}"
        );
    }
}

#[test]
fn finalize_working_tree_clean_passes_in_both_modes_after_rendered_outputs_are_committed() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    write_project_fixture(tmp.path());
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-working-tree-rendered-clean";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);
    commit_all(tmp.path(), "add plan");

    // Full finalize must pass working-tree-clean directly without needing adapter mutation committed
    let (_, full_receipt_pre) = finalize_check(tmp.path(), &prompt_path);
    let pre_row = extract_receipt_line(&full_receipt_pre, "working-tree-clean").unwrap();
    assert!(
        pre_row.contains("| pass |"),
        "full mode must pass working-tree-clean without needing rendered adapters committed: {pre_row}"
    );
    fs::remove_file(tmp.path().join("finalize.receipt.md")).unwrap();

    // When adapters are explicitly rendered and committed, finalize-check continues to pass in both modes
    let init_out = run_gal(tmp.path(), &["init"]);
    assert!(
        init_out.status.success(),
        "gal init failed: {}",
        String::from_utf8_lossy(&init_out.stderr)
    );
    commit_all(tmp.path(), "commit rendered adapters");

    let (_, full_receipt) = finalize_check(tmp.path(), &prompt_path);
    fs::remove_file(tmp.path().join("finalize.receipt.md")).unwrap();
    let (_, hygiene_receipt) = finalize_check_hygiene(tmp.path(), &prompt_path);

    for (mode, receipt) in [("full", full_receipt), ("hygiene-only", hygiene_receipt)] {
        let row = extract_receipt_line(&receipt, "working-tree-clean").unwrap();
        assert!(
            row.contains("| pass |"),
            "{mode} must pass when rendered outputs are committed and stable: {row}"
        );
    }
}

#[test]
fn finalize_working_tree_clean_hygiene_dirty_fixture_fails() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-working-tree-hygiene-dirty";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    let expected_dirty = count_porcelain_entries(tmp.path());
    let (_, receipt) = finalize_check_hygiene(tmp.path(), &prompt_path);
    let row = extract_receipt_line(&receipt, "working-tree-clean")
        .unwrap_or_else(|| panic!("no working-tree-clean row in hygiene receipt: {receipt}"));

    assert!(
        row.contains("| fail |"),
        "a dirty hygiene tree must fail: {row}"
    );
    assert!(
        row.contains(&expected_dirty.to_string()),
        "hygiene row must report the real dirty-entry count ({expected_dirty}): {row}"
    );
}

/// Clearing `PATH` (both casings, for the Windows-insensitive lookup) makes
/// the authoritative `git status` subprocess unfindable — a real spawn
/// failure, not a nonzero exit from a found binary. The row must still fail
/// closed rather than pass or panic the whole command.
#[test]
fn finalize_working_tree_clean_git_spawn_failure_fails_closed() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-working-tree-spawn-failure";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);
    commit_all(tmp.path(), "add plan");

    let (_output, receipt) =
        finalize_check_with_env(tmp.path(), &prompt_path, &[("PATH", ""), ("Path", "")]);

    let row = extract_receipt_line(&receipt, "working-tree-clean")
        .unwrap_or_else(|| panic!("no working-tree-clean row in receipt: {receipt}"));
    assert!(
        row.contains("| fail |"),
        "a `git status` spawn failure must fail the row closed, not pass or panic: {row}"
    );
}

/// A working directory that is deliberately not a Git work tree at all (no
/// `git init`). The `git` binary is still found on `PATH` and spawns
/// successfully, but `git status --porcelain=v1 --untracked-files=all` — like
/// every other authoritative `git` invocation run from that directory — exits
/// nonzero (`fatal: not a git repository`). This is a distinct code path from
/// the spawn-failure fixture above, which never reaches an exit status at
/// all.
#[test]
fn finalize_working_tree_clean_git_nonzero_exit_fails_closed() {
    let tmp = tempfile::tempdir().unwrap();
    let task = tid(1);
    let slug = "finalize-working-tree-nonzero-exit";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *(0000000)*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);

    // Oracle: confirm `git status` really does spawn and exit nonzero from
    // this plain, non-repository directory, rather than assuming it.
    let oracle = Command::new("git")
        .current_dir(tmp.path())
        .args(["status", "--porcelain=v1", "--untracked-files=all"])
        .output()
        .unwrap();
    assert!(
        !oracle.status.success(),
        "fixture must leave `git status` exiting nonzero from a plain non-repo directory"
    );

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);

    let row = extract_receipt_line(&receipt, "working-tree-clean")
        .unwrap_or_else(|| panic!("no working-tree-clean row in receipt: {receipt}"));
    assert!(
        row.contains("| fail |"),
        "a nonzero `git status` exit must fail the row closed, not pass or panic: {row}"
    );
}

// ── `pipeline-goal-binding` must reuse the shared handback evaluator    ─
// ── the freshest goal receipt: prompt path, prompt SHA-256, Git HEAD,    ─
// ── checked-task projection, cleared current task, complete must-haves,  ─
// ── command evidence, and `VERIFIED`. Finalize must not parse goal       ─
// ── receipts through a second, independently-drifting grammar.          ─

/// Locked EF-01 acceptance probe. `finalize-check` does not emit a
/// `pipeline-goal-binding` row yet, so this probe is expected red:
/// `extract_receipt_line` finds no such row and panics naming it — the
/// panic text is exactly the locked failing reason (`matcher_b64` decodes
/// to `pipeline-goal-binding`). Once the row lands, a goal receipt bound to
/// a stale HEAD must fail it, and the identical fixture read through
/// `pipeline-handback-check` — the shared evaluator's other consumer — must
/// reach the same non-ready verdict rather than a divergent one.
#[test]
fn finalize_pipeline_goal_binding_stale_head_fails_and_matches_handback() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-goal-binding-stale-head";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);
    commit_all(tmp.path(), "add plan");
    assert_eq!(
        count_porcelain_entries(tmp.path()),
        0,
        "fixture setup must leave a clean tree so only the goal binding is under test"
    );

    let real_head = full_head(tmp.path());
    let stale_head = "0".repeat(40);
    assert_ne!(
        real_head, stale_head,
        "fixture sanity: the goal receipt's head must genuinely differ from the real HEAD"
    );
    let canonical_prompt = fs::canonicalize(&prompt_path).unwrap();
    let goal_receipt = tmp
        .path()
        .join(".dev/pipeline/receipts")
        .join(slug)
        .join("goal-verification.receipt.md");
    fs::create_dir_all(goal_receipt.parent().unwrap()).unwrap();
    fs::write(
        &goal_receipt,
        format!(
            "prompt_path: {}\nprompt_sha256: {}\nhead: {stale_head}\nverdict: VERIFIED\n\
             checked_tasks: {task}\nmust_have_1: cargo test passed\ncommand: cargo test\n",
            canonical_prompt.display(),
            sha256_hex(&prompt_path),
        ),
    )
    .unwrap();

    let (output, receipt) = finalize_check(tmp.path(), &prompt_path);
    let row = extract_receipt_line(&receipt, "pipeline-goal-binding")
        .unwrap_or_else(|| panic!("no pipeline-goal-binding row in receipt: {receipt}"));
    assert!(
        row.contains("| fail |"),
        "a stale-HEAD goal receipt must fail the row rather than pass or invent a second \
         goal-receipt grammar: {row}"
    );
    assert_ne!(
        output.status.code(),
        Some(0),
        "a failing pipeline-goal-binding row must fail the whole full-mode gate"
    );

    // Same fixture, read through the existing `pipeline-handback-check`
    // binary — the shared evaluator's other consumer — must reach the
    // identical non-ready verdict, proving finalize consumed the shared
    // evaluator rather than a second, independently-drifting binding check.
    let handback_receipt = tmp.path().join("handback.receipt.md");
    let handback_output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(tmp.path())
        .arg("pipeline-handback-check")
        .arg(&prompt_path)
        .arg("--receipt")
        .arg(&handback_receipt)
        .output()
        .unwrap();
    let handback_text = fs::read_to_string(&handback_receipt).unwrap_or_default();
    assert_ne!(
        handback_output.status.code(),
        Some(0),
        "the stale-HEAD fixture must also fail pipeline-handback-check: {handback_text}"
    );
    assert!(
        handback_text.contains("decision: continue"),
        "handback must report the same non-ready verdict as the finalize row: {handback_text}"
    );
}

/// Locked EF-03 acceptance probe. Finalize must render the shared predicate
/// inventory as ordered receipt rows, including the neutral terminal-binding
/// slot, and its failing rows must exactly match handback's ordered diagnostic.
#[test]
fn finalize_and_handback_render_same_ordered_goal_predicate_diagnostics() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-goal-predicate-parity";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, false, &tasks_body);
    commit_all(tmp.path(), "add plan");

    let (finalize_output, finalize_receipt) = finalize_check(tmp.path(), &prompt_path);

    let handback_receipt = tmp.path().join("handback.receipt.md");
    let handback_output = run_gal(
        tmp.path(),
        &[
            "pipeline-handback-check",
            prompt_path.to_str().unwrap(),
            "--receipt",
            handback_receipt.to_str().unwrap(),
        ],
    );
    let handback_text = fs::read_to_string(&handback_receipt).unwrap();

    assert!(
        extract_receipt_line(&finalize_receipt, "terminal-binding").is_some(),
        "terminal-binding predicate row missing from finalize receipt: {finalize_receipt}"
    );

    let expected_order = [
        "prompt-path",
        "prompt-hash",
        "head",
        "checked-task-projection",
        "cursor",
        "must-haves",
        "commands",
        "verdict",
        "terminal-binding",
    ];
    let rendered_order = receipt_check_names(&finalize_receipt)
        .into_iter()
        .filter(|name| expected_order.contains(name))
        .collect::<Vec<_>>();
    assert_eq!(
        rendered_order, expected_order,
        "finalize must render the shared predicate inventory in authority order"
    );

    let finalize_failed = expected_order
        .iter()
        .filter(|name| {
            extract_receipt_line(&finalize_receipt, name)
                .is_some_and(|line| line.contains("| fail |"))
        })
        .copied()
        .collect::<Vec<_>>();
    assert_eq!(
        finalize_failed,
        failed_goal_predicates(&handback_text),
        "finalize and handback must render the same ordered failed predicates"
    );
    assert!(
        !finalize_failed.is_empty(),
        "the missing-goal fixture must not collapse to generic pipeline-goal-binding only"
    );
    assert_ne!(finalize_output.status.code(), Some(0));
    assert_ne!(handback_output.status.code(), Some(0));
}

#[test]
fn finalize_check_marked_prompt_with_valid_terminal_receipt_passes() {
    let tmp = make_git_repo();
    write_project_fixture(tmp.path());
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-marked-valid-terminal";
    let tasks_body = contract_block(&task, &hash);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, true, &tasks_body);
    commit_all(tmp.path(), "add plan prompt");

    let real_head = full_head(tmp.path());
    let canonical_prompt = fs::canonicalize(&prompt_path).unwrap();
    let prompt_sha = sha256_hex(&prompt_path);

    let receipt_dir = tmp.path().join(".dev/pipeline/receipts").join(slug);
    fs::create_dir_all(&receipt_dir).unwrap();

    let terminal_receipt = receipt_dir.join("terminal-reverify.receipt.md");
    let terminal_text = format!(
        "mode: terminal-reverify\noverall: pass\nprompt_path: {}\nprompt_sha256: {prompt_sha}\nhead: {real_head}\n",
        canonical_prompt.display()
    );
    fs::write(&terminal_receipt, &terminal_text).unwrap();
    let term_sha = sha256_hex(&terminal_receipt);

    let goal_receipt = receipt_dir.join("goal-verification.receipt.md");
    fs::write(
        &goal_receipt,
        format!(
            "prompt_path: {}\nprompt_sha256: {prompt_sha}\nhead: {real_head}\nverdict: VERIFIED\n\
             checked_tasks: {task}\nmust_have_1: cargo test passed\ncommand: cargo test\n\
             terminal_receipt_sha256: {term_sha}\n",
            canonical_prompt.display(),
        ),
    )
    .unwrap();

    let (_finalize_output, finalize_receipt) = finalize_check(tmp.path(), &prompt_path);
    let row = extract_receipt_line(&finalize_receipt, "terminal-binding").unwrap();
    assert!(
        row.contains("| pass |"),
        "terminal-binding row must pass: {row}"
    );

    let handback_receipt = tmp.path().join("handback.receipt.md");
    let _handback_output = run_gal(
        tmp.path(),
        &[
            "pipeline-handback-check",
            prompt_path.to_str().unwrap(),
            "--receipt",
            handback_receipt.to_str().unwrap(),
        ],
    );
    let handback_text = fs::read_to_string(&handback_receipt).unwrap();
    assert!(
        !failed_goal_predicates(&handback_text).contains(&"terminal-binding"),
        "handback must report terminal-binding pass: {handback_text}"
    );
}

#[test]
fn finalize_check_marked_prompt_with_tampered_terminal_receipt_fails() {
    let tmp = make_git_repo();
    write_project_fixture(tmp.path());
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-marked-tampered-terminal";
    let tasks_body = contract_block(&task, &hash);
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, true, &tasks_body);
    commit_all(tmp.path(), "add plan prompt");

    let real_head = full_head(tmp.path());
    let canonical_prompt = fs::canonicalize(&prompt_path).unwrap();
    let prompt_sha = sha256_hex(&prompt_path);

    let receipt_dir = tmp.path().join(".dev/pipeline/receipts").join(slug);
    fs::create_dir_all(&receipt_dir).unwrap();

    let terminal_receipt = receipt_dir.join("terminal-reverify.receipt.md");
    let terminal_text = format!(
        "mode: terminal-reverify\noverall: pass\nprompt_path: {}\nprompt_sha256: {prompt_sha}\nhead: {real_head}\n",
        canonical_prompt.display()
    );
    fs::write(&terminal_receipt, &terminal_text).unwrap();
    let term_sha = sha256_hex(&terminal_receipt);

    // Tamper with the terminal receipt text after recording its SHA
    fs::write(&terminal_receipt, format!("{terminal_text}\n# tampered")).unwrap();

    let goal_receipt = receipt_dir.join("goal-verification.receipt.md");
    fs::write(
        &goal_receipt,
        format!(
            "prompt_path: {}\nprompt_sha256: {prompt_sha}\nhead: {real_head}\nverdict: VERIFIED\n\
             checked_tasks: {task}\nmust_have_1: cargo test passed\ncommand: cargo test\n\
             terminal_receipt_sha256: {term_sha}\n",
            canonical_prompt.display(),
        ),
    )
    .unwrap();

    let (finalize_output, finalize_receipt) = finalize_check(tmp.path(), &prompt_path);
    assert_ne!(
        finalize_output.status.code(),
        Some(0),
        "marked prompt with tampered terminal receipt must fail finalize"
    );
    let row = extract_receipt_line(&finalize_receipt, "terminal-binding").unwrap();
    assert!(
        row.contains("| fail |"),
        "terminal-binding must fail: {row}"
    );
}
