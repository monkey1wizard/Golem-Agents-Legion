//! Integration tests for the plan-level `finalize-review` dispatch phase.
//!
//! Validates, with an isolated fake executor and no network:
//! 1. The phase resolves through the AUDITOR route only.
//! 2. A receipt-only review succeeds with a completed attempt log, provider session
//!    evidence, and a fresh non-empty receipt, and needs no product diff.
//! 3. A missing receipt argument, an empty or missing receipt, and missing session
//!    evidence never pass.
//! 4. A genuinely absent route yields `no-routing`, while every present but unusable
//!    route yields `routing-invalid` before any executor starts.
//! 5. A missing executor, a timeout, and a nonzero exit fail without any fallback.
//! 6. Existing phases keep their permissive routing behavior.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use tempfile::TempDir;

use dispatch::cli::DispatchArgs;
use dispatch::dispatch::TerminalState;
use dispatch::routing::{load_selected_route, SelectedRoute};
use dispatch::run::{run_dispatch, DispatchOutcome};
use dispatch::stage::Phase;

static DOUBLE_DIR: OnceLock<TempDir> = OnceLock::new();
static TEST_MUTEX: Mutex<()> = Mutex::new(());

const SESSION: &str = "5b0f7c1e-3a64-4c1b-9d52-0e8a1f6b7c24";
const RECEIPT_REL: &str = ".dev/pipeline/feat-x/finalize-review/finalize-review.receipt.md";

fn ensure_mock_double() -> &'static Path {
    DOUBLE_DIR
        .get_or_init(|| {
            let tmp = TempDir::new().expect("create double temp dir");
            let src_path = tmp.path().join("mock_double.rs");
            let src = r##"
fn main() {
    if let Ok(secs) = std::env::var("MOCK_SLEEP_SECS") {
        if let Ok(secs) = secs.parse::<u64>() {
            std::thread::sleep(std::time::Duration::from_secs(secs));
        }
    }
    if let Ok(receipt_path) = std::env::var("MOCK_RECEIPT_WRITE_PATH") {
        let body = std::env::var("MOCK_RECEIPT_BODY")
            .unwrap_or_else(|_| "# Finalize Review\nVerdict: PASS\n".to_string());
        let _ = std::fs::write(&receipt_path, body);
    }
    if let Ok(code_str) = std::env::var("MOCK_EXIT_CODE") {
        if let Ok(code) = code_str.parse::<i32>() {
            if code != 0 {
                std::process::exit(code);
            }
        }
    }
    if let Ok(file_path) = std::env::var("MOCK_PAYLOAD_FILE") {
        if let Ok(content) = std::fs::read(&file_path) {
            use std::io::Write;
            let _ = std::io::stdout().write_all(&content);
            let _ = std::io::stdout().flush();
            return;
        }
    }
    println!("{{\"conversation_id\":\"5b0f7c1e-3a64-4c1b-9d52-0e8a1f6b7c24\",\"status\":\"SUCCESS\"}}");
}
"##;
            fs::write(&src_path, src).expect("write mock double source");

            #[cfg(target_os = "windows")]
            let exe = "agy.exe";
            #[cfg(not(target_os = "windows"))]
            let exe = "agy";

            let status = Command::new("rustc")
                .arg(&src_path)
                .arg("-o")
                .arg(tmp.path().join(exe))
                .status()
                .expect("compile mock double with rustc");
            assert!(status.success(), "rustc compilation must succeed");
            tmp
        })
        .path()
}

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .expect("git must run");
    assert!(output.status.success(), "git failed: {args:?}");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn init_git_repo(path: &Path) {
    git(path, &["init", "-q", "-b", "main"]);
    git(path, &["config", "user.email", "tester@example.com"]);
    git(path, &["config", "user.name", "Tester"]);
    git(path, &["config", "commit.gpgsign", "false"]);
    fs::write(path.join("file.txt"), "initial content\n").unwrap();
    git(path, &["add", "."]);
    git(path, &["commit", "-q", "-m", "initial commit"]);
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
        std::env::set_var("PATH", std::env::join_paths(paths).expect("join paths"));
        std::env::set_var("GEMINI_API_KEY", "mock-gemini-key");
        for key in [
            "MOCK_PAYLOAD_FILE",
            "MOCK_RECEIPT_WRITE_PATH",
            "MOCK_RECEIPT_BODY",
            "MOCK_EXIT_CODE",
            "MOCK_SLEEP_SECS",
        ] {
            std::env::remove_var(key);
        }

        let workdir = TempDir::new().expect("create workdir");
        init_git_repo(workdir.path());

        Self {
            _lock: lock,
            workdir,
            saved_path,
            saved_key,
        }
    }

    fn receipt(&self) -> PathBuf {
        self.workdir.path().join(RECEIPT_REL)
    }

    /// Make the fake executor write `body` to the review receipt.
    fn executor_writes_receipt(&self, body: &str) {
        fs::create_dir_all(self.receipt().parent().unwrap()).unwrap();
        std::env::set_var("MOCK_RECEIPT_WRITE_PATH", self.receipt());
        std::env::set_var("MOCK_RECEIPT_BODY", body);
    }

    fn write_routing(&self, json: &str) -> PathBuf {
        let path = self.workdir.path().join("routing.json");
        fs::write(&path, json).unwrap();
        path
    }

    fn auditor_routing(&self) -> PathBuf {
        self.write_routing(
            r#"{
                "executorRouting": {
                    "executors": { "agy": "gemini-2.5-pro" },
                    "pipeline": { "AUDITOR": { "executor": "agy" } }
                }
            }"#,
        )
    }

    fn args(&self, phase: Phase, routing_path: PathBuf, receipt: Option<PathBuf>) -> DispatchArgs {
        DispatchArgs {
            phase,
            task: "finalize-review".to_string(),
            workdir: self.workdir.path().to_path_buf(),
            timeout_secs: 30,
            routing_path: Some(routing_path),
            receipt_path: receipt,
            log_dir_override: None,
            stdout_quiet: true,
            contract_provenance: None,
            worker: None,
        }
    }

    fn review_args(&self, routing_path: PathBuf) -> DispatchArgs {
        self.args(Phase::FinalizeReview, routing_path, Some(self.receipt()))
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        std::env::set_var("PATH", &self.saved_path);
        match &self.saved_key {
            Some(key) => std::env::set_var("GEMINI_API_KEY", key),
            None => std::env::remove_var("GEMINI_API_KEY"),
        }
        for key in [
            "MOCK_PAYLOAD_FILE",
            "MOCK_RECEIPT_WRITE_PATH",
            "MOCK_RECEIPT_BODY",
            "MOCK_EXIT_CODE",
            "MOCK_SLEEP_SECS",
        ] {
            std::env::remove_var(key);
        }
    }
}

