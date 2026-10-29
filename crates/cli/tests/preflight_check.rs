//! `cargo test -p gal-cli --test preflight_check`

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::{tempdir, TempDir};

fn test_gal_exe() -> &'static std::path::Path {
    static TEST_EXE: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    TEST_EXE
        .get_or_init(|| {
            let src = std::path::PathBuf::from(env!("CARGO_BIN_EXE_gal"));
            let needs_copy = src.components().any(|component| {
                ["worktrees", ".worktrees"].iter().any(|name| {
                    component
                        .as_os_str()
                        .to_string_lossy()
                        .eq_ignore_ascii_case(name)
                })
            });
            if needs_copy {
                let dir = std::env::temp_dir().join(format!("gal-test-bin-{}", std::process::id()));
                let _ = std::fs::create_dir_all(&dir);
                let dst = dir.join(src.file_name().unwrap());
                let _ = std::fs::copy(&src, &dst);
                dst
            } else {
                src
            }
        })
        .as_path()
}

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
    git(temp.path(), &["config", "core.autocrlf", "false"]);
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
    std::fs::write(
        repo.join(".dev").join("state.md"),
        format!(
            "## Session Continuity\n\n| {name} | .dev/plans/{name}.prompt.md | in progress |\n"
        ),
    )
    .unwrap();
    (
        prompt_path,
        repo.join(format!("{name}.preflight.receipt.md")),
    )
}

fn run_preflight(repo: &Path, prompt_path: &Path, receipt_path: &Path) -> (i32, String) {
    let output = Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["pipeline-preflight"])
        .arg(prompt_path)
        .args(["--receipt"])
        .arg(receipt_path)
        .output()
        .expect("failed to execute gal binary");
    (
        output.status.code().expect("process terminated by signal"),
        std::fs::read_to_string(receipt_path).unwrap_or_default(),
    )
}

fn run_preflight_after_recovery(repo: &Path, prompt_path: &Path, receipt_path: &Path) -> String {
    let output = Command::new(test_gal_exe())
        .current_dir(repo)
        .env("GAL_ENTRY_RECOVERY", "merged")
        .args(["pipeline-preflight"])
        .arg(prompt_path)
        .args(["--receipt"])
        .arg(receipt_path)
        .output()
        .expect("failed to execute gal binary");
    assert!(
        output.status.success(),
        "preflight after recovery failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::read_to_string(receipt_path).unwrap_or_default()
}

fn run_prompt_check(repo: &Path, prompt_path: &Path) -> std::process::Output {
    Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["prompt-check"])
        .arg(prompt_path)
        .output()
        .expect("failed to execute gal binary")
}

fn task_grammar_prompt(task_line: &str) -> String {
    format!(
        "# Plan Prompt: Task Grammar Fixture\n\n## Goal\n\nCheck task identity.\n\n## Requirements\n\n- [ ] R1 — Use the shared task identity grammar.\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {id}\n\n## Tasks\n\n{task_line}\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {id} | unit | cargo test | pass | TESTER | test output |\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {id} | task grammar |\n",
        id = tid(1),
    )
}

fn verification_prompt(map_rows: &str) -> String {
    let task = tid(1);
    format!(
        "## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {task}\n\n\
         ## Tasks\n\n- [ ] {task} — verify map enforcement\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":true,\"cross_boundary\":false}}\n\n\
         ## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n{map_rows}\n\n\
         ## Test Plan\n\n| ID | desc |\n| --- | --- |\n| map-test | map enforcement |\n"
    )
}

fn run_preflight_argument(
    repo: &Path,
    prompt_argument: &str,
    receipt_path: &Path,
) -> std::process::Output {
    Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["pipeline-preflight", prompt_argument, "--receipt"])
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
    assert!(lines
        .next()
        .unwrap_or_default()
        .starts_with("gal pipeline-preflight: "));
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

fn commit_all(repo: &Path, message: &str) {
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", message]);
}

fn terminal_reverify_receipt_path(repo: &Path, slug: &str) -> PathBuf {
    repo.join(".dev")
        .join("pipeline")
        .join(slug)
        .join("terminal-reverify.receipt.md")
}

