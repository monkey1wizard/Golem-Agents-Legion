//! `cargo test -p gal-cli --test quickfix_hook_cli`
//!
//! Real-binary coverage for `gal quickfix-hook --host <host> stop` with recorded
//! Stop payloads of Claude Code and Codex. A failing authoritative check opens an
//! attempt and the block response names the spec path. Hook off, an unchanged
//! snapshot, an open refusal, a host mismatch, malformed input, and oversized
//! input all allow the stop, with a stderr reason where the case is a fault.

use serde_json::{json, Value};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

const FAILING_COMMAND: &str = "git rev-parse --verify refs/heads/no-such-branch";
const MAX_EVENT_BYTES: usize = 1024 * 1024;

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

    fn hook_on(&self) {
        self.write_config(r#"{"quickfix": {"hook": true}}"#);
    }

    fn run(&self, host: &str, input: &[u8]) -> Output {
        let mut child = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(self.repo.path())
            .env("HOME", self.home.path())
            .env("USERPROFILE", self.home.path())
            .args(["quickfix-hook", "--host", host, "stop"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let mut stdin = child.stdin.take().unwrap();
        let bytes = input.to_vec();
        let writer = std::thread::spawn(move || {
            // The command stops reading at its limit, so a broken pipe is expected.
            let _ = stdin.write_all(&bytes);
        });
        let output = child.wait_with_output().unwrap();
        writer.join().unwrap();
        output
    }

    fn stop(&self, host: &str, payload: &Value) -> Output {
        self.run(host, payload.to_string().as_bytes())
    }

    fn attempt_dirs(&self) -> Vec<PathBuf> {
        match fs::read_dir(self.repo.path().join(".dev/pipeline/quickfix")) {
            Ok(entries) => entries
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.file_name().is_some_and(|name| name != "hook"))
                .collect(),
            Err(_) => Vec::new(),
        }
    }
}

fn git(repo: &Path, args: &[&str]) {
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
}

/// Rebases `path` from the fixture directory onto the root that Git reports, which is the root `gal` uses.
fn under_git_toplevel(repo: &Path, path: &Path) -> PathBuf {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    let top = String::from_utf8(output.stdout).unwrap();
    PathBuf::from(top.trim()).join(path.strip_prefix(repo).unwrap())
}

fn project_md(command: &str) -> String {
    format!(
        "## Tech Stack\n<!-- gal:authoritative-check -->\n```json\n{{\n  \"command\": [\"{command}\"]\n}}\n```\n\n## Protected Paths\n\n- `plugins/gal-core/conventions/` — workflow semantics\n"
    )
}

/// A repository whose authoritative check fails and whose working tree has one
/// uncommitted change.
fn failing_fixture() -> Fixture {
    let fixture = Fixture {
        repo: tempfile::tempdir().unwrap(),
        home: tempfile::tempdir().unwrap(),
    };
    let repo = fixture.repo.path();
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["config", "user.email", "t@t"]);
    git(repo, &["config", "user.name", "t"]);
    git(repo, &["config", "commit.gpgsign", "false"]);
    fixture.write(".dev/project.md", &project_md(FAILING_COMMAND));
    fixture.write("a.txt", "one\ntwo\n");
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "fixture"]);
    fixture.write("a.txt", "one\ntwo\nthree\n");
    fixture
}

fn claude_payload(fixture: &Fixture, session: &str) -> Value {
    json!({
        "session_id": session,
        "transcript_path": "/home/user/.claude/projects/p/transcript.jsonl",
        "cwd": fixture.repo.path().to_string_lossy(),
        "permission_mode": "default",
        "hook_event_name": "Stop",
        "stop_hook_active": false
    })
}

fn codex_payload(fixture: &Fixture, session: &str) -> Value {
    json!({
        "session_id": session,
        "turn_id": "turn-1",
        "transcript_path": "/home/user/.codex/sessions/rollout.jsonl",
        "cwd": fixture.repo.path().to_string_lossy(),
        "hook_event_name": "Stop",
        "model": "gpt-6-astra",
        "stop_hook_active": false,
        "last_assistant_message": "Done."
    })
}

