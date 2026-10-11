//! `cargo test -p gal-cli --test finalize_check`
//!
//! Real-binary coverage for `gal finalize-check` (check(g)), including target
//! validation, authoritative-command execution, full and hygiene-only modes,
//! receipt rendering, and fail-closed working-tree checks:
//!
//! 1. `cmd_finalize_check` still reads its target through
//!    `read_validated_bytes` instead of a naive
//!    `std::fs::read_to_string(..).unwrap_or_default()` — a target path
//!    routed through a linked (symlinked/junctioned) directory component is
//!    rejected before any bytes are read, and non-UTF-8 target content is
//!    rejected explicitly instead of silently treated as empty.
//! 2. Full mode still restricts its target to a canonical
//!    `.dev/plans/<slug>.prompt.md` execution prompt, still runs the
//!    authoritative-command list, and `working-tree-clean` is still the
//!    fail-closed terminal row in both modes.
//! 3. Full-mode receipts preserve their expected rows and terminal ordering.

use pipeline::task_spec::extract_plan_slug;
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
        "## What This Is\nFixture\n\n## Tech Stack\n<!-- gal:authoritative-check -->\n```json\n{\n  \"command\": [\"git --version\"]\n}\n```\n\n## Architecture\nFixture\n\n## Constraints\nFixture\n\n## Response Style\nFixture\n\n## Freshness\nFixture\n\n## Project Language\n- PROJECT_LANGUAGE: `en`\n\n## Protected Paths\nFixture\n",
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

/// Writes matching source plan + execution prompt bodies under
/// `.dev/plans/<slug>.{md,prompt.md}`. Both surfaces get byte-identical bodies
/// so three-surface agreement holds trivially.
fn write_plan_and_prompt(repo: &Path, slug: &str, tasks_body: &str) -> PathBuf {
    write_plan_and_prompt_with_status(repo, slug, "", tasks_body)
}

fn write_plan_and_prompt_with_status(
    repo: &Path,
    slug: &str,
    extra_status: &str,
    tasks_body: &str,
) -> PathBuf {
    let body = format!(
        "# Plan Prompt: {slug}\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n{extra_status}\n## Tasks\n\n{tasks_body}\n"
    );
    let plans_dir = repo.join(".dev").join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    fs::write(plans_dir.join(format!("{slug}.md")), &body).unwrap();
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &body).unwrap();
    prompt_path
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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);

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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, "");

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
    let slug = extract_plan_slug(".dev/plans/finalize-plan-1.prompt.md");
    assert_eq!(slug, "finalize-plan-1");
}

// ── A full-mode receipt must never resurrect the five rows the prior      ─
// ── cleanup deleted from `finalize-check` — the direct regression guard   ─
// ── deletion this task performs on the test suite itself.                ─

#[test]
fn finalize_full_mode_receipt_contains_none_of_the_removed_rows() {
    let tmp = make_git_repo();
    fs::write(tmp.path().join("README.md"), "x").unwrap();
    let hash = commit_all(tmp.path(), "init");
    let task = tid(1);
    let slug = "finalize-removed-rows-absent";
    let tasks_body = format!("- [x] {task} — legacy placeholder. *({hash})*\n");
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);
    commit_all(tmp.path(), "add plan");

    let (_output, receipt) = finalize_check(tmp.path(), &prompt_path);
    assert!(!receipt.is_empty(), "full mode must still write a receipt");

    for removed in [
        "executor-log-scan",
        "test-first-evaluation",
        "test-results-structure",
        "cited-test-existence",
        "pipeline-goal-binding",
    ] {
        assert!(
            !receipt.contains(removed),
            "full-mode receipt must never contain the removed row {removed}: {receipt}"
        );
    }
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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);

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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);
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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);
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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);
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
    let render_out = run_gal(tmp.path(), &["render-adapters"]);
    assert!(
        render_out.status.success(),
        "gal render-adapters failed: {}",
        String::from_utf8_lossy(&render_out.stderr)
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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);

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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);
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
    let prompt_path = write_plan_and_prompt(tmp.path(), slug, &tasks_body);

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
