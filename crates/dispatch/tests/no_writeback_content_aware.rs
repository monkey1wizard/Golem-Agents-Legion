//! Integration tests for content-aware NoWriteback classification in headless dispatch.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use dispatch::dispatch::{spawn_executor, SpawnConfig, TerminalState};

fn init_git_repo_with_commit(path: &Path) {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git must be runnable");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test Runner"]);
    git(&["config", "commit.gpgsign", "false"]);
    fs::write(path.join("file.txt"), "initial content\n").expect("failed to write initial file");
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "initial commit"]);
}

fn test_config(
    tmp: &TempDir,
    executor: &str,
    args: Vec<&str>,
    phase: &str,
    timeout_secs: u64,
) -> SpawnConfig {
    SpawnConfig {
        executor: executor.to_string(),
        executor_args: args.iter().map(|s| s.to_string()).collect(),
        spec: phase.to_string(),
        workdir: tmp.path().to_path_buf(),
        timeout_secs,
        task_id: "T-TEST".to_string(),
        phase: phase.to_string(),
        actual_model: "test-model".to_string(),
        log_dir: tmp.path().join("executor-logs"),
        receipt_path: None,
        contract_provenance: None,
    }
}

fn receipt_writer_and_file_editor_args(
    receipt: &Path,
    receipt_content: &str,
    tracked_file: &Path,
    tracked_content: &str,
) -> (&'static str, Vec<String>) {
    #[cfg(target_os = "windows")]
    {
        (
            "cmd",
            vec![
                "/C".to_string(),
                format!(
                    "echo {receipt_content}>{} & echo {tracked_content}>{}",
                    receipt.display(),
                    tracked_file.display()
                ),
            ],
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        (
            "sh",
            vec![
                "-c".to_string(),
                format!(
                    "printf '%s' '{receipt_content}' > '{}' && printf '%s' '{tracked_content}' > '{}'",
                    receipt.display(),
                    tracked_file.display()
                ),
            ],
        )
    }
}

fn receipt_writer_only_args(receipt: &Path, receipt_content: &str) -> (&'static str, Vec<String>) {
    #[cfg(target_os = "windows")]
    {
        (
            "cmd",
            vec![
                "/C".to_string(),
                format!("echo {receipt_content}>{}", receipt.display()),
            ],
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        (
            "sh",
            vec![
                "-c".to_string(),
                format!("printf '%s' '{receipt_content}' > '{}'", receipt.display()),
            ],
        )
    }
}

#[test]
fn spawn_executor_predirty_tracked_edit_classifies_as_completed() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let tracked_file = main_repo.join("file.txt");
    // Pre-dirty the tracked file before dispatch so porcelain snapshot shows " M file.txt"
    fs::write(&tracked_file, "pre-dirty content\n").expect("failed to pre-dirty tracked file");

    let receipt = tmp.path().join("receipt.txt");
    let (exe, args) = receipt_writer_and_file_editor_args(
        &receipt,
        "verified receipt",
        &tracked_file,
        "executor modified content",
    );

    let mut cfg = test_config(
        &tmp,
        exe,
        args.iter().map(|s| s.as_str()).collect(),
        "implement",
        30,
    );
    cfg.workdir = main_repo.clone();
    cfg.receipt_path = Some(receipt);

    let result = spawn_executor(&cfg).expect("spawn_executor must return result");

    assert_eq!(
        result.terminal_state,
        TerminalState::Completed,
        "pre-dirty tracked edit must not classify as no-writeback"
    );
}

#[test]
fn spawn_executor_predirty_untracked_edit_classifies_as_completed() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let untracked_file = main_repo.join("untracked.txt");
    // Pre-dirty the untracked file before dispatch so porcelain snapshot shows "?? untracked.txt"
    fs::write(&untracked_file, "pre-dirty untracked content\n")
        .expect("failed to pre-dirty untracked file");

    let receipt = tmp.path().join("receipt.txt");
    let (exe, args) = receipt_writer_and_file_editor_args(
        &receipt,
        "verified receipt",
        &untracked_file,
        "executor modified untracked content",
    );

    let mut cfg = test_config(
        &tmp,
        exe,
        args.iter().map(|s| s.as_str()).collect(),
        "implement",
        30,
    );
    cfg.workdir = main_repo.clone();
    cfg.receipt_path = Some(receipt);

    let result = spawn_executor(&cfg).expect("spawn_executor must return result");

    assert_eq!(
        result.terminal_state,
        TerminalState::Completed,
        "pre-dirty untracked edit must not classify as no-writeback"
    );
}

#[test]
fn spawn_executor_clean_tree_zero_write_remains_no_writeback() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let receipt = tmp.path().join("receipt.txt");
    let (exe, args) = receipt_writer_only_args(&receipt, "verified receipt");

    let mut cfg = test_config(
        &tmp,
        exe,
        args.iter().map(|s| s.as_str()).collect(),
        "implement",
        30,
    );
    cfg.workdir = main_repo.clone();
    cfg.receipt_path = Some(receipt);

    let result = spawn_executor(&cfg).expect("spawn_executor must return result");

    assert_eq!(
        result.terminal_state,
        TerminalState::NoWriteback,
        "clean tree zero write in implement phase must classify as no-writeback"
    );
}

#[test]
fn spawn_executor_zero_write_audit_remains_completed() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let receipt = tmp.path().join("receipt.txt");
    let (exe, args) = receipt_writer_only_args(&receipt, "verified receipt");

    let mut cfg = test_config(
        &tmp,
        exe,
        args.iter().map(|s| s.as_str()).collect(),
        "audit",
        30,
    );
    cfg.workdir = main_repo.clone();
    cfg.receipt_path = Some(receipt);

    let result = spawn_executor(&cfg).expect("spawn_executor must return result");

    assert_eq!(
        result.terminal_state,
        TerminalState::Completed,
        "zero write in audit phase must remain completed"
    );
}
