use dispatch::dispatch::{
    spawn_executor_with_context, spawn_generic_executor, AttemptContext, EvidenceContract,
    InvocationKind, SpawnConfig, TerminalState,
};
use tempfile::TempDir;

#[cfg(windows)]
fn shell() -> (&'static str, Vec<String>, &'static str) {
    ("cmd", vec!["/C".into(), "echo generic".into()], "generic")
}

#[cfg(not(windows))]
fn shell() -> (&'static str, Vec<String>, &'static str) {
    ("sh", vec!["-c".into(), "printf generic".into()], "generic")
}

#[cfg(windows)]
fn powershell(temp: &TempDir, id: &str, script: &str, timeout: u64) -> SpawnConfig {
    SpawnConfig::generic(
        "powershell.exe",
        vec!["-NoProfile".into(), "-Command".into(), script.into()],
        "",
        temp.path(),
        timeout,
        "test-model",
        id,
        temp.path().join("invocations").join(id),
    )
}

#[test]
fn generic_invocation_uses_its_own_identity_and_journal() {
    let temp = TempDir::new().unwrap();
    let log_dir = temp.path().join("invocations").join("invoke-1");
    let (executor, args, _) = shell();
    let cfg = SpawnConfig::generic(
        executor,
        args,
        "",
        temp.path(),
        10,
        "test-model",
        "invoke-1",
        log_dir.clone(),
    );
    let context = AttemptContext::generic("invoke-1", Some("route-abc"));
    assert!(matches!(context.invocation, InvocationKind::Generic { .. }));

    let outcome = spawn_generic_executor(&cfg, &context).unwrap();
    assert_eq!(outcome.result.terminal_state.as_str(), "completed");
    assert_eq!(outcome.journal_path, log_dir.join("terminal.json"));
    assert!(std::fs::read_to_string(&outcome.journal_path)
        .unwrap()
        .contains("route-abc"));
    assert_eq!(
        String::from_utf8_lossy(&outcome.stdout.tail).trim(),
        "generic"
    );
}

#[test]
fn generic_config_does_not_request_a_receipt_or_pipeline_phase() {
    let temp = TempDir::new().unwrap();
    let (executor, args, _) = shell();
    let cfg = SpawnConfig::generic(
        executor,
        args,
        "",
        temp.path(),
        10,
        "test-model",
        "invoke-2",
        temp.path().join("invoke-2"),
    );
    assert_eq!(cfg.task_id, "invoke-2");
    assert_eq!(cfg.phase, "generic");
    assert!(cfg.receipt_path.is_none());
}

#[cfg(unix)]
fn generic_with_script(temp: &TempDir, id: &str, script: &str, timeout: u64) -> SpawnConfig {
    SpawnConfig::generic(
        "sh",
        vec!["-c".into(), script.into()],
        "",
        temp.path(),
        timeout,
        "test-model",
        id,
        temp.path().join("invocations").join(id),
    )
}

#[cfg(unix)]
#[test]
fn generic_summaries_support_empty_and_non_utf8_output() {
    let temp = TempDir::new().unwrap();
    let cfg = generic_with_script(&temp, "empty", "exit 0", 5);
    let outcome = spawn_generic_executor(&cfg, &AttemptContext::generic("empty", None)).unwrap();
    assert_eq!(outcome.stdout.stored_bytes, 0);
    assert_eq!(
        outcome.stdout.stored_sha256,
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );

    let cfg = generic_with_script(&temp, "binary", "printf '\\377\\376'", 5);
    let outcome = spawn_generic_executor(&cfg, &AttemptContext::generic("binary", None)).unwrap();
    let normalized = [0xef, 0xbf, 0xbd, 0xef, 0xbf, 0xbd];
    assert_eq!(outcome.stdout.stored_bytes, normalized.len() as u64);
    assert_eq!(
        outcome.stdout.stored_sha256,
        "52793f8dc1d85e409f8c88be99d8b31d58f676246340150f406289e04a11151e"
    );
    assert_eq!(outcome.stdout.tail, normalized);
    assert_eq!(std::fs::read(&outcome.stdout.log_path).unwrap(), normalized);
}