fn run_terminal_reverify(repo: &Path, prompt_path: &Path) -> (i32, String) {
    let output = Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["pipeline-preflight", "--terminal-reverify"])
        .arg(prompt_path)
        .output()
        .expect("failed to execute gal binary");
    let mut combined = String::from_utf8_lossy(&output.stdout).to_string();
    combined.push_str(&String::from_utf8_lossy(&output.stderr));
    (
        output.status.code().expect("process terminated by signal"),
        combined,
    )
}

fn run_terminal_reverify_argument(repo: &Path, prompt_argument: &str) -> std::process::Output {
    Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["pipeline-preflight", "--terminal-reverify", prompt_argument])
        .output()
        .expect("failed to execute gal binary")
}

fn run_terminal_reverify_output(repo: &Path, prompt_path: &Path) -> std::process::Output {
    Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["pipeline-preflight", "--terminal-reverify"])
        .arg(prompt_path)
        .output()
        .expect("failed to execute gal binary")
}

fn terminal_prompt(t1: &str, task_line: &str, handoff: &str) -> String {
    format!(
        "# Plan Prompt: Terminal Reverify Fixture\n\n## Status\n\nWorkflow: DONE\nCurrent Task: —\nTest Retry Count: 0\nReview Retry Count: 0\n\n### Handoff Notes\n{handoff}\n\n## Tasks\n\n{task_line}\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {t1} | unit | cargo test | pass | TESTER | test output |\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {t1} | test feature |\n"
    )
}

fn write_terminal_fixture(repo: &Path, name: &str, prompt_text: &str) -> PathBuf {
    let plans_dir = repo.join(".dev").join("plans");
    std::fs::create_dir_all(&plans_dir).unwrap();
    let prompt_path = plans_dir.join(format!("{name}.prompt.md"));
    std::fs::write(&prompt_path, prompt_text).unwrap();
    std::fs::write(
        repo.join(".dev").join("state.md"),
        format!(
            "## Session Continuity\n\n| {name} | .dev/plans/{name}.prompt.md | in progress |\n"
        ),
    )
    .unwrap();
    prompt_path
}

#[test]
fn terminal_reverify_rejects_non_done_workflow() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let prompt = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n")
        .replace("Workflow: DONE", "Workflow: IMPLEMENT");
    let path = write_terminal_fixture(temp.path(), "terminal-non-done", &prompt);
    commit_all(temp.path(), "fixture: non-DONE workflow");
    let (code, output) = run_terminal_reverify(temp.path(), &path);
    assert_ne!(code, 0, "output: {output}");
    assert!(output.contains("terminal-reverify"));
    assert!(output.to_lowercase().contains("done"));
    assert!(std::fs::read_to_string(terminal_reverify_receipt_path(
        temp.path(),
        "terminal-non-done"
    ))
    .unwrap()
    .contains("overall: fail"));
}

#[test]
fn terminal_reverify_rejects_unchecked_task() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let path = write_terminal_fixture(
        temp.path(),
        "terminal-unchecked",
        &terminal_prompt(&t1, &format!("- [ ] {t1} — pending."), "\n(none)\n"),
    );
    commit_all(temp.path(), "fixture: unchecked task under DONE");
    let (code, output) = run_terminal_reverify(temp.path(), &path);
    assert_ne!(code, 0, "output: {output}");
    assert!(output.contains("terminal-reverify"));
    assert!(std::fs::read_to_string(terminal_reverify_receipt_path(
        temp.path(),
        "terminal-unchecked"
    ))
    .unwrap()
    .contains("overall: fail"));
}

#[test]
fn terminal_reverify_rejects_open_retry_handoff() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let handoff = format!("\n#### Retry Handoff — {t1} / TEST\n\n- Status: OPEN\n");
    let path = write_terminal_fixture(
        temp.path(),
        "terminal-open-handoff",
        &terminal_prompt(&t1, &format!("- [x] {t1} — complete."), &handoff),
    );
    commit_all(temp.path(), "fixture: open retry handoff");
    let (code, output) = run_terminal_reverify(temp.path(), &path);
    assert_ne!(code, 0, "output: {output}");
    assert!(output.contains("terminal-reverify"));
    assert!(std::fs::read_to_string(terminal_reverify_receipt_path(
        temp.path(),
        "terminal-open-handoff"
    ))
    .unwrap()
    .contains("overall: fail"));
}

