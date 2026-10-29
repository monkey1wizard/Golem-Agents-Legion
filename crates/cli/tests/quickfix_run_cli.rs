//! `cargo test -p gal-cli --test quickfix_run_cli`
//!
//! Real-binary coverage for `gal quickfix-run`. `--open`: each refusal status leaves
//! no attempt directory behind, and one successful open writes a manifest and a
//! spec whose fields match what the attempt started from. `--close`: every terminal
//! status, driven by Claude Code and Codex session transcripts written into an
//! isolated home, a commit during the attempt, a fix to a file changed before the
//! open, a malformed manifest, the close lock, and a refused repeated close.

use pipeline::coordinator::CoordinatorState;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

const PASSING_COMMAND: &str = "git --version";
const FAILING_COMMAND: &str = "git rev-parse --verify refs/heads/no-such-branch";
/// The branch that `FAILING_COMMAND` looks for. Creating it makes the check pass.
const FAILING_BRANCH: &str = "no-such-branch";
const CLAUDE_AGENT: &str = "quickfix-agent";
const CLAUDE_MODEL: &str = "claude-sonnet-4-5";
const CODEX_ROLLOUT: &str = "rollout-child";
const CODEX_MODEL: &str = "gpt-6-astra";
const CODEX_EFFORT: &str = "high";
const CODEX_FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../dispatch/tests/fixtures/host_session/codex/rollout-desktop.jsonl"
);
const CODEX_FIXTURE_CWD: &str = "/workspace/project";
const CODEX_FIXTURE_ATTEMPT: &str = "attempt-7f30";
const CODEX_FIXTURE_DATE: &str = "2026-10-13T";

struct Fixture {
    repo: TempDir,
    home: TempDir,
}

impl Fixture {
    fn write(&self, relative: &str, text: &str) {
        let path = self.repo.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn write_config(&self, json: &str) {
        let dir = self.home.path().join(".gal").join("config");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("config.json"), json).unwrap();
    }

    fn open(&self, host: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .args(["quickfix-run", "--open", "--host", host])
            .output()
            .unwrap()
    }

    fn attempts_root(&self) -> PathBuf {
        self.repo.path().join(".dev/pipeline/quickfix")
    }

    fn attempt_dirs(&self) -> Vec<PathBuf> {
        match fs::read_dir(self.attempts_root()) {
            Ok(entries) => entries.flatten().map(|entry| entry.path()).collect(),
            Err(_) => Vec::new(),
        }
    }
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Rebases `path` from the fixture directory onto the root that Git reports, which is the root `gal` uses.
fn under_git_toplevel(repo: &Path, path: &Path) -> PathBuf {
    PathBuf::from(git(repo, &["rev-parse", "--show-toplevel"]))
        .join(path.strip_prefix(repo).unwrap())
}

fn project_md(command: &str) -> String {
    format!(
        "## Tech Stack\n<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"{command}\"]\n}}\n```\n\n## Protected Paths\n\n- `plugins/gal-core/conventions/` — workflow semantics\n"
    )
}

fn fixture_with_command(command: &str) -> Fixture {
    let fixture = Fixture {
        repo: tempfile::tempdir().unwrap(),
        home: tempfile::tempdir().unwrap(),
    };
    let repo = fixture.repo.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "t@t"]);
    git(repo, &["config", "user.name", "t"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    fixture.write(".dev/project.md", &project_md(command));
    fixture.write("a.txt", "one\ntwo\n");
    fixture.write("b.txt", "one\ntwo\n");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    fixture
}

fn failing_fixture() -> Fixture {
    fixture_with_command(FAILING_COMMAND)
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn assert_refused(fixture: &Fixture, output: &Output, status: &str) {
    let text = stdout(output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.lines().any(|line| line.starts_with(status)),
        "expected {status}, got: {text}"
    );
    assert!(
        fixture.attempt_dirs().is_empty(),
        "refusal created an attempt directory"
    );
}

#[test]
fn coordinator_lock_refuses_with_pipeline_active() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    fixture.write(".dev/pipeline/some-plan/.lock", "");

    // Pipeline activity is checked before the host, so an unsupported host still
    // reports the active pipeline.
    let output = fixture.open("gemini");

    assert_refused(&fixture, &output, "QUICKFIX: pipeline-active");
}

#[test]
fn unparsable_coordinator_state_counts_as_active() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    fixture.write(".dev/pipeline/some-plan/coordinator.json", "{ not json");

    let output = fixture.open("claude");

    assert_refused(&fixture, &output, "QUICKFIX: pipeline-active");
}

#[test]
fn coordinator_state_with_an_active_attempt_refuses() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    let mut state =
        CoordinatorState::new_legacy("some-plan", "hash", format!("T-{:02}", 1), "implement");
    state.claim_attempt("attempt".to_string()).unwrap();
    fixture.write(
        ".dev/pipeline/some-plan/coordinator.json",
        &serde_json::to_string(&state).unwrap(),
    );

    let output = fixture.open("claude");

    assert_refused(&fixture, &output, "QUICKFIX: pipeline-active");
}

#[test]
fn idle_coordinator_state_does_not_block_an_open() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    let state =
        CoordinatorState::new_legacy("some-plan", "hash", format!("T-{:02}", 1), "implement");
    fixture.write(
        ".dev/pipeline/some-plan/coordinator.json",
        &serde_json::to_string(&state).unwrap(),
    );

    let output = fixture.open("claude");

    assert!(output.status.success(), "{}", stdout(&output));
}

