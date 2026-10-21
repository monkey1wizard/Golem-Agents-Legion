use dispatch::host_attempt_state::{HostAttemptState, HostAttemptStateError};
use sha2::Digest;
use std::path::PathBuf;

fn open_args(
    root: &std::path::Path,
    state: HostAttemptState,
) -> (dispatch::cli::DispatchArgs, String) {
    let spec = "host spec".to_string();
    let args = dispatch::cli::DispatchArgs {
        phase: dispatch::stage::Phase::Implement,
        task: state.task.clone(),
        workdir: root.to_path_buf(),
        routing_path: Some(root.join("absent-routing.json")),
        pinned_mode: Some(dispatch::stage::DispatchMode::HostSubagent),
        host_attempt: Some(dispatch::cli::HostAttemptOpenOptions {
            state: Some(state),
            prompt_snapshot: Some(b"canonical prompt bytes".to_vec()),
            task_scope_dir: Some(root.join(".dev/pipeline/task-a")),
            ..Default::default()
        }),
        ..Default::default()
    };
    (args, spec)
}

fn local_state(root: &std::path::Path) -> HostAttemptState {
    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .status()
        .unwrap();
    assert!(init.success());
    let prompt = root.join("plan.prompt.md");
    std::fs::write(&prompt, "canonical prompt bytes").unwrap();
    std::process::Command::new("git")
        .args(["add", "plan.prompt.md"])
        .current_dir(root)
        .status()
        .unwrap();
    std::process::Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "baseline",
        ])
        .current_dir(root)
        .status()
        .unwrap();
    let mut state = state();
    state.worktree = root.to_path_buf();
    state.prompt = prompt;
    state.task = "task-a".into();
    state.phase = "implement".into();
    state.spec_path = root.join(".dev/pipeline/task-a/spec.md");
    state.spec_sha256 = format!("{:x}", sha2::Sha256::digest(b"host spec"));
    state.receipt_path = Some(root.join(".dev/pipeline/open.receipt.md"));
    state.log_path = root.join(".dev/pipeline/open.log");
    state.snapshot_path = root.join(".dev/pipeline/open.snapshot");
    state.prompt_sha256 = Some(format!(
        "{:x}",
        sha2::Sha256::digest(b"canonical prompt bytes")
    ));
    state
}

#[test]
fn host_subagent_tier_policy_is_bound_before_open_publication() {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host_session/claude");
    let fixture_text = std::fs::read_to_string(fixture.join("agent-valid.jsonl")).unwrap();
    state.started_at =
        serde_json::from_str::<serde_json::Value>(fixture_text.lines().next().unwrap()).unwrap()
            ["timestamp"]
            .as_str()
            .unwrap()
            .to_owned();
    let manifest_path = state.manifest_path().unwrap();
    let (args, spec) = open_args(root.path(), state.clone());
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    assert_eq!(outcome.exit_code, 0, "{:?}", outcome.reason);
    assert!(state.snapshot_path.is_file());
    assert!(state.spec_path.is_file());
    assert!(manifest_path.is_file());
    let log = std::fs::read_to_string(&state.log_path).unwrap();
    assert!(log.contains("executor:        host-subagent\nhost: claude\n"));
    assert!(log.contains("terminal_state:  started"));
    assert!(log.contains("tier:            standard"));
    assert!(log.contains("tier_source:     standard"));
    assert!(log.contains("model_tier_applied: yes"));
    let manifest = HostAttemptState::load(&manifest_path, &state.attempt_id).unwrap();
    assert_eq!(manifest.tier.as_deref(), Some("standard"));
    assert_eq!(manifest.model, "sonnet");
    assert_eq!(manifest.effort, None);
    let leases = root.path().join(dispatch::dispatch::LEASE_ROOT);
    assert!(std::fs::read_dir(leases.join("prompt-writers"))
        .unwrap()
        .next()
        .is_some());
    assert!(std::fs::read_dir(leases).unwrap().any(|entry| entry
        .unwrap()
        .path()
        .extension()
        .is_some_and(|ext| ext == "lock")));
}