#[test]
fn terminal_reverify_rejects_dirty_tree() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let path = write_terminal_fixture(
        temp.path(),
        "terminal-dirty",
        &terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n"),
    );
    commit_all(temp.path(), "fixture: clean baseline");
    std::fs::write(temp.path().join("uncommitted.txt"), "stray change\n").unwrap();
    let (code, output) = run_terminal_reverify(temp.path(), &path);
    assert_ne!(code, 0, "output: {output}");
    assert!(output.contains("terminal-reverify"));
    assert!(std::fs::read_to_string(terminal_reverify_receipt_path(
        temp.path(),
        "terminal-dirty"
    ))
    .unwrap()
    .contains("overall: fail"));
}

#[test]
fn terminal_reverify_accepts_clean_all_checked_terminal_prompt() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let path = write_terminal_fixture(
        temp.path(),
        "terminal-clean",
        &terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n"),
    );
    commit_all(temp.path(), "fixture: clean terminal prompt");
    let (code, output) = run_terminal_reverify(temp.path(), &path);
    let receipt = std::fs::read_to_string(terminal_reverify_receipt_path(
        temp.path(),
        "terminal-clean",
    ))
    .unwrap();
    assert_eq!(code, 0, "output: {output}, receipt: {receipt}");
    assert!(receipt.contains("overall: pass"));
}

#[test]
fn terminal_reverify_reaches_no_dispatch_mutation_or_commit_surface() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let path = write_terminal_fixture(
        temp.path(),
        "terminal-isolation",
        &terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n"),
    );
    commit_all(temp.path(), "fixture: isolation baseline");
    let head = git(temp.path(), &["rev-parse", "HEAD"]);
    let bytes = std::fs::read(&path).unwrap();
    let _ = run_terminal_reverify(temp.path(), &path);
    assert_eq!(head, git(temp.path(), &["rev-parse", "HEAD"]));
    assert_eq!(bytes, std::fs::read(&path).unwrap());
    let scope = temp.path().join(".dev/pipeline/terminal-isolation");
    if scope.is_dir() {
        assert_eq!(std::fs::read_dir(scope).unwrap().count(), 1);
    }
}

#[test]
fn ordinary_preflight_still_rejects_done_workflow() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let (path, receipt_path) = write_plan_and_state(
        temp.path(),
        "ordinary-done",
        &terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n"),
    );
    commit_all(temp.path(), "fixture: ordinary DONE prompt");
    let (code, receipt) = run_preflight(temp.path(), &path, &receipt_path);
    assert_ne!(code, 0);
    assert!(receipt.contains("overall: fail"));
}

#[test]
fn preflight_requires_complete_verification_map_without_pinning_or_running_commands() {
    let task = tid(1);
    let proof_command = "cmd /c echo executed > proof-command-ran.txt";
    let valid_rows = format!(
        "| {task} | unit | {proof_command} | pass | TESTER | test output |\n\
         | {task} | cli | {proof_command} | pass | TESTER | cli output |"
    );
    let undercovered_rows =
        format!("| {task} | unit | {proof_command} | pass | TESTER | test output |");

    for (name, rows, accepted) in [
        ("valid-map", valid_rows.as_str(), true),
        ("missing-map", "", false),
        ("missing-layer", undercovered_rows.as_str(), false),
    ] {
        let temp = create_temp_repo();
        let prompt_text = verification_prompt(rows);
        let (prompt, receipt_path) = write_plan_and_state(temp.path(), name, &prompt_text);
        let (code, receipt) = run_preflight(temp.path(), &prompt, &receipt_path);
        assert_eq!(code == 0, accepted, "{name} receipt:\n{receipt}");
        assert_eq!(
            receipt.contains(&format!("| verification-map-{task} | pass |")),
            accepted,
            "{name} receipt:\n{receipt}"
        );
        if !accepted {
            assert!(receipt.contains("overall: fail"), "{receipt}");
            assert!(
                !receipt.lines().any(|line| line.starts_with("run_id: ")),
                "{receipt}"
            );
            assert!(
                !receipt.lines().any(|line| line.starts_with("host: ")),
                "{receipt}"
            );
            assert!(
                !receipt
                    .lines()
                    .any(|line| line.starts_with("dispatch_mode: ")),
                "{receipt}"
            );
        }
        assert!(
            !temp.path().join("proof-command-ran.txt").exists(),
            "preflight executed the opaque Proof Command for {name}"
        );
    }
}

