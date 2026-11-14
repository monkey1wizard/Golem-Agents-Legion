//! Quota attached to dispatch terminal paths, driven by fake executors that print fixture output.
//!
//! Every case runs in one test because each case rewrites `PATH` to find its own fake executor.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use dispatch::cli::DispatchArgs;
use dispatch::dispatch::{spawn_executor, DispatchQuota, DispatchResult, SpawnConfig};
use dispatch::quota::Acquisition;
use dispatch::run::{run_dispatch, DispatchOutcome};
use dispatch::stage::Phase;
use dispatch::usage::Availability;
use serde_json::Value;
use tempfile::TempDir;

const FIXTURE: &str = include_str!("fixtures/quota_processes.json");

/// Serializes the tests that rewrite the process-wide `PATH` and `GEMINI_API_KEY`.
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// Writes a fake executor named `executor` that prints `emit` lines, then holds for `hold_secs`.
fn write_fake_executor(bin_dir: &Path, executor: &str, emit: &[String], hold_secs: u64) {
    #[cfg(target_os = "windows")]
    {
        let mut script = String::from("@echo off\r\n");
        for line in emit {
            script.push_str(&format!("echo {line}\r\n"));
        }
        if hold_secs > 0 {
            script.push_str(&format!("ping -n {} 127.0.0.1 >nul\r\n", hold_secs + 1));
        }
        script.push_str("exit /b 0\r\n");
        fs::write(bin_dir.join(format!("{executor}.cmd")), script).unwrap();
    }
    #[cfg(not(target_os = "windows"))]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut script = String::from("#!/bin/sh\n");
        for line in emit {
            script.push_str(&format!("printf '%s\\n' '{line}'\n"));
        }
        if hold_secs > 0 {
            script.push_str(&format!("sleep {hold_secs}\n"));
        }
        let path = bin_dir.join(executor);
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }
}

fn test_config(tmp: &TempDir, executor: &str, timeout_secs: u64) -> SpawnConfig {
    SpawnConfig {
        executor: executor.to_string(),
        executor_args: Vec::new(),
        spec: "quota probe".to_string(),
        workdir: tmp.path().to_path_buf(),
        timeout_secs,
        task_id: "T-TEST".to_string(),
        phase: "test".to_string(),
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

fn acquisition_kind(acquisition: &Acquisition) -> &'static str {
    match acquisition {
        Acquisition::Passive => "passive",
        Acquisition::Queried => "queried",
        Acquisition::Failed { .. } => "failed",
        Acquisition::TimedOut { .. } => "timed-out",
        Acquisition::Unsupported { .. } => "unsupported",
    }
}

fn assert_case(case: &Value, result: &DispatchResult) {
    let name = case["name"].as_str().unwrap();
    let expect = &case["expect"];

    assert_eq!(
        result.terminal_state.as_str(),
        expect["terminal_state"].as_str().unwrap(),
        "{name}: terminal state"
    );

    let (kind, observations) = match &result.quota {
        DispatchQuota::NotAttempted => ("not-attempted", &[][..]),
        DispatchQuota::UnknownScope { reason } => {
            assert!(
                !reason.is_empty(),
                "{name}: unknown scope must name a reason"
            );
            ("unknown-scope", &[][..])
        }
        DispatchQuota::Collected { observations } => ("collected", observations.as_slice()),
    };
    assert_eq!(
        kind,
        expect["quota"].as_str().unwrap(),
        "{name}: quota kind"
    );

    if let Some(expected) = expect.get("acquisitions").and_then(Value::as_array) {
        let observed: Vec<&str> = observations
            .iter()
            .map(|observation| acquisition_kind(&observation.acquisition))
            .collect();
        let expected: Vec<&str> = expected.iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(observed, expected, "{name}: acquisitions in order");
    }

    match expect["utilization"].as_str() {
        Some(expected) => {
            let first = observations
                .first()
                .expect("utilization needs an observation");
            match &first.utilization {
                Availability::Reported { value } => {
                    assert_eq!(value.value().to_string(), expected, "{name}: utilization")
                }
                other => panic!("{name}: utilization not reported: {other:?}"),
            }
        }
        None => {}
    }

    // A gap never reads as a value: every observation without a reported utilization names why.
    for observation in observations {
        if let Availability::Unavailable { reason } | Availability::Malformed { reason } =
            &observation.utilization
        {
            assert!(!reason.is_empty(), "{name}: gap must carry a reason");
        }
    }
}

#[test]
fn quota_is_attached_on_every_terminal_case() {
    let _lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
    let original_path = std::env::var_os("PATH").unwrap_or_default();

    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let executor = case["executor"].as_str().unwrap();
        let emit: Vec<String> = case["emit"]
            .as_array()
            .unwrap()
            .iter()
            .map(|line| line.as_str().unwrap().to_string())
            .collect();
        let hold_secs = case["hold_secs"].as_u64().unwrap();
        let timeout_secs = case["timeout_secs"].as_u64().unwrap();

        let tmp = TempDir::new().unwrap();
        let bin_dir = tmp.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        if case["fake"].as_bool() != Some(false) {
            write_fake_executor(&bin_dir, executor, &emit, hold_secs);
        }

        let mut paths = vec![bin_dir];
        paths.extend(std::env::split_paths(&original_path));
        std::env::set_var("PATH", std::env::join_paths(paths).unwrap());

        let cfg = test_config(&tmp, executor, timeout_secs);
        let result = spawn_executor(&cfg).unwrap();
        assert_case(case, &result);
        eprintln!(
            "quota_dispatch case={name} terminal={} quota={:?}",
            result.terminal_state.as_str(),
            result.quota
        );
    }
}