#[cfg(unix)]
#[test]
fn generic_streams_are_capped_and_keep_the_stored_prefix_hash_and_tail() {
    let temp = TempDir::new().unwrap();
    let script =
        "head -c 33554432 /dev/zero | tr '\\000' x; head -c 33554432 /dev/zero | tr '\\000' y >&2";
    let cfg = generic_with_script(&temp, "large", script, 30);
    let outcome = spawn_generic_executor(&cfg, &AttemptContext::generic("large", None)).unwrap();
    for (summary, byte) in [(&outcome.stdout, b'x'), (&outcome.stderr, b'y')] {
        assert_eq!(summary.stored_bytes, 16 * 1024 * 1024);
        assert_eq!(summary.discarded_bytes, 16 * 1024 * 1024);
        assert!(summary.truncated);
        assert_eq!(summary.tail.len(), 1024 * 1024);
        assert!(summary.tail.iter().all(|value| *value == byte));
        assert!(std::fs::read(&summary.log_path)
            .unwrap()
            .ends_with(b"[gal: truncated, 16777216 bytes discarded]\n"));
    }
}

#[cfg(unix)]
#[test]
fn generic_stream_persistence_failure_rewrites_bounded_journal() {
    let temp = TempDir::new().unwrap();
    let log_dir = temp.path().join("invocations").join("stream-fail");
    std::fs::create_dir_all(log_dir.join("stdout.log")).unwrap();
    let cfg = SpawnConfig::generic(
        "sh",
        vec!["-c".into(), "printf output".into()],
        "",
        temp.path(),
        5,
        "test-model",
        "stream-fail",
        log_dir.clone(),
    );
    let outcome = spawn_generic_executor(
        &cfg,
        &AttemptContext::generic("stream-fail", Some("route-stream")),
    )
    .unwrap();
    assert!(outcome.evidence_write_failed);
    let journal = std::fs::read_to_string(outcome.journal_path).unwrap();
    assert!(journal.len() <= 4096);
    assert!(journal.contains("evidence-write-failed"));
    assert!(journal.contains("route-stream"));
}

#[cfg(unix)]
#[test]
fn generic_total_journal_failure_returns_bounded_emergency_stderr() {
    let temp = TempDir::new().unwrap();
    let log_dir = temp.path().join("invocations").join("journal-fail");
    std::fs::create_dir_all(&log_dir).unwrap();
    let journal = log_dir.join("terminal.json");
    let script = format!(
        "rm -f '{}' && mkdir '{}' && printf output",
        journal.display(),
        journal.display()
    );
    let cfg = SpawnConfig::generic(
        "sh",
        vec!["-c".into(), script],
        "",
        temp.path(),
        5,
        "test-model",
        "journal-fail",
        log_dir,
    );
    let outcome = spawn_generic_executor(
        &cfg,
        &AttemptContext::generic("journal-fail", Some("route-lost")),
    )
    .unwrap();
    let emergency = outcome.emergency_stderr.unwrap();
    assert!(emergency.len() <= 8192);
    assert!(emergency.contains("evidence-persistence-lost"));
    assert!(emergency.contains("identity: journal-fail"));
    assert!(emergency.contains("route_digest: route-lost"));
    assert!(emergency.contains("provider_started: true"));
    assert!(emergency.contains("one_shot_consumed: true"));
}

#[cfg(unix)]
#[test]
fn generic_timeout_returns_terminal_evidence_after_process_cleanup() {
    let temp = TempDir::new().unwrap();
    let cfg = generic_with_script(&temp, "timeout", "sleep 30", 1);
    let outcome = spawn_generic_executor(&cfg, &AttemptContext::generic("timeout", None)).unwrap();
    assert!(matches!(
        outcome.result.terminal_state,
        TerminalState::Timeout | TerminalState::TimeoutNoOutput | TerminalState::TimeoutMidrun
    ));
    assert!(outcome.journal_path.exists());
    assert_eq!(outcome.stdout.stored_bytes, 0);
}

