use dispatch::host_attempt_state::HostAttemptState;
use dispatch::routing::{
    resolve_one_shot_route, HostRouteCapability, OneShotRouteRequest, ResolvedOneShotRoute,
    RoutingTable,
};
use sha2::Digest;
use std::path::Path;

fn capability() -> HostRouteCapability {
    HostRouteCapability {
        host: "claude".into(),
        model: None,
        effort: None,
        supports_model_selection: true,
        supports_effort_selection: true,
    }
}

fn explicit_request(model: &str) -> OneShotRouteRequest {
    OneShotRouteRequest {
        host_subagent: Some("claude".into()),
        model: Some(model.into()),
        effort: Some("high".into()),
        ..Default::default()
    }
}

fn create_worktree(root: &Path) {
    std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(root)
        .status()
        .unwrap();
    std::fs::write(root.join("plan.prompt.md"), "canonical prompt bytes").unwrap();
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
}

fn open_state(root: &Path) -> HostAttemptState {
    serde_json::from_value(serde_json::json!({
        "attempt_id": "explicit_attempt_01",
        "worktree": root,
        "prompt": root.join("plan.prompt.md"),
        "task": "task-a",
        "phase": "test",
        "profile": "default",
        "run_id": "run-01",
        "host": "claude",
        "mode": "host-subagent",
        "spec_path": root.join(".dev/pipeline/task-a/spec.md"),
        "spec_sha256": format!("{:x}", sha2::Sha256::digest(b"host spec")),
        "receipt_path": root.join(".dev/pipeline/receipt.md"),
        "log_path": root.join(".dev/pipeline/host-attempts/explicit_attempt_01.log.md"),
        "snapshot_path": root.join(".dev/pipeline/host-attempts/explicit_attempt_01.snapshot"),
        "started_at": "epoch+1s",
        "model": "unused",
        "effort": null,
        "prompt_sha256": format!("{:x}", sha2::Sha256::digest(b"canonical prompt bytes")),
    }))
    .unwrap()
}

fn host_home(root: &Path, state: &mut HostAttemptState) -> tempfile::TempDir {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host_session/claude");
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
    let text = fixture_text
        .replace("/workspace/project", &cwd)
        .replace("attempt-id-123", &state.attempt_id)
        .replace(
            "{\"type\":\"assistant\",\"message\":{\"content\"",
            "{\"type\":\"assistant\",\"message\":{\"model\":\"claude-haiku-4-5-20251001\",\"content\"",
        );
    std::fs::write(agents.join("agent-a1.jsonl"), text).unwrap();
    std::fs::copy(
        fixture.join("agent-valid.meta.json"),
        agents.join("agent-a1.meta.json"),
    )
    .unwrap();
    home
}

fn open_explicit(
    root: &Path,
    mut state: HostAttemptState,
) -> (HostAttemptState, tempfile::TempDir) {
    let home = host_home(root, &mut state);
    let args = dispatch::cli::DispatchArgs {
        phase: dispatch::stage::Phase::Test,
        task: state.task.clone(),
        workdir: root.to_path_buf(),
        pinned_mode: Some(dispatch::stage::DispatchMode::HostSubagent),
        host_attempt: Some(
            dispatch::cli::HostAttemptOpenOptions {
                state: Some(state.clone()),
                prompt_snapshot: Some(b"canonical prompt bytes".to_vec()),
                task_scope_dir: Some(root.join(".dev/pipeline/task-a")),
                prerequisite_refs: vec!["accepted-implement".into()],
                task_signals: Some(dispatch::model_tier::TaskSignals {
                    phase: dispatch::stage::Phase::Test,
                    prior_failed_attempts: 0,
                    implement: Some(dispatch::model_tier::PrerequisiteEvidence {
                        accepted: true,
                        mode: "routed".into(),
                        tier: None,
                    }),
                    test: None,
                }),
                ..Default::default()
            }
            .with_one_shot_route(explicit_request("claude-opus-4-1-20250805"), capability()),
        ),
        ..Default::default()
    };
    let outcome = dispatch::run::run_dispatch(&args, "host spec");
    assert_eq!(outcome.exit_code, 0, "{:?}", outcome.reason);
    (
        HostAttemptState::load(&state.manifest_path().unwrap(), &state.attempt_id).unwrap(),
        home,
    )
}