#[test]
fn preflight_accepts_bare_and_prefixed_prompt_references() {
    for (label, reference) in [
        ("bare-reference", ".dev/plans/reference.prompt.md"),
        ("file-reference", "#file:.dev/plans/reference.prompt.md"),
        ("at-reference", "@.dev/plans/reference.prompt.md"),
    ] {
        let temp = create_temp_repo();
        let t1 = tid(1);
        let prompt = format!(
            "# Plan Prompt: Reference Fixture\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {t1}\n\n## Tasks\n\n- [ ] {t1} — reference fixture.\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {t1} | unit | cargo test | pass | TESTER | test output |\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {t1} | reference |\n"
        );
        write_plan_and_state(temp.path(), "reference", &prompt);
        let receipt_path = temp.path().join(format!("{label}.receipt.md"));
        let output = run_preflight_argument(temp.path(), reference, &receipt_path);
        let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_default();
        assert_eq!(
            output.status.code(),
            Some(0),
            "output: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            receipt.contains("prompt: reference.prompt.md"),
            "receipt: {receipt}"
        );
    }
}

#[test]
fn preflight_receipts_record_normalized_prompt_paths() {
    for (label, reference) in [
        ("bare-receipt", ".dev/plans/receipt-reference.prompt.md"),
        (
            "file-receipt",
            "#file:.dev/plans/receipt-reference.prompt.md",
        ),
        ("at-receipt", "@.dev/plans/receipt-reference.prompt.md"),
    ] {
        let temp = create_temp_repo();
        let t1 = tid(1);
        let prompt = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
        write_terminal_fixture(temp.path(), "receipt-reference", &prompt);
        commit_all(temp.path(), "fixture: normalized prompt receipt");
        let output = run_terminal_reverify_argument(temp.path(), reference);
        let receipt_path = terminal_reverify_receipt_path(temp.path(), "receipt-reference");
        let receipt = std::fs::read_to_string(receipt_path).unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "output: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            receipt.contains("prompt_path: .dev/plans/receipt-reference.prompt.md"),
            "{label} receipt: {receipt}"
        );
    }
}

#[test]
fn preflight_rejects_empty_references_without_receipts() {
    for reference in ["#file:", "@"] {
        let temp = create_temp_repo();
        let receipt_path = temp.path().join("empty.receipt.md");
        let output = run_preflight_argument(temp.path(), reference, &receipt_path);
        assert_eq!(
            output.status.code(),
            Some(64),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!receipt_path.exists());
    }
}

#[test]
fn preflight_rejects_prefixed_source_plans_without_receipts() {
    for reference in [
        "#file:.dev/plans/source-plan.md",
        "@.dev/plans/source-plan.md",
    ] {
        let temp = create_temp_repo();
        let receipt_path = temp.path().join("source.receipt.md");
        let output = run_preflight_argument(temp.path(), reference, &receipt_path);
        assert_eq!(
            output.status.code(),
            Some(64),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stderr).contains("is a source plan"));
        assert!(!receipt_path.exists());
    }
}

#[test]
fn terminal_reverify_emits_deterministic_immutable_receipt_fields() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let path = write_terminal_fixture(
        temp.path(),
        "terminal-receipt-fields",
        &terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n"),
    );
    commit_all(temp.path(), "fixture: terminal prompt for receipt fields");
    let _ = run_terminal_reverify(temp.path(), &path);
    let receipt = std::fs::read_to_string(terminal_reverify_receipt_path(
        temp.path(),
        "terminal-receipt-fields",
    ))
    .unwrap();
    assert!(receipt.contains("prompt_path"));
    assert!(receipt.contains("prompt_sha256"));
    assert!(receipt.contains("head:"));
    assert!(receipt.contains("mode: terminal-reverify"));
    assert!(receipt.contains("overall:"));
}