fn stdout_json(output: &Output) -> Value {
    let text = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|error| panic!("stdout '{text}': {error}"))
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_allow(output: &Output) {
    assert!(output.status.success(), "{}", stderr(output));
    let value = stdout_json(output);
    assert_eq!(value["continue"], json!(true), "{value}");
    assert!(value.get("decision").is_none(), "{value}");
}

fn assert_block_names_spec(fixture: &Fixture, output: &Output) {
    assert!(output.status.success(), "{}", stderr(output));
    let value = stdout_json(output);
    assert_eq!(value["decision"], json!("block"), "{value}");
    let reason = value["reason"].as_str().unwrap();
    let dirs = fixture.attempt_dirs();
    assert_eq!(dirs.len(), 1, "expected one attempt: {dirs:?}");
    let spec = dirs[0].join("spec.md");
    assert!(spec.is_file());
    let named = under_git_toplevel(fixture.repo.path(), &spec);
    assert!(
        reason
            .replace('\\', "/")
            .contains(&named.display().to_string().replace('\\', "/")),
        "reason does not name the spec path: {reason}"
    );
    assert!(reason.contains("gal quickfix-run --close"), "{reason}");
    assert!(reason.contains("model"), "{reason}");
}

#[test]
fn claude_payload_with_a_failing_check_blocks_naming_the_spec() {
    let fixture = failing_fixture();
    fixture.hook_on();

    let output = fixture.stop("claude", &claude_payload(&fixture, "session-claude"));

    assert_block_names_spec(&fixture, &output);
}

#[test]
fn codex_payload_with_a_failing_check_blocks_naming_the_spec() {
    let fixture = failing_fixture();
    fixture.hook_on();

    let output = fixture.stop("codex", &codex_payload(&fixture, "session-codex"));

    assert_block_names_spec(&fixture, &output);
}

