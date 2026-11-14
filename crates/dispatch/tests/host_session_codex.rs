use dispatch::host_session::{
    locate_codex, verify, verify_at_root, HostKind, HostSessionError, HostSessionRef,
};
use std::{fs, path::PathBuf};
use tempfile::TempDir;

const ATTEMPT: &str = "attempt-7f30";
const THREAD: &str = "thread-parent";
const ROLLOUT: &str = "rollout-child";
const START: &str = "2026-10-13T08:52:20.000Z";

fn setup() -> (TempDir, PathBuf) {
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
    fs::write(date.join(format!("{ROLLOUT}.jsonl")), text).unwrap();
    (root, cwd)
}

fn reference() -> HostSessionRef {
    HostSessionRef {
        host: HostKind::Codex,
        reference: ROLLOUT.into(),
    }
}
fn verify_fixture(
    home: &TempDir,
    cwd: &std::path::Path,
    phase: &str,
) -> Result<dispatch::host_session::VerifiedHostSession, HostSessionError> {
    verify_at_root(&reference(), cwd, home.path(), START, ATTEMPT, phase, &[])
}

#[test]
fn codex_desktop_custom_exec_rollout_passes_and_returns_rollout_id() {
    let (home, cwd) = setup();
    assert_eq!(
        verify_fixture(&home, &cwd, "implement").unwrap().session_id,
        ROLLOUT
    );
}

#[test]
fn codex_epoch_attempt_start_locates_fixture_rollout() {
    let (home, cwd) = setup();
    assert_eq!(
        verify_at_root(
            &reference(),
            &cwd,
            home.path(),
            "epoch+1791871940s",
            ATTEMPT,
            "test",
            &[],
        )
        .unwrap()
        .session_id,
        ROLLOUT
    );
}

#[test]
fn codex_rejects_attempt_id_that_is_only_a_substring() {
    let (home, cwd) = setup();
    let path = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    let s = fs::read_to_string(&path)
        .unwrap()
        .replace(ATTEMPT, "xattempt-7f30z");
    fs::write(path, s).unwrap();
    assert_eq!(
        verify_fixture(&home, &cwd, "test"),
        Err(HostSessionError::AttemptIdNotInUserMessage)
    );
}

#[test]
fn codex_rejects_ambiguous_exact_rollout_ids() {
    let (home, cwd) = setup();
    let date = home.path().join("sessions/2026/10/14");
    fs::create_dir_all(&date).unwrap();
    fs::copy(
        home.path()
            .join("sessions/2026/10/13")
            .join(format!("{ROLLOUT}.jsonl")),
        date.join(format!("copy-{ROLLOUT}.jsonl")),
    )
    .unwrap();
    assert_eq!(
        locate_codex(home.path(), &cwd, ROLLOUT, START),
        Err(HostSessionError::AmbiguousRecord)
    );
}

#[test]
fn codex_rejects_other_thread_source() {
    let (home, cwd) = setup();
    let path = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    let s = fs::read_to_string(&path).unwrap().replace(
        "\"thread_source\":\"subagent\"",
        "\"thread_source\":\"cli\"",
    );
    fs::write(path, s).unwrap();
    assert_eq!(
        verify_fixture(&home, &cwd, "test"),
        Err(HostSessionError::OrchestratedOnlyAgent)
    );
}

#[test]
fn codex_function_call_write_tool_is_accepted() {
    let (home, cwd) = setup();
    let path = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    let s = fs::read_to_string(&path)
        .unwrap()
        .replace("custom_tool_call", "function_call")
        .replace("\"name\":\"exec\"", "\"name\":\"Write\"");
    fs::write(path, s).unwrap();
    assert!(verify_fixture(&home, &cwd, "implement").is_ok());
}

#[test]
fn codex_custom_apply_patch_write_tool_is_accepted() {
    let (home, cwd) = setup();
    let path = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    let s = fs::read_to_string(&path)
        .unwrap()
        .replace("\"name\":\"exec\"", "\"name\":\"apply_patch\"");
    fs::write(path, s).unwrap();
    assert!(verify_fixture(&home, &cwd, "implement").is_ok());
}

#[test]
fn codex_codex_home_can_differ_from_user_home() {
    let (home, cwd) = setup();
    let user_home = TempDir::new().unwrap();
    let old = std::env::var_os("CODEX_HOME");
    unsafe {
        std::env::set_var("CODEX_HOME", home.path());
    }
    let result = verify(
        &reference(),
        &cwd,
        user_home.path(),
        START,
        ATTEMPT,
        "test",
        &[],
    );
    unsafe {
        if let Some(old) = old {
            std::env::set_var("CODEX_HOME", old);
        } else {
            std::env::remove_var("CODEX_HOME");
        }
    }
    assert!(result.is_ok());
}

#[test]
fn codex_rejects_cwd_mismatch() {
    let (home, _) = setup();
    let other = home.path().join("elsewhere");
    fs::create_dir_all(&other).unwrap();
    assert_eq!(
        verify_fixture(&home, &other, "test"),
        Err(HostSessionError::CwdMismatch)
    );
}