#[test]
fn explicit_host_route_bypasses_tier_map_and_retains_auto_tier_prerequisite() {
    let root = tempfile::tempdir().unwrap();
    create_worktree(root.path());
    let state = open_state(root.path());
    let (manifest, _) = open_explicit(root.path(), state.clone());
    assert_eq!(manifest.model, "claude-opus-4-1-20250805");
    assert_eq!(manifest.effort.as_deref(), Some("high"));
    let manifest_bytes = manifest.encode_immutable().unwrap();
    let manifest_json: serde_json::Value = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest_json["state"]["model_source"], "explicit-one-shot");
    assert!(manifest_json["state"]["route_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(
        manifest.tier_source.as_deref(),
        Some("prerequisite-tier-unknown")
    );
    assert_eq!(manifest.tier.as_deref(), Some("light"));
    assert_eq!(manifest.prerequisite_refs, ["accepted-implement"]);
}

#[test]
fn explicit_host_capability_rejects_unsupported_model_before_open() {
    let mut denied = capability();
    denied.supports_model_selection = false;
    assert!(resolve_one_shot_route(
        &explicit_request("claude-opus-4-1-20250805"),
        &RoutingTable::default(),
        Some(&denied),
    )
    .is_err());
}

#[test]
fn explicit_host_route_accepts_model_absent_from_tier_map() {
    let resolved = resolve_one_shot_route(
        &explicit_request("provider-custom-model"),
        &RoutingTable::default(),
        Some(&capability()),
    )
    .unwrap();
    let ResolvedOneShotRoute::Host(binding) = resolved else {
        panic!("expected host route");
    };
    assert_eq!(binding.route().model, "provider-custom-model");
    assert_eq!(binding.route().effort.as_deref(), Some("high"));
    assert!(binding.digest().starts_with("sha256:"));
}

#[test]
fn legacy_manifest_bytes_decode_and_default_route_fields_stay_omitted() {
    let root = tempfile::tempdir().unwrap();
    let state = open_state(root.path());
    let legacy_state = serde_json::to_vec(&state).unwrap();
    let digest = format!("{:x}", sha2::Sha256::digest(&legacy_state));
    let legacy_manifest = format!(
        "{{\"state\":{},\"sha256\":\"{}\"}}",
        String::from_utf8(legacy_state).unwrap(),
        digest
    )
    .into_bytes();

    let decoded = HostAttemptState::decode_immutable(&legacy_manifest).unwrap();
    let encoded = decoded.encode_immutable().unwrap();
    assert_eq!(encoded, legacy_manifest);
    let encoded_json: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
    assert!(encoded_json["state"].get("model_source").is_none());
    assert!(encoded_json["state"].get("route_digest").is_none());
}

#[test]
fn immutable_manifest_rejects_changed_state_bytes() {
    let root = tempfile::tempdir().unwrap();
    let state = open_state(root.path());
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&state.encode_immutable().unwrap()).unwrap();
    manifest["state"]["model"] = serde_json::Value::String("tampered".into());
    let changed = serde_json::to_vec(&manifest).unwrap();

    assert_eq!(
        HostAttemptState::decode_immutable(&changed),
        Err(dispatch::host_attempt_state::HostAttemptStateError::DigestMismatch)
    );
}

#[test]
fn close_model_observation_mismatch_is_terminal_before_placement() {
    let root = tempfile::tempdir().unwrap();
    create_worktree(root.path());
    let state = open_state(root.path());
    let (manifest, home) = open_explicit(root.path(), state.clone());
    std::fs::write(manifest.receipt_path.as_ref().unwrap(), b"receipt").unwrap();
    let mut placement_called = false;
    let result = dispatch::run::close_host_attempt(
        root.path(),
        &root.path().join(".dev/pipeline/task-a"),
        "task-a",
        &manifest.attempt_id,
        "task-a",
        "test",
        "claude",
        "claude:a1",
        home.path(),
        &[],
        &mut |_| {
            placement_called = true;
            Ok(())
        },
        &Default::default(),
    );
    assert_eq!(
        result,
        Err(dispatch::run::HostAttemptCloseError::ModelTierMismatch)
    );
    assert!(!placement_called);
    let log = std::fs::read_to_string(&manifest.log_path).unwrap();
    assert!(log.contains("terminal_state:  model-tier-mismatch"));
}
