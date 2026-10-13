//! `cargo test -p gal-cli --test preflight_check`

use std::path::{Path, PathBuf};
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
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
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
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
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

fn run_terminal_reverify_output(repo: &Path, prompt_path: &Path) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(["pipeline-preflight", "--terminal-reverify"])
        .arg(prompt_path)
        .output()
        .expect("failed to execute gal binary")
}

fn terminal_prompt(t1: &str, task_line: &str, handoff: &str) -> String {
    format!(
        "# Plan Prompt: Terminal Reverify Fixture\n\n## Status\n\nWorkflow: DONE\nCurrent Task: —\nTest Retry Count: 0\nReview Retry Count: 0\n\n### Handoff Notes\n{handoff}\n\n## Tasks\n\n{task_line}\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {t1} | test feature |\n"
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
        "# Plan Prompt: Route Line Passing Fixture\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {t1}\n\n## Tasks\n\n- [ ] {t1} — route line fixture.\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {t1} | route line |\n"
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
        "# Plan Prompt: Retired Marker Fixture\n\n## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {t1}\n\nPipeline Contract: test-first-v1\n\n## Tasks\n\n- [ ] {t1} — reject marker.\n\n## Test Plan\n\n| ID | desc |\n| --- | --- |\n| {t1} | retired marker |\n"
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
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(["pipeline-preflight"])
        .arg(prompt_path)
        .args(["--receipt"])
        .arg(receipt_path)
        .output()
        .expect("failed to execute gal binary")
}