#[test]
fn unsupported_host_is_named_in_the_refusal() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");

    let output = fixture.open("gemini");

    assert_refused(&fixture, &output, "QUICKFIX: host-subagent-unsupported");
    assert!(stdout(&output).contains("gemini"), "{}", stdout(&output));
}

#[test]
fn clean_working_tree_refuses_with_no_change() {
    let fixture = failing_fixture();

    let output = fixture.open("claude");

    assert_refused(&fixture, &output, "QUICKFIX: no-change");
}

#[test]
fn passing_checks_refuse_with_checks_pass() {
    let fixture = fixture_with_command(PASSING_COMMAND);
    fixture.write("a.txt", "one\ntwo\nthree\n");

    let output = fixture.open("claude");

    assert_refused(&fixture, &output, "QUICKFIX: checks-pass");
}

/// Record a terminal status in the manifest of the attempt directory `dir`.
fn mark_terminal(dir: &Path) {
    let path = dir.join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["terminalStatus"] = json!("completed");
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
}

/// Write a preflight receipt in the real format, naming the prompt by file name only.
fn write_preflight_receipt(fixture: &Fixture, scope: &str, prompt: &str) {
    fixture.write(
        &format!(".dev/pipeline/{scope}/preflight.receipt.md"),
        &format!(
            "# pipeline-preflight receipt\n\nprompt: {prompt}\nresume-cursor: -\nhost: claude\n"
        ),
    );
}

fn write_prompt(fixture: &Fixture, name: &str, body: &str) {
    fixture.write(&format!(".dev/plans/{name}"), body);
}

fn assert_refused_keeping(fixture: &Fixture, output: &Output, status: &str, dirs: usize) {
    let text = stdout(output);
    assert!(!output.status.success(), "{text}");
    assert!(
        text.lines().any(|line| line.starts_with(status)),
        "expected {status}, got: {text}"
    );
    assert_eq!(
        fixture.attempt_dirs().len(),
        dirs,
        "refusal changed the attempt directories"
    );
}

#[test]
fn preflight_receipt_for_an_unfinished_prompt_refuses() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    write_prompt(
        &fixture,
        "feat-x.prompt.md",
        "# P\n\n## Status\n\nWorkflow: IMPLEMENT\n",
    );
    write_preflight_receipt(&fixture, "feat-x", "feat-x.prompt.md");

    let output = fixture.open("claude");

    assert_refused(&fixture, &output, "QUICKFIX: pipeline-active");
}

#[test]
fn preflight_receipt_for_a_prompt_without_a_workflow_line_refuses() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    write_prompt(
        &fixture,
        "feat-x.prompt.md",
        "# P\n\n## Status\n\nStep: 1\n",
    );
    write_preflight_receipt(&fixture, "feat-x", "feat-x.prompt.md");

    let output = fixture.open("claude");

    assert_refused(&fixture, &output, "QUICKFIX: pipeline-active");
}

#[test]
fn preflight_receipt_for_a_done_prompt_does_not_block() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    write_prompt(
        &fixture,
        "feat-x.prompt.md",
        "# P\n\n## Status\n\nWorkflow: DONE\n",
    );
    write_preflight_receipt(&fixture, "feat-x", "feat-x.prompt.md");

    let output = fixture.open("claude");

    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(fixture.attempt_dirs().len(), 1);
}

#[test]
fn preflight_receipt_for_a_missing_prompt_does_not_block() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    write_preflight_receipt(&fixture, "feat-x", "feat-gone.prompt.md");

    let output = fixture.open("claude");

    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(fixture.attempt_dirs().len(), 1);
}

#[test]
fn an_attempt_without_a_terminal_status_refuses_with_attempt_open() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    let first = fixture.open("claude");
    assert!(first.status.success(), "{}", stdout(&first));
    let dirs = fixture.attempt_dirs();
    assert_eq!(dirs.len(), 1);
    let name = dirs[0].file_name().unwrap().to_string_lossy().into_owned();

    let output = fixture.open("claude");

    assert_refused_keeping(&fixture, &output, "QUICKFIX: attempt-open", 1);
    let text = stdout(&output);
    assert!(text.contains(&name), "{text}");
    assert!(text.contains("--close"), "{text}");
    assert!(text.contains("close.lock"), "{text}");
}

#[test]
fn an_unparsable_manifest_refuses_with_attempt_open() {
    let fixture = failing_fixture();
    fixture.write("a.txt", "one\ntwo\nthree\n");
    fixture.write(
        ".dev/pipeline/quickfix/qf_broken/manifest.json",
        "{ not json",
    );

    let output = fixture.open("claude");

    assert_refused_keeping(&fixture, &output, "QUICKFIX: attempt-open", 1);
    assert!(stdout(&output).contains("qf_broken"), "{}", stdout(&output));
}