fn describe(outcome: &DispatchOutcome) -> String {
    format!(
        "exit={} state={:?} reason={:?} session={:?} log={:?} provider_started={:?}",
        outcome.exit_code,
        outcome.terminal_state,
        outcome.reason,
        outcome.session_id,
        outcome.log_path,
        outcome.evidence.provider_started,
    )
}

fn assert_degraded_before_launch(outcome: &DispatchOutcome, reason: &str) {
    assert_eq!(outcome.exit_code, 2, "{}", describe(outcome));
    assert_eq!(
        outcome.reason.as_deref(),
        Some(reason),
        "{}",
        describe(outcome)
    );
    assert!(outcome.terminal_state.is_none(), "{}", describe(outcome));
    assert!(outcome.log_path.is_none(), "{}", describe(outcome));
    assert!(outcome.session_id.is_none(), "{}", describe(outcome));
    assert_eq!(outcome.evidence.provider_started, Some(false));
}

// phase and role

#[test]
fn phase_parses_as_auditor_and_old_phases_are_unchanged() {
    let phase = Phase::from_str("finalize-review").unwrap();
    assert_eq!(phase, Phase::FinalizeReview);
    assert_eq!(phase.role(), "AUDITOR");
    assert_eq!(Phase::Audit.role(), "AUDITOR");
    assert_eq!(Phase::Implement.role(), "CODER");
    assert_eq!(Phase::Test.role(), "TESTER");
    assert_eq!(Phase::Investigate.role(), "RESEARCHER");
    let message = Phase::from_str("review").unwrap_err().to_string();
    assert!(message.contains("finalize-review"), "{message}");
}

// receipt-only review with evidence

#[test]
fn receipt_only_review_succeeds_with_scoped_log_session_and_fresh_receipt() {
    let env = TestEnv::new();
    env.executor_writes_receipt("# Finalize Review\nVerdict: PASS\n");
    let head_before = git(env.workdir.path(), &["rev-parse", "HEAD"]);

    let outcome = run_dispatch(&env.review_args(env.auditor_routing()), "review spec");

    assert_eq!(outcome.exit_code, 0, "{}", describe(&outcome));
    assert_eq!(outcome.terminal_state, Some(TerminalState::Completed));
    assert!(outcome.reason.is_none(), "{}", describe(&outcome));
    assert_eq!(outcome.session_id.as_deref(), Some(SESSION));
    let provider = outcome.evidence.provider_session.as_ref().expect("session");
    assert_eq!(provider.provider, "agy");
    assert_eq!(provider.session_id, SESSION);

    let log = fs::read_to_string(outcome.log_path.as_ref().expect("log")).unwrap();
    assert!(log.contains("terminal_state:  completed"), "{log}");
    assert!(
        log.contains(&format!("session_id:      {SESSION}")),
        "{log}"
    );
    assert!(log.contains("finalize-review"), "{log}");

    let receipt = fs::read_to_string(env.receipt()).unwrap();
    assert!(receipt.contains("Verdict: PASS"), "{receipt}");

    // Receipt-only review: no tracked file changed and nothing was committed.
    assert_eq!(git(env.workdir.path(), &["rev-parse", "HEAD"]), head_before);
    assert_eq!(git(env.workdir.path(), &["diff", "--name-only"]), "");
}