#[test]
fn host_attempt_open_failure_does_not_publish_manifest_or_spawn() {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    let blocked_parent = root.path().join("blocked");
    std::fs::write(&blocked_parent, "file").unwrap();
    state.snapshot_path = blocked_parent.join("snapshot");
    let (args, spec) = open_args(root.path(), state.clone());
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    assert_eq!(outcome.exit_code, 2);
    assert_eq!(
        outcome.reason.as_deref(),
        Some("host-attempt-open-publication-failed")
    );
    assert!(!state.manifest_path().unwrap().exists());
    assert!(!state.log_path.exists());
}

#[test]
fn host_attempt_open_injected_publication_boundaries_fail_closed() {
    for boundary in ["snapshot", "spec", "manifest", "started"] {
        let root = tempfile::tempdir().unwrap();
        let mut state = local_state(root.path());
        let blocker = root.path().join("blocker");
        std::fs::write(&blocker, "file").unwrap();
        match boundary {
            "snapshot" => state.snapshot_path = blocker.join("snapshot"),
            "spec" => state.spec_path = blocker.join("spec"),
            "manifest" => {
                let manifest = state.manifest_path().unwrap();
                std::fs::create_dir_all(manifest.parent().unwrap()).unwrap();
                std::fs::write(manifest, "already exists").unwrap();
            }
            "started" => state.log_path = blocker.join("started.log"),
            _ => unreachable!(),
        }
        let (args, spec) = open_args(root.path(), state);
        let outcome = dispatch::run::run_dispatch(&args, &spec);
        assert_eq!(outcome.exit_code, 2, "boundary={boundary}");
        assert_eq!(
            outcome.reason.as_deref(),
            Some("host-attempt-open-publication-failed"),
            "boundary={boundary}"
        );
    }
}

fn state() -> HostAttemptState {
    HostAttemptState {
        attempt_id: "attempt_01".into(),
        worktree: PathBuf::from("C:/work"),
        prompt: PathBuf::from("C:/work/prompt.md"),
        task: "task-a".into(),
        phase: "implement".into(),
        profile: "default".into(),
        run_id: "run-01".into(),
        host: "claude".into(),
        mode: "host-subagent".into(),
        spec_path: PathBuf::from("C:/work/spec.md"),
        spec_sha256: "a".repeat(64),
        receipt_path: None,
        log_path: PathBuf::from("C:/work/log"),
        snapshot_path: PathBuf::from("C:/work/snapshot"),
        started_at: "epoch+1s".into(),
        model: "default".into(),
        effort: None,
        tier: Some("standard".into()),
        tier_source: Some("profile".into()),
        override_reason: Some("fixture".into()),
        policy_applied: true,
        prerequisite_refs: vec![],
        failure_history: vec![],
        observed_model: None,
        observed_effort: None,
        prompt_sha256: Some("b".repeat(64)),
        receipt_sha256: Some("c".repeat(64)),
        log_sha256: Some("d".repeat(64)),
        snapshot_sha256: Some("e".repeat(64)),
        worktree_status_sha256: Some("f".repeat(64)),
        worktree_diff_sha256: Some("0".repeat(64)),
    }
}

fn host_fixture(root: &std::path::Path, state: &mut HostAttemptState) -> tempfile::TempDir {
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host_session/claude");
    let fixture_text = std::fs::read_to_string(fixture.join("agent-valid.jsonl")).unwrap();
    state.started_at =
        serde_json::from_str::<serde_json::Value>(fixture_text.lines().next().unwrap()).unwrap()
            ["timestamp"]
            .as_str()
            .unwrap()
            .to_owned();
    let home = tempfile::tempdir().unwrap();
    let agents = home.path().join(".claude/projects/hash/instance/subagents");
    std::fs::create_dir_all(&agents).unwrap();
    let cwd = root.to_string_lossy().replace('\\', "/");
    let mut text = fixture_text
        .replace("/workspace/project", &cwd)
        .replace("attempt-id-123", &state.attempt_id);
    text = text.replace(
        "{\"type\":\"assistant\",\"message\":{\"content\"",
        "{\"type\":\"assistant\",\"message\":{\"model\":\"claude-sonnet-4-5-20250929\",\"content\"",
    );
    assert!(text.contains("claude-sonnet-4-5-20250929"));
    std::fs::write(agents.join("agent-a1.jsonl"), text).unwrap();
    std::fs::copy(
        fixture.join("agent-valid.meta.json"),
        agents.join("agent-a1.meta.json"),
    )
    .unwrap();
    home
}

