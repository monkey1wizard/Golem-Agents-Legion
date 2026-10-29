use dispatch::host_attempt_state::{HostAttemptState, HostAttemptStateError};
use sha2::Digest;
use std::collections::BTreeMap;
use std::io::{BufRead, Read, Write};
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
    assert!(log.contains("tier:            light"));
    assert!(log.contains("tier_source:     default"));
    assert!(log.contains("model_tier_applied: yes"));
    let manifest = HostAttemptState::load(&manifest_path, &state.attempt_id).unwrap();
    assert_eq!(manifest.tier.as_deref(), Some("light"));
    assert_eq!(manifest.model, "haiku");
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
        "{\"type\":\"assistant\",\"message\":{\"model\":\"claude-haiku-4-5-20251001\",\"content\"",
    );
    assert!(text.contains("claude-haiku-4-5-20251001"));
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
        "{\"type\":\"assistant\",\"message\":{\"model\":\"claude-haiku-4-5-20251001\",\"content\"",
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
    assert!(terminal_log.contains("actual_model:    claude-haiku-4-5-20251001"));
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

fn host_attempt_abandon_fixture(
    terminal: &str,
) -> (tempfile::TempDir, HostAttemptState, tempfile::TempDir) {
    let (root, state, home) = host_attempt_close_fixture();
    let mut log = std::fs::read_to_string(&state.log_path).unwrap();
    log = log.replace(
        "terminal_state:  started",
        &format!("terminal_state:  {terminal}"),
    );
    std::fs::write(&state.log_path, log).unwrap();
    (root, state, home)
}

fn host_attempt_abandon_call(
    root: &std::path::Path,
    state: &HostAttemptState,
    hooks: &dispatch::run::HostAttemptAbandonHooks,
) -> Result<(), dispatch::run::HostAttemptAbandonError> {
    dispatch::run::abandon_host_attempt(
        root,
        &root.join(".dev/pipeline/task-a"),
        "task-a",
        &state.attempt_id,
        "task-a",
        "implement",
        "claude",
        hooks,
    )
}

fn host_attempt_abandon_fail_validation() -> Result<(), String> {
    Err("injected post-restore validation failure".into())
}

fn host_attempt_abandon_fail_restore(_: &std::path::Path, _: &[u8]) -> Result<(), String> {
    Err("injected restore failure".into())
}

fn host_attempt_abandon_fail_publish(_: &std::path::Path, _: &[u8]) -> Result<(), String> {
    Err("injected abandoned publication failure".into())
}

fn host_attempt_abandon_fail_cleanup() -> Result<(), String> {
    Err("injected abandon cleanup failure".into())
}

fn host_attempt_abandon_fail_after_first_cleanup() -> Result<(), String> {
    Err("injected partial cleanup failure".into())
}