#[test]
fn closed_attempts_and_the_hook_directory_do_not_block() {
    let fixture = failing_fixture();
    fixture.write_config(r#"{"quickfix":{"maxFiles":10}}"#);
    fixture.write("a.txt", "one\ntwo\nthree\n");
    fixture.write(
        ".dev/pipeline/quickfix/qf_done/manifest.json",
        r#"{"terminalStatus":"completed"}"#,
    );
    fixture.write(".dev/pipeline/quickfix/hook/state.json", "{ not json");
    fixture.write(".dev/pipeline/quickfix/check.receipt.md", "overall: pass\n");

    let output = fixture.open("claude");

    assert!(output.status.success(), "{}", stdout(&output));
    assert_eq!(fixture.attempt_dirs().len(), 4);
}

#[test]
fn repeated_failure_stops_at_the_attempt_limit() {
    let fixture = failing_fixture();
    fixture.write_config(r#"{"quickfix":{"maxAttempts":1}}"#);
    fixture.write("a.txt", "one\ntwo\nthree\n");

    let first = fixture.open("claude");
    assert!(first.status.success(), "{}", stdout(&first));
    assert_eq!(fixture.attempt_dirs().len(), 1);

    let first_dir = fixture.attempt_dirs().remove(0);
    mark_terminal(&first_dir);

    let second = fixture.open("claude");

    let text = stdout(&second);
    assert!(!second.status.success(), "{text}");
    assert!(text.contains("QUICKFIX: attempt-limit"), "{text}");
    assert_eq!(
        fixture.attempt_dirs().len(),
        1,
        "the refused attempt must not create a directory"
    );
}

#[test]
fn successful_open_records_manifest_and_spec() {
    let fixture = failing_fixture();
    fixture.write_config(
        r#"{"quickfix":{"maxFiles":3,"maxLines":40,"maxAttempts":2},"hostSubagentModels":{"claude":{"light":{"model":"sonnet"}}}}"#,
    );
    fixture.write("a.txt", "one\ntwo\nthree\n");
    fixture.write("fresh.txt", "brand new\n");
    let head = git(fixture.repo.path(), &["rev-parse", "HEAD"]);
    let branch = git(fixture.repo.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);

    let output = fixture.open("claude");
    let text = stdout(&output);

    assert!(output.status.success(), "{text}");
    let line = text
        .lines()
        .find(|line| line.starts_with("QUICKFIX_ATTEMPT: "))
        .unwrap_or_else(|| panic!("no attempt line in: {text}"));
    let dirs = fixture.attempt_dirs();
    assert_eq!(dirs.len(), 1);
    let attempt_id = dirs[0].file_name().unwrap().to_string_lossy().into_owned();
    assert!(
        attempt_id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_'),
        "{attempt_id}"
    );
    let spec_path = dirs[0].join("spec.md");
    assert!(line.contains(&attempt_id), "{line}");
    assert!(line.contains("spec="), "{line}");
    let named = under_git_toplevel(fixture.repo.path(), &spec_path);
    let normalize = |text: &str| text.replace('\\', "/");
    assert!(
        normalize(line).contains(&normalize(&named.display().to_string())),
        "{line} should name {}",
        named.display()
    );
    assert!(line.ends_with("model=sonnet"), "{line}");

    let manifest: Value =
        serde_json::from_slice(&fs::read(dirs[0].join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["attemptId"], attempt_id.as_str());
    assert_eq!(manifest["host"], "claude");
    assert_eq!(manifest["originalBranch"], branch.as_str());
    assert_eq!(manifest["model"], "sonnet");
    let started = manifest["startedAt"].as_str().unwrap();
    assert!(
        started.len() == 20 && started.ends_with('Z') && started.contains('T'),
        "{started}"
    );
    assert!(manifest["startedAtEpochSeconds"].as_u64().unwrap() > 1_700_000_000);
    let failing = manifest["failingCommands"].as_array().unwrap();
    assert_eq!(failing.len(), 1);
    assert_eq!(failing[0]["command"], FAILING_COMMAND);
    assert!(failing[0]["output"].as_str().unwrap().len() <= 16 * 1024);
    assert_eq!(manifest["limits"]["maxFiles"], 3);
    assert_eq!(manifest["limits"]["maxLines"], 40);
    assert_eq!(manifest["limits"]["maxAttempts"], 2);
    assert!(manifest.get("dirtyFileHashes").is_none(), "{manifest}");
    assert_eq!(manifest["headAtOpen"], head.as_str());
    let open_tree = manifest["openTree"].as_str().unwrap();
    assert_eq!(
        git(fixture.repo.path(), &["cat-file", "-t", open_tree]),
        "tree"
    );

    let spec = fs::read_to_string(&spec_path).unwrap();
    assert!(spec.contains(&attempt_id), "{spec}");
    assert!(spec.contains("Fix only the failing findings"), "{spec}");
    assert!(spec.contains("Do not commit and do not push"), "{spec}");
    assert!(spec.contains("Do not edit anything under"), "{spec}");
    assert!(spec.contains(FAILING_COMMAND), "{spec}");
    assert!(!dirs[0].join("manifest.tmp").exists());
    assert!(!dirs[0].join("spec.tmp").exists());
    let leftovers: Vec<String> = fs::read_dir(&dirs[0])
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.ends_with(".tmp") || name.ends_with(".index"))
        .collect();
    assert!(leftovers.is_empty(), "{leftovers:?}");
}

fn slash_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

/// A transcript timestamp that is not before the attempt start.
fn session_timestamp(manifest: &Value) -> String {
    manifest["startedAt"]
        .as_str()
        .unwrap()
        .replace('Z', ".000Z")
}

impl Fixture {
    fn close(&self, attempt: &str, session: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .env_remove("CODEX_HOME")
            .args(["quickfix-run", "--close", attempt, "--session", session])
            .output()
            .unwrap()
    }

    /// Open an attempt on `host` and return its id.
    fn open_attempt(&self, host: &str) -> String {
        let output = self.open(host);
        let text = stdout(&output);
        assert!(output.status.success(), "{text}");
        text.lines()
            .find_map(|line| line.strip_prefix("QUICKFIX_ATTEMPT: "))
            .and_then(|rest| rest.split_whitespace().next())
            .map(str::to_owned)
            .unwrap_or_else(|| panic!("no attempt line in: {text}"))
    }

    fn manifest(&self, attempt: &str) -> Value {
        let path = self.attempts_root().join(attempt).join("manifest.json");
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn write_models(&self, claude_model: &str, codex_model: &str, codex_effort: &str) {
        self.write_config(
            &json!({
                "quickfix": {"maxFiles": 3, "maxLines": 50, "maxAttempts": 2},
                "hostSubagentModels": {
                    "claude": {"light": {"model": claude_model}},
                    "codex": {"light": {"model": codex_model, "effort": codex_effort}}
                }
            })
            .to_string(),
        );
    }

    /// Write a Claude Code subagent transcript that names `attempt`, then return
    /// its session reference.
    fn write_claude_session(&self, attempt: &str, model: &str, timestamp: &str) -> String {
        let dir = self
            .home
            .path()
            .join(".claude/projects/quickfix/session-1/subagents");
        fs::create_dir_all(&dir).unwrap();
        let lines = [
            json!({"type": "progress", "cwd": slash_path(self.repo.path()), "timestamp": timestamp}),
            json!({"type": "user", "message": {"content": [
                {"type": "text", "text": format!("Attempt {attempt}")}
            ]}}),
            json!({"type": "assistant", "message": {"model": model, "content": [
                {"type": "tool_use", "name": "Edit", "input": {}}
            ]}}),
        ];
        let text: String = lines.iter().map(|line| format!("{line}\n")).collect();
        fs::write(dir.join(format!("agent-{CLAUDE_AGENT}.jsonl")), text).unwrap();
        fs::write(
            dir.join(format!("agent-{CLAUDE_AGENT}.meta.json")),
            r#"{"agentType":"worker:quickfix"}"#,
        )
        .unwrap();
        format!("claude:{CLAUDE_AGENT}")
    }

    /// Write the recorded Codex rollout fixture with `attempt` in place of its own
    /// attempt id and this repository as its working directory. `future` moves its
    /// timestamps past the attempt start; without it the rollout predates the attempt.
    fn write_codex_session(&self, attempt: &str, future: bool) -> String {
        let mut text = fs::read_to_string(CODEX_FIXTURE)
            .unwrap()
            .replace(CODEX_FIXTURE_CWD, &slash_path(self.repo.path()))
            .replace(CODEX_FIXTURE_ATTEMPT, attempt);
        if future {
            text = text.replace(CODEX_FIXTURE_DATE, "2099-10-13T");
        }
        let dir = self.home.path().join(".codex/sessions/2099/10/13");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(format!("{CODEX_ROLLOUT}.jsonl")), text).unwrap();
        format!("codex:{CODEX_ROLLOUT}")
    }

    /// Make the failing authoritative check pass by creating the branch it names.
    fn make_check_pass(&self) {
        git(self.repo.path(), &["branch", FAILING_BRANCH]);
    }
}

/// Dirty `a.txt` before opening, so it is a file that was dirty at open.
fn open_fixture_with_dirty_file(host: &str, models: (&str, &str, &str)) -> (Fixture, String) {
    let fixture = failing_fixture();
    fixture.write_models(models.0, models.1, models.2);
    fixture.write("a.txt", "one\ntwo\nthree\n");
    let attempt = fixture.open_attempt(host);
    (fixture, attempt)
}

fn attempt_dir(fixture: &Fixture, attempt: &str) -> PathBuf {
    fixture.attempts_root().join(attempt)
}

fn default_models() -> (&'static str, &'static str, &'static str) {
    ("sonnet", CODEX_MODEL, CODEX_EFFORT)
}

fn assert_terminal(fixture: &Fixture, attempt: &str, output: &Output, status: &str) {
    let text = stdout(output);
    assert!(
        text.lines()
            .any(|line| line == format!("QUICKFIX: {status}")),
        "expected QUICKFIX: {status}, got: {text}"
    );
    assert_eq!(output.status.success(), status == "completed", "{text}");
    assert_eq!(fixture.manifest(attempt)["terminalStatus"], status);
}

#[test]
fn close_completes_a_claude_attempt_whose_check_passes() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");
    fixture.make_check_pass();
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );

    let output = fixture.close(&attempt, &session);
    let text = stdout(&output);

    assert_terminal(&fixture, &attempt, &output, "completed");
    let manifest = fixture.manifest(&attempt);
    assert_eq!(manifest["sessionId"], CLAUDE_AGENT);
    assert_eq!(manifest["observedModel"], CLAUDE_MODEL);
    assert_eq!(manifest["changedFiles"], json!(["fresh.txt"]));
    let close_tree = manifest["closeTree"].as_str().unwrap();
    assert_eq!(
        git(fixture.repo.path(), &["cat-file", "-t", close_tree]),
        "tree"
    );
    assert_ne!(manifest["closeTree"], manifest["openTree"]);
    assert!(
        text.lines().any(|line| line == "CHANGED: fresh.txt"),
        "{text}"
    );
    assert!(!attempt_dir(&fixture, &attempt).join("close.lock").exists());
    let receipt = fs::read_to_string(
        fixture
            .attempts_root()
            .join(&attempt)
            .join("close.receipt.md"),
    )
    .unwrap();
    assert!(receipt.contains("overall: pass"), "{receipt}");
}