#[test]
fn host_attempt_manifest_round_trips_and_binds_identity() {
    let original = state();
    let bytes = original.encode_immutable().unwrap();
    let decoded = HostAttemptState::decode_immutable(&bytes).unwrap();
    assert_eq!(decoded, original);
    assert!(decoded
        .matches_identity(
            PathBuf::from("C:/work").as_path(),
            "task-a",
            "implement",
            "claude"
        )
        .is_ok());
    assert_eq!(
        decoded.matches_identity(
            PathBuf::from("C:/elsewhere").as_path(),
            "task-a",
            "implement",
            "claude"
        ),
        Err(HostAttemptStateError::IdentityMismatch)
    );
}

#[test]
fn host_attempt_manifest_rejects_invalid_attempt_path_identity() {
    let mut manifest = state();
    manifest.attempt_id = "../attempt".into();
    assert_eq!(manifest.validate(), Err(HostAttemptStateError::Malformed));
}

#[test]
fn host_attempt_manifest_write_once_validates_digest_and_custom_receipt() {
    let root = tempfile::tempdir().unwrap();
    let mut manifest = state();
    manifest.worktree = root.path().to_path_buf();
    manifest.prompt = root.path().join("plan").join("plan.prompt.md");
    manifest.receipt_path = Some(root.path().join("custom").join("receipt.md"));
    let path = manifest.write_once().unwrap();
    assert_eq!(
        HostAttemptState::load(&path, "attempt_01").unwrap(),
        manifest
    );
    assert_eq!(
        manifest.write_once(),
        Err(HostAttemptStateError::AlreadyExists)
    );
    let mut bytes = std::fs::read(&path).unwrap();
    bytes[10] ^= 1;
    std::fs::write(&path, bytes).unwrap();
    assert!(matches!(
        HostAttemptState::load(&path, "attempt_01"),
        Err(HostAttemptStateError::DigestMismatch) | Err(HostAttemptStateError::Malformed)
    ));
}

#[test]
fn host_attempt_close_claim_is_exclusive_and_terminal_states_round_trip() {
    let root = tempfile::tempdir().unwrap();
    let mut manifest = state();
    manifest.worktree = root.path().to_path_buf();
    manifest.prompt = root.path().join("plan").join("plan.prompt.md");
    manifest.write_once().unwrap();
    manifest.claim_close().unwrap();
    assert_eq!(
        manifest.claim_close(),
        Err(HostAttemptStateError::CloseClaimed)
    );
    for state in [
        dispatch::host_attempt_state::HostTerminalState::Completed,
        dispatch::host_attempt_state::HostTerminalState::HostSessionInvalid,
        dispatch::host_attempt_state::HostTerminalState::NoWriteback,
        dispatch::host_attempt_state::HostTerminalState::LeaseOwnerMismatch,
        dispatch::host_attempt_state::HostTerminalState::LeaseMissing,
        dispatch::host_attempt_state::HostTerminalState::IdentityMismatch,
        dispatch::host_attempt_state::HostTerminalState::TerminalReplay,
        dispatch::host_attempt_state::HostTerminalState::PlacementFailed,
        dispatch::host_attempt_state::HostTerminalState::ModelTierMismatch,
    ] {
        assert_eq!(
            dispatch::host_attempt_state::HostTerminalState::parse(state.as_str()),
            Some(state)
        );
    }
}