#[cfg(unix)]
#[test]
fn generic_phase_skips_pipeline_worktree_behavior_and_pipeline_keeps_full_output() {
    let temp = TempDir::new().unwrap();
    let workdir = temp.path().join("repo");
    std::fs::create_dir_all(&workdir).unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&workdir)
        .status()
        .unwrap()
        .success());
    std::fs::write(workdir.join("dirty.txt"), "dirty").unwrap();
    let cfg = SpawnConfig::generic(
        "sh",
        vec!["-c".into(), "printf generic".into()],
        "",
        &workdir,
        5,
        "test-model",
        "no-pipeline",
        temp.path().join("invocations").join("no-pipeline"),
    );
    let result =
        spawn_executor_with_context(&cfg, Some(&AttemptContext::generic("no-pipeline", None)))
            .unwrap()
            .0;
    let log = std::fs::read_to_string(result.log_path).unwrap();
    assert!(!log.contains("workdir_dirty_at_dispatch:"));
    assert!(!log.contains("worktree containment evidence:"));

    let pipeline = SpawnConfig {
        executor: "sh".into(),
        executor_args: vec![
            "-c".into(),
            "head -c 20971520 /dev/zero | tr '\\000' p".into(),
        ],
        spec: String::new(),
        workdir: temp.path().into(),
        timeout_secs: 30,
        task_id: "T-full-output".into(),
        phase: "verify".into(),
        actual_model: "test-model".into(),
        log_dir: temp.path().join("pipeline-full"),
        receipt_path: None,
        contract_provenance: None,
    };
    let (result, _) =
        spawn_executor_with_context(&pipeline, Some(&AttemptContext::default())).unwrap();
    assert_eq!(result.stdout.len(), 20 * 1024 * 1024);
    assert!(result.stdout.bytes().all(|byte| byte == b'p'));
}

#[test]
fn generic_route_digest_header_precedes_terminal_state_and_pipeline_header_stays_legacy() {
    let temp = TempDir::new().unwrap();
    let cfg = SpawnConfig::generic(
        "sh",
        vec!["-c".into(), "printf generic".into()],
        "",
        temp.path(),
        5,
        "test-model",
        "route-header",
        temp.path().join("generic"),
    );
    let context = AttemptContext::generic("route-header", Some("sha256:abc"));
    let outcome = spawn_executor_with_context(&cfg, Some(&context)).unwrap().0;
    let log = std::fs::read_to_string(outcome.log_path).unwrap();
    assert!(
        log.find("route_digest:    sha256:abc").unwrap() < log.find("terminal_state:").unwrap()
    );

    let (executor, _, _) = shell();
    #[cfg(windows)]
    let executor_args = vec!["/C".into(), "echo pipeline".into()];
    #[cfg(not(windows))]
    let executor_args = vec!["-c".into(), "printf pipeline".into()];
    let pipeline = SpawnConfig {
        executor: executor.into(),
        executor_args,
        spec: String::new(),
        workdir: temp.path().into(),
        timeout_secs: 5,
        task_id: "T-legacy".into(),
        phase: "verify".into(),
        actual_model: "test-model".into(),
        log_dir: temp.path().join("pipeline"),
        receipt_path: None,
        contract_provenance: None,
    };
    let (result, _) =
        spawn_executor_with_context(&pipeline, Some(&AttemptContext::default())).unwrap();
    let legacy = std::fs::read_to_string(result.log_path).unwrap();
    assert!(!legacy.contains("route_digest:"));
    assert!(legacy.starts_with("GAL-DISPATCH-LOG v1\n"));
    assert_eq!(result.stdout.trim_end(), "pipeline");
    assert_eq!(
        AttemptContext::default().invocation,
        InvocationKind::Pipeline
    );
    assert_eq!(
        AttemptContext::default().evidence_contract,
        EvidenceContract::V1
    );
}

