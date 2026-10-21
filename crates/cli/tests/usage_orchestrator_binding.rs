//! Orchestrator binding at `gal pipeline-preflight`, and collection of the recorded binding.
//!
//! The Codex home is a fixture root reached through `CODEX_HOME`, so no real session is read.
//!
//! `cargo test -p gal-cli --test usage_orchestrator_binding -- --nocapture`

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::{tempdir, TempDir};

#[path = "../src/commands/usage_capture.rs"]
#[allow(dead_code)]
mod usage_capture;

const PLAN: &str = "plan-orchestrator";
const SESSION_ID: &str = "orch-session-1";
/// Usage rows are dated after every preflight window, so they fall inside the open window.
const FUTURE: &str = "2099-01-01T00:00:00.000Z";

/// A git repository with one execution prompt, and a separate Codex home that holds one
/// orchestrator rollout whose working directory is that repository.
struct Fixture {
    repo: TempDir,
    codex_home: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let repo = tempdir().expect("repository tempdir");
        let codex_home = tempdir().expect("codex home tempdir");
        git(repo.path(), &["init", "-q", "-b", "main"]);
        git(
            repo.path(),
            &["config", "user.email", "fixture@example.com"],
        );
        git(repo.path(), &["config", "user.name", "CLI Fixture"]);
        git(repo.path(), &["config", "commit.gpgsign", "false"]);
        write_prompt(repo.path());
        write_orchestrator_rollout(codex_home.path(), repo.path());
        Self { repo, codex_home }
    }

    fn prompt(&self) -> PathBuf {
        self.repo
            .path()
            .join(".dev")
            .join("plans")
            .join(format!("{PLAN}.prompt.md"))
    }

    fn receipt(&self) -> PathBuf {
        self.repo
            .path()
            .join(".dev")
            .join("pipeline")
            .join(PLAN)
            .join("preflight.receipt.md")
    }

    fn gal(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .env("CODEX_HOME", self.codex_home.path())
            .args(args)
            .output()
            .expect("gal binary runs")
    }

    /// Runs preflight pinned to Codex with extra arguments. Returns the exit code and the receipt.
    fn preflight(&self, extra: &[&str]) -> (i32, String) {
        let prompt = self.prompt().to_string_lossy().into_owned();
        let mut args = vec!["pipeline-preflight", prompt.as_str(), "--host", "codex"];
        args.extend_from_slice(extra);
        let output = self.gal(&args);
        let receipt = fs::read_to_string(self.receipt()).expect("preflight writes its receipt");
        (output.status.code().expect("exit code"), receipt)
    }

    fn run_id(&self) -> String {
        let receipt = fs::read_to_string(self.receipt()).expect("receipt exists");
        field(&receipt, "run_id")
            .expect("receipt names its run")
            .to_owned()
    }
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("git is available");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Execution prompt that passes every preflight check. The task ID is composed here so that
/// no plan-task literal sits in source.
fn write_prompt(repo: &Path) {
    let task = format!("T-{:02}", 1usize);
    let prompt = format!(
        "## Status\n\nWorkflow: IMPLEMENT\nCurrent Task: {task}\n\n\
         ## Tasks\n\n- [ ] {task} — do something\n\n\
         ## Test Plan\n\n| ID | desc |\n| --- | --- |\n| preflight-pass | basic |\n"
    );
    let plans = repo.join(".dev").join("plans");
    fs::create_dir_all(&plans).expect("plans directory");
    fs::write(plans.join(format!("{PLAN}.prompt.md")), prompt).expect("prompt is written");
    fs::write(
        repo.join(".dev").join("state.md"),
        format!(
            "## Session Continuity\n\n| {PLAN} | .dev/plans/{PLAN}.prompt.md | in progress |\n"
        ),
    )
    .expect("state is written");
}

fn token_usage(input: u64, output: u64) -> Value {
    json!({
        "input_tokens": input,
        "cached_input_tokens": 0,
        "output_tokens": output,
        "reasoning_output_tokens": 0,
        "cache_write_input_tokens": 0,
        "total_tokens": input + output,
    })
}

/// A top-level Codex rollout. Its session identity is `SESSION_ID`, and it has one usage row.
fn write_orchestrator_rollout(codex_home: &Path, repo: &Path) {
    let sessions = codex_home.join("sessions");
    fs::create_dir_all(&sessions).expect("sessions directory");
    let cwd = repo.to_string_lossy().into_owned();
    let rows = [
        json!({
            "timestamp": FUTURE,
            "type": "session_meta",
            "payload": { "id": SESSION_ID, "cwd": cwd },
        }),
        json!({
            "timestamp": "2099-01-01T00:01:00.000Z",
            "type": "event_msg",
            "payload": {
                "type": "token_count",
                "info": {
                    "total_token_usage": token_usage(1000, 100),
                    "last_token_usage": token_usage(1000, 100),
                },
            },
        }),
    ];
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();
    fs::write(sessions.join(format!("rollout-{SESSION_ID}.jsonl")), text)
        .expect("rollout is written");
}

/// Value of a `name: value` line, or `None` when the receipt has no such line.
fn field<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{name}: ")))
}