#[test]
fn review_without_a_receipt_argument_is_rejected_before_launch() {
    let env = TestEnv::new();
    env.executor_writes_receipt("# Finalize Review\n");
    let args = env.args(Phase::FinalizeReview, env.auditor_routing(), None);

    let outcome = run_dispatch(&args, "review spec");

    assert_degraded_before_launch(&outcome, "pipeline-receipt-required");
    assert!(!env.receipt().exists());
}

#[test]
fn review_with_no_receipt_written_does_not_pass() {
    let env = TestEnv::new();

    let outcome = run_dispatch(&env.review_args(env.auditor_routing()), "review spec");

    assert_eq!(outcome.exit_code, 1, "{}", describe(&outcome));
    assert_eq!(outcome.terminal_state, Some(TerminalState::NoReceipt));
}

#[test]
fn review_with_an_empty_receipt_does_not_pass() {
    let env = TestEnv::new();
    env.executor_writes_receipt("");

    let outcome = run_dispatch(&env.review_args(env.auditor_routing()), "review spec");

    assert_eq!(outcome.exit_code, 1, "{}", describe(&outcome));
    assert_eq!(outcome.terminal_state, Some(TerminalState::NoReceipt));
}

#[test]
fn review_without_session_evidence_does_not_pass() {
    let env = TestEnv::new();
    env.executor_writes_receipt("# Finalize Review\nVerdict: PASS\n");
    let payload = env.workdir.path().join("payload.txt");
    fs::write(&payload, "plain text with no session object\n").unwrap();
    std::env::set_var("MOCK_PAYLOAD_FILE", &payload);

    let outcome = run_dispatch(&env.review_args(env.auditor_routing()), "review spec");

    assert_ne!(outcome.exit_code, 0, "{}", describe(&outcome));
    assert_ne!(outcome.terminal_state, Some(TerminalState::Completed));
    assert!(
        outcome.evidence.provider_session.is_none(),
        "{}",
        describe(&outcome)
    );
}

// absent route versus hard failure

#[test]
fn only_the_auditor_route_is_selected() {
    let env = TestEnv::new();
    env.executor_writes_receipt("# Finalize Review\n");
    let routing = env.write_routing(
        r#"{
            "executorRouting": {
                "executors": { "agy": "gemini-2.5-pro" },
                "pipeline": { "TESTER": { "executor": "agy" } }
            }
        }"#,
    );

    let outcome = run_dispatch(&env.review_args(routing), "review spec");

    assert_degraded_before_launch(&outcome, "no-routing");
    assert!(!env.receipt().exists());
}

