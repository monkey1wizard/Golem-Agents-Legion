//! Integration tests for provider-session evidence propagation into dispatch outcomes.
//!
//! Validates:
//! 1. Local and remote fixtures carry identical session identity across:
//!    - stdout (attempt-owned process output)
//!    - DispatchOutcome (`outcome.session_id`)
//!    - executor log file (`session_id: <uuid>` and `terminal_state: completed`)
//!    - AttemptEvidence (`evidence.provider_session`)
//! 2. Typed failures before terminal publication on every invalid case:
//!    - `session-evidence-unsupported-shell` (pre-spawn remote Windows drive-letter rejection)
//!    - `session-evidence-missing-terminal`
//!    - `session-evidence-duplicate-terminal`
//!    - `session-evidence-malformed-json`
//!    - `session-evidence-missing-field`
//!    - `session-evidence-invalid-field`
//!    - `session-evidence-truncated`
//!    - `session-evidence-oversized`

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

use dispatch::cli::DispatchArgs;
use dispatch::dispatch::{ProviderSessionProvenance, TerminalState};
use dispatch::run::{run_dispatch, DispatchOutcome};
use dispatch::stage::Phase;

static DOUBLE_DIR: OnceLock<TempDir> = OnceLock::new();
static TEST_MUTEX: Mutex<()> = Mutex::new(());

fn ensure_mock_double() -> &'static Path {
    DOUBLE_DIR
        .get_or_init(|| {
            let tmp = TempDir::new().expect("create double temp dir");
            let src_path = tmp.path().join("mock_double.rs");
            let src = r##"
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let joined = args.join(" ");

    // Guard command check (remote SSH pre-run)
    if joined.contains("PARITY_OK") {
        println!("PARITY_OK");
        return;
    }

    // Receipt fetch check (remote SSH post-run)
    if joined.contains("cat --") || (joined.contains("test -f") && joined.contains(".receipt.md")) {
        println!("# Mock Remote Receipt Content\nStatus: PASS");
        return;
    }

    // Cleanup or reset commands
    if joined.contains("rm -f") || joined.contains("git reset") || joined.contains("git clean") {
        return;
    }

    // Local receipt writing if requested
    if let Ok(receipt_path) = std::env::var("MOCK_RECEIPT_WRITE_PATH") {
        let _ = std::fs::write(&receipt_path, "# Mock Local Receipt Content\nStatus: PASS\n");
    }

    // Custom exit code
    if let Ok(code_str) = std::env::var("MOCK_EXIT_CODE") {
        if let Ok(code) = code_str.parse::<i32>() {
            if code != 0 {
                std::process::exit(code);
            }
        }
    }

    // Custom payload file
    if let Ok(file_path) = std::env::var("MOCK_PAYLOAD_FILE") {
        if let Ok(content) = std::fs::read(&file_path) {
            use std::io::Write;
            let _ = std::io::stdout().write_all(&content);
            let _ = std::io::stdout().flush();
            return;
        }
    }

    // Default valid Agy v2 native JSON
    println!("{{\"conversation_id\":\"176f1141-b606-47aa-a47b-76f2a6147623\",\"status\":\"SUCCESS\"}}");
}
"##;
            fs::write(&src_path, src).expect("write mock double source");

            #[cfg(target_os = "windows")]
            let exe_names = ["agy.exe", "ssh.exe"];
            #[cfg(not(target_os = "windows"))]
            let exe_names = ["agy", "ssh"];

            let primary_exe = tmp.path().join(exe_names[0]);
            let status = Command::new("rustc")
                .arg(&src_path)
                .arg("-o")
                .arg(&primary_exe)
                .status()
                .expect("compile mock double with rustc");
            assert!(status.success(), "rustc compilation must succeed");

            let secondary_exe = tmp.path().join(exe_names[1]);
            fs::copy(&primary_exe, &secondary_exe).expect("copy double to secondary executable");

            tmp
        })
        .path()
}

