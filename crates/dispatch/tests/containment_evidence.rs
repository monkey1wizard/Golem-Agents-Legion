//! Integration tests for containment evidence emission on unmeasurable reads and sibling snapshots.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

use dispatch::dispatch::{
    spawn_executor, spawn_executor_with_context, AttemptContext, SpawnConfig, TerminalState,
};

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

#[test]
fn red_proof_cleanup_removes_only_registered_task_worktrees() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    init_git_repo_with_commit(&repo);
    let red_proof = repo.join(".dev/pipeline/example-plan/TASK-EXAMPLE/red-proof");
    fs::create_dir_all(&red_proof).unwrap();
    fs::write(red_proof.join("probe.txt"), "evidence").unwrap();
    let paths = [
        red_proof.join("one"),
        red_proof.join("two"),
        tmp.path().join("outside"),
    ];
    for path in &paths {
        let output = Command::new("git")
            .current_dir(&repo)
            .args(["worktree", "add", "--detach"])
            .arg(path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let removed =
        dispatch::run::remove_red_proof_worktrees(&repo, "example-plan", "TASK-EXAMPLE").unwrap();
    assert_eq!(removed.len(), 2);
    assert!(!paths[0].exists() && !paths[1].exists());
    assert!(paths[2].exists());
    assert_eq!(
        fs::read_to_string(red_proof.join("probe.txt")).unwrap(),
        "evidence"
    );
    assert!(dispatch::run::remove_red_proof_worktrees(&repo, "..", "TASK-EXAMPLE").is_err());
}

#[test]
fn red_proof_cleanup_removes_deeply_nested_worktree() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    init_git_repo_with_commit(&repo);
    let checkout = repo.join(".dev/pipeline/example-plan/TASK-EXAMPLE/red-proof/deep");
    fs::create_dir_all(checkout.parent().unwrap()).unwrap();
    let output = Command::new("git")
        .current_dir(&repo)
        .args(["worktree", "add", "--detach"])
        .arg(&checkout)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let mut nested = checkout.clone();
    while nested.to_string_lossy().len() <= 300 {
        nested.push("nested_directory_for_long_path");
    }
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("file.txt"), "content").unwrap();

    let removed =
        dispatch::run::remove_red_proof_worktrees(&repo, "example-plan", "TASK-EXAMPLE").unwrap();
    assert_eq!(removed.len(), 1);
    assert!(!checkout.exists());
}