#[test]
fn close_completes_a_codex_attempt_whose_check_passes() {
    let (fixture, attempt) = open_fixture_with_dirty_file("codex", default_models());
    fixture.write("fresh.txt", "brand new\n");
    fixture.make_check_pass();
    let session = fixture.write_codex_session(&attempt, true);

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "completed");
    assert_eq!(fixture.manifest(&attempt)["sessionId"], CODEX_ROLLOUT);
}

#[test]
fn close_with_a_failing_check_records_check_fail() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "check-fail");
}

#[test]
fn close_touching_a_protected_path_records_scope_fail() {
    let (fixture, attempt) = open_fixture_with_dirty_file("codex", default_models());
    fixture.write("plugins/gal-core/conventions/extra.md", "not allowed\n");
    fixture.make_check_pass();
    let session = fixture.write_codex_session(&attempt, true);

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "scope-fail");
}

#[test]
fn close_after_a_commit_during_the_attempt_records_head_moved() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");
    git(fixture.repo.path(), &["add", "-A", "--", "fresh.txt"]);
    git(fixture.repo.path(), &["commit", "-q", "-m", "sneaky"]);
    fixture.make_check_pass();
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "head-moved");
    assert!(fixture.manifest(&attempt).get("closeTree").is_none());
}

#[test]
fn close_counts_a_fix_to_a_file_that_was_changed_before_the_open() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("a.txt", "one\ntwo\nthree\nfour\n");
    fixture.make_check_pass();
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "completed");
    assert_eq!(fixture.manifest(&attempt)["changedFiles"], json!(["a.txt"]));
}