#[test]
fn host_attempt_abandon_child_process_helper() {
    let Ok(root) = std::env::var("GAL_HOST_ABANDON_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let attempt = std::env::var("GAL_HOST_ABANDON_ATTEMPT").unwrap();
    let manifest =
        HostAttemptState::manifest_path_for(&root.join(".dev/pipeline/task-a"), &attempt).unwrap();
    let state = HostAttemptState::load(&manifest, &attempt).unwrap();
    let target = std::env::var("GAL_HOST_ABANDON_BOUNDARY").unwrap();
    let boundary = std::sync::Arc::new(move |current| {
        let name = match current {
            dispatch::run::HostAttemptAbandonBoundary::ClaimCreated => "claim",
            dispatch::run::HostAttemptAbandonBoundary::Restored => "restore",
            dispatch::run::HostAttemptAbandonBoundary::Published => "publication",
            dispatch::run::HostAttemptAbandonBoundary::PartialCleanup => "partial-cleanup",
            _ => return,
        };
        if target == name {
            println!("READY:{name}");
            std::io::stdout().flush().unwrap();
            let mut byte = [0];
            let _ = std::io::stdin().read_exact(&mut byte);
        }
    });
    let hooks = dispatch::run::HostAttemptAbandonHooks {
        boundary,
        ..Default::default()
    };
    let _ = host_attempt_abandon_call(&root, &state, &hooks);
}

#[test]
fn host_attempt_abandon_started_restores_and_publishes_prior_reason() {
    let (root, state, _) = host_attempt_abandon_fixture("started");
    std::fs::write(&state.prompt, b"host prompt edit").unwrap();
    assert_eq!(
        host_attempt_abandon_call(root.path(), &state, &Default::default()),
        Ok(())
    );
    assert_eq!(
        std::fs::read(&state.prompt).unwrap(),
        b"canonical prompt bytes"
    );
    let log = std::fs::read_to_string(&state.log_path).unwrap();
    assert!(log.contains("terminal_state:  abandoned"));
    assert!(log.contains("prior_terminal_state: started"));
    assert_eq!(lease_counts(root.path()), (0, 0));
    assert!(!close_claim_path(root.path(), &state.attempt_id).exists());
}

#[test]
fn host_attempt_abandon_model_and_session_failures_are_eligible() {
    for terminal in [
        "host-session-invalid",
        "no-writeback",
        "lease-owner-mismatch",
        "lease-missing",
        "identity-mismatch",
        "terminal-replay",
        "placement-failed",
        "model-tier-mismatch",
    ] {
        let (root, state, _) = host_attempt_abandon_fixture(terminal);
        std::fs::write(&state.prompt, b"host prompt edit").unwrap();
        assert_eq!(
            host_attempt_abandon_call(root.path(), &state, &Default::default()),
            Ok(())
        );
        assert_eq!(
            std::fs::read(&state.prompt).unwrap(),
            b"canonical prompt bytes"
        );
        assert!(std::fs::read_to_string(&state.log_path)
            .unwrap()
            .contains(&format!("prior_terminal_state: {terminal}")));
    }
}

#[test]
fn host_attempt_abandon_refuses_coordinator_profile_before_custody_changes() {
    let (root, state, _) = host_attempt_abandon_fixture("started");
    let manifest_path = state.manifest_path().unwrap();
    let mut coordinator = HostAttemptState::load(&manifest_path, &state.attempt_id).unwrap();
    coordinator.profile = "codex_stop_v1".into();
    std::fs::remove_file(&manifest_path).unwrap();
    coordinator.write_once().unwrap();

    let claim_path = close_claim_path(root.path(), &state.attempt_id);
    let lock_path = HostAttemptState::custody_lock_path_for(
        &root.path().join(".dev/pipeline/task-a"),
        &state.attempt_id,
    )
    .unwrap();
    let before = close_protected_bytes(root.path(), &state);
    assert_eq!(
        host_attempt_abandon_call(root.path(), &state, &Default::default()),
        Err(dispatch::run::HostAttemptAbandonError::CoordinatorOwned)
    );
    assert_eq!(close_protected_bytes(root.path(), &state), before);
    assert!(!claim_path.exists());
    assert!(!lock_path.exists());
}

#[test]
fn host_attempt_abandon_terminal_cleanup_retry_never_restores_or_republishes() {
    for terminal in ["completed", "abandoned"] {
        let (root, state, _) = host_attempt_abandon_fixture(terminal);
        std::fs::write(&state.prompt, b"keep prompt exactly").unwrap();
        let before_digest = format!(
            "{:x}",
            sha2::Sha256::digest(std::fs::read(&state.prompt).unwrap())
        );
        let before_publications = std::fs::read_to_string(&state.log_path)
            .unwrap()
            .matches(&format!("terminal_state:  {terminal}"))
            .count();
        let restores = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let publications = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let restore_count = restores.clone();
        let publication_count = publications.clone();
        let hooks = dispatch::run::HostAttemptAbandonHooks {
            restore: std::sync::Arc::new(move |_, _| {
                restore_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }),
            publish_terminal: std::sync::Arc::new(move |_, _| {
                publication_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }),
            ..Default::default()
        };
        assert_eq!(
            host_attempt_abandon_call(root.path(), &state, &hooks),
            Ok(())
        );
        assert_eq!(restores.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(publications.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(
            format!(
                "{:x}",
                sha2::Sha256::digest(std::fs::read(&state.prompt).unwrap())
            ),
            before_digest
        );
        assert_eq!(
            std::fs::read_to_string(&state.log_path)
                .unwrap()
                .matches(&format!("terminal_state:  {terminal}"))
                .count(),
            before_publications
        );
        assert_eq!(lease_counts(root.path()), (0, 0));
        assert_eq!(
            host_attempt_abandon_call(root.path(), &state, &Default::default()),
            Ok(())
        );
    }
}

#[test]
fn host_attempt_abandon_prewrite_rejection_removes_only_new_claim() {
    for existing_claim in [false, true] {
        let (root, state, _) = host_attempt_abandon_fixture("started");
        std::fs::write(&state.prompt, b"host prompt edit").unwrap();
        if existing_claim {
            std::fs::create_dir(close_claim_path(root.path(), &state.attempt_id)).unwrap();
        }
        std::fs::write(&state.snapshot_path, b"mismatched snapshot").unwrap();
        assert_eq!(
            host_attempt_abandon_call(root.path(), &state, &Default::default()),
            Err(dispatch::run::HostAttemptAbandonError::Ownership)
        );
        assert_eq!(
            close_claim_path(root.path(), &state.attempt_id).exists(),
            existing_claim
        );
        assert_eq!(std::fs::read(&state.prompt).unwrap(), b"host prompt edit");
        assert!(
            HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
                .unwrap()
                .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
                .is_ok()
        );
    }
}

#[test]
fn host_attempt_abandon_postwrite_errors_retain_claim_and_release_mutex() {
    for hooks in [
        dispatch::run::HostAttemptAbandonHooks {
            restore: std::sync::Arc::new(host_attempt_abandon_fail_restore),
            ..Default::default()
        },
        dispatch::run::HostAttemptAbandonHooks {
            validate_after_restore: std::sync::Arc::new(host_attempt_abandon_fail_validation),
            ..Default::default()
        },
        dispatch::run::HostAttemptAbandonHooks {
            publish_terminal: std::sync::Arc::new(host_attempt_abandon_fail_publish),
            ..Default::default()
        },
        dispatch::run::HostAttemptAbandonHooks {
            before_cleanup: std::sync::Arc::new(host_attempt_abandon_fail_cleanup),
            ..Default::default()
        },
        dispatch::run::HostAttemptAbandonHooks {
            after_first_cleanup: std::sync::Arc::new(host_attempt_abandon_fail_after_first_cleanup),
            ..Default::default()
        },
    ] {
        for existing_claim in [false, true] {
            let (root, state, _) = host_attempt_abandon_fixture("started");
            std::fs::write(&state.prompt, b"host prompt edit").unwrap();
            if existing_claim {
                std::fs::create_dir(close_claim_path(root.path(), &state.attempt_id)).unwrap();
            }
            assert!(host_attempt_abandon_call(root.path(), &state, &hooks).is_err());
            assert!(close_claim_path(root.path(), &state.attempt_id).is_dir());
            assert!(
                HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
                    .unwrap()
                    .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
                    .is_ok()
            );
            assert_eq!(
                host_attempt_abandon_call(root.path(), &state, &Default::default()),
                Ok(())
            );
        }
    }
}

#[test]
fn host_attempt_abandon_cleanup_error_retains_new_and_taken_over_claims() {
    for terminal in ["completed", "abandoned"] {
        for existing_claim in [false, true] {
            let (root, state, _) = host_attempt_abandon_fixture(terminal);
            std::fs::write(&state.prompt, b"keep prompt exactly").unwrap();
            let prompt_digest = format!(
                "{:x}",
                sha2::Sha256::digest(std::fs::read(&state.prompt).unwrap())
            );
            let log_before = std::fs::read(&state.log_path).unwrap();
            let claim = close_claim_path(root.path(), &state.attempt_id);
            if existing_claim {
                std::fs::create_dir(&claim).unwrap();
            }
            let restores = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let publications = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let restore_count = restores.clone();
            let publication_count = publications.clone();
            let hooks = dispatch::run::HostAttemptAbandonHooks {
                restore: std::sync::Arc::new(move |_, _| {
                    restore_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }),
                publish_terminal: std::sync::Arc::new(move |_, _| {
                    publication_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }),
                after_first_cleanup: std::sync::Arc::new(
                    host_attempt_abandon_fail_after_first_cleanup,
                ),
                ..Default::default()
            };

            assert_eq!(
                host_attempt_abandon_call(root.path(), &state, &hooks),
                Err(dispatch::run::HostAttemptAbandonError::Cleanup)
            );
            assert!(
                claim.is_dir(),
                "claim lost for {terminal}, existing={existing_claim}"
            );
            assert!(
                HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
                    .unwrap()
                    .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
                    .is_ok()
            );
            assert_eq!(
                format!(
                    "{:x}",
                    sha2::Sha256::digest(std::fs::read(&state.prompt).unwrap())
                ),
                prompt_digest
            );
            assert_eq!(std::fs::read(&state.log_path).unwrap(), log_before);
            assert_eq!(restores.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert_eq!(publications.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert_eq!(lease_counts(root.path()), (1, 0));

            assert_eq!(
                host_attempt_abandon_call(root.path(), &state, &Default::default()),
                Ok(())
            );
            assert!(!claim.exists());
            assert_eq!(lease_counts(root.path()), (0, 0));
        }
    }
}

#[test]
fn host_attempt_abandon_preserves_mismatched_lease_and_claim() {
    let (root, state, _) = host_attempt_abandon_fixture("completed");
    std::fs::create_dir(close_claim_path(root.path(), &state.attempt_id)).unwrap();
    let path = std::fs::read_dir(
        root.path()
            .join(dispatch::dispatch::LEASE_ROOT)
            .join("prompt-writers"),
    )
    .unwrap()
    .next()
    .unwrap()
    .unwrap()
    .path();
    std::fs::write(&path, "owner_token=someone-else\n").unwrap();
    let before = std::fs::read(&path).unwrap();
    assert_eq!(
        host_attempt_abandon_call(root.path(), &state, &Default::default()),
        Err(dispatch::run::HostAttemptAbandonError::Lease)
    );
    assert_eq!(std::fs::read(&path).unwrap(), before);
    assert!(close_claim_path(root.path(), &state.attempt_id).is_dir());
}

#[test]
fn host_attempt_abandon_preserves_mismatched_snapshot_and_spec() {
    for artifact in ["snapshot", "spec"] {
        let (root, state, _) = host_attempt_abandon_fixture("completed");
        let path = if artifact == "snapshot" {
            &state.snapshot_path
        } else {
            &state.spec_path
        };
        std::fs::write(path, b"other owner's bytes").unwrap();
        let before = std::fs::read(path).unwrap();
        std::fs::create_dir(close_claim_path(root.path(), &state.attempt_id)).unwrap();
        assert_eq!(
            host_attempt_abandon_call(root.path(), &state, &Default::default()),
            Err(dispatch::run::HostAttemptAbandonError::Ownership)
        );
        assert_eq!(std::fs::read(path).unwrap(), before);
        assert!(close_claim_path(root.path(), &state.attempt_id).is_dir());
    }
}

#[test]
fn host_attempt_abandon_tolerates_already_cleaned_artifacts() {
    for terminal in ["completed", "abandoned"] {
        for missing_artifact in ["receipt lease", "prompt lease", "snapshot", "spec"] {
            let (root, state, _) = host_attempt_abandon_fixture(terminal);
            let locks = root.path().join(dispatch::dispatch::LEASE_ROOT);
            let prompt_leases = std::fs::read_dir(locks.join("prompt-writers"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .collect::<Vec<_>>();
            let receipt_leases = std::fs::read_dir(&locks)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "lock"))
                .collect::<Vec<_>>();
            assert_eq!(prompt_leases.len(), 1);
            assert_eq!(receipt_leases.len(), 1);

            let missing_path = match missing_artifact {
                "receipt lease" => &receipt_leases[0],
                "prompt lease" => &prompt_leases[0],
                "snapshot" => &state.snapshot_path,
                "spec" => &state.spec_path,
                _ => unreachable!(),
            };
            assert!(missing_path.is_file());
            std::fs::remove_file(missing_path).unwrap();
            assert!(!missing_path.exists());
            for remaining_path in prompt_leases
                .iter()
                .chain(receipt_leases.iter())
                .chain([&state.snapshot_path, &state.spec_path])
                .filter(|path| *path != missing_path)
            {
                assert!(remaining_path.is_file(), "{remaining_path:?}");
            }

            std::fs::write(&state.prompt, b"keep prompt exactly").unwrap();
            let before_prompt = std::fs::read(&state.prompt).unwrap();
            let before_terminal = std::fs::read(&state.log_path).unwrap();
            let restores = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let publications = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let restore_count = restores.clone();
            let publication_count = publications.clone();
            let hooks = dispatch::run::HostAttemptAbandonHooks {
                restore: std::sync::Arc::new(move |_, _| {
                    restore_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }),
                publish_terminal: std::sync::Arc::new(move |_, _| {
                    publication_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok(())
                }),
                ..Default::default()
            };
            assert_eq!(
                host_attempt_abandon_call(root.path(), &state, &hooks),
                Ok(()),
                "terminal={terminal}, missing={missing_artifact}"
            );
            assert_eq!(restores.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert_eq!(publications.load(std::sync::atomic::Ordering::SeqCst), 0);
            assert_eq!(std::fs::read(&state.prompt).unwrap(), before_prompt);
            assert_eq!(std::fs::read(&state.log_path).unwrap(), before_terminal);
            assert_eq!(lease_counts(root.path()), (0, 0));
            assert!(!state.snapshot_path.exists());
            assert!(!state.spec_path.exists());
            assert!(!close_claim_path(root.path(), &state.attempt_id).exists());
            let stable_lock = HostAttemptState::custody_lock_path_for(
                &root.path().join(".dev/pipeline/task-a"),
                &state.attempt_id,
            )
            .unwrap();
            assert!(stable_lock.is_file());
        }
    }
}

#[test]
fn host_attempt_abandon_races_close_and_other_abandon_without_loser_writes() {
    let (root, state, home) = host_attempt_abandon_fixture("started");
    std::fs::write(&state.prompt, b"host prompt edit").unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Arc::new(std::sync::Mutex::new(release_rx));
    let boundary = std::sync::Arc::new(move |current| {
        if current == dispatch::run::HostAttemptAbandonBoundary::ClaimCreated {
            ready_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }
    });
    let hooks = dispatch::run::HostAttemptAbandonHooks {
        boundary,
        ..Default::default()
    };
    let root_a = root.path().to_path_buf();
    let state_a = state.clone();
    let abandon = std::thread::spawn(move || host_attempt_abandon_call(&root_a, &state_a, &hooks));
    ready_rx.recv().unwrap();
    let before = close_protected_bytes(root.path(), &state);
    let mut place = |_: &[u8]| Ok(());
    let close_result = host_attempt_close_call(
        root.path(),
        &state,
        home.path(),
        &mut place,
        &Default::default(),
    );
    assert_eq!(
        close_result,
        Err(dispatch::run::HostAttemptCloseError::Mutex)
    );
    assert_eq!(close_protected_bytes(root.path(), &state), before);
    assert!(close_claim_path(root.path(), &state.attempt_id).is_dir());
    release_tx.send(()).unwrap();
    assert_eq!(abandon.join().unwrap(), Ok(()));
    assert!(
        std::fs::read_to_string(&state.log_path)
            .unwrap()
            .contains("terminal_state:  completed")
            || std::fs::read_to_string(&state.log_path)
                .unwrap()
                .contains("terminal_state:  abandoned")
    );
}

#[test]
fn host_attempt_abandon_race_admits_one_writer_and_preserves_loser_state() {
    let (root, state, _) = host_attempt_abandon_fixture("started");
    std::fs::write(&state.prompt, b"host prompt edit").unwrap();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let release_rx = std::sync::Arc::new(std::sync::Mutex::new(release_rx));
    let boundary = std::sync::Arc::new(move |current| {
        if current == dispatch::run::HostAttemptAbandonBoundary::ClaimCreated {
            ready_tx.send(()).unwrap();
            release_rx.lock().unwrap().recv().unwrap();
        }
    });
    let hooks = dispatch::run::HostAttemptAbandonHooks {
        boundary,
        ..Default::default()
    };
    let root_a = root.path().to_path_buf();
    let state_a = state.clone();
    let winner = std::thread::spawn(move || host_attempt_abandon_call(&root_a, &state_a, &hooks));
    ready_rx.recv().unwrap();
    let before = close_protected_bytes(root.path(), &state);
    let loser = host_attempt_abandon_call(root.path(), &state, &Default::default());
    assert_eq!(loser, Err(dispatch::run::HostAttemptAbandonError::Mutex));
    assert_eq!(close_protected_bytes(root.path(), &state), before);
    release_tx.send(()).unwrap();
    assert_eq!(winner.join().unwrap(), Ok(()));
    let log = std::fs::read_to_string(&state.log_path).unwrap();
    assert_eq!(log.matches("terminal_state:  abandoned").count(), 1);
    assert_eq!(lease_counts(root.path()), (0, 0));
}

#[test]
fn host_attempt_abandon_crash_recovery_converges_at_each_boundary() {
    for boundary in ["claim", "restore", "publication", "partial-cleanup"] {
        let (root, state, _) = host_attempt_abandon_fixture("started");
        std::fs::write(&state.prompt, b"host prompt edit").unwrap();
        if boundary == "partial-cleanup" {
            std::fs::write(
                &state.log_path,
                std::fs::read_to_string(&state.log_path)
                    .unwrap()
                    .replace("terminal_state:  started", "terminal_state:  completed"),
            )
            .unwrap();
        }
        let mut child = ChildProcess(Some(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "host_attempt_abandon_child_process_helper",
                    "--nocapture",
                ])
                .env("GAL_HOST_ABANDON_ROOT", root.path())
                .env("GAL_HOST_ABANDON_ATTEMPT", &state.attempt_id)
                .env("GAL_HOST_ABANDON_BOUNDARY", boundary)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .unwrap(),
        ));
        let mut stdout = std::io::BufReader::new(child.0.as_mut().unwrap().stdout.take().unwrap());
        let mut ready = String::new();
        loop {
            ready.clear();
            assert!(
                stdout.read_line(&mut ready).unwrap() > 0,
                "child exited before {boundary}"
            );
            if ready.trim().starts_with("READY:") {
                break;
            }
        }
        assert_eq!(ready.trim(), format!("READY:{boundary}"));
        child.0.as_mut().unwrap().kill().unwrap();
        let output = child.wait_with_output();
        assert!(
            !output.status.success(),
            "child unexpectedly exited normally at {boundary}"
        );
        assert_eq!(
            host_attempt_abandon_call(root.path(), &state, &Default::default()),
            Ok(()),
            "boundary={boundary}"
        );
        let log = std::fs::read_to_string(&state.log_path).unwrap();
        assert!(
            log.contains("terminal_state:  abandoned")
                || log.contains("terminal_state:  completed")
        );
        assert!(
            HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
                .unwrap()
                .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
                .is_ok()
        );
    }
}

#[test]
fn host_attempt_abandon_with_satisfied_owner_preconditions_allows_next_open() {
    let owner_approval_satisfied = true;
    let stopped_subagent_confirmed = true;
    assert!(owner_approval_satisfied && stopped_subagent_confirmed);
    let (root, state, _) = host_attempt_abandon_fixture("started");
    std::fs::write(&state.prompt, b"host prompt edit").unwrap();
    assert_eq!(
        host_attempt_abandon_call(root.path(), &state, &Default::default()),
        Ok(())
    );
    let mut next = state.clone();
    next.attempt_id = "attempt_02".into();
    next.snapshot_path = root.path().join(".dev/pipeline/next.snapshot");
    next.spec_path = root.path().join(".dev/pipeline/next.spec");
    next.log_path = root.path().join(".dev/pipeline/next.log");
    next.receipt_path = Some(root.path().join(".dev/pipeline/next.receipt"));
    next.prompt_sha256 = Some(format!(
        "{:x}",
        sha2::Sha256::digest(b"canonical prompt bytes")
    ));
    let (args, spec) = open_args(root.path(), next);
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
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

fn close_claim_path(root: &std::path::Path, attempt_id: &str) -> PathBuf {
    HostAttemptState::manifest_path_for(&root.join(".dev/pipeline/task-a"), attempt_id)
        .unwrap()
        .with_extension("close-claim")
}

fn close_protected_bytes(
    root: &std::path::Path,
    state: &HostAttemptState,
) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn visit(path: &std::path::Path, files: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        if path.is_dir() {
            for entry in std::fs::read_dir(path).unwrap() {
                visit(&entry.unwrap().path(), files);
            }
        } else {
            files.insert(path.to_path_buf(), std::fs::read(path).ok());
        }
    }

    let mut files = BTreeMap::new();
    for path in [
        state.manifest_path().unwrap(),
        state.snapshot_path.clone(),
        state.spec_path.clone(),
        state.prompt.clone(),
        state.receipt_path.as_ref().unwrap().clone(),
        state.log_path.clone(),
    ] {
        files.insert(path.clone(), std::fs::read(path).ok());
    }
    visit(&root.join(dispatch::dispatch::LEASE_ROOT), &mut files);
    files
}

struct ChildProcess(Option<std::process::Child>);

impl ChildProcess {
    fn wait_with_output(mut self) -> std::process::Output {
        self.0.take().unwrap().wait_with_output().unwrap()
    }
}

impl Drop for ChildProcess {
    fn drop(&mut self) {
        if let Some(child) = self.0.as_mut() {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

#[test]
fn host_attempt_close_child_process_helper() {
    let Ok(mode) = std::env::var("GAL_HOST_CLOSE_CHILD_MODE") else {
        return;
    };
    let root = PathBuf::from(std::env::var_os("GAL_HOST_CLOSE_ROOT").unwrap());
    let home = PathBuf::from(std::env::var_os("GAL_HOST_CLOSE_HOME").unwrap());
    let attempt_id = std::env::var("GAL_HOST_CLOSE_ATTEMPT").unwrap();
    let manifest_path =
        HostAttemptState::manifest_path_for(&root.join(".dev/pipeline/task-a"), &attempt_id)
            .unwrap();
    let state = HostAttemptState::load(&manifest_path, &attempt_id).unwrap();

    match mode.as_str() {
        "hold-mutex" => {
            let _guard = state
                .acquire_custody_lock_at(&root.join(".dev/pipeline/task-a"))
                .unwrap();
            println!("READY");
            std::io::stdout().flush().unwrap();
            let mut release = [0_u8; 1];
            std::io::stdin().read_exact(&mut release).unwrap();
        }
        "rejected" | "possible-write" => {
            use dispatch::run::{
                HostAttemptPlacementFailure as Failure, HostAttemptPlacementOutcome as Outcome,
            };
            let (cause, outcome) = if mode == "rejected" {
                ("child rejected before write", Outcome::RejectedBeforeWrite)
            } else {
                ("child may have written", Outcome::MayHaveWritten)
            };
            let mut placement = |_: &[u8]| {
                Err(Failure {
                    cause: cause.into(),
                    outcome,
                })
            };
            assert_eq!(
                dispatch::run::close_host_attempt_with_typed_placement(
                    &root,
                    &root.join(".dev/pipeline/task-a"),
                    "task-a",
                    &state.attempt_id,
                    "task-a",
                    "implement",
                    "claude",
                    "claude:a1",
                    &home,
                    &[],
                    &mut placement,
                    &Default::default(),
                ),
                Err(dispatch::run::HostAttemptCloseError::Placement(Failure {
                    cause: cause.into(),
                    outcome,
                }))
            );
        }
        other => panic!("unexpected child helper mode {other}"),
    }
}

fn lease_counts(root: &std::path::Path) -> (usize, usize) {
    let locks = root.join(dispatch::dispatch::LEASE_ROOT);
    let prompts = std::fs::read_dir(locks.join("prompt-writers"))
        .unwrap()
        .count();
    let receipts = std::fs::read_dir(&locks)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "lock"))
        .count();
    (prompts, receipts)
}

#[test]
fn host_attempt_close_typed_rejection_removes_claim_and_retries_directly() {
    use dispatch::run::{
        HostAttemptPlacementFailure as Failure, HostAttemptPlacementOutcome as Outcome,
    };
    let (root, state, home) = host_attempt_close_fixture();
    let prompt_before = std::fs::read(&state.prompt).unwrap();
    let leases_before = lease_counts(root.path());
    let mut reject = |_: &[u8]| {
        Err(Failure {
            cause: "render rejected before write".into(),
            outcome: Outcome::RejectedBeforeWrite,
        })
    };
    let result = dispatch::run::close_host_attempt_with_typed_placement(
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
        &mut reject,
        &Default::default(),
    );
    assert_eq!(
        result,
        Err(dispatch::run::HostAttemptCloseError::Placement(Failure {
            cause: "render rejected before write".into(),
            outcome: Outcome::RejectedBeforeWrite,
        }))
    );
    assert!(!close_claim_path(root.path(), &state.attempt_id).exists());
    assert_eq!(lease_counts(root.path()), leases_before);
    assert_eq!(std::fs::read(&state.prompt).unwrap(), prompt_before);
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  started"));

    let mut place = |bytes: &[u8]| {
        std::fs::write(state.receipt_path.as_ref().unwrap(), bytes).unwrap();
        Ok(())
    };
    assert_eq!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut place,
            &Default::default()
        )
        .unwrap(),
        "a1"
    );
}

#[test]
fn host_attempt_close_typed_possible_write_retains_claim_and_leases() {
    use dispatch::run::{
        HostAttemptPlacementFailure as Failure, HostAttemptPlacementOutcome as Outcome,
    };
    let (root, state, home) = host_attempt_close_fixture();
    let prompt_after_write = b"possibly written prompt";
    let mut place = |_: &[u8]| {
        std::fs::write(&state.prompt, prompt_after_write).unwrap();
        Err(Failure {
            cause: "prompt write failed".into(),
            outcome: Outcome::MayHaveWritten,
        })
    };
    assert_eq!(
        dispatch::run::close_host_attempt_with_typed_placement(
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
            &mut place,
            &Default::default(),
        ),
        Err(dispatch::run::HostAttemptCloseError::Placement(Failure {
            cause: "prompt write failed".into(),
            outcome: Outcome::MayHaveWritten,
        }))
    );
    assert!(close_claim_path(root.path(), &state.attempt_id).is_dir());
    assert_eq!(lease_counts(root.path()), (1, 1));
    assert_eq!(std::fs::read(&state.prompt).unwrap(), prompt_after_write);
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  started"));
    let guard = HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
        .unwrap()
        .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"));
    assert!(
        guard.is_ok(),
        "CLOSE must release the mutex after a returned error"
    );
    drop(guard);

    let mut retried_placement = false;
    let mut retry = |_: &[u8]| {
        retried_placement = true;
        Ok(())
    };
    assert_eq!(
        host_attempt_close_call(
            root.path(),
            &state,
            home.path(),
            &mut retry,
            &Default::default(),
        ),
        Err(dispatch::run::HostAttemptCloseError::Claim)
    );
    assert!(!retried_placement);
    assert!(
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
            .unwrap()
            .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
            .is_ok()
    );
}

#[test]
fn host_attempt_close_mutex_contention_precedes_claim_access() {
    let (root, state, home) = host_attempt_close_fixture();
    let mut child = ChildProcess(Some(
        std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host_attempt_close_child_process_helper",
                "--nocapture",
            ])
            .env("GAL_HOST_CLOSE_CHILD_MODE", "hold-mutex")
            .env("GAL_HOST_CLOSE_ROOT", root.path())
            .env("GAL_HOST_CLOSE_HOME", home.path())
            .env("GAL_HOST_CLOSE_ATTEMPT", &state.attempt_id)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap(),
    ));
    let mut stdout = std::io::BufReader::new(child.0.as_mut().unwrap().stdout.take().unwrap());
    let mut ready = String::new();
    let mut became_ready = false;
    loop {
        ready.clear();
        if stdout.read_line(&mut ready).unwrap() == 0 {
            break;
        }
        if ready.trim() == "READY" {
            became_ready = true;
            break;
        }
    }
    if !became_ready {
        let _ = child.0.as_mut().unwrap().kill();
        drop(stdout);
        let output = child.wait_with_output();
        panic!(
            "mutex child did not become ready: {ready}{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let protected_before = close_protected_bytes(root.path(), &state);
    let claim_path = close_claim_path(root.path(), &state.attempt_id);
    let mut place = |_: &[u8]| Ok(());
    let close_result = host_attempt_close_call(
        root.path(),
        &state,
        home.path(),
        &mut place,
        &Default::default(),
    );
    let protected_after = close_protected_bytes(root.path(), &state);
    let claim_after = std::fs::read(&claim_path).ok();
    child
        .0
        .as_mut()
        .unwrap()
        .stdin
        .take()
        .unwrap()
        .write_all(b"x")
        .unwrap();
    let mut trailing_stdout = String::new();
    stdout.read_to_string(&mut trailing_stdout).unwrap();
    let child_output = child.wait_with_output();

    assert!(
        child_output.status.success(),
        "{}",
        String::from_utf8_lossy(&child_output.stderr)
    );
    assert_eq!(
        close_result,
        Err(dispatch::run::HostAttemptCloseError::Mutex)
    );
    assert_eq!(protected_after, protected_before);
    assert_eq!(
        claim_after, None,
        "mutex contention must precede claim access"
    );
}

#[test]
fn host_attempt_close_captures_typed_failure_causes_from_child_stderr() {
    for (mode, cause) in [
        ("rejected", "child rejected before write"),
        ("possible-write", "child may have written"),
    ] {
        let (root, state, home) = host_attempt_close_fixture();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host_attempt_close_child_process_helper",
                "--nocapture",
            ])
            .env("GAL_HOST_CLOSE_CHILD_MODE", mode)
            .env("GAL_HOST_CLOSE_ROOT", root.path())
            .env("GAL_HOST_CLOSE_HOME", home.path())
            .env("GAL_HOST_CLOSE_ATTEMPT", &state.attempt_id)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed for {mode}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(cause),
            "child stderr omitted {cause:?}: {stderr}"
        );
    }
}