#[test]
fn host_attempt_workdir_unchanged_requires_all_matching_inputs() {
    use dispatch::host_attempt_state::workdir_unchanged;
    assert!(workdir_unchanged(
        Some("a"),
        Some("a"),
        Some("b"),
        Some("b")
    ));
    assert!(!workdir_unchanged(Some("a"), Some("a"), None, Some("b")));
    assert!(!workdir_unchanged(
        Some("a"),
        Some("x"),
        Some("b"),
        Some("b")
    ));
}

#[test]
fn host_attempt_close_publishes_completed_and_releases_leases() {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host_session/claude");
    let fixture_text = std::fs::read_to_string(fixture.join("agent-valid.jsonl")).unwrap();
    state.started_at =
        serde_json::from_str::<serde_json::Value>(fixture_text.lines().next().unwrap()).unwrap()
            ["timestamp"]
            .as_str()
            .unwrap()
            .to_owned();
    let (args, spec) = open_args(root.path(), state.clone());
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    std::fs::write(state.receipt_path.as_ref().unwrap(), b"receipt body").unwrap();
    std::fs::write(root.path().join("work-product.txt"), b"implementation").unwrap();

    let home = tempfile::tempdir().unwrap();
    let agents = home.path().join(".claude/projects/hash/instance/subagents");
    std::fs::create_dir_all(&agents).unwrap();
    let transcript = agents.join("agent-a1.jsonl");
    let cwd = root.path().to_string_lossy().replace('\\', "/");
    let text = std::fs::read_to_string(fixture.join("agent-valid.jsonl"))
        .unwrap()
        .replace("/workspace/project", &cwd)
        .replace("attempt-id-123", &state.attempt_id);
    let text = text.replace(
        "{\"type\":\"assistant\",\"message\":{\"content\"",
        "{\"type\":\"assistant\",\"message\":{\"model\":\"claude-sonnet-4-5-20250929\",\"content\"",
    );
    std::fs::write(&transcript, &text).unwrap();
    std::fs::copy(
        fixture.join("agent-valid.meta.json"),
        agents.join("agent-a1.meta.json"),
    )
    .unwrap();
    let mut received = Vec::new();
    let mut placement = |bytes: &[u8]| {
        received.push(bytes.to_vec());
        Ok(())
    };
    let result = dispatch::run::close_host_attempt(
        root.path(),
        &root.path().join(".dev/pipeline/task-a"),
        "task-a",
        &state.attempt_id,
        "task-a",
        "implement",
        "claude",
        "claude:a1",
        home.path(),
        &[],
        &mut placement,
        &dispatch::run::HostAttemptCloseHooks::default(),
    );
    assert_eq!(result.unwrap(), "a1");
    assert_eq!(received, vec![b"receipt body".to_vec()]);
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  completed"));
    let terminal_log = std::fs::read_to_string(&state.log_path).unwrap();
    assert!(terminal_log.contains("actual_model:    claude-sonnet-4-5-20250929"));
    assert!(terminal_log.contains("model_tier_applied: yes"));
    assert!(!root
        .path()
        .join(dispatch::dispatch::LEASE_ROOT)
        .join("prompt-writers")
        .read_dir()
        .unwrap()
        .next()
        .is_some());
    assert!(!root
        .path()
        .join(dispatch::dispatch::LEASE_ROOT)
        .read_dir()
        .unwrap()
        .any(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_some_and(|ext| ext == "lock")));
}