#[test]
fn close_ignores_edits_under_dev_and_reports_outside_changes() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write(".dev/notes.md", "scratch\n");
    fixture.write("fresh.txt", "brand new\n");
    fixture.make_check_pass();
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );

    let output = fixture.close(&attempt, &session);
    let text = stdout(&output);

    assert_terminal(&fixture, &attempt, &output, "completed");
    assert_eq!(
        fixture.manifest(&attempt)["changedFiles"],
        json!(["fresh.txt"])
    );
    assert!(text.lines().any(|line| line == "OUTSIDE: a.txt"), "{text}");
    assert!(!text.contains(".dev/notes.md"), "{text}");
}

#[test]
fn close_with_a_manifest_missing_a_field_records_session_invalid() {
    for field in [
        "headAtOpen",
        "openTree",
        "model",
        "host",
        "limits",
        "failingCommands",
        "originalBranch",
        "startedAtEpochSeconds",
        "attemptId",
    ] {
        let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
        fixture.write("fresh.txt", "brand new\n");
        let session = fixture.write_claude_session(
            &attempt,
            CLAUDE_MODEL,
            &session_timestamp(&fixture.manifest(&attempt)),
        );
        let mut manifest = fixture.manifest(&attempt);
        manifest.as_object_mut().unwrap().remove(field);
        fs::write(
            attempt_dir(&fixture, &attempt).join("manifest.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();

        let output = fixture.close(&attempt, &session);

        assert_terminal(&fixture, &attempt, &output, "session-invalid");
    }
}

#[test]
fn close_with_a_malformed_manifest_field_records_session_invalid() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );
    let mut manifest = fixture.manifest(&attempt);
    manifest["openTree"] = json!("not-a-tree");
    fs::write(
        attempt_dir(&fixture, &attempt).join("manifest.json"),
        serde_json::to_vec(&manifest).unwrap(),
    )
    .unwrap();

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "session-invalid");
}

#[test]
fn a_held_close_lock_refuses_the_close_and_is_left_alone() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );
    let lock = attempt_dir(&fixture, &attempt).join("close.lock");
    fs::write(&lock, "").unwrap();

    let output = fixture.close(&attempt, &session);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("close.lock"), "{stderr}");
    assert!(lock.exists(), "a lock held by someone else must stay");
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], Value::Null);
}

#[test]
fn the_close_lock_is_gone_after_an_error_exit_and_the_attempt_stays_open() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");

    // No session transcript exists yet, so the evidence is not ready.
    let output = fixture.close(&attempt, &format!("claude:{CLAUDE_AGENT}"));

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not ready"), "{stderr}");
    assert!(!attempt_dir(&fixture, &attempt).join("close.lock").exists());
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], Value::Null);

    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );
    let retry = fixture.close(&attempt, &session);
    assert_terminal(&fixture, &attempt, &retry, "check-fail");
}

#[test]
fn close_with_no_new_change_records_no_change() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "no-change");
}

