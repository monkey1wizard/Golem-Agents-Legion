use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostKind {
    Claude,
    Codex,
}

impl HostKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostSessionRef {
    pub host: HostKind,
    pub reference: String,
}

impl HostSessionRef {
    pub fn parse(value: &str) -> Result<Self, HostSessionError> {
        let (host, reference) = value.split_once(':').ok_or(HostSessionError::UnknownHost)?;
        let host = match host {
            "claude" => HostKind::Claude,
            "codex" => HostKind::Codex,
            _ => return Err(HostSessionError::UnknownHost),
        };
        if reference.trim().is_empty() {
            return Err(HostSessionError::EmptyRef);
        }
        Ok(Self {
            host,
            reference: reference.to_owned(),
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostSessionError {
    #[error("unknown host")]
    UnknownHost,
    #[error("empty session reference")]
    EmptyRef,
    #[error("invalid Claude agent identifier")]
    InvalidReference,
    #[error("session record missing")]
    RecordMissing,
    #[error("working directory mismatch")]
    CwdMismatch,
    #[error("malformed session record")]
    MalformedRecord,
    #[error("session timestamp predates attempt start")]
    TimestampBeforeStart,
    #[error("session has no tool use")]
    NoToolUse,
    #[error("attempt id absent from user message")]
    AttemptIdNotInUserMessage,
    #[error("agent is orchestrated-only")]
    OrchestratedOnlyAgent,
    #[error("implement session has no write tool")]
    NoWriteTool,
    #[error("Codex rollout reference is invalid")]
    InvalidCodexReference,
    #[error("Codex rollout match is ambiguous")]
    AmbiguousRecord,
    #[error("session has no per-turn model evidence")]
    MissingModelEvidence,
    #[error("observed model does not match expected model")]
    ModelMismatch,
    #[error("observed effort does not match expected effort")]
    EffortMismatch,
}

pub fn epoch_seconds(value: &str) -> Result<u64, HostSessionError> {
    if let Some(raw) = value
        .strip_prefix("epoch+")
        .and_then(|v| v.strip_suffix('s'))
    {
        if raw.is_empty() || !raw.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(HostSessionError::MalformedRecord);
        }
        return raw.parse().map_err(|_| HostSessionError::MalformedRecord);
    }
    let b = value.as_bytes();
    if b.len() != 24
        || b[4] != b'-'
        || b[7] != b'-'
        || b[10] != b'T'
        || b[13] != b':'
        || b[16] != b':'
        || b[19] != b'.'
        || &b[23..] != b"Z"
    {
        return Err(HostSessionError::MalformedRecord);
    }
    let number = |s: &[u8]| -> Result<u32, HostSessionError> {
        std::str::from_utf8(s)
            .ok()
            .and_then(|x| x.parse().ok())
            .ok_or(HostSessionError::MalformedRecord)
    };
    let (y, m, d, hh, mm, ss) = (
        number(&b[0..4])? as i64,
        number(&b[5..7])?,
        number(&b[8..10])?,
        number(&b[11..13])?,
        number(&b[14..16])?,
        number(&b[17..19])?,
    );
    let leap_year = y % 4 == 0 && (y % 100 != 0 || y % 400 == 0);
    let month_days = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap_year => 29,
        2 => 28,
        _ => return Err(HostSessionError::MalformedRecord),
    };
    if d == 0 || d > month_days || hh > 23 || mm > 59 || ss > 59 {
        return Err(HostSessionError::MalformedRecord);
    }
    let y = y - i64::from(m <= 2);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = m as i64 + if m > 2 { -3 } else { 9 };
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let seconds =
        (era * 146097 + doe - 719468) * 86400 + hh as i64 * 3600 + mm as i64 * 60 + ss as i64;
    u64::try_from(seconds).map_err(|_| HostSessionError::MalformedRecord)
}

#[derive(Debug, PartialEq, Eq)]
pub struct VerifiedHostSession {
    pub session_id: String,
    model_evidence: Vec<ModelEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModelEvidence {
    model: String,
    effort: Option<String>,
    claude_family: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedModelPolicy {
    pub model: String,
    pub effort: Option<String>,
}

pub fn observed_model_policy(verified: &VerifiedHostSession) -> Option<ObservedModelPolicy> {
    verified
        .model_evidence
        .first()
        .map(|observation| ObservedModelPolicy {
            model: observation.model.clone(),
            effort: observation.effort.clone(),
        })
}

/// Check every model observation retained by host session verification.
pub fn verify_model_policy(
    verified: &VerifiedHostSession,
    expected_model: &str,
    expected_effort: Option<&str>,
) -> Result<ObservedModelPolicy, HostSessionError> {
    let first = verified
        .model_evidence
        .first()
        .ok_or(HostSessionError::MissingModelEvidence)?;
    for observation in &verified.model_evidence {
        if observation.model.trim().is_empty() {
            return Err(HostSessionError::MissingModelEvidence);
        }
        if !(observation.model == expected_model
            || (observation.claude_family
                && (observation
                    .model
                    .strip_prefix(expected_model)
                    .is_some_and(|suffix| suffix.starts_with('-'))
                    || observation
                        .model
                        .strip_prefix("claude-")
                        .and_then(|model| model.strip_prefix(expected_model))
                        .is_some_and(|suffix| suffix.starts_with('-')))))
        {
            return Err(HostSessionError::ModelMismatch);
        }
        if !observation.claude_family {
            if let Some(expected) = expected_effort {
                if observation.effort.as_deref() != Some(expected) {
                    return Err(HostSessionError::EffortMismatch);
                }
            }
        }
    }
    Ok(ObservedModelPolicy {
        model: first.model.clone(),
        effort: first.effort.clone(),
    })
}

pub(crate) fn canonical(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Locates a top-level Claude session transcript at `projects/<dir>/<session>.jsonl`.
pub fn locate_claude_session(
    home: &Path,
    workdir: &Path,
    session_id: &str,
) -> Result<PathBuf, HostSessionError> {
    validate_claude_reference(session_id)?;
    let projects = home.join(".claude/projects");
    let wanted = canonical(workdir);
    let entries = fs::read_dir(&projects).map_err(|_| HostSessionError::RecordMissing)?;
    let mut malformed_candidate = false;
    let mut found_candidate = false;
    for entry in entries.flatten() {
        let transcript = entry.path().join(format!("{session_id}.jsonl"));
        if !transcript.is_file() {
            continue;
        }
        found_candidate = true;
        let Some(cwd) = first_cwd(&transcript) else {
            malformed_candidate = true;
            continue;
        };
        if canonical(Path::new(&cwd)) == wanted {
            return Ok(transcript);
        }
    }
    if malformed_candidate {
        Err(HostSessionError::MalformedRecord)
    } else if found_candidate {
        Err(HostSessionError::CwdMismatch)
    } else {
        Err(HostSessionError::RecordMissing)
    }
}

/// Reads the first working directory recorded near the top of a transcript.
fn first_cwd(transcript: &Path) -> Option<String> {
    let file = fs::File::open(transcript).ok()?;
    std::io::BufRead::lines(std::io::BufReader::new(file))
        .take(256)
        .map_while(Result::ok)
        .filter_map(|line| serde_json::from_str::<Value>(&line).ok())
        .find_map(|record| record.get("cwd").and_then(Value::as_str).map(str::to_owned))
}

pub fn locate_claude(
    home: &Path,
    workdir: &Path,
    agent_id: &str,
) -> Result<(PathBuf, PathBuf), HostSessionError> {
    validate_claude_reference(agent_id)?;
    let projects = home.join(".claude/projects");
    let wanted = canonical(workdir);
    let entries = fs::read_dir(&projects).map_err(|_| HostSessionError::RecordMissing)?;
    let mut malformed_candidate = false;
    let mut found_candidate = false;
    for a in entries.flatten() {
        if !a.path().is_dir() {
            continue;
        }
        let Ok(children) = fs::read_dir(a.path()) else {
            continue;
        };
        for b in children.flatten() {
            let transcript = b
                .path()
                .join("subagents")
                .join(format!("agent-{agent_id}.jsonl"));
            if !transcript.is_file() {
                continue;
            }
            found_candidate = true;
            let record = fs::read_to_string(&transcript)
                .ok()
                .and_then(|contents| contents.lines().next().map(str::to_owned))
                .and_then(|line| serde_json::from_str::<Value>(&line).ok());
            let Some(cwd) = record
                .as_ref()
                .and_then(|record| record.get("cwd"))
                .and_then(Value::as_str)
            else {
                malformed_candidate = true;
                continue;
            };
            if canonical(Path::new(cwd)) == wanted {
                return Ok((
                    transcript.clone(),
                    transcript.with_file_name(format!("agent-{agent_id}.meta.json")),
                ));
            }
        }
    }
    if malformed_candidate {
        Err(HostSessionError::MalformedRecord)
    } else if found_candidate {
        Err(HostSessionError::CwdMismatch)
    } else {
        Err(HostSessionError::RecordMissing)
    }
}

pub fn verify(
    reference: &HostSessionRef,
    workdir: &Path,
    home: &Path,
    attempt_start: &str,
    attempt_id: &str,
    phase: &str,
    orchestrated_only: &[&str],
) -> Result<VerifiedHostSession, HostSessionError> {
    if reference.host == HostKind::Claude {
        validate_claude_reference(&reference.reference)?;
    }
    if reference.host == HostKind::Codex {
        let root = std::env::var_os("CODEX_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".codex"));
        return verify_codex(
            reference,
            workdir,
            &root,
            attempt_start,
            attempt_id,
            phase,
            orchestrated_only,
        );
    }
    let (transcript, meta_path) = locate_claude(home, workdir, &reference.reference)?;
    let meta: Value =
        serde_json::from_slice(&fs::read(meta_path).map_err(|_| HostSessionError::RecordMissing)?)
            .map_err(|_| HostSessionError::MalformedRecord)?;
    let text = fs::read_to_string(transcript).map_err(|_| HostSessionError::MalformedRecord)?;
    let values: Vec<Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|_| HostSessionError::MalformedRecord)?;
    let first_timestamp = values
        .iter()
        .find_map(|v| v.get("timestamp").and_then(Value::as_str))
        .ok_or(HostSessionError::MalformedRecord)?;
    if epoch_seconds(first_timestamp)? < epoch_seconds(attempt_start)? {
        return Err(HostSessionError::TimestampBeforeStart);
    }
    let tools: Vec<&Value> = values
        .iter()
        .flat_map(|v| {
            let direct = (v.get("type").and_then(Value::as_str) == Some("tool_use")).then_some(v);
            let nested = v
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|x| x.get("type").and_then(Value::as_str) == Some("tool_use"));
            direct.into_iter().chain(nested)
        })
        .collect();
    let model_evidence = values
        .iter()
        .filter_map(|value| {
            // Synthetic progress and fixture rows are not host model evidence.
            if value.get("synthetic").and_then(Value::as_bool) == Some(true)
                || value.get("type").and_then(Value::as_str) == Some("synthetic")
            {
                return None;
            }
            if value.get("type").and_then(Value::as_str) != Some("assistant") {
                return None;
            }
            let message = value.get("message")?;
            let model = message.get("model")?.as_str()?.trim();
            if model == "<synthetic>" {
                return None;
            }
            (!model.is_empty()).then(|| ModelEvidence {
                model: model.to_owned(),
                effort: message
                    .get("effort")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
                claude_family: true,
            })
        })
        .collect();
    if tools.is_empty() {
        return Err(HostSessionError::NoToolUse);
    }
    if !values.iter().any(|v| {
        if v.get("type").and_then(Value::as_str) != Some("user") {
            return false;
        }
        let Some(content) = v.get("message").and_then(|m| m.get("content")) else {
            return false;
        };
        let texts: Vec<&str> = match content {
            Value::String(s) => vec![s],
            Value::Array(blocks) => blocks
                .iter()
                .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|block| block.get("text").and_then(Value::as_str))
                .collect(),
            _ => Vec::new(),
        };
        texts.iter().any(|text| contains_token(text, attempt_id))
    }) {
        return Err(HostSessionError::AttemptIdNotInUserMessage);
    }
    let agent_type = meta
        .get("agentType")
        .and_then(Value::as_str)
        .ok_or(HostSessionError::MalformedRecord)?;
    let role = agent_type.rsplit(':').next().unwrap_or(agent_type);
    if orchestrated_only.contains(&role) {
        return Err(HostSessionError::OrchestratedOnlyAgent);
    }
    if phase == "implement"
        && !tools.iter().any(|v| {
            matches!(
                v.get("name").and_then(Value::as_str),
                Some("Write" | "Edit" | "NotebookEdit" | "Bash" | "PowerShell")
            )
        })
    {
        return Err(HostSessionError::NoWriteTool);
    }
    Ok(VerifiedHostSession {
        session_id: reference.reference.clone(),
        model_evidence,
    })
}

/// Verify against an explicit Codex home, primarily for isolated fixture roots.
pub fn verify_at_root(
    reference: &HostSessionRef,
    workdir: &Path,
    codex_home: &Path,
    attempt_start: &str,
    attempt_id: &str,
    phase: &str,
    orchestrated_only: &[&str],
) -> Result<VerifiedHostSession, HostSessionError> {
    if reference.host == HostKind::Codex {
        return verify_codex(
            reference,
            workdir,
            codex_home,
            attempt_start,
            attempt_id,
            phase,
            orchestrated_only,
        );
    }
    verify(
        reference,
        workdir,
        codex_home,
        attempt_start,
        attempt_id,
        phase,
        orchestrated_only,
    )
}

/// Locate and verify a Codex rollout. `root` may be either CODEX_HOME or an
/// injected directory containing date-partitioned rollouts.
pub fn locate_codex(
    root: &Path,
    workdir: &Path,
    reference: &str,
    attempt_start: &str,
) -> Result<PathBuf, HostSessionError> {
    // A reference is either a rollout ID or the agent path that `spawn_agent` returns,
    // such as `/root/<task_name>`. Agent path segments are lowercase names.
    let by_agent_path = reference.starts_with('/');
    let valid = if by_agent_path {
        reference.len() > 1
            && reference[1..].split('/').all(|segment| {
                !segment.is_empty()
                    && segment
                        .bytes()
                        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            })
    } else {
        !reference.is_empty()
            && reference
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    if !valid {
        return Err(HostSessionError::InvalidCodexReference);
    }
    let start_day = epoch_seconds(attempt_start)? / 86_400;
    let sessions = if root.join("sessions").is_dir() {
        root.join("sessions")
    } else {
        root.to_path_buf()
    };
    let wanted = canonical(workdir);
    let mut found = Vec::new();
    let mut malformed = false;
    let mut cwd_mismatch = false;
    let mut stack = vec![sessions.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(rel) = path.strip_prefix(&sessions) {
                    let parts: Vec<_> = rel
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().into_owned())
                        .collect();
                    if parts.len() >= 3 {
                        let partition_day = parts[0]
                            .parse::<u32>()
                            .ok()
                            .zip(parts[1].parse::<u32>().ok())
                            .zip(parts[2].parse::<u32>().ok())
                            .and_then(|((year, month), day)| {
                                let date = format!("{year:04}-{month:02}-{day:02}T00:00:00.000Z");
                                epoch_seconds(&date).ok().map(|seconds| seconds / 86_400)
                            });
                        if partition_day.is_some_and(|day| day < start_day) {
                            continue;
                        }
                    }
                }
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|x| x.to_str()) != Some("jsonl") {
                continue;
            }
            let Ok(file) = fs::File::open(&path) else {
                continue;
            };
            let mut reader = std::io::BufReader::new(file);
            let mut line = String::new();
            if std::io::BufRead::read_line(&mut reader, &mut line).is_err() {
                continue;
            }
            let Ok(first) = serde_json::from_str::<Value>(&line) else {
                if path
                    .file_name()
                    .and_then(|x| x.to_str())
                    .is_some_and(|name| name.ends_with(&format!("-{reference}.jsonl")))
                {
                    malformed = true;
                }
                continue;
            };
            let meta = (first.get("type").and_then(Value::as_str) == Some("session_meta"))
                .then_some(&first);
            let exact_id = meta
                .and_then(|v| v.get("payload"))
                .and_then(|p| p.get("id"))
                .and_then(Value::as_str)
                == Some(reference);
            let filename_id = !by_agent_path
                && path
                    .file_name()
                    .and_then(|x| x.to_str())
                    .is_some_and(|name| name.ends_with(&format!("-{reference}.jsonl")));
            let agent_path_match = by_agent_path
                && meta
                    .and_then(|v| v.get("payload"))
                    .and_then(|p| p.get("source"))
                    .and_then(|s| s.get("subagent"))
                    .and_then(|s| s.get("thread_spawn"))
                    .and_then(|s| s.get("agent_path"))
                    .and_then(Value::as_str)
                    == Some(reference);
            if !exact_id && !filename_id && !agent_path_match {
                continue;
            }
            let Some(meta) = meta else {
                malformed = true;
                continue;
            };
            let Some(cwd) = meta
                .get("payload")
                .and_then(|p| p.get("cwd"))
                .and_then(Value::as_str)
            else {
                malformed = true;
                continue;
            };
            if canonical(Path::new(cwd)) != wanted {
                cwd_mismatch = true;
                continue;
            }
            found.push(path);
        }
    }
    match found.len() {
        1 => Ok(found.remove(0)),
        n if n > 1 => Err(HostSessionError::AmbiguousRecord),
        _ if malformed => Err(HostSessionError::MalformedRecord),
        _ if cwd_mismatch => Err(HostSessionError::CwdMismatch),
        _ => Err(HostSessionError::RecordMissing),
    }
}

fn verify_codex(
    reference: &HostSessionRef,
    workdir: &Path,
    root: &Path,
    start: &str,
    attempt: &str,
    phase: &str,
    blocked: &[&str],
) -> Result<VerifiedHostSession, HostSessionError> {
    let path = locate_codex(root, workdir, &reference.reference, start)?;
    let text = fs::read_to_string(path).map_err(|_| HostSessionError::MalformedRecord)?;
    let records: Vec<Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .map_err(|_| HostSessionError::MalformedRecord)?;
    let meta = records
        .iter()
        .find(|v| v.get("type").and_then(Value::as_str) == Some("session_meta"))
        .ok_or(HostSessionError::MalformedRecord)?;
    let payload = meta
        .get("payload")
        .ok_or(HostSessionError::MalformedRecord)?;
    if payload.get("thread_source").and_then(Value::as_str) != Some("subagent") {
        return Err(HostSessionError::OrchestratedOnlyAgent);
    }
    let spawn = payload
        .get("source")
        .and_then(|s| s.get("subagent"))
        .and_then(|s| s.get("thread_spawn"))
        .ok_or(HostSessionError::MalformedRecord)?;
    let agent_path = spawn
        .get("agent_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .ok_or(HostSessionError::MalformedRecord)?;
    let parent = spawn.get("parent_thread_id").and_then(Value::as_str);
    if parent.is_some_and(str::is_empty) {
        return Err(HostSessionError::MalformedRecord);
    }
    if let Some(role) = spawn.get("agent_role").and_then(Value::as_str) {
        if blocked.contains(&role.rsplit(':').next().unwrap_or(role)) {
            return Err(HostSessionError::OrchestratedOnlyAgent);
        }
    }
    let mut first_timestamp = None;
    let mut bound_attempt = false;
    let mut tools = Vec::new();
    let mut model_evidence = Vec::new();
    for row in &records {
        if let Some(ts) = row.get("timestamp").and_then(Value::as_str) {
            first_timestamp.get_or_insert(ts);
        }
        if row.get("type").and_then(Value::as_str) == Some("turn_context") {
            let context = row
                .get("payload")
                .ok_or(HostSessionError::MalformedRecord)?;
            let model = context
                .get("model")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or(HostSessionError::MalformedRecord)?;
            let effort = context
                .get("effort")
                .and_then(Value::as_str)
                .filter(|s| !s.trim().is_empty())
                .ok_or(HostSessionError::MalformedRecord)?;
            model_evidence.push(ModelEvidence {
                model: model.to_owned(),
                effort: Some(effort.to_owned()),
                claude_family: false,
            });
        }
        let p = row.get("payload").unwrap_or(row);
        let kind = p.get("type").and_then(Value::as_str).unwrap_or("");
        // A spawned Codex subagent receives its task as an agent message addressed to
        // its own agent path, not as a user message.
        let user_message =
            kind == "message" && p.get("role").and_then(Value::as_str) == Some("user");
        let task_message = kind == "agent_message"
            && p.get("recipient").and_then(Value::as_str) == Some(agent_path);
        if row.get("type").and_then(Value::as_str) == Some("response_item")
            && (user_message || task_message)
        {
            if let Some(blocks) = p.get("content").and_then(Value::as_array) {
                bound_attempt |= blocks
                    .iter()
                    .filter(|b| {
                        matches!(
                            b.get("type").and_then(Value::as_str),
                            Some("input_text" | "text")
                        )
                    })
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .any(|s| contains_token(s, attempt));
            }
        }
        if matches!(kind, "function_call" | "custom_tool_call") {
            tools.push(p.get("name").and_then(Value::as_str).unwrap_or(""));
        }
    }
    let first = first_timestamp.ok_or(HostSessionError::MalformedRecord)?;
    if epoch_seconds(first)? < epoch_seconds(start)? {
        return Err(HostSessionError::TimestampBeforeStart);
    }
    if !bound_attempt {
        return Err(HostSessionError::AttemptIdNotInUserMessage);
    }
    if tools.is_empty() {
        return Err(HostSessionError::NoToolUse);
    }
    if phase == "implement"
        && !tools.iter().any(|n| {
            matches!(
                *n,
                "Write" | "Edit" | "Bash" | "PowerShell" | "exec" | "shell" | "apply_patch"
            )
        })
    {
        return Err(HostSessionError::NoWriteTool);
    }
    let rollout_id = payload
        .get("id")
        .and_then(Value::as_str)
        .ok_or(HostSessionError::MalformedRecord)?;
    Ok(VerifiedHostSession {
        session_id: rollout_id.to_owned(),
        model_evidence,
    })
}

fn validate_claude_reference(reference: &str) -> Result<(), HostSessionError> {
    if !reference.is_empty()
        && reference
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        Ok(())
    } else {
        Err(HostSessionError::InvalidReference)
    }
}

pub(crate) fn contains_token(text: &str, token: &str) -> bool {
    text.match_indices(token).any(|(start, matched)| {
        let end = start + matched.len();
        let is_token_char = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-';
        let before_ok = text[..start]
            .chars()
            .next_back()
            .is_none_or(|c| !is_token_char(c));
        let after_ok = text[end..].chars().next().is_none_or(|c| !is_token_char(c));
        before_ok && after_ok
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn host_session_ref_parses_claude() {
        assert_eq!(
            HostSessionRef::parse("claude:abc").unwrap().host,
            HostKind::Claude
        );
    }
    #[test]
    fn host_session_ref_parses_codex() {
        assert_eq!(
            HostSessionRef::parse("codex:C:/x").unwrap().reference,
            "C:/x"
        );
    }
    #[test]
    fn host_session_ref_rejects_unknown() {
        assert_eq!(
            HostSessionRef::parse("other:x"),
            Err(HostSessionError::UnknownHost)
        );
    }
    #[test]
    fn host_session_ref_rejects_empty() {
        assert_eq!(
            HostSessionRef::parse("claude:"),
            Err(HostSessionError::EmptyRef)
        );
    }
    #[test]
    fn host_session_ref_parses_epoch() {
        assert_eq!(epoch_seconds("epoch+123s").unwrap(), 123);
    }
    #[test]
    fn host_session_ref_parses_iso() {
        assert_eq!(epoch_seconds("1970-01-01T00:00:01.123Z").unwrap(), 1);
    }

    #[test]
    fn host_session_ref_compares_epoch_and_iso_seconds() {
        assert_eq!(
            epoch_seconds("epoch+1791527210s").unwrap(),
            epoch_seconds("2026-10-09T06:26:50.274Z").unwrap()
        );
    }

    #[test]
    fn host_session_ref_rejects_malformed_timestamp() {
        assert_eq!(
            epoch_seconds("2026-10-09T06:26:50Z"),
            Err(HostSessionError::MalformedRecord)
        );
    }

    #[test]
    fn host_session_ref_rejects_invalid_calendar_date() {
        assert_eq!(
            epoch_seconds("2026-02-30T09:22:20.123Z"),
            Err(HostSessionError::MalformedRecord)
        );
    }

    #[test]
    fn host_session_ref_rejects_signed_epoch_seconds() {
        assert_eq!(
            epoch_seconds("epoch++123s"),
            Err(HostSessionError::MalformedRecord)
        );
    }

    #[test]
    fn host_session_ref_claude_identifier_allows_ascii_identifier_characters() {
        assert_eq!(validate_claude_reference("Agent_01-x"), Ok(()));
    }

    #[test]
    fn host_session_ref_claude_identifier_rejects_other_characters() {
        for value in ["", "../agent", "a/b", "a\\b", "a:b", "é"] {
            assert_eq!(
                validate_claude_reference(value),
                Err(HostSessionError::InvalidReference)
            );
        }
    }
}