#[test]
fn host_attempt_close_validation_failures_preserve_log_and_skip_placement() {
    for failure in ["task", "log", "prompt", "unchanged"] {
        let root = tempfile::tempdir().unwrap();
        let mut state = local_state(root.path());
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/host_session/claude");
        let fixture_text = std::fs::read_to_string(fixture.join("agent-valid.jsonl")).unwrap();
        state.started_at =
            serde_json::from_str::<serde_json::Value>(fixture_text.lines().next().unwrap())
                .unwrap()["timestamp"]
                .as_str()
                .unwrap()
                .to_owned();
        let (args, spec) = open_args(root.path(), state.clone());
        assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
        std::fs::write(state.receipt_path.as_ref().unwrap(), b"receipt").unwrap();
        if failure != "unchanged" {
            std::fs::write(root.path().join("work-product.txt"), b"changed").unwrap();
        }
        if failure == "prompt" {
            std::fs::write(&state.prompt, b"mutated prompt").unwrap();
        }
        if failure == "log" {
            std::fs::write(&state.log_path, b"terminal_state:  completed\n").unwrap();
        }
        let home = tempfile::tempdir().unwrap();
        let agents = home.path().join(".claude/projects/hash/instance/subagents");
        std::fs::create_dir_all(&agents).unwrap();
        let cwd = root.path().to_string_lossy().replace('\\', "/");
        let text = fixture_text
            .replace("/workspace/project", &cwd)
            .replace("attempt-id-123", &state.attempt_id);
        std::fs::write(agents.join("agent-a1.jsonl"), text).unwrap();
        std::fs::copy(
            fixture.join("agent-valid.meta.json"),
            agents.join("agent-a1.meta.json"),
        )
        .unwrap();
        let log_before = std::fs::read(&state.log_path).unwrap();
        let mut called = false;
        let mut placement = |_: &[u8]| {
            called = true;
            Ok(())
        };
        let task = if failure == "task" {
            "task-b"
        } else {
            "task-a"
        };
        let result = dispatch::run::close_host_attempt(
            root.path(),
            &root.path().join(".dev/pipeline/task-a"),
            "task-a",
            &state.attempt_id,
            task,
            "implement",
            "claude",
            "claude:a1",
            home.path(),
            &[],
            &mut placement,
            &dispatch::run::HostAttemptCloseHooks::default(),
        );
        assert!(result.is_err(), "failure={failure}");
        assert!(!called, "failure={failure}");
        assert_eq!(
            std::fs::read(&state.log_path).unwrap(),
            log_before,
            "failure={failure}"
        );
    }
}

#[test]
fn host_attempt_close_pipeline_only_change_is_no_writeback() {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    let home = host_fixture(root.path(), &mut state);
    let (args, spec) = open_args(root.path(), state.clone());
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    std::fs::write(state.receipt_path.as_ref().unwrap(), b"receipt").unwrap();
    std::fs::write(
        root.path().join(".dev/pipeline/generated.txt"),
        b"pipeline only",
    )
    .unwrap();
    let result = dispatch::run::close_host_attempt(
        root.path(),
        &root.path().join(".dev/pipeline/task-a"),
        "task-a",
        &state.attempt_id,
        "task-a",
        "implement",
        "claude",
        "claude:a1",
        home.path(),
        &[],
        &mut |_| Ok(()),
        &dispatch::run::HostAttemptCloseHooks::default(),
    );
    assert_eq!(
        result,
        Err(dispatch::run::HostAttemptCloseError::NoWriteback)
    );
}

#[test]
fn host_attempt_close_preexisting_dirty_file_is_not_writeback() {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    let home = host_fixture(root.path(), &mut state);
    std::fs::write(root.path().join("preexisting.txt"), b"dirty before open").unwrap();
    let (args, spec) = open_args(root.path(), state.clone());
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    std::fs::write(state.receipt_path.as_ref().unwrap(), b"receipt").unwrap();
    let result = dispatch::run::close_host_attempt(
        root.path(),
        &root.path().join(".dev/pipeline/task-a"),
        "task-a",
        &state.attempt_id,
        "task-a",
        "implement",
        "claude",
        "claude:a1",
        home.path(),
        &[],
        &mut |_| Ok(()),
        &dispatch::run::HostAttemptCloseHooks::default(),
    );
    assert_eq!(
        result,
        Err(dispatch::run::HostAttemptCloseError::NoWriteback)
    );
}

