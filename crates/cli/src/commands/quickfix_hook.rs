//! `gal quickfix-hook --host <claude|codex> stop` — Stop adapter for the quickfix lane.
//!
//! The host runs this command when a turn ends and passes the Stop payload on
//! stdin. The adapter never blocks a stop by accident: every disabled, refused,
//! mismatched, malformed, oversized, or failing case allows the stop, and the
//! cases that deserve attention write the reason to stderr. Only a successfully
//! opened quickfix attempt produces a block response, which names the spec path,
//! the model, and the close command.
//!
//! The adapter keeps one working-tree snapshot per `session_id` under
//! `.dev/pipeline/quickfix/hook/`. When the snapshot taken at a Stop equals the
//! stored one, nothing changed since the previous check and the stop is allowed
//! without running any check.

use super::quickfix_check::{load_quickfix_config, run_git, run_git_env};
use super::quickfix_run::{open_attempt, OpenOutcome};
use gal_engine::ExitCode;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_EVENT_BYTES: usize = 1024 * 1024;
const MAX_SESSION_NAME: usize = 64;
const HOOK_DIR: &str = ".dev/pipeline/quickfix/hook";
const DEV_EXCLUDE_PATHSPEC: &str = ":(exclude).dev";
const STOP_EVENT: &str = "Stop";
const SNAPSHOT_SUFFIX: &str = ".tree";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookHost {
    Claude,
    Codex,
}

impl HookHost {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            _ => None,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

/// What the hook tells the host.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Decision {
    Allow,
    Block(String),
}

impl Decision {
    fn render(&self) -> Value {
        match self {
            Self::Allow => json!({"continue": true}),
            Self::Block(reason) => json!({"decision": "block", "reason": reason}),
        }
    }
}

/// Parse `--host <host> stop` and return the host.
fn parse_arguments(args: &[String]) -> Result<HookHost, String> {
    let mut host: Option<HookHost> = None;
    let mut stop = false;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--host" => {
                let value = iter.next().ok_or("--host requires a value")?;
                host = Some(HookHost::parse(value).ok_or_else(|| {
                    format!("unsupported host '{value}', expected claude or codex")
                })?);
            }
            "stop" | "Stop" => stop = true,
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    if !stop {
        return Err("only the stop event is supported".to_string());
    }
    host.ok_or_else(|| "--host is required".to_string())
}

fn has_text(payload: &Value, key: &str) -> bool {
    payload
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|text| !text.is_empty())
}

/// Check that `payload` is a Stop payload of the declared host and return its
/// session id.
///
/// A Codex Stop payload carries `turn_id` and `model`. A Claude Code Stop payload
/// carries `permission_mode` and has no `turn_id`.
fn validate_payload(host: HookHost, payload: &Value) -> Result<String, String> {
    if !payload.is_object() {
        return Err("hook payload is not a JSON object".to_string());
    }
    if !has_text(payload, "session_id") {
        return Err("hook payload has no session_id".to_string());
    }
    match payload.get("hook_event_name").and_then(Value::as_str) {
        Some(STOP_EVENT) => {}
        Some(other) => return Err(format!("hook_event_name is '{other}', expected Stop")),
        None => return Err("hook payload has no hook_event_name".to_string()),
    }
    let looks_codex = has_text(payload, "turn_id") && has_text(payload, "model");
    let looks_claude = has_text(payload, "permission_mode") && payload.get("turn_id").is_none();
    let matches_host = match host {
        HookHost::Codex => looks_codex,
        HookHost::Claude => looks_claude,
    };
    if !matches_host {
        return Err(format!(
            "payload does not match --host {}: host mismatch",
            host.as_str()
        ));
    }
    Ok(payload["session_id"]
        .as_str()
        .unwrap_or_default()
        .to_string())
}

/// File-name stem for a session id: safe characters only, plus a digest so that
/// two ids that sanitize alike stay apart.
fn session_stem(session_id: &str) -> String {
    let safe: String = session_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(MAX_SESSION_NAME)
        .collect();
    let digest = format!("{:x}", Sha256::digest(session_id.as_bytes()));
    format!("{safe}-{}", &digest[..12])
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{}-{nanos}", std::process::id())
}

fn is_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64) && text.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// Throwaway index file, removed when dropped.
struct TemporaryIndex {
    path: PathBuf,
}

impl Drop for TemporaryIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Write the working tree outside `.dev` as a tree object and return its id.
/// The tree is built the way an attempt's `openTree` is, in a temporary index
/// file with a unique name inside `scratch_dir`.
fn working_tree_snapshot(repo: &Path, scratch_dir: &Path) -> Result<String, String> {
    let index = TemporaryIndex {
        path: scratch_dir.join(format!("snapshot-{}.index", unique_suffix())),
    };
    let envs = [("GIT_INDEX_FILE", index.path.as_os_str())];
    run_git_env(repo, &["read-tree", "HEAD"], &envs)?;
    run_git_env(repo, &["add", "-A", "--", ".", DEV_EXCLUDE_PATHSPEC], &envs)?;
    let written = run_git_env(repo, &["write-tree"], &envs)?;
    let tree = String::from_utf8_lossy(&written).trim().to_string();
    if is_object_id(&tree) {
        Ok(tree)
    } else {
        Err(format!("git write-tree returned '{tree}'"))
    }
}

