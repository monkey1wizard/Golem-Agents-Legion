use dispatch::host_session::{locate_claude, verify, HostSessionError, HostSessionRef};
use std::fs;
use tempfile::TempDir;

const ATTEMPT: &str = "attempt-id-123";
fn setup() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
    let home = TempDir::new().unwrap();
    let work = home.path().join("work");
    fs::create_dir_all(&work).unwrap();
    let dir = home.path().join(".claude/projects/hash/instance/subagents");
    fs::create_dir_all(&dir).unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/host_session/claude");
    fs::copy(
        fixture.join("agent-valid.jsonl"),
        dir.join("agent-a1.jsonl"),
    )
    .unwrap();
    let transcript = dir.join("agent-a1.jsonl");
    let cwd = work.to_string_lossy().replace('\\', "/");
    let text = fs::read_to_string(&transcript)
        .unwrap()
        .replace("/workspace/project", &cwd);
    fs::write(transcript, text).unwrap();
    fs::copy(
        fixture.join("agent-valid.meta.json"),
        dir.join("agent-a1.meta.json"),
    )
    .unwrap();
    (home, work, dir)
}
fn run(
    home: &TempDir,
    work: &std::path::Path,
    phase: &str,
) -> Result<dispatch::host_session::VerifiedHostSession, HostSessionError> {
    verify(
        &HostSessionRef::parse("claude:a1").unwrap(),
        work,
        home.path(),
        &attempt_start(home),
        ATTEMPT,
        phase,
        &[],
    )
}
fn attempt_start(home: &TempDir) -> String {
    let path = home
        .path()
        .join(".claude/projects/hash/instance/subagents/agent-a1.jsonl");
    let first = fs::read_to_string(path)
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .to_owned();
    serde_json::from_str::<serde_json::Value>(&first).unwrap()["timestamp"]
        .as_str()
        .unwrap()
        .to_owned()
}
fn fixture_cwd(work: &std::path::Path) -> String {
    work.to_string_lossy().replace('\\', "/")
}
fn rewrite(dir: &std::path::Path, transform: impl FnOnce(String) -> String) {
    let p = dir.join("agent-a1.jsonl");
    let value = transform(fs::read_to_string(&p).unwrap());
    fs::write(p, value).unwrap();
}
#[test]
fn claude_valid_session_passes() {
    let (h, w, _) = setup();
    assert_eq!(run(&h, &w, "implement").unwrap().session_id, "a1");
}
#[test]
fn claude_synthetic_meta_parse_failure_is_malformed() {
    let (h, w, d) = setup();
    fs::write(d.join("agent-a1.meta.json"), "{").unwrap();
    assert_eq!(
        run(&h, &w, "implement"),
        Err(HostSessionError::MalformedRecord)
    );
}
#[test]
fn claude_synthetic_timestamp_before_start_fails() {
    let (h, w, d) = setup();
    let start = attempt_start(&h);
    rewrite(&d, |s| s.replacen(&start, "2020-01-01T00:00:00.000Z", 1));
    assert_eq!(
        verify(
            &HostSessionRef::parse("claude:a1").unwrap(),
            &w,
            h.path(),
            &start,
            ATTEMPT,
            "implement",
            &[]
        ),
        Err(HostSessionError::TimestampBeforeStart)
    );
}
#[test]
fn claude_synthetic_without_tool_use_fails() {
    let (h, w, d) = setup();
    rewrite(&d, |s| s.replace("tool_use", "text"));
    assert_eq!(run(&h, &w, "implement"), Err(HostSessionError::NoToolUse));
}
#[test]
fn claude_synthetic_missing_attempt_user_message_fails() {
    let (h, w, d) = setup();
    rewrite(&d, |s| s.replace(ATTEMPT, "other"));
    assert_eq!(
        run(&h, &w, "implement"),
        Err(HostSessionError::AttemptIdNotInUserMessage)
    );
}
#[test]
fn claude_synthetic_orchestrated_agent_fails() {
    let (h, w, d) = setup();
    fs::write(
        d.join("agent-a1.meta.json"),
        r#"{"agentType":"subagent:planner"}"#,
    )
    .unwrap();
    assert_eq!(
        verify(
            &HostSessionRef::parse("claude:a1").unwrap(),
            &w,
            h.path(),
            &attempt_start(&h),
            ATTEMPT,
            "implement",
            &["planner"]
        ),
        Err(HostSessionError::OrchestratedOnlyAgent)
    );
}