#[test]
fn genuinely_absent_routing_reports_no_routing() {
    let cases = [
        ("missing file", None),
        ("empty object", Some("{}")),
        (
            "no pipeline group",
            Some(r#"{"executorRouting":{"executors":{"agy":"m"}}}"#),
        ),
        (
            "no AUDITOR key",
            Some(r#"{"executorRouting":{"pipeline":{"CODER":{"executor":"agy","model":"m"}}}}"#),
        ),
    ];
    for (label, json) in cases {
        let env = TestEnv::new();
        let routing = match json {
            Some(json) => env.write_routing(json),
            None => env.workdir.path().join("does-not-exist.json"),
        };
        let outcome = run_dispatch(&env.review_args(routing), "review spec");
        assert_degraded_before_launch(&outcome, "no-routing");
        assert!(!label.is_empty());
    }
}

#[test]
fn present_but_unusable_routing_is_a_hard_failure() {
    let cases = [
        ("invalid JSON", "{ not json"),
        ("non-object root", "[]"),
        ("executorRouting not an object", r#"{"executorRouting":[]}"#),
        (
            "pipeline not an object",
            r#"{"executorRouting":{"pipeline":"AUDITOR"}}"#,
        ),
        (
            "selected entry not an object",
            r#"{"executorRouting":{"pipeline":{"AUDITOR":"agy"}}}"#,
        ),
        (
            "selected entry has no executor",
            r#"{"executorRouting":{"pipeline":{"AUDITOR":{"model":"m"}}}}"#,
        ),
        (
            "selected entry resolves to no model",
            r#"{"executorRouting":{"pipeline":{"AUDITOR":{"executor":"agy"}}}}"#,
        ),
        (
            "selected entry half-configured for ssh",
            r#"{"executorRouting":{"pipeline":{"AUDITOR":{"executor":"agy","model":"m","sshTarget":"u@h"}}}}"#,
        ),
    ];
    for (label, json) in cases {
        let env = TestEnv::new();
        env.executor_writes_receipt("# Finalize Review\n");
        let outcome = run_dispatch(&env.review_args(env.write_routing(json)), "review spec");
        assert_degraded_before_launch(&outcome, "routing-invalid");
        assert!(!env.receipt().exists(), "{label}: executor must not start");
    }
}

#[test]
fn unreadable_routing_path_is_a_hard_failure() {
    let env = TestEnv::new();
    // A directory at the config path exists but cannot be read as a file.
    let routing = env.workdir.path().join("routing-dir");
    fs::create_dir(&routing).unwrap();

    let outcome = run_dispatch(&env.review_args(routing), "review spec");

    assert_degraded_before_launch(&outcome, "routing-invalid");
}

#[test]
fn malformed_unrelated_role_does_not_block_the_selected_auditor_route() {
    let env = TestEnv::new();
    env.executor_writes_receipt("# Finalize Review\nVerdict: PASS\n");
    let routing = env.write_routing(
        r#"{
            "executorRouting": {
                "executors": { "agy": "gemini-2.5-pro" },
                "pipeline": {
                    "CODER": "not-an-object",
                    "AUDITOR": { "executor": "agy" }
                }
            }
        }"#,
    );

    let outcome = run_dispatch(&env.review_args(routing), "review spec");

    assert_eq!(outcome.exit_code, 0, "{}", describe(&outcome));
    assert_eq!(outcome.terminal_state, Some(TerminalState::Completed));
}

#[test]
fn selected_route_loader_separates_absent_from_invalid() {
    let env = TestEnv::new();
    let absent = load_selected_route(&env.workdir.path().join("none.json"), "AUDITOR");
    assert_eq!(absent, SelectedRoute::Absent);

    match load_selected_route(&env.auditor_routing(), "AUDITOR") {
        SelectedRoute::Route(entry) => {
            assert_eq!(entry.executor, "agy");
            assert_eq!(entry.model, "gemini-2.5-pro");
        }
        other => panic!("expected a route, got {other:?}"),
    }

    let invalid = env.write_routing(r#"{"executorRouting":{"pipeline":{"AUDITOR":42}}}"#);
    assert!(matches!(
        load_selected_route(&invalid, "AUDITOR"),
        SelectedRoute::Invalid(_)
    ));
}

#[test]
fn existing_phase_keeps_permissive_routing_for_a_malformed_config() {
    let env = TestEnv::new();
    let routing = env.write_routing("{ not json");
    let args = env.args(Phase::Audit, routing, None);

    let outcome = run_dispatch(&args, "audit spec");

    assert_degraded_before_launch(&outcome, "no-routing");
}

#[test]
fn missing_executor_fails_without_fallback() {
    let env = TestEnv::new();
    env.executor_writes_receipt("# Finalize Review\n");
    let routing = env.write_routing(
        r#"{
            "executorRouting": {
                "pipeline": {
                    "AUDITOR": { "executor": "no-such-executor-xyz", "model": "m" }
                }
            }
        }"#,
    );

    let outcome = run_dispatch(&env.review_args(routing), "review spec");

    assert_eq!(outcome.exit_code, 2, "{}", describe(&outcome));
    assert_eq!(outcome.reason.as_deref(), Some("executor-unavailable"));
    assert!(outcome.terminal_state.is_none());
    assert!(!env.receipt().exists());
}

#[test]
fn executor_timeout_fails_without_fallback() {
    let env = TestEnv::new();
    std::env::set_var("MOCK_SLEEP_SECS", "20");
    let mut args = env.review_args(env.auditor_routing());
    args.timeout_secs = 1;

    let outcome = run_dispatch(&args, "review spec");

    assert_ne!(outcome.exit_code, 0, "{}", describe(&outcome));
    assert_ne!(outcome.terminal_state, Some(TerminalState::Completed));
    assert!(outcome.session_id.is_none(), "{}", describe(&outcome));
}

#[test]
fn nonzero_executor_exit_fails_even_with_a_receipt() {
    let env = TestEnv::new();
    env.executor_writes_receipt("# Finalize Review\nVerdict: PASS\n");
    std::env::set_var("MOCK_EXIT_CODE", "3");

    let outcome = run_dispatch(&env.review_args(env.auditor_routing()), "review spec");

    assert_ne!(outcome.exit_code, 0, "{}", describe(&outcome));
    assert_ne!(outcome.terminal_state, Some(TerminalState::Completed));
}