fn overall(receipt: &str) -> &str {
    field(receipt, "overall").expect("receipt names its overall result")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn preflight_with_a_named_session_records_the_binding_and_keeps_the_result() {
    let fixture = Fixture::new();
    let session = format!("codex:{SESSION_ID}");

    let (bound_code, bound) = fixture.preflight(&["--host-session", &session]);
    let (unbound_code, unbound) = fixture.preflight(&[]);

    assert_eq!(
        overall(&bound),
        "pass",
        "the fixture prompt must pass preflight"
    );
    assert_eq!(
        bound_code, unbound_code,
        "the binding must not change the exit code"
    );
    assert_eq!(overall(&bound), overall(&unbound));
    assert_eq!(field(&bound, "orchestrator"), Some("bound"));
    assert_eq!(
        field(&bound, "orchestrator_session"),
        Some(session.as_str())
    );
    assert!(field(&bound, "orchestrator_window_started_at")
        .expect("bound receipt names its window start")
        .starts_with("epoch+"));
    assert_eq!(field(&unbound, "orchestrator"), Some("unavailable"));
    assert_eq!(
        field(&unbound, "orchestrator_reason"),
        Some("no --host-session was given")
    );
}

#[test]
fn preflight_records_the_reason_when_the_named_session_does_not_verify() {
    let fixture = Fixture::new();
    let (code, receipt) = fixture.preflight(&["--host-session", "codex:no-such-session"]);
    let (baseline_code, baseline) = fixture.preflight(&[]);

    assert_eq!(field(&receipt, "orchestrator"), Some("unavailable"));
    let reason = field(&receipt, "orchestrator_reason").expect("unavailable names a reason");
    assert!(reason.contains("did not verify"), "reason: {reason}");
    assert_eq!(
        code, baseline_code,
        "a failed binding must not change the exit code"
    );
    assert_eq!(overall(&receipt), overall(&baseline));
}

#[test]
fn collect_without_a_binding_file_reads_the_recorded_orchestrator_binding() {
    let fixture = Fixture::new();
    let session = format!("codex:{SESSION_ID}");
    let (code, _) = fixture.preflight(&["--host-session", &session]);
    assert_eq!(code, 0, "the bound preflight passes");
    let run_id = fixture.run_id();

    let collect = fixture.gal(&["usage", "collect", "--run", &run_id]);
    assert!(
        collect.status.success(),
        "collect failed: {}",
        stderr(&collect)
    );
    let collected = stdout(&collect);
    assert!(
        collected.contains(&format!("source: {session} (orchestrator)")),
        "collect output: {collected}"
    );
    assert!(
        collected.contains("observations read: 1"),
        "collect output: {collected}"
    );

    let report = fixture.gal(&["usage", "report", "--run", &run_id]);
    assert!(
        report.status.success(),
        "report failed: {}",
        stderr(&report)
    );
    let reported = stdout(&report);
    assert!(
        reported.contains(&format!("orchestrator: bound to {session} from epoch+")),
        "report header: {reported}"
    );
}

/// One output-token observation, the shape that an attempt log carries.
fn automatic_observation(output: u64) -> dispatch::usage::UsageObservation {
    serde_json::from_value(json!({
        "tokens": {
            "output": {
                "count": {
                    "value": { "state": "reported", "value": output },
                    "semantics": "delta",
                    "descendants": "own",
                    "window": { "start_unix_ms": 1000, "end_unix_ms": 2000 }
                },
                "relation": { "kind": "exclusive" }
            }
        }
    }))
    .expect("observation is valid")
}

#[test]
fn collect_without_a_binding_adds_orchestrator_rows_to_a_run_with_an_automatic_capture() {
    let fixture = Fixture::new();
    let session = format!("codex:{SESSION_ID}");
    let (code, _) = fixture.preflight(&["--host-session", &session]);
    assert_eq!(code, 0, "the bound preflight passes");
    let run_id = fixture.run_id();

    let store =
        pipeline::usage::UsageStore::open(&fixture.repo.path().join(".git")).expect("store opens");
    let attempt = usage_capture::AttemptUsage {
        attempt_id: "attempt-1".to_owned(),
        observations: vec![automatic_observation(7)],
        elapsed_ms: None,
    };
    usage_capture::capture_attempt(&store, &run_id, &attempt, usage_capture::unix_ms_now())
        .expect("automatic capture is stored");

    let collect = fixture.gal(&["usage", "collect", "--run", &run_id]);
    assert!(
        collect.status.success(),
        "collect failed: {}",
        stderr(&collect)
    );
    let collected = stdout(&collect);
    assert!(
        collected.contains(&format!("source: {session} (orchestrator)")),
        "collect output: {collected}"
    );
    assert!(
        collected.contains("observations read: 1, new to run: 1, in run: 2"),
        "collect output: {collected}"
    );

    let report = fixture.gal(&["usage", "report", "--run", &run_id, "--json"]);
    assert!(
        report.status.success(),
        "report failed: {}",
        stderr(&report)
    );
    let document: Value = serde_json::from_slice(&report.stdout).expect("report is JSON");
    assert_eq!(document["orchestrator"]["state"], "bound");
    assert_eq!(document["orchestrator"]["host_session"], session.as_str());
    assert!(document["orchestrator"]["window_started_at"]
        .as_str()
        .is_some_and(|started| started.starts_with("epoch+")));
    assert_eq!(document["observations"].as_array().map(Vec::len), Some(2));
}

#[test]
fn collect_refuses_an_unbound_run_and_the_report_names_the_reason() {
    let fixture = Fixture::new();
    let (code, _) = fixture.preflight(&[]);
    assert_eq!(code, 0, "the unbound preflight passes");
    let run_id = fixture.run_id();

    let collect = fixture.gal(&["usage", "collect", "--run", &run_id]);
    assert!(
        !collect.status.success(),
        "collect must refuse an unbound run"
    );
    assert!(
        stderr(&collect).contains("no bound orchestrator, because no --host-session was given"),
        "collect error: {}",
        stderr(&collect)
    );

    let report = fixture.gal(&["usage", "report", "--run", &run_id]);
    assert!(
        stdout(&report).contains("orchestrator: unavailable, because no --host-session was given"),
        "report header: {}",
        stdout(&report)
    );
}