#[test]
fn host_attempt_close_holds_mutex_at_each_custody_boundary() {
    use dispatch::run::HostAttemptCloseBoundary as Boundary;
    for target in [
        Boundary::Placement,
        Boundary::Publication,
        Boundary::PreCleanup,
    ] {
        let (root, state, home) = host_attempt_close_fixture();
        let (arrived_tx, arrived_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let release_rx = std::sync::Arc::new(std::sync::Mutex::new(release_rx));
        let boundary = std::sync::Arc::new(move |current| {
            if current == target {
                arrived_tx.send(()).unwrap();
                release_rx.lock().unwrap().recv().unwrap();
            }
        });
        let hooks = dispatch::run::HostAttemptCloseHooks {
            boundary,
            ..Default::default()
        };
        let root_path = root.path().to_path_buf();
        let home_path = home.path().to_path_buf();
        let state_for_close = state.clone();
        let closer = std::thread::spawn(move || {
            let mut place = |_: &[u8]| Ok(());
            host_attempt_close_call(&root_path, &state_for_close, &home_path, &mut place, &hooks)
        });
        arrived_rx.recv().unwrap();
        let manifest =
            HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
        assert!(manifest
            .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
            .is_err());
        release_tx.send(()).unwrap();
        assert_eq!(closer.join().unwrap(), Ok("a1".into()));
        assert!(
            HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
                .unwrap()
                .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
                .is_ok()
        );
    }
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
    assert_eq!(lease_counts(root.path()), (1, 1));
    assert!(
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
            .unwrap()
            .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
            .is_ok()
    );
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
        .replace("claude-haiku-4-5-20251001", "claude-opus-4-1-20250805");
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
        .replace(r#""model":"claude-haiku-4-5-20251001","#, "");
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
        r#"{"hostSubagentModels":{"claude":{"light":{"model":"inherit"}}}}"#,
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
    assert!(log.contains("actual_model:    claude-haiku-4-5-20251001"));
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
    assert!(close_claim_path(root.path(), &state.attempt_id).is_dir());
    assert_eq!(lease_counts(root.path()), (1, 1));
    assert!(
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
            .unwrap()
            .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
            .is_ok()
    );
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

#[test]
fn host_attempt_close_artifact_removal_failure_retains_claim_and_releases_mutex() {
    let (root, state, home) = host_attempt_close_fixture();
    // A non-empty directory at the spec path makes remove_file fail with a
    // non-NotFound error after the terminal state and lease release.
    std::fs::remove_file(&state.spec_path).unwrap();
    std::fs::create_dir_all(&state.spec_path).unwrap();
    std::fs::write(state.spec_path.join("blocker"), b"x").unwrap();
    let prompt_before = std::fs::read(&state.prompt).unwrap();
    let mut calls = 0;
    let mut place = |_: &[u8]| {
        calls += 1;
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
        Err(dispatch::run::HostAttemptCloseError::Cleanup)
    );
    assert!(std::fs::read_to_string(&state.log_path)
        .unwrap()
        .contains("terminal_state:  completed"));
    assert!(close_claim_path(root.path(), &state.attempt_id).is_dir());
    assert_eq!(std::fs::read(&state.prompt).unwrap(), prompt_before);
    assert!(state.spec_path.join("blocker").is_file());
    assert!(
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id)
            .unwrap()
            .acquire_custody_lock_at(&root.path().join(".dev/pipeline/task-a"))
            .is_ok()
    );
    assert_eq!(calls, 1);
}

fn finalize_state(root: &std::path::Path) -> HostAttemptState {
    let mut state = local_state(root);
    state.phase = "finalize-review".into();
    state
}

fn finalize_args(
    root: &std::path::Path,
    state: HostAttemptState,
    routing: PathBuf,
) -> (dispatch::cli::DispatchArgs, String) {
    let (mut args, spec) = open_args(root, state);
    args.phase = dispatch::stage::Phase::FinalizeReview;
    args.routing_path = Some(routing);
    (args, spec)
}

fn implement_signals(prior_failed_attempts: usize) -> dispatch::model_tier::TaskSignals {
    dispatch::model_tier::TaskSignals {
        phase: dispatch::stage::Phase::Implement,
        prior_failed_attempts,
        implement: None,
        test: None,
    }
}

#[test]
fn host_subagent_finalize_review_inherits_with_missing_map() {
    let root = tempfile::tempdir().unwrap();
    let state = finalize_state(root.path());
    let (args, spec) = finalize_args(root.path(), state.clone(), root.path().join("absent.json"));
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    assert_eq!(outcome.exit_code, 0, "{:?}", outcome.reason);
    let manifest =
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    assert_eq!(manifest.tier.as_deref(), Some("standard"));
    assert_eq!(manifest.tier_source.as_deref(), Some("finalize-review"));
    assert_eq!(manifest.model, "inherit");
    assert_eq!(manifest.effort, None);
    assert!(!manifest.policy_applied);
    let log = std::fs::read_to_string(&state.log_path).unwrap();
    assert!(log.contains("tier:            standard"));
    assert!(log.contains("tier_source:     finalize-review"));
    assert!(log.contains("model_tier_applied: no"));
}

#[test]
fn host_subagent_finalize_review_inherits_with_malformed_map() {
    let root = tempfile::tempdir().unwrap();
    let state = finalize_state(root.path());
    let map_path = root.path().join("malformed-map.json");
    std::fs::write(&map_path, r#"{"hostSubagentModels":"not-an-object"}"#).unwrap();
    let (args, spec) = finalize_args(root.path(), state.clone(), map_path);
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    assert_eq!(outcome.exit_code, 0, "{:?}", outcome.reason);
    let manifest =
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    assert_eq!(manifest.model, "inherit");
    assert!(!manifest.policy_applied);
}

#[test]
fn host_subagent_finalize_review_override_resolves_through_map() {
    let root = tempfile::tempdir().unwrap();
    let state = finalize_state(root.path());
    let map_path = root.path().join("model-map.json");
    std::fs::write(
        &map_path,
        r#"{"hostSubagentModels":{"claude":{"deep":{"model":"fable"}}}}"#,
    )
    .unwrap();
    let (mut args, spec) = finalize_args(root.path(), state.clone(), map_path);
    let options = args.host_attempt.as_mut().unwrap();
    options.tier_override = Some(dispatch::model_tier::ModelTier::Deep);
    options.override_reason = Some("cross-cutting final review".into());
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    assert_eq!(outcome.exit_code, 0, "{:?}", outcome.reason);
    let manifest =
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    assert_eq!(manifest.tier.as_deref(), Some("deep"));
    assert_eq!(manifest.tier_source.as_deref(), Some("override"));
    assert_eq!(manifest.model, "fable");
    assert_eq!(manifest.effort, None);
    assert!(manifest.policy_applied);
    let log = std::fs::read_to_string(&state.log_path).unwrap();
    assert!(log.contains("tier_source:     override"));
    assert!(log.contains("model_tier_applied: yes"));
}

#[test]
fn host_subagent_finalize_review_override_with_malformed_map_fails_closed() {
    let root = tempfile::tempdir().unwrap();
    let state = finalize_state(root.path());
    let map_path = root.path().join("malformed-map.json");
    std::fs::write(&map_path, r#"{"hostSubagentModels":"not-an-object"}"#).unwrap();
    let (mut args, spec) = finalize_args(root.path(), state.clone(), map_path);
    let options = args.host_attempt.as_mut().unwrap();
    options.tier_override = Some(dispatch::model_tier::ModelTier::Deep);
    options.override_reason = Some("cross-cutting final review".into());
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    assert_eq!(outcome.exit_code, 2);
    assert_eq!(
        outcome.reason.as_deref(),
        Some("host-attempt-model-policy-invalid")
    );
    assert!(!state.manifest_path().unwrap().exists());
}

#[test]
fn host_subagent_routed_prerequisite_evidence_resolves_through_map_at_open() {
    let root = tempfile::tempdir().unwrap();
    let mut state = local_state(root.path());
    state.phase = "test".into();
    let (mut args, spec) = open_args(root.path(), state.clone());
    args.phase = dispatch::stage::Phase::Test;
    args.host_attempt.as_mut().unwrap().task_signals = Some(dispatch::model_tier::TaskSignals {
        phase: dispatch::stage::Phase::Test,
        prior_failed_attempts: 0,
        implement: Some(dispatch::model_tier::PrerequisiteEvidence {
            accepted: true,
            mode: "routed".into(),
            tier: None,
        }),
        test: None,
    });
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    assert_eq!(outcome.exit_code, 0, "{:?}", outcome.reason);
    let manifest =
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    assert_eq!(manifest.tier.as_deref(), Some("light"));
    assert_eq!(
        manifest.tier_source.as_deref(),
        Some("prerequisite-tier-unknown")
    );
    assert_eq!(manifest.model, "haiku");
    assert!(manifest.policy_applied);
}

#[test]
fn host_subagent_open_preserves_accepted_retry_evidence() {
    let root = tempfile::tempdir().unwrap();
    let state = local_state(root.path());
    let (mut args, spec) = open_args(root.path(), state.clone());
    args.host_attempt.as_mut().unwrap().task_signals = Some(implement_signals(2));
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    let manifest =
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    assert_eq!(manifest.tier.as_deref(), Some("standard"));
    assert_eq!(manifest.tier_source.as_deref(), Some("retry"));
    assert_eq!(manifest.model, "sonnet");

    let root = tempfile::tempdir().unwrap();
    let state = local_state(root.path());
    let (mut args, spec) = open_args(root.path(), state.clone());
    args.host_attempt.as_mut().unwrap().failure_history = vec!["attempt-1 failed".into()];
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    let manifest =
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    assert_eq!(manifest.tier.as_deref(), Some("standard"));
    assert_eq!(manifest.tier_source.as_deref(), Some("retry"));
    assert_eq!(
        manifest.failure_history,
        vec!["attempt-1 failed".to_string()]
    );
}

#[test]
fn host_subagent_prior_standard_advisory_does_not_mutate_open_state() {
    let mut observed = Vec::new();
    for advisory in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let state = local_state(root.path());
        let (mut args, spec) = open_args(root.path(), state.clone());
        let options = args.host_attempt.as_mut().unwrap();
        options.task_signals = Some(implement_signals(1));
        options.prior_standard_failure = advisory;
        assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
        let manifest =
            HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
        observed.push((
            manifest.tier,
            manifest.tier_source,
            manifest.model,
            manifest.effort,
            manifest.policy_applied,
            manifest.failure_history,
        ));
    }
    assert_eq!(observed[0], observed[1]);
    assert_eq!(observed[1].0.as_deref(), Some("standard"));
    assert_eq!(observed[1].1.as_deref(), Some("retry"));
}

#[test]
fn host_subagent_finalize_review_inherit_closes_without_model_verification() {
    let root = tempfile::tempdir().unwrap();
    let mut state = finalize_state(root.path());
    let home = host_fixture(root.path(), &mut state);
    let (args, spec) = finalize_args(root.path(), state.clone(), root.path().join("absent.json"));
    assert_eq!(dispatch::run::run_dispatch(&args, &spec).exit_code, 0);
    let state = HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
    std::fs::write(state.receipt_path.as_ref().unwrap(), b"receipt").unwrap();
    assert_eq!(
        dispatch::run::close_host_attempt(
            root.path(),
            &root.path().join(".dev/pipeline/task-a"),
            "task-a",
            &state.attempt_id,
            "task-a",
            "finalize-review",
            "claude",
            "claude:a1",
            home.path(),
            &[],
            &mut |_| Ok(()),
            &Default::default(),
        ),
        Ok("a1".to_string())
    );
    let log = std::fs::read_to_string(&state.log_path).unwrap();
    assert!(log.contains("terminal_state:  completed"));
    assert!(log.contains("model_tier_applied: no"));
}

#[test]
fn host_subagent_finalize_review_routed_route_stays_routed_without_host_inherit() {
    let root = tempfile::tempdir().unwrap();
    let state = finalize_state(root.path());
    let routing = root.path().join("routing.json");
    std::fs::write(
        &routing,
        r#"{"executorRouting":{"pipeline":{"AUDITOR":{"executor":"claude","model":"sonnet"}}}}"#,
    )
    .unwrap();
    let selected = dispatch::routing::load_selected_route(&routing, "AUDITOR");
    assert!(matches!(
        selected,
        dispatch::routing::SelectedRoute::Route(_)
    ));
    // The host request stays pinned here, so the finalize-review precedence is what keeps the route.
    assert_eq!(
        dispatch::run::resolve_dispatch_mode(
            dispatch::stage::Phase::FinalizeReview,
            &selected,
            Some(dispatch::stage::DispatchMode::HostSubagent),
            None,
        ),
        Ok(dispatch::stage::DispatchMode::Routed)
    );
    let (args, spec) = finalize_args(root.path(), state.clone(), routing);
    let outcome = dispatch::run::run_dispatch(&args, &spec);
    // No receipt path is configured, so the routed run stops at the receipt gate before any
    // executor launches, and the host inherit path never opens an attempt.
    assert_eq!(outcome.exit_code, 2);
    assert_eq!(outcome.reason.as_deref(), Some("pipeline-receipt-required"));
    assert!(!state.manifest_path().unwrap().exists());
    assert!(!state.log_path.exists());
}

fn map_based_open(
    root: &std::path::Path,
    phase: dispatch::stage::Phase,
    routing: PathBuf,
    signals: Option<dispatch::model_tier::TaskSignals>,
) -> (dispatch::run::DispatchOutcome, HostAttemptState) {
    let mut state = local_state(root);
    state.phase = phase.as_str().into();
    let (mut args, spec) = open_args(root, state.clone());
    args.phase = phase;
    args.routing_path = Some(routing);
    let options = args.host_attempt.as_mut().unwrap();
    options.task_signals = signals;
    (dispatch::run::run_dispatch(&args, &spec), state)
}

fn phase_signals(phase: dispatch::stage::Phase) -> Option<dispatch::model_tier::TaskSignals> {
    use dispatch::model_tier::{PrerequisiteEvidence, TaskSignals};
    use dispatch::stage::Phase;
    let accepted_implement = Some(PrerequisiteEvidence {
        accepted: true,
        mode: "routed".into(),
        tier: None,
    });
    Some(match phase {
        Phase::Test => TaskSignals {
            phase,
            prior_failed_attempts: 0,
            implement: accepted_implement,
            test: None,
        },
        Phase::Audit => TaskSignals {
            phase,
            prior_failed_attempts: 0,
            implement: accepted_implement,
            test: Some(PrerequisiteEvidence {
                accepted: true,
                mode: "routed".into(),
                tier: None,
            }),
        },
        _ => implement_signals(0),
    })
}

#[test]
fn host_subagent_map_based_malformed_map_fails_closed_for_each_phase() {
    use dispatch::stage::Phase;
    for phase in [Phase::Implement, Phase::Test, Phase::Audit] {
        let root = tempfile::tempdir().unwrap();
        let map_path = root.path().join("malformed-map.json");
        std::fs::write(&map_path, r#"{"hostSubagentModels":"not-an-object"}"#).unwrap();
        // Control: the same open with no map file resolves through the built-in defaults.
        let control_root = tempfile::tempdir().unwrap();
        let (control, _) = map_based_open(
            control_root.path(),
            phase,
            control_root.path().join("absent.json"),
            phase_signals(phase),
        );
        assert_eq!(control.exit_code, 0, "phase={}", phase.as_str());

        let (outcome, state) = map_based_open(root.path(), phase, map_path, phase_signals(phase));
        assert_eq!(outcome.exit_code, 2, "phase={}", phase.as_str());
        assert_eq!(
            outcome.reason.as_deref(),
            Some("host-attempt-model-policy-invalid"),
            "phase={}",
            phase.as_str()
        );
        assert!(!state.manifest_path().unwrap().exists());
        assert!(!state.log_path.exists());
    }
}

#[test]
fn host_subagent_map_based_missing_map_resolves_builtin_defaults() {
    use dispatch::stage::Phase;
    // A missing map file is not a failure for map-based phases: the loader falls back to the
    // built-in defaults, so the open publishes a policy-applied attempt.
    for phase in [Phase::Implement, Phase::Test, Phase::Audit] {
        let root = tempfile::tempdir().unwrap();
        let (outcome, state) = map_based_open(
            root.path(),
            phase,
            root.path().join("absent.json"),
            phase_signals(phase),
        );
        assert_eq!(outcome.exit_code, 0, "phase={}", phase.as_str());
        let manifest =
            HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap();
        assert!(manifest.policy_applied, "phase={}", phase.as_str());
        assert!(!manifest.model.is_empty(), "phase={}", phase.as_str());
    }
}
