//! Usage capture on the dispatch paths that have no pipeline caller: standalone `gal dispatch`
//! and executor smoke. Two review cases cover the plan-level host-subagent open and close.
//!
//! The standalone cases run the real `gal` binary with a fake `agy` executor on `PATH`. The run is
//! pinless, so the capture prints its standalone run id on stderr. The test then reads that run back
//! through `gal usage report --json`, which proves the rows were stored under the printed run.
//!
//! The review cases run the real binary against a pinned host-subagent run. A host subagent runs
//! outside this process, and its attempt log holds no usage block. The open therefore stores nothing.
//! The verified close collects the child session's usage under the pinned run, and the report reads
//! those rows back. A close whose session is not verified stores nothing and prints the reason.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

/// Usage block the fake `agy` executor prints, in the form the agy parser maps.
const AGY_USAGE: &str = r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"SUCCESS","usage":{"input_tokens":120,"output_tokens":30,"thinking_tokens":5,"cache_read_tokens":40,"total_tokens":150}}"#;

/// Prefix of the stderr line that names the standalone run of one attempt.
const STANDALONE_LINE: &str = "gal usage: standalone run ";

struct Harness {
    home: TempDir,
    _bin: TempDir,
    repo: TempDir,
    path: std::ffi::OsString,
}

impl Harness {
    fn new() -> Self {
        let home = TempDir::new().expect("home dir");
        let config_dir = home.path().join(".gal/config");
        fs::create_dir_all(&config_dir).expect("config dir");
        fs::write(
            config_dir.join("config.json"),
            r#"{
              "executorRouting": {
                "executors": { "agy": "gemini-2.5-pro" },
                "pipeline": { "AUDITOR": { "executor": "agy", "effort": "medium" } }
              }
            }"#,
        )
        .expect("config file");

        let bin = TempDir::new().expect("bin dir");
        write_fake_agy(bin.path());
        let mut paths: Vec<PathBuf> = vec![bin.path().to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        let path = std::env::join_paths(paths).expect("PATH entries");

        let repo = TempDir::new().expect("repo dir");
        let init = Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .expect("git init runs");
        assert!(init.success(), "git init must succeed");

        Self {
            home,
            _bin: bin,
            repo,
            path,
        }
    }