/// Initializes a git repository, because the local dispatch flow expects its workdir to be one.
fn init_git_repo(path: &Path) {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git must run");
        assert!(output.status.success(), "git failed: {args:?}");
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "tester@example.com"]);
    git(&["config", "user.name", "Tester"]);
    git(&["config", "commit.gpgsign", "false"]);
    fs::write(path.join("file.txt"), "initial content\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "initial commit"]);
}

/// Writes a routing table that sends the tester phase to `executor`.
fn write_routing(dir: &Path, executor: &str) -> PathBuf {
    let path = dir.join("routing.json");
    fs::write(
        &path,
        format!(
            r#"{{"executorRouting":{{"executors":{{"{executor}":"gemini-2.5-pro"}},"pipeline":{{"TESTER":{{"executor":"{executor}"}}}}}}}}"#
        ),
    )
    .unwrap();
    path
}

fn dispatch_args(workdir: &Path, routing_path: PathBuf) -> DispatchArgs {
    DispatchArgs {
        phase: Phase::Test,
        task: "T-QUOTA".to_string(),
        workdir: workdir.to_path_buf(),
        timeout_secs: 30,
        routing_path: Some(routing_path),
        stdout_quiet: true,
        ..DispatchArgs::default()
    }
}

#[test]
fn run_dispatch_outcome_carries_the_return_quota() {
    let _lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    let bin_dir = tmp.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let emit = [concat!(
        r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"SUCCESS","#,
        r#""quota":{"pro":{"remaining_fraction":0.42}}}"#
    )
    .to_string()];
    write_fake_executor(&bin_dir, "agy", &emit, 0);

    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let original_key = std::env::var_os("GEMINI_API_KEY");
    let mut paths = vec![bin_dir];
    paths.extend(std::env::split_paths(&original_path));
    std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
    std::env::set_var("GEMINI_API_KEY", "mock-gemini-key");

    let args = dispatch_args(tmp.path(), write_routing(tmp.path(), "agy"));
    let outcome: DispatchOutcome = run_dispatch(&args, "quota probe");

    std::env::set_var("PATH", original_path);
    match original_key {
        Some(key) => std::env::set_var("GEMINI_API_KEY", key),
        None => std::env::remove_var("GEMINI_API_KEY"),
    }

    let DispatchQuota::Collected { observations } = &outcome.quota else {
        panic!("return quota must be collected, got {:?}", outcome.quota);
    };
    let passive = observations
        .iter()
        .find(|observation| matches!(observation.acquisition, Acquisition::Passive))
        .expect("the return carries a passive quota observation");
    match &passive.utilization {
        Availability::Reported { value } => assert_eq!(value.value().to_string(), "0.42"),
        other => panic!("utilization not reported: {other:?}"),
    }
    eprintln!(
        "quota_dispatch run_dispatch exit={} terminal={:?} quota={:?}",
        outcome.exit_code, outcome.terminal_state, outcome.quota
    );
}

#[test]
fn run_dispatch_degraded_outcome_reads_no_quota() {
    let _lock = ENV_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let tmp = TempDir::new().unwrap();
    init_git_repo(tmp.path());
    let args = dispatch_args(
        tmp.path(),
        write_routing(tmp.path(), "gal-quota-missing-executor"),
    );

    let outcome = run_dispatch(&args, "quota probe");

    assert_eq!(outcome.exit_code, 2, "degraded exit code");
    assert!(outcome.terminal_state.is_none(), "degraded terminal state");
    assert!(
        matches!(outcome.quota, DispatchQuota::NotAttempted),
        "degraded return launched no process: {:?}",
        outcome.quota
    );
    eprintln!(
        "quota_dispatch degraded exit={} reason={:?} quota={:?}",
        outcome.exit_code, outcome.reason, outcome.quota
    );
}