#[cfg(windows)]
#[test]
fn windows_generic_covers_binary_large_streams_failures_timeout_and_pipeline_output() {
    let temp = TempDir::new().unwrap();
    let empty = powershell(&temp, "empty", "$null", 5);
    let outcome = spawn_generic_executor(&empty, &AttemptContext::generic("empty", None)).unwrap();
    assert_eq!(outcome.stdout.stored_bytes, 0);

    let binary = powershell(
        &temp,
        "binary",
        "[Console]::OpenStandardOutput().Write([byte[]](255,254),0,2)",
        5,
    );
    let outcome =
        spawn_generic_executor(&binary, &AttemptContext::generic("binary", None)).unwrap();
    assert_eq!(outcome.stdout.stored_bytes, 6);
    assert_eq!(
        outcome.stdout.stored_sha256,
        "52793f8dc1d85e409f8c88be99d8b31d58f676246340150f406289e04a11151e"
    );

    let large_script = "$b=[Text.Encoding]::ASCII.GetBytes('x'*1048576); $o=[Console]::OpenStandardOutput(); $e=[Console]::OpenStandardError(); for($i=0;$i -lt 32;$i++){$o.Write($b,0,$b.Length);$e.Write($b,0,$b.Length)}";
    let large = powershell(&temp, "large", large_script, 30);
    let outcome = spawn_generic_executor(&large, &AttemptContext::generic("large", None)).unwrap();
    let diagnostics = outcome.result.usage.stdout_source.as_ref().unwrap();
    assert_eq!(diagnostics.completeness, "partial");
    assert!(diagnostics
        .reasons
        .iter()
        .any(|reason| reason == "event-oversized"));
    assert!(diagnostics.dropped_range.is_some());
    for summary in [&outcome.stdout, &outcome.stderr] {
        assert_eq!(summary.stored_bytes, 16 * 1024 * 1024);
        assert_eq!(summary.discarded_bytes, 16 * 1024 * 1024);
        assert!(summary.truncated);
        assert_eq!(summary.tail.len(), 1024 * 1024);
        assert_eq!(
            summary.stored_sha256,
            "a06c26cbac8b80704f420222dae5658b88ff2da96702d12ef7a4223e9361f7c1"
        );
        assert!(std::fs::read(&summary.log_path)
            .unwrap()
            .ends_with(b"[gal: truncated, 16777216 bytes discarded]\n"));
    }

    let stream_log = temp.path().join("invocations").join("stream-fail");
    std::fs::create_dir_all(stream_log.join("stdout.log")).unwrap();
    let cfg = SpawnConfig::generic(
        "powershell.exe",
        vec![
            "-NoProfile".into(),
            "-Command".into(),
            "Write-Output output".into(),
        ],
        "",
        temp.path(),
        5,
        "test-model",
        "stream-fail",
        stream_log,
    );
    let outcome =
        spawn_generic_executor(&cfg, &AttemptContext::generic("stream-fail", None)).unwrap();
    assert!(outcome.evidence_write_failed);
    assert!(std::fs::read_to_string(outcome.journal_path)
        .unwrap()
        .contains("evidence-write-failed"));

    let journal_log = temp.path().join("invocations").join("journal-fail");
    std::fs::create_dir_all(&journal_log).unwrap();
    let journal = journal_log.join("terminal.json");
    let script = format!(
        "Remove-Item -Force '{}' ; New-Item -ItemType Directory -Path '{}' | Out-Null; [Console]::Out.Write('output')",
        journal.display(),
        journal.display()
    );
    let cfg = SpawnConfig::generic(
        "powershell.exe",
        vec!["-NoProfile".into(), "-Command".into(), script],
        "",
        temp.path(),
        5,
        "test-model",
        "journal-fail",
        journal_log,
    );
    let outcome = spawn_generic_executor(
        &cfg,
        &AttemptContext::generic("journal-fail", Some("route-lost")),
    )
    .unwrap();
    let emergency = outcome.emergency_stderr.unwrap();
    assert!(emergency.len() <= 8192);
    assert!(emergency.contains("evidence-persistence-lost"));
    assert!(emergency.contains("identity: journal-fail"));
    assert!(emergency.contains("route_digest: route-lost"));
    assert!(emergency.contains("provider_started: true"));
    assert!(emergency.contains("one_shot_consumed: true"));

    let pid_path = temp.path().join("timed-pid.txt");
    let timed_script = format!(
        "Set-Content '{}' $PID; Start-Sleep -Seconds 30",
        pid_path.display()
    );
    let timed = powershell(&temp, "timeout", &timed_script, 1);
    let outcome =
        spawn_generic_executor(&timed, &AttemptContext::generic("timeout", None)).unwrap();
    assert!(matches!(
        outcome.result.terminal_state,
        TerminalState::Timeout | TerminalState::TimeoutNoOutput | TerminalState::TimeoutMidrun
    ));
    let pid = std::fs::read_to_string(pid_path)
        .unwrap()
        .trim()
        .parse::<u32>()
        .unwrap();
    let cleanup = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &format!("Get-Process -Id {pid}")])
        .status()
        .unwrap();
    assert!(!cleanup.success(), "timed process {pid} remained alive");

    let repo = temp.path().join("dirty-repo");
    std::fs::create_dir_all(&repo).unwrap();
    assert!(std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(&repo)
        .status()
        .unwrap()
        .success());
    std::fs::write(repo.join("dirty.txt"), "dirty").unwrap();
    let generic = SpawnConfig::generic(
        "powershell.exe",
        vec![
            "-NoProfile".into(),
            "-Command".into(),
            "[Console]::Out.Write('generic')".into(),
        ],
        "",
        &repo,
        5,
        "test-model",
        "generic-phase",
        temp.path().join("invocations").join("generic-phase"),
    );
    let result = spawn_executor_with_context(
        &generic,
        Some(&AttemptContext::generic("generic-phase", None)),
    )
    .unwrap()
    .0;
    let log = std::fs::read_to_string(result.log_path).unwrap();
    assert!(!log.contains("workdir_dirty_at_dispatch:"));
    assert!(!log.contains("worktree containment evidence:"));

    let pipeline = SpawnConfig {
        executor: "powershell.exe".into(),
        executor_args: vec![
            "-NoProfile".into(),
            "-Command".into(),
            "[Console]::Out.Write('p'*20971520)".into(),
        ],
        spec: String::new(),
        workdir: temp.path().into(),
        timeout_secs: 30,
        task_id: "T-full-output".into(),
        phase: "verify".into(),
        actual_model: "test-model".into(),
        log_dir: temp.path().join("pipeline-full"),
        receipt_path: None,
        contract_provenance: None,
    };
    let (result, _) =
        spawn_executor_with_context(&pipeline, Some(&AttemptContext::default())).unwrap();
    assert_eq!(result.stdout.len(), 20 * 1024 * 1024);
}