#[test]
fn host_attempt_close_manifest_is_scoped_by_plan_directory() {
    let root = tempfile::tempdir().unwrap();
    let state = local_state(root.path());
    let mut paths = Vec::new();
    for scope in ["plan-one", "plan-two"] {
        let mut attempt = state.clone();
        attempt.attempt_id = format!("attempt_{scope}");
        attempt.log_path = root.path().join(format!("{scope}.log"));
        attempt.snapshot_path = root.path().join(format!("{scope}.snapshot"));
        attempt.spec_path = root.path().join(format!("{scope}.spec"));
        attempt.prompt = root.path().join(format!("{scope}.prompt.md"));
        std::fs::write(&attempt.prompt, b"canonical prompt bytes").unwrap();
        attempt.receipt_path = Some(root.path().join(format!("{scope}.receipt.md")));
        let mut args = open_args(root.path(), attempt.clone()).0;
        args.host_attempt.as_mut().unwrap().task_scope_dir =
            Some(root.path().join(".dev/pipeline").join(scope).join("task-a"));
        let opened = dispatch::run::run_dispatch(&args, "host spec");
        assert_eq!(opened.exit_code, 0, "{:?}", opened.reason);
        paths.push(
            root.path()
                .join(".dev/pipeline")
                .join(scope)
                .join("task-a/host-attempts")
                .join(format!("{}.json", attempt.attempt_id)),
        );
    }
    assert_ne!(paths[0], paths[1]);
    assert!(paths.iter().all(|path| path.is_file()));
    assert!(!root.path().join(".dev/plans").exists());
}

fn host_attempt_close_fixture() -> (tempfile::TempDir, HostAttemptState, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    let home = host_fixture(root.path(), &mut state);
    let (args, spec) = open_args(root.path(), state.clone());
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    std::fs::write(state.receipt_path.as_ref().unwrap(), b"receipt").unwrap();
    std::fs::write(root.path().join("product.txt"), b"changed").unwrap();
    (root, state, home)
}

fn host_attempt_fail_publish(_: &std::path::Path, _: &[u8]) -> Result<(), String> {
    Err("injected publication failure".into())
}

fn host_attempt_fail_cleanup() -> Result<(), String> {
    Err("injected cleanup failure".into())
}

fn host_attempt_close_call(
    root: &std::path::Path,
    state: &HostAttemptState,
    home: &std::path::Path,
    placement: &mut dyn FnMut(&[u8]) -> Result<(), String>,
    hooks: &dispatch::run::HostAttemptCloseHooks,
) -> Result<String, dispatch::run::HostAttemptCloseError> {
    dispatch::run::close_host_attempt(
        root,
        &root.join(".dev/pipeline/task-a"),
        "task-a",
        &state.attempt_id,
        "task-a",
        "implement",
        "claude",
        "claude:a1",
        home,
        &[],
        placement,
        hooks,
    )
}

#[test]
fn host_attempt_close_publication_failure_retains_claim_and_started_log() {
    let (root, state, home) = host_attempt_close_fixture();
    let calls = std::cell::Cell::new(0);
    let mut placement = |_: &[u8]| {
        calls.set(calls.get() + 1);
        Ok(())
    };
    let hooks = dispatch::run::HostAttemptCloseHooks {
        publish_terminal: host_attempt_fail_publish,
        ..Default::default()
    };
    assert_eq!(
        host_attempt_close_call(root.path(), &state, home.path(), &mut placement, &hooks),
        Err(dispatch::run::HostAttemptCloseError::PlacedNotPublished)
    );
    assert_eq!(calls.get(), 1);
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  started"));
    assert!(HostAttemptState::manifest_path_for(
        &root.path().join(".dev/pipeline/task-a"),
        &state.attempt_id
    )
    .unwrap()
    .with_extension("close-claim")
    .is_dir());
    assert_eq!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut placement,
            &Default::default()
        ),
        Err(dispatch::run::HostAttemptCloseError::Claim)
    );
    assert_eq!(calls.get(), 1);
}