#[test]
fn terminal_reverify_markerless_prompt_no_journal_passes() {
    let temp = create_temp_repo();
    let t1 = tid(1);
    let path = write_terminal_fixture(
        temp.path(),
        "markerless-no-journal",
        &terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n"),
    );
    commit_all(temp.path(), "fixture: clean markerless prompt");
    let (code, output) = run_terminal_reverify(temp.path(), &path);
    let receipt = std::fs::read_to_string(terminal_reverify_receipt_path(
        temp.path(),
        "markerless-no-journal",
    ))
    .unwrap();
    assert_eq!(code, 0, "output: {output}, receipt: {receipt}");
    assert!(receipt.contains("overall: pass"));
}

#[test]
fn preflight_prints_route_line_before_summary_in_both_modes() {
    let passing = create_temp_repo();
    let t1 = tid(1);
    let passing_prompt = format!(
        "# Plan Prompt: Route Line Passing Fixture\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {t1}\n\n## Tasks\n\n- [ ] {t1} — route line fixture.\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {t1} | unit | cargo test | pass | TESTER | test output |\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {t1} | route line |\n"
    );
    let (path, receipt) = write_plan_and_state(passing.path(), "route-line-pass", &passing_prompt);
    let output = run_preflight_output(passing.path(), &path, &receipt);
    assert_eq!(output.status.code(), Some(0));
    assert_route_line(&String::from_utf8_lossy(&output.stdout));

    let failing = create_temp_repo();
    let failing_prompt = passing_prompt.replace("Workflow: IMPLEMENT", "Workflow: DONE");
    let (path, receipt) = write_plan_and_state(failing.path(), "route-line-fail", &failing_prompt);
    let output = run_preflight_output(failing.path(), &path, &receipt);
    assert_ne!(output.status.code(), Some(0));
    assert_route_line(&String::from_utf8_lossy(&output.stdout));

    let terminal = create_temp_repo();
    let prompt = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let path = write_terminal_fixture(terminal.path(), "route-line-terminal", &prompt);
    commit_all(terminal.path(), "fixture: route line terminal");
    let output = run_terminal_reverify_output(terminal.path(), &path);
    assert_route_line_shape(&String::from_utf8_lossy(&output.stdout));
}

#[test]
fn preflight_rejects_retired_marker_but_accepts_in_sentence_mention() {
    let rejected = create_temp_repo();
    let t1 = tid(1);
    let rejected_prompt = format!(
        "# Plan Prompt: Retired Marker Fixture\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {t1}\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {t1} — reject marker.\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":false,\"cross_boundary\":false}}\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {t1} | unit | cargo test | pass | TESTER | test output |\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {t1} | retired marker |\n"
    );
    let (rejected_path, rejected_receipt) =
        write_plan_and_state(rejected.path(), "retired-marker", &rejected_prompt);
    let (rejected_code, rejected_output) =
        run_preflight(&rejected.path(), &rejected_path, &rejected_receipt);
    assert_ne!(rejected_code, 0, "output: {rejected_output}");
    let rejected_receipt_text = std::fs::read_to_string(&rejected_receipt).unwrap();
    assert!(rejected_receipt_text.contains(
        "| retired-marker | fail | — | prompt carries a retired contract marker, regenerate with /plan-to-prompt |"
    ));

    let accepted = create_temp_repo();
    let accepted_prompt = rejected_prompt.replace(
        "Pipeline Contract: test-first-v1",
        "The prompt mentions Pipeline Contract: test-first-v1 in this sentence.",
    );
    let (accepted_path, accepted_receipt) =
        write_plan_and_state(accepted.path(), "marker-mention", &accepted_prompt);
    let (accepted_code, accepted_output) =
        run_preflight(&accepted.path(), &accepted_path, &accepted_receipt);
    assert_eq!(accepted_code, 0, "output: {accepted_output}");
    let accepted_receipt_text = std::fs::read_to_string(&accepted_receipt).unwrap();
    assert!(accepted_receipt_text.contains(
        "| retired-marker | pass | — | prompt does not carry a retired contract marker |"
    ));
}

fn run_preflight_output(
    repo: &Path,
    prompt_path: &Path,
    receipt_path: &Path,
) -> std::process::Output {
    Command::new(test_gal_exe())
        .current_dir(repo)
        .args(["pipeline-preflight"])
        .arg(prompt_path)
        .args(["--receipt"])
        .arg(receipt_path)
        .output()
        .expect("failed to execute gal binary")
}