fn write_snapshot(path: &Path, tree: &str) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", unique_suffix()));
    std::fs::write(&temporary, tree)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("cannot publish {}: {error}", path.display())
    })
}

fn repository_root(cwd: Option<&str>) -> Result<PathBuf, String> {
    let here = match cwd {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => std::env::current_dir().map_err(|error| format!("cannot read cwd: {error}"))?,
    };
    let top = run_git(&here, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(String::from_utf8_lossy(&top).trim()))
}

fn block_reason(opened: &super::quickfix_run::OpenedAttempt, host: HookHost) -> String {
    format!(
        "The authoritative checks fail. A quickfix attempt {id} is open. Spawn a {host} subagent on model {model} with the spec at {spec}, wait for it to finish, then run `gal quickfix-run --close {id} --session <subagent session ref>`. Do not commit.",
        id = opened.attempt_id,
        host = host.as_str(),
        model = opened.model,
        spec = opened.spec_path.display(),
    )
}

/// Decide for one validated Stop payload.
fn decide(host: HookHost, payload: &Value, session_id: &str) -> Result<Decision, String> {
    let config = load_quickfix_config()?;
    if !config.hook {
        return Ok(Decision::Allow);
    }
    let repo = repository_root(payload.get("cwd").and_then(Value::as_str))?;
    let hook_dir = repo.join(HOOK_DIR);
    std::fs::create_dir_all(&hook_dir)
        .map_err(|error| format!("cannot create {}: {error}", hook_dir.display()))?;
    let snapshot_path = hook_dir.join(format!("{}{SNAPSHOT_SUFFIX}", session_stem(session_id)));
    let tree = working_tree_snapshot(&repo, &hook_dir)?;
    let stored = std::fs::read_to_string(&snapshot_path).ok();
    if stored.as_deref().map(str::trim) == Some(tree.as_str()) {
        return Ok(Decision::Allow);
    }
    let outcome = open_attempt(&repo, host.as_str())?;
    write_snapshot(&snapshot_path, &tree)?;
    Ok(match outcome {
        OpenOutcome::Opened(opened) => Decision::Block(block_reason(&opened, host)),
        OpenOutcome::Refused(_) => Decision::Allow,
    })
}

fn read_event() -> Result<Vec<u8>, String> {
    let mut input = Vec::new();
    std::io::stdin()
        .take((MAX_EVENT_BYTES + 1) as u64)
        .read_to_end(&mut input)
        .map_err(|error| format!("cannot read hook payload: {error}"))?;
    if input.len() > MAX_EVENT_BYTES {
        return Err(format!("hook payload exceeds {MAX_EVENT_BYTES} bytes"));
    }
    Ok(input)
}

fn handle(host: HookHost) -> Result<Decision, String> {
    let input = read_event()?;
    let payload: Value = serde_json::from_slice(&input)
        .map_err(|error| format!("hook payload is not valid JSON: {error}"))?;
    let session_id = validate_payload(host, &payload)?;
    decide(host, &payload, &session_id)
}

/// Run `gal quickfix-hook`.
pub(crate) fn cmd_quickfix_hook(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let host = match parse_arguments(&rest) {
        Ok(host) => host,
        Err(error) => {
            eprintln!("gal quickfix-hook: {error}");
            return ExitCode::Usage;
        }
    };
    let decision = handle(host).unwrap_or_else(|reason| {
        eprintln!("gal quickfix-hook: allowing stop: {reason}");
        Decision::Allow
    });
    println!("{}", decision.render());
    ExitCode::Success
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|part| part.to_string()).collect()
    }

    #[test]
    fn arguments_require_host_and_stop() {
        assert_eq!(
            parse_arguments(&args(&["--host", "claude", "stop"])).unwrap(),
            HookHost::Claude
        );
        assert_eq!(
            parse_arguments(&args(&["stop", "--host", "codex"])).unwrap(),
            HookHost::Codex
        );
        assert!(parse_arguments(&args(&["--host", "claude"])).is_err());
        assert!(parse_arguments(&args(&["stop"])).is_err());
        assert!(parse_arguments(&args(&["--host", "other", "stop"])).is_err());
    }

    #[test]
    fn payload_fields_tell_the_hosts_apart() {
        let claude =
            json!({"session_id": "s", "hook_event_name": "Stop", "permission_mode": "default"});
        let codex =
            json!({"session_id": "s", "hook_event_name": "Stop", "turn_id": "t", "model": "m"});
        assert!(validate_payload(HookHost::Claude, &claude).is_ok());
        assert!(validate_payload(HookHost::Codex, &codex).is_ok());
        assert!(validate_payload(HookHost::Codex, &claude).is_err());
        assert!(validate_payload(HookHost::Claude, &codex).is_err());
    }

    #[test]
    fn session_stem_is_a_safe_file_name_and_keeps_ids_apart() {
        let stem = session_stem("../../evil id");
        assert!(stem
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'));
        assert_ne!(session_stem("a/b"), session_stem("a_b"));
    }
}