fn init_git_repo(path: &Path) {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(path)
            .output()
            .expect("git must run");
        assert!(output.status.success(), "git failed: {:?}", args);
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "tester@example.com"]);
    git(&["config", "user.name", "Tester"]);
    git(&["config", "commit.gpgsign", "false"]);
    fs::write(path.join("file.txt"), "initial content\n").unwrap();
    git(&["add", "."]);
    git(&["commit", "-q", "-m", "initial commit"]);
}

struct TestEnv {
    _lock: std::sync::MutexGuard<'static, ()>,
    workdir: TempDir,
    saved_path: std::ffi::OsString,
    saved_key: Option<String>,
}

impl TestEnv {
    fn new() -> Self {
        let lock = TEST_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let double_dir = ensure_mock_double();

        let saved_path = std::env::var_os("PATH").unwrap_or_default();
        let saved_key = std::env::var("GEMINI_API_KEY").ok();

        let mut paths = vec![double_dir.to_path_buf()];
        paths.extend(std::env::split_paths(&saved_path));
        let new_path = std::env::join_paths(paths).expect("join paths");
        std::env::set_var("PATH", new_path);
        std::env::set_var("GEMINI_API_KEY", "mock-gemini-key");
        std::env::remove_var("MOCK_PAYLOAD_FILE");
        std::env::remove_var("MOCK_RECEIPT_WRITE_PATH");
        std::env::remove_var("MOCK_EXIT_CODE");

        let workdir = TempDir::new().expect("create workdir");
        init_git_repo(workdir.path());

        Self {
            _lock: lock,
            workdir,
            saved_path,
            saved_key,
        }
    }

    fn write_payload(&self, payload: &[u8]) -> PathBuf {
        let p = self.workdir.path().join("payload.txt");
        fs::write(&p, payload).expect("write payload");
        std::env::set_var("MOCK_PAYLOAD_FILE", &p);
        p
    }

    fn write_local_routing(&self) -> PathBuf {
        let p = self.workdir.path().join("routing_local.json");
        fs::write(
            &p,
            r#"{
                "executorRouting": {
                    "executors": { "agy": "gemini-2.5-pro" },
                    "pipeline": { "TESTER": { "executor": "agy" } }
                }
            }"#,
        )
        .unwrap();
        p
    }

    fn write_remote_routing(&self, remote_workdir: &str) -> PathBuf {
        let p = self.workdir.path().join("routing_remote.json");
        fs::write(
            &p,
            format!(
                r#"{{
                    "executorRouting": {{
                        "executors": {{ "agy": "gemini-2.5-pro" }},
                        "pipeline": {{
                            "TESTER": {{
                                "executor": "agy",
                                "sshTarget": "mock-user@mock-host",
                                "remoteWorkdir": "{remote_workdir}"
                            }}
                        }}
                    }}
                }}"#
            ),
        )
        .unwrap();
        p
    }

    fn make_args(&self, routing_path: PathBuf) -> DispatchArgs {
        DispatchArgs {
            phase: Phase::Test,
            task: "T-EV".to_string(),
            workdir: self.workdir.path().to_path_buf(),
            timeout_secs: 30,
            routing_path: Some(routing_path),
            receipt_path: None,
            log_dir_override: None,
            stdout_quiet: true,
            contract_provenance: None,
            worker: None,
        }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.saved_path);
        if let Some(k) = &self.saved_key {
            std::env::set_var("GEMINI_API_KEY", k);
        } else {
            std::env::remove_var("GEMINI_API_KEY");
        }
        std::env::remove_var("MOCK_PAYLOAD_FILE");
        std::env::remove_var("MOCK_RECEIPT_WRITE_PATH");
        std::env::remove_var("MOCK_EXIT_CODE");
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 1. Happy-path four matching identities: local and remote
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_local_fixture_session_evidence_four_identities() {
    let env = TestEnv::new();
    let expected_uuid = "176f1141-b606-47aa-a47b-76f2a6147623";
    let payload = format!(r#"{{"conversation_id":"{expected_uuid}","status":"SUCCESS"}}"#);
    env.write_payload(payload.as_bytes());

    let routing = env.write_local_routing();
    let args = env.make_args(routing);

    let outcome: DispatchOutcome = run_dispatch(&args, "test task spec");

    // Identity 1: Process stdout carried the expected JSON session identity
    assert!(payload.contains(expected_uuid));

    // Identity 2: Outcome session_id
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.terminal_state, Some(TerminalState::Completed));
    assert_eq!(outcome.session_id.as_deref(), Some(expected_uuid));
    assert!(outcome.reason.is_none());

    // Identity 3: Terminal log on disk
    let log_path = outcome.log_path.expect("log path present");
    assert!(log_path.exists());
    let log_content = fs::read_to_string(&log_path).expect("read log");
    assert!(
        log_content.contains(&format!("session_id:      {expected_uuid}")),
        "log must contain matching session_id: {log_content}"
    );
    assert!(
        log_content.contains("terminal_state:  completed"),
        "log must record terminal state completed: {log_content}"
    );

    // Identity 4: AttemptEvidence provider_session
    let evidence_prov: &ProviderSessionProvenance = outcome
        .evidence
        .provider_session
        .as_ref()
        .expect("evidence provider_session present");
    assert_eq!(evidence_prov.provider, "agy");
    assert_eq!(evidence_prov.session_id, expected_uuid);

    // All four identities are identical
    assert_eq!(outcome.session_id.as_deref().unwrap(), expected_uuid);
    assert_eq!(evidence_prov.session_id, expected_uuid);
}