#[test]
fn colon_task_identity_is_shared_by_prompt_check_and_preflight() {
    let temp = create_temp_repo();
    let prompt = task_grammar_prompt(&format!("- [ ] {}: valid colon task.", tid(1)));
    let (path, receipt_path) = write_plan_and_state(temp.path(), "colon-task", &prompt);

    let prompt_output = run_prompt_check(temp.path(), &path);
    let prompt_receipt = std::fs::read_to_string(
        temp.path()
            .join(".dev/pipeline/colon-task/prompt-check.receipt.md"),
    )
    .unwrap_or_default();
    assert!(
        prompt_receipt.contains("| unchecked-task-anchor | pass |"),
        "prompt-check output: {}\nreceipt: {}",
        String::from_utf8_lossy(&prompt_output.stdout),
        prompt_receipt,
    );

    let (preflight_code, receipt) = run_preflight(temp.path(), &path, &receipt_path);
    assert_eq!(preflight_code, 0, "preflight receipt: {receipt}");
    assert!(
        receipt.contains("| tasks-well-formed | pass |"),
        "{receipt}"
    );
}

#[test]
fn fenced_and_malformed_task_examples_do_not_count_for_prompt_or_preflight() {
    for (slug, task_line) in [
        (
            "fenced-task",
            format!("```markdown\n- [ ] {} — fenced example.\n```", tid(1)),
        ),
        ("malformed-task", format!("- [ ] T-{} — one digit.", 1)),
    ] {
        let temp = create_temp_repo();
        let prompt = task_grammar_prompt(&task_line);
        let (path, receipt_path) = write_plan_and_state(temp.path(), slug, &prompt);

        let prompt_output = run_prompt_check(temp.path(), &path);
        let prompt_receipt = std::fs::read_to_string(
            temp.path()
                .join(format!(".dev/pipeline/{slug}/prompt-check.receipt.md")),
        )
        .unwrap_or_default();
        assert!(
            prompt_receipt.contains("| unchecked-task-anchor | fail |"),
            "{slug} prompt-check output: {}\nreceipt: {}",
            String::from_utf8_lossy(&prompt_output.stdout),
            prompt_receipt,
        );

        let (preflight_code, receipt) = run_preflight(temp.path(), &path, &receipt_path);
        assert_ne!(preflight_code, 0, "{slug} unexpectedly passed preflight");
        assert!(
            receipt.contains("| tasks-well-formed | fail |"),
            "{slug} receipt: {receipt}"
        );
    }
}

#[test]
fn post_recovery_worktree_checks_are_recorded_in_preflight_receipt() {
    let temp = create_temp_repo();
    let prompt = task_grammar_prompt(&format!("- [ ] {}: tracked prompt", tid(1)));
    let (path, receipt_path) = write_plan_and_state(temp.path(), "recovery-record", &prompt);
    git(temp.path(), &["add", "--all"]);
    git(temp.path(), &["commit", "-qm", "fixture prompt"]);

    let receipt = run_preflight_after_recovery(temp.path(), &path, &receipt_path);
    assert!(
        receipt.contains("| entry-recovery-worktree-clean | pass |"),
        "{receipt}"
    );
    assert!(
        receipt.contains("| entry-recovery-prompt-tracked | pass |"),
        "{receipt}"
    );
}

fn create_linked_worktree(primary: &Path, checkout: &Path) {
    git(
        primary,
        &[
            "worktree",
            "add",
            "-q",
            "-b",
            "fixture-linked",
            checkout.to_str().unwrap(),
            "HEAD",
        ],
    );
}