#[test]
fn close_with_a_stale_claude_transcript_records_session_invalid() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");
    let session = fixture.write_claude_session(&attempt, CLAUDE_MODEL, "2000-01-01T00:00:00.000Z");

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "session-invalid");
}

#[test]
fn close_with_a_stale_codex_rollout_records_session_invalid() {
    let (fixture, attempt) = open_fixture_with_dirty_file("codex", default_models());
    fixture.write("fresh.txt", "brand new\n");
    let session = fixture.write_codex_session(&attempt, false);

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "session-invalid");
}

#[test]
fn close_with_a_different_claude_model_records_model_tier_mismatch() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    fixture.write("fresh.txt", "brand new\n");
    fixture.make_check_pass();
    let session = fixture.write_claude_session(
        &attempt,
        "claude-opus-4-1",
        &session_timestamp(&fixture.manifest(&attempt)),
    );

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "model-tier-mismatch");
}

#[test]
fn close_with_a_different_codex_effort_records_model_tier_mismatch() {
    let (fixture, attempt) =
        open_fixture_with_dirty_file("codex", ("sonnet", CODEX_MODEL, "medium"));
    fixture.write("fresh.txt", "brand new\n");
    fixture.make_check_pass();
    let session = fixture.write_codex_session(&attempt, true);

    let output = fixture.close(&attempt, &session);

    assert_terminal(&fixture, &attempt, &output, "model-tier-mismatch");
}

#[test]
fn repeated_close_is_refused_and_leaves_the_manifest_unchanged() {
    let (fixture, attempt) = open_fixture_with_dirty_file("codex", default_models());
    fixture.write("fresh.txt", "brand new\n");
    fixture.make_check_pass();
    let session = fixture.write_codex_session(&attempt, true);
    let first = fixture.close(&attempt, &session);
    assert_terminal(&fixture, &attempt, &first, "completed");
    assert!(!attempt_dir(&fixture, &attempt).join("close.lock").exists());
    let before = fixture.manifest(&attempt);

    let second = fixture.close(&attempt, &session);

    let text = stdout(&second);
    assert!(!second.status.success(), "{text}");
    assert!(text.contains("QUICKFIX: attempt-closed"), "{text}");
    assert_eq!(fixture.manifest(&attempt), before);
    assert!(!attempt_dir(&fixture, &attempt).join("close.lock").exists());
}

#[test]
fn close_requires_a_session_reference() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());

    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(fixture.repo.path())
        .args(["quickfix-run", "--close", &attempt])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--session is required"), "{stderr}");
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], Value::Null);
}

impl Fixture {
    fn land(&self, flag: &str, attempt: &str) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .args(["quickfix-run", flag, attempt])
            .output()
            .unwrap()
    }

    fn branch(&self) -> String {
        git(self.repo.path(), &["branch", "--show-current"])
    }

    fn branch_exists(&self, name: &str) -> bool {
        Command::new("git")
            .current_dir(self.repo.path())
            .args(["rev-parse", "--verify", "-q", &format!("refs/heads/{name}")])
            .output()
            .unwrap()
            .status
            .success()
    }
}

/// A closed `completed` attempt. Before the open, `a.txt` is edited (an untouched
/// pre-existing change) and `staged.txt` is staged (an unrelated staged file).
/// The attempt then edits `b.txt` and adds `fresh.txt`, plus scratch under `.dev`.
fn completed_attempt() -> (Fixture, String) {
    let fixture = failing_fixture();
    fixture.write_models("sonnet", CODEX_MODEL, CODEX_EFFORT);
    fixture.write("a.txt", "one\ntwo\nthree\n");
    fixture.write("staged.txt", "staged\n");
    git(fixture.repo.path(), &["add", "--", "staged.txt"]);
    let attempt = fixture.open_attempt("claude");
    fixture.write("b.txt", "one\ntwo\nfixed\n");
    fixture.write("fresh.txt", "brand new\n");
    fixture.write(".dev/notes.md", "scratch\n");
    fixture.make_check_pass();
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );
    let closed = fixture.close(&attempt, &session);
    assert_terminal(&fixture, &attempt, &closed, "completed");
    (fixture, attempt)
}

fn committed_attempt() -> (Fixture, String) {
    let (fixture, attempt) = completed_attempt();
    let output = fixture.land("--commit", &attempt);
    assert!(output.status.success(), "{}", stdout(&output));
    (fixture, attempt)
}

fn lines_of(text: &str) -> Vec<String> {
    text.lines().map(str::to_owned).collect()
}

#[test]
fn commit_puts_only_the_attempt_files_on_its_own_branch() {
    let (fixture, attempt) = completed_attempt();
    let repo = fixture.repo.path();
    let base = git(repo, &["rev-parse", "HEAD"]);

    let output = fixture.land("--commit", &attempt);
    let text = stdout(&output);

    assert!(output.status.success(), "{text}");
    let branch = format!("quickfix/{attempt}");
    let tip = git(repo, &["rev-parse", &branch]);
    assert!(text.lines().any(|l| l == "QUICKFIX: committed"), "{text}");
    assert!(
        text.lines().any(|l| l == format!("COMMIT: {tip}")),
        "{text}"
    );
    assert!(
        text.lines().any(|l| l == format!("BRANCH: {branch}")),
        "{text}"
    );
    assert_eq!(git(repo, &["rev-parse", &format!("{branch}^")]), base);
    let stat = git(repo, &["show", "--stat", "--format=", &branch]);
    let names = git(repo, &["show", "--name-only", "--format=", &branch]);
    assert_eq!(lines_of(&names), ["b.txt", "fresh.txt"], "{stat}");
    assert!(!names.contains(".dev"), "{stat}");
    let message = git(repo, &["log", "-1", "--format=%B", &branch]);
    assert!(message.contains(FAILING_COMMAND), "{message}");
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "committed");
    assert_eq!(fixture.manifest(&attempt)["commitId"], tip.as_str());
}