#[test]
fn claude_synthetic_different_cwd_candidate_is_skipped() {
    let (h, w, d) = setup();
    let other = h.path().join(".claude/projects/other/instance/subagents");
    fs::create_dir_all(&other).unwrap();
    fs::copy(d.join("agent-a1.jsonl"), other.join("agent-a1.jsonl")).unwrap();
    let contents = fs::read_to_string(other.join("agent-a1.jsonl"))
        .unwrap()
        .replace(&fixture_cwd(&w), "/wrong/path");
    fs::write(other.join("agent-a1.jsonl"), contents).unwrap();
    assert_eq!(run(&h, &w, "implement").unwrap().session_id, "a1");
}

#[test]
fn claude_synthetic_no_matching_cwd_returns_cwd_mismatch() {
    let (h, w, d) = setup();
    rewrite(&d, |s| s.replace(&fixture_cwd(&w), "/wrong/path"));
    assert_eq!(run(&h, &w, "implement"), Err(HostSessionError::CwdMismatch));
}

#[test]
fn claude_synthetic_orchestrated_only_role_fails() {
    let (h, w, d) = setup();
    fs::write(
        d.join("agent-a1.meta.json"),
        r#"{"agentType":"gal:golem-implementer"}"#,
    )
    .unwrap();
    assert_eq!(
        verify(
            &HostSessionRef::parse("claude:a1").unwrap(),
            &w,
            h.path(),
            &attempt_start(&h),
            ATTEMPT,
            "implement",
            &["golem-implementer"]
        ),
        Err(HostSessionError::OrchestratedOnlyAgent)
    );
}

#[test]
fn claude_synthetic_read_then_write_tool_passes() {
    let (h, w, d) = setup();
    rewrite(&d, |s| {
        s.replace(
            "\"name\":\"Write\"",
            "\"name\":\"Read\"},{\"type\":\"tool_use\",\"name\":\"Write\"",
        )
    });
    assert!(run(&h, &w, "implement").is_ok());
}

#[test]
fn claude_synthetic_attempt_only_in_tool_result_fails() {
    let (h, w, d) = setup();
    rewrite(&d, |s| s.replace(ATTEMPT, "other-token"));
    let mut contents = fs::read_to_string(d.join("agent-a1.jsonl"))
        .unwrap()
        .trim_end()
        .to_owned();
    let result = serde_json::json!({"type":"user", "message":{"content":[{"type":"tool_result", "content":ATTEMPT}]}});
    contents.push_str(&format!("\n{}", result));
    fs::write(d.join("agent-a1.jsonl"), contents).unwrap();
    assert_eq!(
        run(&h, &w, "implement"),
        Err(HostSessionError::AttemptIdNotInUserMessage)
    );
}
#[test]
fn claude_synthetic_implement_without_write_tool_fails() {
    let (h, w, d) = setup();
    rewrite(&d, |s| s.replace("Write", "Read"));
    assert_eq!(run(&h, &w, "implement"), Err(HostSessionError::NoWriteTool));
}

#[test]
fn claude_invalid_references_are_rejected_before_lookup() {
    let (h, w, _) = setup();
    for reference in ["../a1", "a1/child", "a1\\child", "a1:child"] {
        let session = HostSessionRef {
            host: dispatch::host_session::HostKind::Claude,
            reference: reference.to_owned(),
        };
        assert_eq!(
            verify(
                &session,
                &w,
                h.path(),
                "epoch+0s",
                ATTEMPT,
                "implement",
                &[]
            ),
            Err(HostSessionError::InvalidReference),
            "verify should reject {reference:?}"
        );
        assert_eq!(
            locate_claude(h.path(), &w, reference),
            Err(HostSessionError::InvalidReference),
            "locator should reject {reference:?}"
        );
    }
}

#[test]
fn claude_traversal_reference_cannot_select_forged_record() {
    let (h, w, _) = setup();
    let traversal = "x/../../../../../../Code/gal-wt-hsd/fake";
    let forged = h
        .path()
        .join(".claude/projects/hash/instance/subagents/agent-fake.jsonl");
    fs::write(&forged, "{\"cwd\":\"/workspace/project\"}\n").unwrap();
    fs::write(
        h.path()
            .join(".claude/projects/hash/instance/subagents/agent-fake.meta.json"),
        "{\"agentType\":\"subagent:coder\"}",
    )
    .unwrap();
    let session = HostSessionRef {
        host: dispatch::host_session::HostKind::Claude,
        reference: traversal.to_owned(),
    };
    assert_eq!(
        verify(
            &session,
            &w,
            h.path(),
            "epoch+0s",
            ATTEMPT,
            "implement",
            &[]
        ),
        Err(HostSessionError::InvalidReference)
    );
}