#[test]
fn generic_external_cancel_terminates_executor_and_returns_journal() {
    let temp = TempDir::new().unwrap();
    let pid_path = temp.path().join("cancel-pid.txt");
    #[cfg(windows)]
    let cfg = {
        let script = format!(
            "Set-Content '{}' $PID; Start-Sleep -Seconds 30",
            pid_path.display()
        );
        powershell(&temp, "cancel", &script, 30)
    };
    #[cfg(unix)]
    let cfg = generic_with_script(
        &temp,
        "cancel",
        &format!("echo $$ > '{}' ; exec sleep 30", pid_path.display()),
        30,
    );

    let context = AttemptContext::generic("cancel", None);
    let worker = std::thread::spawn(move || spawn_generic_executor(&cfg, &context));
    // A cold PowerShell start on a loaded CI runner can take several seconds.
    // Wait for a parsable pid, well inside the 30 second executor timeout.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    let pid = loop {
        let published = std::fs::read_to_string(&pid_path)
            .ok()
            .and_then(|text| text.trim().parse::<u32>().ok());
        if let Some(pid) = published {
            break pid;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "executor did not publish its pid"
        );
        std::thread::sleep(std::time::Duration::from_millis(25));
    };
    #[cfg(windows)]
    let canceled = std::process::Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status()
        .unwrap();
    #[cfg(unix)]
    let canceled = std::process::Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()
        .unwrap();
    assert!(canceled.success(), "failed to cancel executor {pid}");

    let outcome = worker.join().unwrap().unwrap();
    assert_ne!(outcome.result.terminal_state, TerminalState::Completed);
    assert!(outcome.journal_path.exists());
    #[cfg(windows)]
    let still_running = std::process::Command::new("powershell.exe")
        .args(["-NoProfile", "-Command", &format!("Get-Process -Id {pid}")])
        .status()
        .unwrap()
        .success();
    #[cfg(unix)]
    let still_running = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .unwrap()
        .success();
    assert!(!still_running, "canceled executor {pid} remained alive");
}