#[test]
fn fresh_preflight_merges_prompt_from_primary_before_checking_it() {
    let primary = create_temp_repo();
    std::fs::write(primary.path().join("base.txt"), "base\n").unwrap();
    commit_all(primary.path(), "fixture base");

    let linked_root = tempdir().expect("linked worktree parent");
    let linked = linked_root.path().join("checkout");
    create_linked_worktree(primary.path(), &linked);

    let prompt = task_grammar_prompt(&format!("- [ ] {}: supplied by primary", tid(1)));
    let (primary_prompt, _) = write_plan_and_state(primary.path(), "primary-prompt", &prompt);
    commit_all(primary.path(), "fixture primary prompt");
    let before = git(&linked, &["rev-parse", "HEAD"]);
    let linked_prompt = linked.join(".dev/plans/primary-prompt.prompt.md");
    let receipt = primary.path().join("fresh-entry.receipt.md");

    let output = run_preflight_output(&linked, &linked_prompt, &receipt);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(git(&linked, &["rev-parse", "HEAD"]), before);
    assert!(linked_prompt.is_file(), "primary prompt was not merged");
    let receipt_text = std::fs::read_to_string(receipt).unwrap_or_default();
    assert!(
        receipt_text.contains("| entry-recovery-prompt-tracked | pass |"),
        "{receipt_text}"
    );
    assert!(
        receipt_text.contains("| entry-recovery-worktree-clean | pass |"),
        "{receipt_text}"
    );
    assert!(primary_prompt.is_file());
}