#[test]
fn commit_returns_to_the_original_branch_and_lists_what_it_left() {
    let (fixture, attempt) = completed_attempt();
    let repo = fixture.repo.path();

    let output = fixture.land("--commit", &attempt);
    let text = stdout(&output);

    assert!(output.status.success(), "{text}");
    assert_eq!(fixture.branch(), "main");
    assert!(text.lines().any(|l| l == "OUTSIDE: a.txt"), "{text}");
    assert!(text.lines().any(|l| l == "OUTSIDE: staged.txt"), "{text}");
    assert!(!text.contains(".dev/"), "{text}");
    assert!(!text.lines().any(|l| l == "OUTSIDE: b.txt"), "{text}");
    // The pre-existing change and the staged file are still uncommitted.
    assert_eq!(
        fs::read_to_string(repo.join("a.txt"))
            .unwrap()
            .replace("\r\n", "\n"),
        "one\ntwo\nthree\n"
    );
    assert_eq!(
        git(repo, &["diff", "--cached", "--name-only"]),
        "staged.txt"
    );
    assert!(!git(repo, &["ls-tree", "-r", "--name-only", "main"]).contains("staged.txt"));
    // The fix is on the branch only, until the merge.
    assert!(!repo.join("fresh.txt").exists());
}

#[test]
fn commit_is_refused_when_head_moved() {
    let (fixture, attempt) = completed_attempt();
    let repo = fixture.repo.path();
    git(repo, &["commit", "-q", "--allow-empty", "-m", "elsewhere"]);

    let output = fixture.land("--commit", &attempt);

    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("QUICKFIX: head-moved"),
        "{}",
        stdout(&output)
    );
    assert!(!fixture.branch_exists(&format!("quickfix/{attempt}")));
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "completed");
}

#[test]
fn commit_is_refused_when_the_snapshot_changed_after_close() {
    let (fixture, attempt) = completed_attempt();
    fixture.write("fresh.txt", "edited after close\n");

    let output = fixture.land("--commit", &attempt);

    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("QUICKFIX: snapshot-changed"),
        "{}",
        stdout(&output)
    );
    assert!(!fixture.branch_exists(&format!("quickfix/{attempt}")));
    assert_eq!(fixture.branch(), "main");
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "completed");
}

#[test]
fn commit_is_refused_unless_the_attempt_is_completed() {
    let (fixture, attempt) = open_fixture_with_dirty_file("claude", default_models());
    let open = fixture.land("--commit", &attempt);
    assert!(!open.status.success());
    assert!(
        stdout(&open).contains("QUICKFIX: not-completed status=open"),
        "{}",
        stdout(&open)
    );

    fixture.write("fresh.txt", "brand new\n");
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );
    let closed = fixture.close(&attempt, &session);
    assert_terminal(&fixture, &attempt, &closed, "check-fail");

    let failed = fixture.land("--commit", &attempt);

    assert!(!failed.status.success());
    assert!(
        stdout(&failed).contains("QUICKFIX: not-completed status=check-fail"),
        "{}",
        stdout(&failed)
    );
    assert!(!fixture.branch_exists(&format!("quickfix/{attempt}")));
}

#[test]
fn commit_is_refused_when_the_close_receipt_is_not_a_pass() {
    let (fixture, attempt) = completed_attempt();
    fs::write(
        attempt_dir(&fixture, &attempt).join("close.receipt.md"),
        "overall: fail\n",
    )
    .unwrap();

    let output = fixture.land("--commit", &attempt);

    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("QUICKFIX: receipt-not-pass"),
        "{}",
        stdout(&output)
    );
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "completed");
}

#[test]
fn a_git_failure_stops_the_commit_and_leaves_the_status_unchanged() {
    let (fixture, attempt) = completed_attempt();
    let repo = fixture.repo.path();
    let branch = format!("quickfix/{attempt}");
    git(repo, &["branch", &branch]);

    let output = fixture.land("--commit", &attempt);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("switch -c"), "{stderr}");
    assert_eq!(fixture.branch(), "main");
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "completed");
    assert!(repo.join("fresh.txt").exists());
}

#[test]
fn a_failed_commit_returns_to_the_original_branch() {
    let (fixture, attempt) = completed_attempt();
    let repo = fixture.repo.path();
    let hooks = repo.join(".git/hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::write(hooks.join("pre-commit"), "#!/bin/sh\nexit 1\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(hooks.join("pre-commit"), fs::Permissions::from_mode(0o755)).unwrap();
    }

    let output = fixture.land("--commit", &attempt);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("returned to main"), "{stderr}");
    assert_eq!(fixture.branch(), "main");
    assert!(!fixture.branch_exists(&format!("quickfix/{attempt}")));
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "completed");
    assert!(repo.join("fresh.txt").exists());
    assert_eq!(
        git(repo, &["diff", "--cached", "--name-only"]),
        "staged.txt"
    );
}