#[test]
fn hook_off_allows_without_running_a_check() {
    let fixture = failing_fixture();
    fixture.write_config(r#"{"quickfix": {"hook": false}}"#);

    let output = fixture.stop("claude", &claude_payload(&fixture, "session-off"));

    assert_allow(&output);
    assert!(stderr(&output).is_empty(), "{}", stderr(&output));
    assert!(fixture.attempt_dirs().is_empty());
    assert!(!fixture
        .repo
        .path()
        .join(".dev/pipeline/quickfix/hook")
        .exists());
}

#[test]
fn missing_config_defaults_to_hook_off() {
    let fixture = failing_fixture();

    let output = fixture.stop("codex", &codex_payload(&fixture, "session-default"));

    assert_allow(&output);
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn unchanged_snapshot_allows_without_a_new_check() {
    let fixture = failing_fixture();
    fixture.hook_on();
    let payload = claude_payload(&fixture, "session-snapshot");
    // A pipeline lock makes the first call a refusal, which still stores the snapshot.
    fixture.write(".dev/pipeline/some-plan/.lock", "");
    assert_allow(&fixture.stop("claude", &payload));
    assert!(fixture.attempt_dirs().is_empty());
    fs::remove_file(fixture.repo.path().join(".dev/pipeline/some-plan/.lock")).unwrap();

    // Without the stored snapshot this call would open an attempt and block.
    let second = fixture.stop("claude", &payload);
    assert_allow(&second);
    assert!(fixture.attempt_dirs().is_empty());

    fixture.write("a.txt", "one\ntwo\nthree\nfour\n");
    let third = fixture.stop("claude", &payload);
    assert_block_names_spec(&fixture, &third);
}

#[test]
fn snapshots_are_kept_per_session() {
    let fixture = failing_fixture();
    fixture.hook_on();
    fixture.write(".dev/pipeline/some-plan/.lock", "");
    assert_allow(&fixture.stop("claude", &claude_payload(&fixture, "session-one")));
    fs::remove_file(fixture.repo.path().join(".dev/pipeline/some-plan/.lock")).unwrap();

    let other = fixture.stop("claude", &claude_payload(&fixture, "session-two"));

    assert_block_names_spec(&fixture, &other);
}

#[test]
fn unsafe_session_id_stays_inside_the_hook_directory() {
    let fixture = failing_fixture();
    fixture.hook_on();
    fixture.write(".dev/pipeline/some-plan/.lock", "");

    let output = fixture.stop("claude", &claude_payload(&fixture, "../../escape/..\\x"));

    assert_allow(&output);
    let hook_dir = fixture.repo.path().join(".dev/pipeline/quickfix/hook");
    let entries: Vec<_> = fs::read_dir(&hook_dir).unwrap().flatten().collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
    assert!(!fixture.repo.path().join(".dev/escape").exists());
}

#[test]
fn pipeline_active_refusal_allows_for_codex() {
    let fixture = failing_fixture();
    fixture.hook_on();
    fixture.write(".dev/pipeline/some-plan/.lock", "");

    let output = fixture.stop("codex", &codex_payload(&fixture, "session-pipeline"));

    assert_allow(&output);
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn open_attempt_refusal_allows() {
    let fixture = failing_fixture();
    fixture.hook_on();
    assert_block_names_spec(
        &fixture,
        &fixture.stop("claude", &claude_payload(&fixture, "session-first")),
    );

    // A different session has no stored snapshot, so the open code refuses.
    let output = fixture.stop("claude", &claude_payload(&fixture, "session-second"));

    assert_allow(&output);
    assert_eq!(fixture.attempt_dirs().len(), 1);
}

#[test]
fn passing_checks_refusal_allows() {
    let fixture = failing_fixture();
    fixture.hook_on();
    fixture.write(".dev/project.md", &project_md("git --version"));
    git(fixture.repo.path(), &["add", ".dev/project.md"]);
    git(fixture.repo.path(), &["commit", "-q", "-m", "passing"]);
    fixture.write("a.txt", "one\ntwo\nthree\nfour\n");

    let output = fixture.stop("claude", &claude_payload(&fixture, "session-pass"));

    assert_allow(&output);
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn claude_payload_with_codex_host_allows_with_a_mismatch_reason() {
    let fixture = failing_fixture();
    fixture.hook_on();

    let output = fixture.stop("codex", &claude_payload(&fixture, "session-mismatch"));

    assert_allow(&output);
    assert!(stderr(&output).contains("mismatch"), "{}", stderr(&output));
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn codex_payload_with_claude_host_allows_with_a_mismatch_reason() {
    let fixture = failing_fixture();
    fixture.hook_on();

    let output = fixture.stop("claude", &codex_payload(&fixture, "session-mismatch"));

    assert_allow(&output);
    assert!(stderr(&output).contains("mismatch"), "{}", stderr(&output));
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn other_event_and_missing_session_allow_with_a_reason() {
    let fixture = failing_fixture();
    fixture.hook_on();

    let mut other_event = claude_payload(&fixture, "session-event");
    other_event["hook_event_name"] = json!("PreToolUse");
    let output = fixture.stop("claude", &other_event);
    assert_allow(&output);
    assert!(stderr(&output).contains("Stop"), "{}", stderr(&output));

    let mut no_session = codex_payload(&fixture, "x");
    no_session.as_object_mut().unwrap().remove("session_id");
    let output = fixture.stop("codex", &no_session);
    assert_allow(&output);
    assert!(
        stderr(&output).contains("session_id"),
        "{}",
        stderr(&output)
    );
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn malformed_input_allows_with_a_reason() {
    let fixture = failing_fixture();
    fixture.hook_on();

    for input in [&b"{ not json"[..], b"", b"[1, 2]"] {
        let output = fixture.run("claude", input);
        assert_allow(&output);
        assert!(!stderr(&output).is_empty());
    }
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn oversized_input_allows_with_a_reason() {
    let fixture = failing_fixture();
    fixture.hook_on();
    let mut input = br#"{"session_id":"s","pad":""#.to_vec();
    input.resize(MAX_EVENT_BYTES + 16, b'a');
    input.extend_from_slice(b"\"}");

    let output = fixture.run("claude", &input);

    assert_allow(&output);
    assert!(stderr(&output).contains("exceeds"), "{}", stderr(&output));
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn unreadable_config_allows_with_a_reason() {
    let fixture = failing_fixture();
    fixture.write_config("{ not json");

    let output = fixture.stop("claude", &claude_payload(&fixture, "session-config"));

    assert_allow(&output);
    assert!(stderr(&output).contains("config"), "{}", stderr(&output));
    assert!(fixture.attempt_dirs().is_empty());
}

#[test]
fn bad_arguments_are_a_usage_error() {
    let fixture = failing_fixture();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(fixture.repo.path())
        .args(["quickfix-hook", "--host", "claude"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(!stderr(&output).is_empty());
}
