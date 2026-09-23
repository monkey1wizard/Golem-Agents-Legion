//! Real-binary coverage for the legacy `pipeline-converge-check` lane.

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

fn write_prompt(slug: &str, tasks_body: &str) -> (TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let repo = temp.path().to_path_buf();
    Command::new("git")
        .current_dir(&repo)
        .args(["init", "-q", "-b", "main"])
        .output()
        .unwrap();
    let text = format!(
        "# Plan Prompt: {slug}\n\n## Status\n\nCurrent Task: —\n\nTest Retry Count: 0\n\n## Tasks\n\n{tasks_body}\n"
    );
    let prompt_path = repo.join(".dev/plans").join(format!("{slug}.prompt.md"));
    fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
    fs::write(&prompt_path, text).unwrap();
    (temp, prompt_path)
}

fn extract_receipt_line<'a>(receipt: &'a str, check_name: &str) -> Option<&'a str> {
    receipt
        .lines()
        .find(|line| line.starts_with(&format!("| {check_name} |")))
}

fn converge_check(repo: &Path, prompt: &Path, task: &str) -> String {
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
    assert!(
        output.status.success() || output.status.code().is_some(),
        "converge check did not return a process status"
    );
    fs::read_to_string(receipt_path).unwrap_or_default()
}

#[test]
fn legacy_task_omits_test_first_only_checks() {
    let task = tid(1);
    let tasks_body = format!("- [x] {task} — legacy placeholder.\n");
    let (temp, prompt) = write_prompt("legacy-plan", &tasks_body);
    let receipt = converge_check(temp.path(), &prompt, &task);

    assert!(
        extract_receipt_line(&receipt, "boundary-evidence").is_none(),
        "{receipt}"
    );
    assert!(
        extract_receipt_line(&receipt, "test-first-evaluation").is_none(),
        "{receipt}"
    );
}

#[test]
fn legacy_task_emits_only_legacy_phase_rows() {
    let task = tid(2);
    let tasks_body = format!("- [x] {task} — legacy placeholder.\n");
    let (temp, prompt) = write_prompt("legacy-plan-phases", &tasks_body);
    let receipt = converge_check(temp.path(), &prompt, &task);

    assert!(
        extract_receipt_line(&receipt, "phase-scaffold").is_none(),
        "{receipt}"
    );
    for phase in ["implement", "test", "audit"] {
        assert!(
            extract_receipt_line(&receipt, &format!("phase-{phase}")).is_some(),
            "legacy plan must keep its {phase} row:\n{receipt}"
        );
    }
}
