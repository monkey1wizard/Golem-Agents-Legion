use dispatch::host_session::{
    verify as verify_session, verify_at_root, verify_model_policy, HostKind, HostSessionError,
    HostSessionRef,
};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

const ATTEMPT: &str = "attempt-7f30";
const START: &str = "2026-10-13T08:52:20.000Z";
const ROLLOUT: &str = "rollout-child";

fn setup() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().unwrap();
    let cwd = root.path().join("work");
    fs::create_dir_all(&cwd).unwrap();
    let date = root.path().join("sessions/2026/10/13");
    fs::create_dir_all(&date).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/host_session/codex/rollout-desktop.jsonl");
    let text = fs::read_to_string(fixture).unwrap().replace(
        "/workspace/project",
        &cwd.to_string_lossy().replace('\\', "/"),
    );
    let rollout = date.join(format!("{ROLLOUT}.jsonl"));
    fs::write(&rollout, text).unwrap();
    (root, cwd, rollout)
}

fn verify(
    root: &TempDir,
    cwd: &std::path::Path,
) -> Result<dispatch::host_session::VerifiedHostSession, HostSessionError> {
    verify_at_root(
        &HostSessionRef {
            host: HostKind::Codex,
            reference: ROLLOUT.into(),
        },
        cwd,
        root.path(),
        START,
        ATTEMPT,
        "test",
        &[],
    )
}

#[test]
fn codex_model_and_effort_match_every_turn() {
    let (root, cwd, _) = setup();
    let verified = verify(&root, &cwd).unwrap();
    assert_eq!(
        verify_model_policy(&verified, "gpt-6-astra", Some("high")).unwrap(),
        dispatch::host_session::ObservedModelPolicy {
            model: "gpt-6-astra".into(),
            effort: Some("high".into()),
        }
    );
}

#[test]
fn codex_mixed_turn_models_fail() {
    let (root, cwd, rollout) = setup();
    let mut text = fs::read_to_string(&rollout).unwrap();
    let context = text
        .lines()
        .find(|line| line.contains("turn_context"))
        .unwrap()
        .to_owned();
    text.push_str(&context.replace("gpt-6-astra", "gpt-6-sol"));
    fs::write(rollout, text).unwrap();
    let verified = verify(&root, &cwd).unwrap();
    assert_eq!(
        verify_model_policy(&verified, "gpt-6-astra", Some("high")),
        Err(HostSessionError::ModelMismatch)
    );
}

#[test]
fn codex_effort_mismatch_fails() {
    let (root, cwd, _) = setup();
    let verified = verify(&root, &cwd).unwrap();
    assert_eq!(
        verify_model_policy(&verified, "gpt-6-astra", Some("medium")),
        Err(HostSessionError::EffortMismatch)
    );
}

#[test]
fn codex_missing_turn_context_has_no_model_evidence() {
    let (root, cwd, rollout) = setup();
    let text = fs::read_to_string(&rollout).unwrap();
    let text = text
        .lines()
        .filter(|line| !line.contains("turn_context"))
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(rollout, text).unwrap();
    let verified = verify(&root, &cwd).unwrap();
    assert_eq!(
        verify_model_policy(&verified, "gpt-6-astra", None),
        Err(HostSessionError::MissingModelEvidence)
    );
}

#[test]
fn empty_expected_model_does_not_accept_nonempty_observation() {
    let (root, cwd, _) = setup();
    let verified = verify(&root, &cwd).unwrap();
    assert_eq!(
        verify_model_policy(&verified, "", None),
        Err(HostSessionError::ModelMismatch)
    );
}

#[test]
fn claude_model_family_matches_and_synthetic_records_are_ignored() {
    let home = TempDir::new().unwrap();
    let cwd = home.path().join("work");
    fs::create_dir_all(&cwd).unwrap();
    let dir = home.path().join(".claude/projects/hash/instance/subagents");
    fs::create_dir_all(&dir).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/host_session/claude/agent-valid.jsonl");
    let text = fs::read_to_string(fixture).unwrap().replace(
        "/workspace/project",
        &cwd.to_string_lossy().replace('\\', "/"),
    );
    let transcript = dir.join("agent-a1.jsonl");
    fs::write(&transcript, text).unwrap();
    fs::write(
        dir.join("agent-a1.meta.json"),
        r#"{"agentType":"subagent:coder"}"#,
    )
    .unwrap();
    let mut text = fs::read_to_string(&transcript).unwrap();
    text.push_str("{\"type\":\"assistant\",\"message\":{\"model\":\"claude-sonnet-4-5-20250929\"}}\n{\"type\":\"assistant\",\"message\":{\"model\":\"<synthetic>\"}}\n{\"type\":\"synthetic\",\"message\":{\"model\":\"unexpected\"}}\n");
    fs::write(&transcript, text).unwrap();
    let verified = verify_session(
        &HostSessionRef::parse("claude:a1").unwrap(),
        &cwd,
        home.path(),
        START,
        "attempt-id-123",
        "test",
        &[],
    )
    .unwrap();
    assert_eq!(
        verify_model_policy(&verified, "sonnet", Some("high")).unwrap(),
        dispatch::host_session::ObservedModelPolicy {
            model: "claude-sonnet-4-5-20250929".into(),
            effort: None,
        }
    );
}

#[test]
fn claude_model_alias_rejects_a_different_family() {
    let home = TempDir::new().unwrap();
    let cwd = home.path().join("work");
    fs::create_dir_all(&cwd).unwrap();
    let dir = home.path().join(".claude/projects/hash/instance/subagents");
    fs::create_dir_all(&dir).unwrap();
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/host_session/claude/agent-valid.jsonl");
    let text = fs::read_to_string(fixture).unwrap().replace(
        "/workspace/project",
        &cwd.to_string_lossy().replace('\\', "/"),
    );
    let transcript = dir.join("agent-a1.jsonl");
    fs::write(&transcript, text).unwrap();
    fs::write(
        dir.join("agent-a1.meta.json"),
        r#"{"agentType":"subagent:coder"}"#,
    )
    .unwrap();
    let mut text = fs::read_to_string(&transcript).unwrap();
    text.push_str(
        "{\"type\":\"assistant\",\"message\":{\"model\":\"claude-haiku-4-5-20251001\"}}\n",
    );
    fs::write(&transcript, text).unwrap();
    let verified = verify_session(
        &HostSessionRef::parse("claude:a1").unwrap(),
        &cwd,
        home.path(),
        START,
        "attempt-id-123",
        "test",
        &[],
    )
    .unwrap();
    assert_eq!(
        verify_model_policy(&verified, "sonnet", None),
        Err(HostSessionError::ModelMismatch)
    );
}