#[test]
fn red_proof_cleanup_reports_locked_worktree_and_keeps_evidence() {
    let tmp = TempDir::new().unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    init_git_repo_with_commit(&repo);
    let red_proof = repo.join(".dev/pipeline/example-plan/TASK-EXAMPLE/red-proof");
    fs::create_dir_all(&red_proof).unwrap();
    fs::write(red_proof.join("probe.txt"), "evidence").unwrap();
    let checkout = red_proof.join("locked");
    for args in [
        vec!["worktree", "add", "--detach"],
        vec!["worktree", "lock"],
    ] {
        let output = Command::new("git")
            .current_dir(&repo)
            .args(args)
            .arg(&checkout)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let error = dispatch::run::remove_red_proof_worktrees(&repo, "example-plan", "TASK-EXAMPLE")
        .unwrap_err();
    assert!(error.contains("locked"), "{error}");
    assert!(checkout.exists());
    assert_eq!(
        fs::read_to_string(red_proof.join("probe.txt")).unwrap(),
        "evidence"
    );
}

#[test]
fn bound_test_dispatch_cleans_after_exit_timeout_and_reports_removal_failure() {
    use dispatch::{cli::DispatchArgs, run::run_dispatch_with_context, stage::Phase};
    struct RestoreEnvironment {
        path: Option<std::ffi::OsString>,
        anthropic_api_key: Option<std::ffi::OsString>,
    }

    impl Drop for RestoreEnvironment {
        fn drop(&mut self) {
            if let Some(path) = self.path.take() {
                std::env::set_var("PATH", path);
            } else {
                std::env::remove_var("PATH");
            }
            if let Some(key) = self.anthropic_api_key.take() {
                std::env::set_var("ANTHROPIC_API_KEY", key);
            } else {
                std::env::remove_var("ANTHROPIC_API_KEY");
            }
        }
    }

    let old_path = std::env::var_os("PATH");
    let _restore_environment = RestoreEnvironment {
        path: old_path.clone(),
        anthropic_api_key: std::env::var_os("ANTHROPIC_API_KEY"),
    };
    std::env::set_var("ANTHROPIC_API_KEY", "test-key");
    for scenario in ["success", "timeout", "locked"] {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        let bin = tmp.path().join("bin");
        fs::create_dir_all(&repo).unwrap();
        fs::create_dir_all(&bin).unwrap();
        init_git_repo_with_commit(&repo);
        let prompt = repo.join(".dev/plans/example-plan.prompt.md");
        fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        fs::write(&prompt, "plan").unwrap();
        let red_proof = repo.join(".dev/pipeline/example-plan/TASK-EXAMPLE/red-proof");
        fs::create_dir_all(&red_proof).unwrap();
        fs::write(red_proof.join("probe.txt"), "evidence").unwrap();
        let checkout = red_proof.join("checkout");
        let second = red_proof.join("second");
        let outside = tmp.path().join("outside");
        for path in [&checkout, &second, &outside] {
            let output = Command::new("git")
                .current_dir(&repo)
                .args(["worktree", "add", "--detach"])
                .arg(path)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        if scenario == "locked" {
            assert!(Command::new("git")
                .current_dir(&repo)
                .args(["worktree", "lock"])
                .arg(&checkout)
                .status()
                .unwrap()
                .success());
        }
        #[cfg(windows)]
        fs::write(bin.join("claude.cmd"), format!("@echo off\r\necho %CARGO_TARGET_DIR%>.dev\\pipeline\\example-plan\\TASK-EXAMPLE\\red-proof\\target.txt\r\necho receipt>.dev\\pipeline\\result.receipt.md\r\n{}\r\n", if scenario == "timeout" { "ping -n 6 127.0.0.1 >nul" } else { "exit /b 0" })).unwrap();
        #[cfg(not(windows))]
        {
            use std::os::unix::fs::PermissionsExt;
            let file = bin.join("claude");
            fs::write(&file, format!("#!/bin/sh\nprintf '%s' \"$CARGO_TARGET_DIR\" > .dev/pipeline/example-plan/TASK-EXAMPLE/red-proof/target.txt\nprintf receipt > .dev/pipeline/result.receipt.md\n{}\n", if scenario == "timeout" { "sleep 6" } else { "exit 0" })).unwrap();
            fs::set_permissions(&file, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let mut paths = vec![bin];
        if let Some(old_path) = old_path.as_ref() {
            paths.extend(std::env::split_paths(old_path));
        }
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
        let routing = tmp.path().join("routing.json");
        fs::write(&routing, r#"{"executorRouting":{"executors":{"claude":"sonnet"},"pipeline":{"TESTER":{"executor":"claude"}}}}"#).unwrap();
        let args = DispatchArgs {
            phase: Phase::Test,
            task: "TASK-EXAMPLE".into(),
            workdir: repo.clone(),
            routing_path: Some(routing),
            timeout_secs: if scenario == "timeout" { 1 } else { 30 },
            receipt_path: Some(repo.join(".dev/pipeline/result.receipt.md")),
            stdout_quiet: true,
            ..DispatchArgs::default()
        };
        let outcome = run_dispatch_with_context(
            &args,
            "test",
            Some(&AttemptContext::default().with_canonical_prompt(prompt)),
        );
        assert_eq!(
            checkout.exists(),
            scenario == "locked",
            "{scenario}: {:?}",
            outcome.reason
        );
        assert_eq!(second.exists(), scenario == "locked");
        assert!(outside.exists());
        assert_eq!(
            fs::read_to_string(red_proof.join("probe.txt")).unwrap(),
            "evidence"
        );
        let target = fs::read_to_string(red_proof.join("target.txt")).unwrap();
        let receipt = fs::read_to_string(repo.join(".dev/pipeline/result.receipt.md")).unwrap();
        let receipt = receipt
            .strip_suffix("\r\n")
            .or_else(|| receipt.strip_suffix('\n'))
            .unwrap_or(&receipt);
        assert_eq!(receipt, "receipt");
        assert_eq!(
            std::path::Path::new(target.trim()),
            repo.canonicalize()
                .unwrap()
                .join(".dev/pipeline/.cargo-target")
        );
        if scenario == "locked" {
            assert_ne!(outcome.exit_code, 0);
            assert!(outcome
                .reason
                .unwrap()
                .contains("red-proof cleanup failed:"));
            assert!(fs::read_dir(repo.join(dispatch::dispatch::LEASE_ROOT))
                .unwrap()
                .any(|entry| entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "lock")));
        } else if scenario == "success" {
            assert_eq!(outcome.exit_code, 0, "{:?}", outcome.reason);
        } else {
            assert_ne!(outcome.exit_code, 0);
        }
    }
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
fn workdir_change_identifies_bound_test_probe_directory() {
    for (phase, bound, raw) in [
        ("test", true, false),
        ("audit", true, false),
        ("test", false, false),
        ("test", false, true),
    ] {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).unwrap();
        init_git_repo_with_commit(&repo);
        let prompt = repo.join(".dev/plans/example-plan.prompt.md");
        fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        fs::write(&prompt, "plan").unwrap();

        #[cfg(target_os = "windows")]
        let (executor, args) = ("cmd", vec!["/C", "echo changed>file.txt"]);
        #[cfg(not(target_os = "windows"))]
        let (executor, args) = ("sh", vec!["-c", "printf changed > file.txt"]);
        let mut cfg = test_config(&tmp, executor, args, phase, 30);
        cfg.workdir = repo;
        cfg.task_id = "TASK-EXAMPLE".to_owned();
        let context = if bound {
            AttemptContext::default().with_canonical_prompt(prompt)
        } else {
            AttemptContext::default()
        };
        let (result, _) = spawn_executor_with_context(&cfg, (!raw).then_some(&context)).unwrap();
        if !raw {
            assert_eq!(result.terminal_state, TerminalState::WorkdirChanged);
            assert!(result.stderr.contains("file.txt"), "{}", result.stderr);
        }
        let probe = ".dev/pipeline/example-plan/TASK-EXAMPLE/red-proof/";
        assert_eq!(result.stderr.contains(probe), phase == "test" && bound);
        if phase != "test" || !bound {
            assert!(!result.stderr.contains("suggested probe path:"));
        }
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