#[test]
fn fresh_preflight_refuses_untracked_custody_without_merging() {
    let primary = create_temp_repo();
    std::fs::write(primary.path().join("base.txt"), "base\n").unwrap();
    commit_all(primary.path(), "fixture base");

    let linked_root = tempdir().expect("linked worktree parent");
    let linked = linked_root.path().join("checkout");
    create_linked_worktree(primary.path(), &linked);
    let prompt = task_grammar_prompt(&format!("- [ ] {}: primary prompt", tid(1)));
    write_plan_and_state(primary.path(), "custody-prompt", &prompt);
    commit_all(primary.path(), "fixture primary prompt");

    let custody = linked.join("untracked-custody.txt");
    std::fs::write(&custody, "owned by caller\n").unwrap();
    let before = git(&linked, &["rev-parse", "HEAD"]);
    let receipt = primary.path().join("custody-entry.receipt.md");
    let prompt_path = linked.join(".dev/plans/custody-prompt.prompt.md");

    let (code, output) = run_preflight(&linked, &prompt_path, &receipt);

    assert_ne!(code, 0, "dirty custody unexpectedly passed: {output}");
    assert_eq!(git(&linked, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        std::fs::read_to_string(custody).unwrap(),
        "owned by caller\n"
    );
    assert!(
        !prompt_path.exists(),
        "refused preflight changed the worktree"
    );
}

#[test]
fn fresh_preflight_preserves_branch_only_prompt_while_merging_primary_tip() {
    let primary = create_temp_repo();
    std::fs::write(primary.path().join("base.txt"), "base\n").unwrap();
    commit_all(primary.path(), "fixture base");

    let linked_root = tempdir().expect("linked worktree parent");
    let linked = linked_root.path().join("checkout");
    create_linked_worktree(primary.path(), &linked);
    let prompt = task_grammar_prompt(&format!("- [ ] {}: branch-only prompt", tid(1)));
    let linked_prompt = linked.join(".dev/plans/branch-only.prompt.md");
    std::fs::create_dir_all(linked_prompt.parent().unwrap()).unwrap();
    std::fs::write(&linked_prompt, prompt).unwrap();
    std::fs::write(
        linked.join(".dev/state.md"),
        format!(
            "## Session Continuity\n\n| branch-only | .dev/plans/branch-only.prompt.md | in progress |\n"
        ),
    )
    .unwrap();
    commit_all(&linked, "fixture branch-only prompt");

    std::fs::write(primary.path().join("primary.txt"), "primary\n").unwrap();
    commit_all(primary.path(), "fixture primary advance");
    let before = git(&linked, &["rev-parse", "HEAD"]);
    let receipt = primary.path().join("branch-only-entry.receipt.md");

    let output = run_preflight_output(&linked, &linked_prompt, &receipt);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_ne!(git(&linked, &["rev-parse", "HEAD"]), before);
    assert!(linked_prompt.is_file(), "branch-only prompt was lost");
    assert_eq!(
        std::fs::read_to_string(linked.join("primary.txt")).unwrap(),
        "primary\n"
    );
}

#[test]
fn fresh_preflight_merges_non_conflicting_ancestry_drift() {
    let primary = create_temp_repo();
    std::fs::write(primary.path().join("base.txt"), "base\n").unwrap();
    commit_all(primary.path(), "fixture base");

    let linked_root = tempdir().expect("linked worktree parent");
    let linked = linked_root.path().join("checkout");
    create_linked_worktree(primary.path(), &linked);
    let prompt = task_grammar_prompt(&format!("- [ ] {}: ancestry drift", tid(1)));
    let (primary_prompt, _) = write_plan_and_state(primary.path(), "ancestry-drift", &prompt);
    commit_all(primary.path(), "fixture primary prompt");

    std::fs::write(linked.join("linked-change.txt"), "linked\n").unwrap();
    commit_all(&linked, "fixture linked advance");
    let before = git(&linked, &["rev-parse", "HEAD"]);
    let primary_tip = git(primary.path(), &["rev-parse", "HEAD"]);
    let receipt = linked_root.path().join("ancestry-drift.receipt.md");
    let linked_prompt = linked.join(".dev/plans/ancestry-drift.prompt.md");

    let output = run_preflight_output(&linked, &linked_prompt, &receipt);

    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let merged = git(&linked, &["rev-parse", "HEAD"]);
    assert_ne!(merged, before);
    assert_eq!(git(&linked, &["rev-parse", "HEAD^2"]), primary_tip);
    assert!(linked_prompt.is_file(), "primary prompt was not merged");
    assert!(primary_prompt.is_file());
    assert!(linked.join("linked-change.txt").is_file());
    assert!(receipt.is_file(), "fresh-entry receipt was not written");
}

#[test]
fn terminal_reverify_does_not_merge_a_new_primary_tip() {
    let primary = create_temp_repo();
    std::fs::write(primary.path().join("base.txt"), "base\n").unwrap();
    commit_all(primary.path(), "fixture base");

    let linked_root = tempdir().expect("linked worktree parent");
    let linked = linked_root.path().join("checkout");
    create_linked_worktree(primary.path(), &linked);
    let t1 = tid(1);
    let terminal = terminal_prompt(&t1, &format!("- [x] {t1} — complete."), "\n(none)\n");
    let prompt_path = write_terminal_fixture(&linked, "terminal-bypass", &terminal);
    commit_all(&linked, "fixture terminal prompt");

    std::fs::write(primary.path().join("primary.txt"), "primary\n").unwrap();
    commit_all(primary.path(), "fixture primary advance");
    let before = git(&linked, &["rev-parse", "HEAD"]);

    let (code, output) = run_terminal_reverify(&linked, &prompt_path);

    assert_eq!(code, 0, "terminal reverify output: {output}");
    assert_eq!(git(&linked, &["rev-parse", "HEAD"]), before);
    assert!(!linked.join("primary.txt").exists());
}

#[test]
fn merge_conflict_aborts_and_restores_linked_worktree_state() {
    let primary = create_temp_repo();
    std::fs::write(primary.path().join("base.txt"), "base\n").unwrap();
    commit_all(primary.path(), "fixture base");

    let linked_root = tempdir().expect("linked worktree parent");
    let linked = linked_root.path().join("checkout");
    create_linked_worktree(primary.path(), &linked);
    std::fs::write(linked.join("shared.txt"), "linked value\n").unwrap();
    commit_all(&linked, "fixture linked conflict");

    let prompt = task_grammar_prompt(&format!("- [ ] {}: primary prompt", tid(1)));
    write_plan_and_state(primary.path(), "conflict-prompt", &prompt);
    std::fs::write(primary.path().join("shared.txt"), "primary value\n").unwrap();
    commit_all(primary.path(), "fixture primary conflict");
    let before_head = git(&linked, &["rev-parse", "HEAD"]);
    let before_status = git(&linked, &["status", "--porcelain"]);
    let receipt = primary.path().join("conflict-entry.receipt.md");
    let prompt_path = linked.join(".dev/plans/conflict-prompt.prompt.md");

    let (code, output) = run_preflight(&linked, &prompt_path, &receipt);

    assert_ne!(code, 0, "conflicting merge unexpectedly passed: {output}");
    assert_eq!(git(&linked, &["rev-parse", "HEAD"]), before_head);
    assert_eq!(git(&linked, &["status", "--porcelain"]), before_status);
    assert_eq!(
        std::fs::read_to_string(linked.join("shared.txt")).unwrap(),
        "linked value\n"
    );
    assert!(!prompt_path.exists());
}