#[test]
fn host_attempt_close_concurrent_calls_admit_one_placement() {
    let (root, state, home) = host_attempt_close_fixture();
    let root_path = root.path().to_path_buf();
    let home_path = home.path().to_path_buf();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let mut threads = Vec::new();
    for _ in 0..2 {
        let (root_path, home_path, state, barrier, calls) = (
            root_path.clone(),
            home_path.clone(),
            state.clone(),
            barrier.clone(),
            calls.clone(),
        );
        threads.push(std::thread::spawn(move || {
            barrier.wait();
            let mut place = |_: &[u8]| {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            };
            host_attempt_close_call(
                &root_path,
                &state,
                &home_path,
                &mut place,
                &Default::default(),
            )
        }));
    }
    barrier.wait();
    let results: Vec<_> = threads
        .into_iter()
        .map(|thread| thread.join().unwrap())
        .collect();
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        std::fs::read_to_string(&state.log_path)
            .unwrap()
            .matches("terminal_state:  completed")
            .count(),
        1
    );
}

#[test]
fn host_attempt_close_invalid_owner_missing_lease_and_replay_preserve_log() {
    for failure in ["owner", "lease", "replay"] {
        let (root, state, home) = host_attempt_close_fixture();
        if failure == "owner" {
            let lock = root
                .path()
                .join(dispatch::dispatch::LEASE_ROOT)
                .join("prompt-writers");
            let path = std::fs::read_dir(lock)
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path();
            std::fs::write(path, "owner_token=wrong\n").unwrap();
        } else if failure == "lease" {
            let lock = root
                .path()
                .join(dispatch::dispatch::LEASE_ROOT)
                .join("prompt-writers");
            std::fs::remove_file(
                std::fs::read_dir(lock)
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path(),
            )
            .unwrap();
        } else {
            std::fs::write(&state.log_path, b"terminal_state:  completed\n").unwrap();
        }
        let before = std::fs::read(&state.log_path).unwrap();
        let mut place = |_: &[u8]| Ok(());
        assert!(host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut place,
            &Default::default()
        )
        .is_err());
        assert_eq!(std::fs::read(&state.log_path).unwrap(), before);
    }
}

#[test]
fn host_attempt_close_preplacement_failure_retains_leases_when_receipt_exists() {
    let (root, state, home) = host_attempt_close_fixture();
    let mut place = |_: &[u8]| Ok(());
    let result = dispatch::run::close_host_attempt(
        root.path(),
        &root.path().join(".dev/pipeline/task-a"),
        "task-a",
        &state.attempt_id,
        "task-a",
        "implement",
        "claude",
        "invalid-session",
        home.path(),
        &[],
        &mut place,
        &Default::default(),
    );
    assert_eq!(result, Err(dispatch::run::HostAttemptCloseError::Session));
    assert!(std::fs::read(state.receipt_path.as_ref().unwrap()).is_ok());
    let locks = root.path().join(dispatch::dispatch::LEASE_ROOT);
    assert_eq!(
        std::fs::read_dir(locks.join("prompt-writers"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        std::fs::read_dir(&locks)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "lock"))
            .count(),
        1
    );
    assert!(!root
        .path()
        .join(".dev/pipeline/task-a/host-attempts/attempt_01.close-claim")
        .exists());
    assert_ne!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut place,
            &Default::default()
        ),
        Err(dispatch::run::HostAttemptCloseError::Claim)
    );
}

#[test]
fn host_subagent_tier_model_mismatch_is_terminal_before_placement() {
    let (root, state, home) = host_attempt_close_fixture();
    let transcript = home
        .path()
        .join(".claude/projects/hash/instance/subagents/agent-a1.jsonl");
    let text = std::fs::read_to_string(&transcript)
        .unwrap()
        .replace("claude-sonnet-4-5-20250929", "claude-opus-4-1-20250805");
    std::fs::write(transcript, text).unwrap();
    let mut called = false;
    let mut place = |_: &[u8]| {
        called = true;
        Ok(())
    };
    assert_eq!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut place,
            &Default::default()
        ),
        Err(dispatch::run::HostAttemptCloseError::ModelTierMismatch)
    );
    assert!(!called);
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  model-tier-mismatch"));
}