#[test]
fn merge_adds_the_branch_tip_to_the_original_history_and_keeps_the_branch() {
    let (fixture, attempt) = committed_attempt();
    let repo = fixture.repo.path();
    let branch = format!("quickfix/{attempt}");
    let tip = git(repo, &["rev-parse", &branch]);

    let output = fixture.land("--merge", &attempt);
    let text = stdout(&output);

    assert!(output.status.success(), "{text}");
    assert!(text.lines().any(|l| l == "QUICKFIX: merged"), "{text}");
    let ancestor = Command::new("git")
        .current_dir(repo)
        .args(["merge-base", "--is-ancestor", &tip, "HEAD"])
        .status()
        .unwrap();
    assert!(ancestor.success());
    assert_eq!(fixture.branch(), "main");
    assert!(fixture.branch_exists(&branch));
    assert!(repo.join("fresh.txt").exists());
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "merged");
}

#[test]
fn merge_is_refused_unless_the_attempt_is_committed() {
    let (fixture, attempt) = completed_attempt();

    let output = fixture.land("--merge", &attempt);

    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("QUICKFIX: not-committed status=completed"),
        "{}",
        stdout(&output)
    );
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "completed");
}

#[test]
fn merge_is_refused_on_another_branch() {
    let (fixture, attempt) = committed_attempt();
    let repo = fixture.repo.path();
    git(repo, &["switch", "-q", "-c", "other"]);

    let output = fixture.land("--merge", &attempt);

    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("QUICKFIX: wrong-branch expected=main actual=other"),
        "{}",
        stdout(&output)
    );
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "committed");
    assert!(!repo.join("fresh.txt").exists());
}

#[test]
fn commit_and_merge_require_an_attempt_id() {
    let (fixture, _attempt) = completed_attempt();
    for flag in ["--commit", "--merge"] {
        let output = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(fixture.repo.path())
            .args(["quickfix-run", flag])
            .output()
            .unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("requires an attempt id"), "{stderr}");
    }
}

#[test]
fn commit_records_a_file_the_attempt_removed_with_git_rm() {
    let fixture = failing_fixture();
    fixture.write_models("sonnet", CODEX_MODEL, CODEX_EFFORT);
    fixture.write("a.txt", "one\ntwo\nthree\n");
    let attempt = fixture.open_attempt("claude");
    let repo = fixture.repo.path();
    git(repo, &["rm", "-q", "--", "b.txt"]);
    fixture.write("fresh.txt", "brand new\n");
    fixture.make_check_pass();
    let session = fixture.write_claude_session(
        &attempt,
        CLAUDE_MODEL,
        &session_timestamp(&fixture.manifest(&attempt)),
    );
    let closed = fixture.close(&attempt, &session);
    assert_terminal(&fixture, &attempt, &closed, "completed");

    let output = fixture.land("--commit", &attempt);

    assert!(output.status.success(), "{}", stdout(&output));
    let branch = format!("quickfix/{attempt}");
    let status = git(repo, &["show", "--name-status", "--format=", &branch]);
    let entries: Vec<Vec<&str>> = status
        .lines()
        .map(|line| line.split_whitespace().collect())
        .collect();
    assert!(entries.contains(&vec!["D", "b.txt"]), "{status}");
    assert!(entries.contains(&vec!["A", "fresh.txt"]), "{status}");
    assert_eq!(fixture.branch(), "main");
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "committed");
}

#[test]
fn merge_is_refused_while_another_merge_is_in_progress() {
    let (fixture, attempt) = committed_attempt();
    let repo = fixture.repo.path();
    let head = git(repo, &["rev-parse", "HEAD"]);
    let marker = repo.join(".git").join("MERGE_HEAD");
    fs::write(&marker, format!("{head}\n")).unwrap();

    let output = fixture.land("--merge", &attempt);

    assert!(!output.status.success());
    assert!(
        stdout(&output).contains("QUICKFIX: merge-in-progress"),
        "{}",
        stdout(&output)
    );
    assert!(marker.exists());
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "committed");
    assert_eq!(git(repo, &["rev-parse", "HEAD"]), head);
}

#[test]
fn a_merge_conflict_is_aborted_and_the_status_stays_committed() {
    let (fixture, attempt) = committed_attempt();
    let repo = fixture.repo.path();
    fixture.write("fresh.txt", "written on main\n");
    git(repo, &["add", "--", "fresh.txt"]);
    git(
        repo,
        &["commit", "-q", "-m", "main side", "--", "fresh.txt"],
    );
    let head = git(repo, &["rev-parse", "HEAD"]);

    let output = fixture.land("--merge", &attempt);

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("merge"), "{stderr}");
    assert!(!repo.join(".git").join("MERGE_HEAD").exists());
    assert_eq!(git(repo, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read_to_string(repo.join("fresh.txt"))
            .unwrap()
            .replace("\r\n", "\n"),
        "written on main\n"
    );
    assert_eq!(fixture.branch(), "main");
    assert_eq!(fixture.manifest(&attempt)["terminalStatus"], "committed");
}