    fn gal(&self, cwd: &Path, args: &[&str], stdin_text: Option<&str>) -> Output {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gal"));
        command
            .args(args)
            .current_dir(cwd)
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env("PATH", &self.path)
            // Readiness may look for agy login state outside the test home, so a token keeps
            // the fake agy ready on machines with no agy login.
            .env("GEMINI_API_KEY", "test-key")
            .stdin(if stdin_text.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().expect("gal starts");
        if let Some(text) = stdin_text {
            child
                .stdin
                .take()
                .expect("piped stdin")
                .write_all(text.as_bytes())
                .expect("spec written to stdin");
        }
        child.wait_with_output().expect("gal finishes")
    }

    /// Rows of one run, read through the report that users run.
    fn usage_rows(&self, run_id: &str) -> Vec<Value> {
        let output = self.gal(
            self.repo.path(),
            &["usage", "report", "--run", run_id, "--json"],
            None,
        );
        assert_eq!(
            output.status.code(),
            Some(0),
            "usage report must read run {run_id}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).expect("report is JSON");
        document["factual"]["rows"]
            .as_array()
            .expect("factual rows")
            .clone()
    }
}

#[cfg(not(windows))]
fn write_fake_agy(bin: &Path) {
    use std::os::unix::fs::PermissionsExt;

    let path = bin.join("agy");
    fs::write(&path, format!("#!/bin/sh\nprintf '%s\\n' '{AGY_USAGE}'\n")).expect("fake agy");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("fake agy mode");
}

#[cfg(windows)]
fn write_fake_agy(bin: &Path) {
    fs::write(
        bin.join("agy.cmd"),
        format!("@echo off\r\necho {AGY_USAGE}\r\n"),
    )
    .expect("fake agy");
}

/// Standalone run id and attempt id that the capture printed on stderr.
fn standalone_identity(stderr: &str) -> (String, String) {
    let line = stderr
        .lines()
        .find(|line| line.starts_with(STANDALONE_LINE))
        .unwrap_or_else(|| panic!("no standalone run line in stderr: {stderr}"));
    let words: Vec<&str> = line.split_whitespace().collect();
    // Layout: "gal usage: standalone run <run> for attempt <attempt>".
    assert_eq!(words.get(5), Some(&"for"), "unexpected line: {line}");
    (words[4].to_owned(), words[7].to_owned())
}

fn assert_rows_belong_to_attempt(rows: &[Value], attempt_id: &str) {
    assert!(!rows.is_empty(), "the run must list its usage rows");
    for row in rows {
        let observation = row["observation_id"].as_str().expect("observation id");
        assert!(
            observation.starts_with(attempt_id),
            "row {observation} must belong to attempt {attempt_id}"
        );
    }
}

#[test]
fn standalone_dispatch_prints_run_id_and_report_lists_its_rows() {
    let harness = Harness::new();
    let repo = harness.repo.path().to_path_buf();
    let spec = "## Tasks\n\n- [ ] T-SMOKE — Standalone dispatch usage probe\n";

    let output = harness.gal(
        &repo,
        &[
            "dispatch",
            "--phase",
            "audit",
            "--task",
            "T-SMOKE",
            "--workdir",
            repo.to_str().expect("utf-8 repo path"),
        ],
        Some(spec),
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let (run_id, attempt_id) = standalone_identity(&stderr);
    assert!(
        run_id.starts_with("standalone-"),
        "a pinless run id names the standalone scope: {run_id}"
    );
    assert_eq!(
        output.status.code(),
        Some(0),
        "a completed fake attempt exits 0: stderr={stderr}"
    );

    let rows = harness.usage_rows(&run_id);
    assert_rows_belong_to_attempt(&rows, &attempt_id);
    let input = rows
        .iter()
        .find(|row| row["metric"] == "tokens.input")
        .expect("input tokens row");
    assert_eq!(input["reported"], 120);
    assert_eq!(input["allocation"]["plan_id"], "standalone");
    assert_eq!(input["allocation"]["task_id"], "T-SMOKE");
}

/// Plan slug of the review fixture. Its prompt, source plan, and receipts sit under this name.
const REVIEW_PLAN: &str = "fixture-plan";
/// Preflight run that pins the review fixture to a host subagent.
const REVIEW_RUN: &str = "run-review";
/// Host session reference that the close verifies against the fixture transcript.
const REVIEW_SESSION: &str = "claude:reviewer";
/// Requirement rows of the source plan, which the review receipt must cover.
const REVIEW_ROW: &str = "| R2 | PASS — reviewed against the diff | PASS — file exists | PASS — wired | N/A — no trust boundary |\n";

/// A git repository with a plan pair and a finalize-review contract, and no executor route, so
/// `gal pipeline --phase finalize-review` takes the host-subagent path.
struct ReviewRepo {
    home: TempDir,
    repo: TempDir,
}

impl ReviewRepo {
    fn new() -> Self {
        let home = TempDir::new().expect("home dir");
        fs::create_dir_all(home.path().join(".gal/config")).expect("config dir");
        let repo = TempDir::new().expect("repo dir");
        let root = repo.path();
        for dir in ["plugins/gal-core/skills", "plugins/gal-core/agents"] {
            fs::create_dir_all(root.join(dir)).expect("plugin dir");
        }
        let command_dir = root.join("plugins/gal-core/commands/gal-finalize");
        fs::create_dir_all(&command_dir).expect("command dir");
        fs::write(
            command_dir.join("SKILL.template.md"),
            finalize_command_body(),
        )
        .expect("command template");
        fs::create_dir_all(root.join(".dev/plans")).expect("plans dir");
        fs::write(
            root.join(".dev/project.md"),
            "# Project\n\n<!-- gal:authoritative-check -->\n```json\n{\"command\":[\"cargo test --workspace\"]}\n```\n",
        )
        .expect("project file");
        fs::write(
            root.join(".dev/plans").join(format!("{REVIEW_PLAN}.md")),
            source_plan_text(),
        )
        .expect("source plan");
        fs::write(
            root.join(".dev/plans")
                .join(format!("{REVIEW_PLAN}.prompt.md")),
            prompt_text(),
        )
        .expect("execution prompt");
        let hooks = home.path().join("empty-hooks");
        fs::create_dir_all(&hooks).expect("hooks dir");
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "Fixture"],
            vec!["config", "commit.gpgsign", "false"],
            vec![
                "config",
                "core.hooksPath",
                hooks.to_str().expect("utf-8 hooks"),
            ],
            vec!["add", "."],
            vec!["commit", "-q", "-m", "fixture"],
        ] {
            let status = Command::new("git")
                .args(&args)
                .current_dir(root)
                .status()
                .expect("git runs");
            assert!(status.success(), "git {args:?} must succeed");
        }
        Self { home, repo }
    }

    fn root(&self) -> &Path {
        self.repo.path()
    }

    fn prompt(&self) -> PathBuf {
        self.root()
            .join(".dev/plans")
            .join(format!("{REVIEW_PLAN}.prompt.md"))
    }

    fn pin_host(&self) {
        let path = self
            .root()
            .join(format!(".dev/pipeline/{REVIEW_PLAN}/preflight.receipt.md"));
        fs::create_dir_all(path.parent().expect("pin parent")).expect("pin dir");
        fs::write(
            path,
            format!("host: claude\ndispatch_mode: host-subagent\nrun_id: {REVIEW_RUN}\n"),
        )
        .expect("pin receipt");
    }

    /// Runs `gal pipeline` on the execution prompt. The run is pinned, so no route is read.
    fn pipeline(&self, extra: &[&str]) -> Output {
        let prompt = self.prompt().display().to_string();
        let workdir = self.root().display().to_string();
        let mut args = vec![
            "pipeline",
            prompt.as_str(),
            "--phase",
            "finalize-review",
            "--workdir",
            workdir.as_str(),
        ];
        args.extend_from_slice(extra);
        self.gal(&args)
    }

    fn gal(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .args(args)
            .current_dir(self.root())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .output()
            .expect("gal runs")
    }

    /// The one host attempt that the open wrote. Its manifest names the attempt and its log.
    fn host_attempt(&self) -> Value {
        let dir = self.root().join(format!(
            ".dev/pipeline/{REVIEW_PLAN}/finalize-review/host-attempts"
        ));
        let path = fs::read_dir(dir)
            .expect("host attempts dir")
            .flatten()
            .map(|entry| entry.path())
            .find(|path| path.extension().is_some_and(|ext| ext == "json"))
            .expect("open writes one manifest");
        let envelope: Value = serde_json::from_slice(&fs::read(path).expect("manifest bytes"))
            .expect("manifest JSON");
        envelope["state"].clone()
    }

    /// Writes a reviewer transcript that is bound to the attempt, as the host would record it.
    fn write_host_session(&self, attempt: &Value) {
        let dir = self
            .home
            .path()
            .join(".claude/projects/demo/session/subagents");
        fs::create_dir_all(&dir).expect("session dir");
        let user = serde_json::json!({
            "type": "user",
            "cwd": self.root(),
            "timestamp": attempt["started_at"],
            "message": { "content": attempt["attempt_id"] },
        });
        // A response's usage is committed when a later response starts, so two responses are
        // written. The first one carries the stored counts.
        let response = |id: &str| {
            serde_json::json!({
                "type": "assistant",
                "timestamp": attempt["started_at"],
                "message": {
                    "id": id,
                    "model": attempt["model"],
                    "content": [{ "type": "tool_use", "name": "Read" }],
                    "usage": {
                        "input_tokens": 120,
                        "output_tokens": 30,
                        "cache_read_input_tokens": 40,
                        "cache_creation_input_tokens": 0
                    },
                },
            })
        };
        fs::write(
            dir.join("agent-reviewer.jsonl"),
            format!(
                "{user}\n{}\n{}\n",
                response("msg-review-1"),
                response("msg-review-2")
            ),
        )
        .expect("session transcript");
        fs::write(
            dir.join("agent-reviewer.meta.json"),
            r#"{"agentType":"general-purpose"}"#,
        )
        .expect("session meta");
    }

    fn write_review_receipt(&self) {
        let path = self.root().join(format!(
            ".dev/pipeline/{REVIEW_PLAN}/finalize-review/finalize-review.receipt.md"
        ));
        fs::create_dir_all(path.parent().expect("receipt parent")).expect("receipt dir");
        fs::write(path, review_body("host review", "DEGRADED_HOST_SUBAGENT"))
            .expect("review receipt");
    }

    /// Rows of one run, read through the report that users run. Returns the run's report output.
    fn report(&self, run_id: &str) -> Output {
        self.gal(&["usage", "report", "--run", run_id, "--json"])
    }

    /// Factual rows of one run, read through the report that users run.
    fn rows(&self, run_id: &str) -> Vec<Value> {
        let output = self.report(run_id);
        assert_eq!(
            output.status.code(),
            Some(0),
            "usage report must read run {run_id}: stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        let document: Value = serde_json::from_slice(&output.stdout).expect("report is JSON");
        document["factual"]["rows"]
            .as_array()
            .expect("factual rows")
            .clone()
    }
}

fn finalize_command_body() -> String {
    format!(
        "ORCHESTRATOR-DISPATCH-STEP lives outside the bounds.\n\n{}\n\n\
         REVIEWER-FRAGMENT-SENTINEL\n\n**Permitted inputs.** The plan Goal.\n\n\
         {}\n\nORCHESTRATOR-REPAIR-STEP merges and tags.\n",
        pipeline::task_spec::FINALIZE_REVIEW_CONTRACT_START,
        pipeline::task_spec::FINALIZE_REVIEW_CONTRACT_END
    )
}

fn source_plan_text() -> String {
    "# Plan: Demo\n\n\
     ## Approval\n\nSOURCE-APPROVAL-SENTINEL\n\n\
     ## Goal\n\nSOURCE-GOAL-SENTINEL land the demo.\n\n\
     ## Requirements\n\n- [ ] R1 — first requirement\n- [ ] R2 — second requirement\n\n\
     ## Success Criteria\n\n- It lands.\n\n\
     ## Test Plan\n\n| ID | Type |\n| --- | --- |\n| PROBE-A | unit |\n\n\
     ## Review Results\n\nSOURCE-REVIEW-SENTINEL\n"
        .to_owned()
}

fn prompt_text() -> String {
    // Task ids are composed at runtime, never written as literals.
    let task = format!("T-{:03}", 1);
    format!(
        "# Plan Prompt: Demo\n\n\
         ## Goal\n\nPROMPT-GOAL-SENTINEL\n\n\
         ## Requirements\n\n- [ ] R1 — first requirement\n- [ ] R2 — second requirement\n\n\
         ## Status\n\nWorkflow: DONE\nPROMPT-STATUS-SENTINEL\n\n\
         ### Handoff Notes\n\nPROMPT-HANDOFF-SENTINEL\n\n\
         ## Tasks\n\n- [x] {task} — finished work\n\n\
         ## Test Results\n\nPROMPT-TEST-RESULT-SENTINEL\n\n\
         ## Review Results\n\n### Architecture Review\n\nPROMPT-ARCH-REVIEW-SENTINEL\n"
    )
}

/// A complete review for the two fixture requirements, with its independence line.
fn review_body(note: &str, independence: &str) -> String {
    format!(
        "### Finalize Review 2026-10-08\n\n\
         | Requirement | L1 | L2 | L3 | L4 |\n\
         | --- | --- | --- | --- | --- |\n\
         | R1 | PASS — reviewed against the diff | PASS — file exists | PASS — wired | N/A — no trust boundary |\n\
         {REVIEW_ROW}\n\
         Findings: {note}\n\n\
         Review Independence: {independence}\n"
    )
}

#[test]
fn review_host_open_exits_zero_and_captures_without_a_standalone_run() {
    let fixture = ReviewRepo::new();
    fixture.pin_host();

    let output = fixture.pipeline(&["--dispatch", "host-subagent"]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        output.status.code(),
        Some(0),
        "a pinned host open exits 0: stderr={stderr}"
    );
    assert!(
        !stderr.contains(STANDALONE_LINE),
        "a pinned open names its run, so no standalone run is printed: {stderr}"
    );
    assert!(
        !stderr.contains("not captured") && !stderr.contains("capture failed"),
        "capture must not fail on the pinned open: {stderr}"
    );

    // Nothing launched, so the open has no usage to store and the pinned run has no snapshot.
    let report = fixture.report(REVIEW_RUN);
    assert_eq!(report.status.code(), Some(64), "the run has no snapshot");
    assert!(String::from_utf8_lossy(&report.stderr).contains("has no published snapshot"));
}

#[test]
fn review_host_close_exits_zero_and_captures_under_the_pinned_run() {
    let fixture = ReviewRepo::new();
    fixture.pin_host();
    assert_eq!(
        fixture
            .pipeline(&["--dispatch", "host-subagent"])
            .status
            .code(),
        Some(0)
    );
    let attempt = fixture.host_attempt();
    fixture.write_host_session(&attempt);
    fixture.write_review_receipt();

    let attempt_id = attempt["attempt_id"].as_str().expect("attempt id");
    let output = fixture.pipeline(&["--close", attempt_id, "--session", REVIEW_SESSION]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(
        output.status.code(),
        Some(0),
        "a verified host close exits 0: stderr={stderr}"
    );
    assert!(
        !stderr.contains(STANDALONE_LINE),
        "the pinned close names its run, so no standalone run is printed: {stderr}"
    );
    assert!(
        !stderr.contains("not captured") && !stderr.contains("capture failed"),
        "capture must not fail on the pinned close: {stderr}"
    );
    let log =
        fs::read_to_string(attempt["log_path"].as_str().expect("log path")).expect("host log");
    assert!(log.contains("terminal_state:  completed"), "{log}");
    // Capture reads only a usage block from the log, and a host attempt log has none.
    assert!(!log.contains("usage_extension:"), "{log}");

    assert!(
        stderr.contains(&format!("stored under run {REVIEW_RUN}")),
        "the close names the pinned run it stored under: {stderr}"
    );
    // The first response of the child transcript is committed, and its 120 input tokens are stored.
    let rows = fixture.rows(REVIEW_RUN);
    let input = rows
        .iter()
        .find(|row| row["metric"] == "tokens.input")
        .expect("input tokens row under the pinned run");
    assert_eq!(input["reported"], 120);
    // The child binding carries the host manifest's task, so its rows are allocated to that task.
    assert_eq!(input["allocation"]["kind"], "task");
    assert_eq!(input["allocation"]["task_id"], attempt["task"]);
}

#[test]
fn review_host_close_without_verified_session_stores_nothing_and_prints_reason() {
    let fixture = ReviewRepo::new();
    fixture.pin_host();
    assert_eq!(
        fixture
            .pipeline(&["--dispatch", "host-subagent"])
            .status
            .code(),
        Some(0)
    );
    let attempt = fixture.host_attempt();
    // No child transcript is written, so the session cannot be verified.
    fixture.write_review_receipt();

    let attempt_id = attempt["attempt_id"].as_str().expect("attempt id");
    let output = fixture.pipeline(&["--close", attempt_id, "--session", REVIEW_SESSION]);
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_ne!(
        output.status.code(),
        Some(0),
        "an unverified close is refused: stderr={stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "usage not collected for attempt {attempt_id}: the host session was not verified"
        )),
        "the reason is printed: {stderr}"
    );
    let report = fixture.report(REVIEW_RUN);
    assert_eq!(report.status.code(), Some(64), "nothing was stored");
}

#[test]
fn executor_smoke_attempt_stores_usage_under_its_standalone_run() {
    let harness = Harness::new();
    let repo = harness.repo.path().to_path_buf();
    let report_dir = repo.join("smoke-out");

    let output = harness.gal(
        &repo,
        &[
            "doctor",
            "--executor-smoke",
            "--executor",
            "agy",
            "--timeout",
            "30",
            "--report-dir",
            report_dir.to_str().expect("utf-8 report path"),
        ],
        None,
    );
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let (run_id, attempt_id) = standalone_identity(&stderr);

    let rows = harness.usage_rows(&run_id);
    assert_rows_belong_to_attempt(&rows, &attempt_id);
}