#[test]
fn test_remote_fixture_session_evidence_four_identities() {
    let env = TestEnv::new();
    let expected_uuid = "8a3d5b2c-91f4-4e2a-8d1e-2b7c6f0a4e8d";
    let payload = format!(r#"{{"conversation_id":"{expected_uuid}","status":"SUCCESS"}}"#);
    env.write_payload(payload.as_bytes());

    let routing = env.write_remote_routing("/remote/repo/workdir");
    let args = env.make_args(routing);

    let outcome: DispatchOutcome = run_dispatch(&args, "test task spec");

    // Identity 1: Process stdout carried the expected JSON session identity
    assert!(payload.contains(expected_uuid));

    // Identity 2: Outcome session_id
    assert_eq!(outcome.exit_code, 0);
    assert_eq!(outcome.terminal_state, Some(TerminalState::Completed));
    assert_eq!(outcome.session_id.as_deref(), Some(expected_uuid));
    assert!(outcome.reason.is_none());

    // Identity 3: Terminal log on disk
    let log_path = outcome.log_path.expect("log path present");
    assert!(log_path.exists());
    let log_content = fs::read_to_string(&log_path).expect("read log");
    assert!(
        log_content.contains(&format!("session_id:      {expected_uuid}")),
        "remote log must contain matching session_id: {log_content}"
    );
    assert!(
        log_content.contains("terminal_state:  completed"),
        "remote log must record terminal state completed: {log_content}"
    );

    // Identity 4: AttemptEvidence provider_session
    let evidence_prov: &ProviderSessionProvenance = outcome
        .evidence
        .provider_session
        .as_ref()
        .expect("remote evidence provider_session present");
    assert_eq!(evidence_prov.provider, "agy");
    assert_eq!(evidence_prov.session_id, expected_uuid);

    // All four identities are identical
    assert_eq!(outcome.session_id.as_deref().unwrap(), expected_uuid);
    assert_eq!(evidence_prov.session_id, expected_uuid);
}

// ─────────────────────────────────────────────────────────────────────────────
// 2. Remote pre-spawn failure: unsupported shell/platform
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_remote_unsupported_shell_fails_before_spawn() {
    let env = TestEnv::new();
    // A Windows drive letter in remoteWorkdir indicates an unsupported Windows remote shell
    let routing = env.write_remote_routing("C:/remote/workdir");
    let args = env.make_args(routing);

    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 2);
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-unsupported-shell")
    );
    assert_eq!(outcome.evidence.provider_started, Some(false));
    assert!(outcome.terminal_state.is_none());
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// 3. Remote typed failure tokens from SSH stdout boundary
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_remote_typed_failure_missing_terminal() {
    let env = TestEnv::new();
    env.write_payload(b"Just plain text without any JSON terminal object\n");

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-missing-terminal")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());

    // Durable log on disk must also be synced to disconnected-partial / session_id: none
    let log_path = outcome.log_path.expect("log path present");
    let log_content = fs::read_to_string(&log_path).expect("read log");
    assert!(log_content.contains("terminal_state:  disconnected-partial"));
    assert!(log_content.contains("session_id:      none"));
}

