//! Integration tests for containment evidence emission on unmeasurable reads and sibling snapshots.

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
        log_dir: tmp
            .path()
            .join(".dev/pipeline")
            .join(gal_foundation::time::utc_date_compact())
            .join("test-direct"),
        receipt_path: None,
        contract_provenance: None,
    }
}

#[test]
fn spawn_executor_unmeasurable_assigned_read_reaches_evidence() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let receipt = tmp.path().join("receipt.txt");
    let git_head = main_repo.join(".git").join("HEAD");

    #[cfg(target_os = "windows")]
    let (exe, args) = (
        "cmd",
        vec![
            "/C".to_string(),
            format!(
                "echo verified receipt>{} & del /f /q {}",
                receipt.display(),
                git_head.display()
            ),
        ],
    );
    #[cfg(not(target_os = "windows"))]
    let (exe, args) = (
        "sh",
        vec![
            "-c".to_string(),
            format!(
                "printf '%s' 'verified receipt' > '{}' && rm -f '{}'",
                receipt.display(),
                git_head.display()
            ),
        ],
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

    assert_ne!(
        result.terminal_state,
        TerminalState::NoWriteback,
        "unmeasurable assigned read must not classify as no-writeback"
    );
    assert!(
        result.stderr.contains("worktree containment evidence"),
        "unmeasurable read must reach evidence without a sibling change"
    );
}

#[test]
fn spawn_executor_unmeasurable_sibling_snapshot_reaches_evidence() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let wt2 = tmp.path().join("wt2");
    let output = Command::new("git")
        .args(["worktree", "add", "-b", "wt2-branch", wt2.to_str().unwrap()])
        .current_dir(&main_repo)
        .output()
        .expect("git worktree add must succeed");
    assert!(output.status.success(), "git worktree add failed");

    // Remove the sibling directory so its snapshot cannot be taken
    fs::remove_dir_all(&wt2).expect("failed to remove sibling directory");

    let receipt = tmp.path().join("receipt.txt");
    #[cfg(target_os = "windows")]
    let (exe, args) = (
        "cmd",
        vec![
            "/C".to_string(),
            format!("echo verified receipt>{}", receipt.display()),
        ],
    );
    #[cfg(not(target_os = "windows"))]
    let (exe, args) = (
        "sh",
        vec![
            "-c".to_string(),
            format!("printf '%s' 'verified receipt' > '{}'", receipt.display()),
        ],
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

    assert!(
        result.stderr.contains("worktree containment evidence"),
        "unmeasurable read must reach evidence without a sibling change"
    );
}

#[test]
fn spawn_executor_sibling_diff_exceeding_line_cap_emits_truncation_marker() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let wt2 = tmp.path().join("wt2");
    let output = Command::new("git")
        .args(["worktree", "add", "-b", "wt2-branch", wt2.to_str().unwrap()])
        .current_dir(&main_repo)
        .output()
        .expect("git worktree add must succeed");
    assert!(output.status.success(), "git worktree add failed");

    let receipt = tmp.path().join("receipt.txt");
    #[cfg(target_os = "windows")]
    let mut cmd_str = format!("echo verified receipt>{}", receipt.display());
    #[cfg(target_os = "windows")]
    for i in 0..25 {
        cmd_str.push_str(&format!(" & echo test>{}\\extra_{i}.txt", wt2.display()));
    }
    #[cfg(target_os = "windows")]
    let (exe, args) = ("cmd", vec!["/C".to_string(), cmd_str]);

    #[cfg(not(target_os = "windows"))]
    let mut sh_str = format!("printf '%s' 'verified receipt' > '{}'", receipt.display());
    #[cfg(not(target_os = "windows"))]
    for i in 0..25 {
        sh_str.push_str(&format!(
            " && printf '%s' 'test' > '{}/extra_{i}.txt'",
            wt2.display()
        ));
    }
    #[cfg(not(target_os = "windows"))]
    let (exe, args) = ("sh", vec!["-c".to_string(), sh_str]);

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

    assert!(
        result.stderr.contains("worktree containment evidence"),
        "sibling change must reach containment evidence"
    );
    assert!(
        result.stderr.contains("truncated"),
        "per-worktree line cap must emit a truncation marker"
    );
}

#[test]
fn spawn_executor_assigned_workdir_evidence_carries_diff_lines() {
    let tmp = TempDir::new().expect("failed to create temp dir");
    let main_repo = tmp.path().join("main");
    fs::create_dir_all(&main_repo).expect("failed to create main repo dir");
    init_git_repo_with_commit(&main_repo);

    let wt2 = tmp.path().join("wt2");
    let output = Command::new("git")
        .args(["worktree", "add", "-b", "wt2-branch", wt2.to_str().unwrap()])
        .current_dir(&main_repo)
        .output()
        .expect("git worktree add must succeed");
    assert!(output.status.success(), "git worktree add failed");

    // Remove the sibling directory so its snapshot cannot be taken, triggering evidence emission
    fs::remove_dir_all(&wt2).expect("failed to remove sibling directory");

    let receipt = tmp.path().join("receipt.txt");
    let new_file = main_repo.join("created_by_executor.txt");

    #[cfg(target_os = "windows")]
    let (exe, args) = (
        "cmd",
        vec![
            "/C".to_string(),
            format!(
                "echo verified receipt>{} & echo payload>{}",
                receipt.display(),
                new_file.display()
            ),
        ],
    );
    #[cfg(not(target_os = "windows"))]
    let (exe, args) = (
        "sh",
        vec![
            "-c".to_string(),
            format!(
                "printf '%s' 'verified receipt' > '{}' && printf '%s' 'payload' > '{}'",
                receipt.display(),
                new_file.display()
            ),
        ],
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

    assert!(
        result.stderr.contains("worktree containment evidence"),
        "containment evidence must be emitted"
    );
    assert!(
        !result.stderr.contains("diff item")
            && result.stderr.contains("+?? created_by_executor.txt"),
        "assigned workdir evidence must carry diff lines not a count"
    );
}