#[test]
fn codex_null_agent_role_uses_default_role() {
    let (home, cwd) = setup();
    assert!(verify_fixture(&home, &cwd, "test").is_ok());
}

#[test]
fn codex_malformed_rollout_is_rejected() {
    let (home, cwd) = setup();
    let path = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    let mut text = fs::read_to_string(&path).unwrap();
    text.push_str("not json\n");
    fs::write(path, text).unwrap();
    assert_eq!(
        verify_fixture(&home, &cwd, "test"),
        Err(HostSessionError::MalformedRecord)
    );
}

#[test]
fn codex_missing_rollout_returns_record_missing() {
    let (home, cwd) = setup();
    fs::remove_file(
        home.path()
            .join("sessions/2026/10/13")
            .join(format!("{ROLLOUT}.jsonl")),
    )
    .unwrap();
    assert_eq!(
        verify_fixture(&home, &cwd, "test"),
        Err(HostSessionError::RecordMissing)
    );
}

#[test]
fn codex_skips_partitions_before_attempt_start_date() {
    let (home, cwd) = setup();
    let old = home.path().join("sessions/2026/10/12");
    fs::create_dir_all(&old).unwrap();
    let valid = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    fs::copy(valid, old.join(format!("old-{ROLLOUT}.jsonl"))).unwrap();
    assert!(verify_fixture(&home, &cwd, "test").is_ok());
}

#[test]
fn codex_detects_duplicate_in_partition_on_or_after_attempt_start_date() {
    let (home, cwd) = setup();
    let current = home.path().join("sessions/2026/10/13");
    let later = home.path().join("sessions/2026/10/14");
    fs::create_dir_all(&later).unwrap();
    let valid = current.join(format!("{ROLLOUT}.jsonl"));
    fs::copy(valid, later.join(format!("copy-{ROLLOUT}.jsonl"))).unwrap();
    assert_eq!(
        verify_fixture(&home, &cwd, "test"),
        Err(HostSessionError::AmbiguousRecord)
    );
}

#[test]
fn codex_reference_matches_exact_metadata_not_filename_substring() {
    let (home, cwd) = setup();
    assert_eq!(
        locate_codex(home.path(), &cwd, "rollout", START),
        Err(HostSessionError::RecordMissing)
    );
}

#[test]
fn codex_parent_thread_fixture_is_present_when_available() {
    let (home, _) = setup();
    let path = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    assert!(fs::read_to_string(path).unwrap().contains(THREAD));
}

#[test]
fn codex_spawn_agent_path_reference_locates_rollout() {
    let (home, cwd) = setup();
    let path = home
        .path()
        .join("sessions/2026/10/13")
        .join(format!("{ROLLOUT}.jsonl"));
    let s = fs::read_to_string(&path)
        .unwrap()
        .replace("\"agents/general-purpose\"", "\"/root/host_1_2\"");
    fs::write(&path, s).unwrap();
    assert_eq!(
        locate_codex(home.path(), &cwd, "/root/host_1_2", START),
        Ok(path)
    );
    let by_path = HostSessionRef {
        host: HostKind::Codex,
        reference: "/root/host_1_2".into(),
    };
    assert_eq!(
        verify_at_root(
            &by_path,
            &cwd,
            home.path(),
            START,
            ATTEMPT,
            "implement",
            &[]
        )
        .unwrap()
        .session_id,
        ROLLOUT
    );
    for invalid in ["/", "/root/", "/root/../x", "/Root/host", "/root/host-1"] {
        assert_eq!(
            locate_codex(home.path(), &cwd, invalid, START),
            Err(HostSessionError::InvalidCodexReference),
            "{invalid}"
        );
    }
    assert_eq!(
        locate_codex(home.path(), &cwd, "/root/other", START),
        Err(HostSessionError::RecordMissing)
    );
}

#[test]
fn codex_spawn_task_agent_message_binds_attempt_only_for_own_agent_path() {
    let rewrite = |recipient: &str| {
        let (home, cwd) = setup();
        let path = home
            .path()
            .join("sessions/2026/10/13")
            .join(format!("{ROLLOUT}.jsonl"));
        let text = fs::read_to_string(&path).unwrap();
        let rewritten = text
            .lines()
            .map(|line| {
                if line.contains("\"role\":\"user\"") && line.contains(ATTEMPT) {
                    serde_json::json!({
                        "timestamp": "2026-10-13T08:52:29.000Z",
                        "type": "response_item",
                        "payload": {
                            "type": "agent_message",
                            "author": "/root",
                            "recipient": recipient,
                            "content": [{"type": "input_text", "text": format!("Attempt {ATTEMPT}: run the phase.")}]
                        }
                    })
                    .to_string()
                } else {
                    line.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&path, format!("{rewritten}\n")).unwrap();
        verify_at_root(
            &reference(),
            &cwd,
            home.path(),
            START,
            ATTEMPT,
            "implement",
            &[],
        )
    };
    assert_eq!(
        rewrite("agents/general-purpose").unwrap().session_id,
        ROLLOUT
    );
    assert_eq!(
        rewrite("/root/someone_else"),
        Err(HostSessionError::AttemptIdNotInUserMessage)
    );
}