#[test]
fn host_subagent_tier_missing_model_evidence_is_session_invalid() {
    let (root, state, home) = host_attempt_close_fixture();
    let transcript = home
        .path()
        .join(".claude/projects/hash/instance/subagents/agent-a1.jsonl");
    let text = std::fs::read_to_string(&transcript)
        .unwrap()
        .replace(r#""model":"claude-sonnet-4-5-20250929","#, "");
    std::fs::write(transcript, text).unwrap();
    let mut called = false;
    let mut place = |_: &[u8]| {
        called = true;
        Ok(())
    };
    assert_eq!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut place,
            &Default::default()
        ),
        Err(dispatch::run::HostAttemptCloseError::ModelEvidenceMissing)
    );
    assert!(!called);
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  host-session-invalid"));
}

#[test]
fn host_subagent_tier_inherit_skips_policy_verification_and_records_observation() {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    let home = host_fixture(root.path(), &mut state);
    let map_path = root.path().join("model-map.json");
    std::fs::write(
        &map_path,
        r#"{"hostSubagentModels":{"claude":{"standard":{"model":"inherit"}}}}"#,
    )
    .unwrap();
    let (mut args, spec) = open_args(root.path(), state.clone());
    args.routing_path = Some(map_path);
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    state = HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    std::fs::write(state.receipt_path.as_ref().unwrap(), b"receipt").unwrap();
    std::fs::write(root.path().join("product.txt"), b"changed").unwrap();
    assert_eq!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut |_| Ok(()),
            &Default::default()
        )
        .unwrap(),
        "a1"
    );
    let log = std::fs::read_to_string(&state.log_path).unwrap();
    assert!(log.contains("terminal_state:  completed"));
    assert!(log.contains("actual_model:    claude-sonnet-4-5-20250929"));
    assert!(log.contains("model_tier_applied: no"));
}

#[test]
fn host_attempt_close_preplacement_failure_releases_leases_without_receipt() {
    let (root, state, home) = host_attempt_close_fixture();
    std::fs::remove_file(state.receipt_path.as_ref().unwrap()).unwrap();
    let mut place = |_: &[u8]| Ok(());
    assert!(host_attempt_close_call(
        root.path(),
        &state,
        home.path(),
        &mut place,
        &Default::default()
    )
    .is_err());
    let locks = root.path().join(dispatch::dispatch::LEASE_ROOT);
    assert_eq!(
        std::fs::read_dir(locks.join("prompt-writers"))
            .unwrap()
            .count(),
        0
    );
    assert_eq!(
        std::fs::read_dir(&locks)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "lock"))
            .count(),
        0
    );
    assert!(!root
        .path()
        .join(".dev/pipeline/task-a/host-attempts/attempt_01.close-claim")
        .exists());
}

#[test]
fn host_attempt_close_cleanup_failure_is_terminal_and_not_replayed() {
    let (root, state, home) = host_attempt_close_fixture();
    let mut calls = 0;
    let mut place = |_: &[u8]| {
        calls += 1;
        Ok(())
    };
    let hooks = dispatch::run::HostAttemptCloseHooks {
        before_cleanup: host_attempt_fail_cleanup,
        ..Default::default()
    };
    assert_eq!(
        host_attempt_close_call(root.path(), &state, home.path(), &mut place, &hooks),
        Err(dispatch::run::HostAttemptCloseError::Cleanup)
    );
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  completed"));
    assert_eq!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut place,
            &Default::default()
        ),
        Err(dispatch::run::HostAttemptCloseError::Claim)
    );
    assert_eq!(calls, 1);
}