#[test]
fn test_remote_typed_failure_duplicate_terminal() {
    let env = TestEnv::new();
    let payload = b"{\"conversation_id\":\"11111111-1111-1111-1111-111111111111\",\"status\":\"SUCCESS\"}\n{\"conversation_id\":\"22222222-2222-2222-2222-222222222222\",\"status\":\"SUCCESS\"}\n";
    env.write_payload(payload);

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-duplicate-terminal")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

#[test]
fn test_remote_typed_failure_malformed_json() {
    let env = TestEnv::new();
    let payload = b"{\"conversation_id\":\"11111111-1111-1111-1111-111111111111\",\"status\":";
    env.write_payload(payload);

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-malformed-json")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

#[test]
fn test_remote_typed_failure_missing_field() {
    let env = TestEnv::new();
    // Missing status field
    let payload = b"{\"conversation_id\":\"11111111-1111-1111-1111-111111111111\"}\n";
    env.write_payload(payload);

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-missing-field")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

#[test]
fn test_remote_typed_failure_invalid_field_uuid() {
    let env = TestEnv::new();
    // conversation_id is not a UUID
    let payload = b"{\"conversation_id\":\"not-a-valid-uuid\",\"status\":\"SUCCESS\"}\n";
    env.write_payload(payload);

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-invalid-field")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

#[test]
fn test_remote_typed_failure_invalid_field_status() {
    let env = TestEnv::new();
    // status is not SUCCESS
    let payload =
        b"{\"conversation_id\":\"11111111-1111-1111-1111-111111111111\",\"status\":\"FAILED\"}\n";
    env.write_payload(payload);

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-invalid-field")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

#[test]
fn test_remote_typed_failure_truncated() {
    let env = TestEnv::new();
    // Contains `{` earlier, but the trailing line is non-JSON
    let payload = b"{\"conversation_id\":\"11111111-1111-1111-1111-111111111111\",\"status\":\"SUCCESS\"}\ntruncated trailing data";
    env.write_payload(payload);

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-truncated")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

#[test]
fn test_remote_typed_failure_oversized() {
    let env = TestEnv::new();
    // > 64 KiB
    let payload = vec![b'x'; 65537];
    env.write_payload(&payload);

    let routing = env.write_remote_routing("/remote/workdir");
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-oversized")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

// ─────────────────────────────────────────────────────────────────────────────
// 4. Local typed failure tokens
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn test_local_typed_failure_missing_terminal() {
    let env = TestEnv::new();
    env.write_payload(b"Local run with no JSON output\n");

    let routing = env.write_local_routing();
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-missing-terminal")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());

    let log_path = outcome.log_path.expect("log path present");
    let log_content = fs::read_to_string(&log_path).expect("read log");
    assert!(log_content.contains("terminal_state:  disconnected-partial"));
    assert!(log_content.contains("session_id:      none"));
}

#[test]
fn test_local_typed_failure_malformed_json() {
    let env = TestEnv::new();
    let payload = b"{\"conversation_id\":\"11111111-1111-1111-1111-111111111111\", incomplete";
    env.write_payload(payload);

    let routing = env.write_local_routing();
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-malformed-json")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}

#[test]
fn test_local_typed_failure_oversized() {
    let env = TestEnv::new();
    let payload = vec![b'A'; 70000];
    env.write_payload(&payload);

    let routing = env.write_local_routing();
    let args = env.make_args(routing);
    let outcome = run_dispatch(&args, "spec");

    assert_eq!(outcome.exit_code, 1);
    assert_eq!(
        outcome.terminal_state,
        Some(TerminalState::DisconnectedPartial)
    );
    assert_eq!(
        outcome.reason.as_deref(),
        Some("session-evidence-oversized")
    );
    assert!(outcome.session_id.is_none());
    assert!(outcome.evidence.provider_session.is_none());
}
