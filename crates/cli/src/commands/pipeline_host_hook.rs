//! Bounded Codex host-hook bootstrap handshake.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_EVENT: usize = 1024 * 1024;
const MAX_STATE: usize = 64 * 1024;
const MAX_PROMPT: usize = 1024 * 1024;
const HANDLER: &str = "gal.pipeline-host-hook.v1";
const ACTION_PREFIX: &str = "gal pipeline --require-codex-stop-v1";
const PENDING_TTL: u64 = 120;
const GRANT_TTL: u64 = 30;
const MAX_STOPS: u64 = 3;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn safe(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str)
        .filter(|s| !s.is_empty() && s.len() <= 4096)
        .map(str::to_owned)
}
fn read_record(path: &Path) -> Option<Value> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() > MAX_STATE {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}
fn write_record(path: &Path, record: &Value) -> io::Result<()> {
    let bytes = serde_json::to_vec(record).map_err(io::Error::other)?;
    if bytes.len() > MAX_STATE {
        return Err(io::Error::other("hook state limit exceeded"));
    }
    fs::create_dir_all(
        path.parent()
            .ok_or_else(|| io::Error::other("missing state directory"))?,
    )?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(tmp, path)
}
fn output(value: Value) {
    println!("{value}");
}

pub(crate) fn cmd_pipeline_host_hook(args: &[String]) -> gal_engine::ExitCode {
    let event = match args.first().map(String::as_str) {
        Some("user-prompt-submit") | Some("UserPromptSubmit") => "UserPromptSubmit",
        Some("pre-tool-use") | Some("PreToolUse") => "PreToolUse",
        Some("stop") | Some("Stop") => "Stop",
        _ => {
            eprintln!("usage: gal pipeline-host-hook <user-prompt-submit|pre-tool-use|stop>");
            return gal_engine::ExitCode::Usage;
        }
    };
    let mut input = Vec::new();
    if io::stdin()
        .take((MAX_EVENT + 1) as u64)
        .read_to_end(&mut input)
        .is_err()
        || input.len() > MAX_EVENT
    {
        output(json!({"continue":false,"decision":"deny","reason":"hook-event-too-large"}));
        return gal_engine::ExitCode::Error;
    }
    let value: Value = match serde_json::from_slice::<Value>(&input) {
        Ok(v) if v.is_object() => v,
        _ => {
            output(json!({"continue":false,"decision":"deny","reason":"invalid-hook-event"}));
            return gal_engine::ExitCode::Error;
        }
    };
    match handle(event, &value) {
        Ok(v) => {
            output(v);
            gal_engine::ExitCode::Success
        }
        Err(reason) => {
            output(json!({"continue":false,"decision":"deny","reason":reason}));
            gal_engine::ExitCode::Error
        }
    }
}

fn paths(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    (
        root.join("bootstrap-pending.json"),
        root.join("canary-ack.json"),
        root.join("ready-grant.json"),
    )
}

#[derive(Debug)]
struct ResolvedPipelineRequest {
    prompt_arg: String,
    prompt_hash: String,
    continuation: bool,
}

fn handle(event: &str, value: &Value) -> Result<Value, &'static str> {
    let data = std::env::var_os("PLUGIN_DATA").ok_or("plugin-data-unavailable")?;
    let plugin = PathBuf::from(data);
    fs::create_dir_all(&plugin).map_err(|_| "plugin-data-unavailable")?;
    let (pending, ack, grant) = paths(&plugin);
    let session = safe(value.get("session_id")).ok_or("missing-session")?;
    let turn = safe(value.get("turn_id").or_else(|| value.get("turn_chain_id")))
        .ok_or("missing-turn-chain")?;
    let cwd = safe(value.get("cwd")).ok_or("missing-worktree")?;
    let worktree_path = fs::canonicalize(cwd).map_err(|_| "invalid-worktree")?;
    let worktree = worktree_path.to_string_lossy().into_owned();
    let prompt = safe(value.get("prompt")).unwrap_or_default();
    let request = resolve_pipeline_request(&worktree_path, &prompt)?;
    let generation = value
        .get("generation")
        .and_then(Value::as_u64)
        .ok_or("missing-generation")?;
    let nonce = safe(value.get("nonce")).ok_or("missing-nonce")?;
    let prompt_hash = request
        .as_ref()
        .map(|request| request.prompt_hash.clone())
        .unwrap_or_else(|| digest(prompt.as_bytes()));
    let handler_digest = digest(HANDLER.as_bytes());
    let binding = json!({"session_id":session,"turn_id":turn,"worktree":worktree,"prompt_hash":prompt_hash,"generation":generation,"nonce":nonce,"handler_digest":handler_digest});
    match event {
        "UserPromptSubmit" => {
            let Some(request) = request.as_ref() else {
                return Ok(json!({"continue":true}));
            };
            if request.continuation {
                present_grant(&pending, &grant, &worktree, request, &binding, &turn)?;
                return Ok(json!({"continue":true}));
            }
            let rec = json!({"state":"BootstrapPending","binding":binding,"expires":now().saturating_add(PENDING_TTL),"stops":0});
            write_record(&pending, &rec).map_err(|_| "bootstrap-write-failed")?;
            Ok(json!({"continue":true}))
        }
        "PreToolUse" => {
            if read_record(&grant).is_some_and(|record| {
                let consumed_binding = record.get("binding");
                record.get("state").and_then(Value::as_str) == Some("Consumed")
                    && consumed_binding.and_then(|value| value.get("session_id"))
                        == binding.get("session_id")
                    && consumed_binding.and_then(|value| value.get("worktree"))
                        == binding.get("worktree")
            }) {
                return Ok(json!({"continue":true,"decision":"allow"}));
            }
            let Some(mut rec) = read_record(&pending) else {
                if pending.exists() {
                    return Err("bootstrap-state-invalid");
                }
                return Ok(json!({"continue":true}));
            };
            if rec.get("expires").and_then(Value::as_u64).unwrap_or(0) < now() {
                return Err("bootstrap-expired");
            }
            if rec.get("binding") != Some(&binding) {
                return Err("bootstrap-binding-mismatch");
            }
            let tool = safe(value.get("tool_name")).ok_or("missing-tool-name")?;
            let grant_rec = read_record(&grant).ok_or("grant-missing-or-invalid")?;
            if grant_rec.get("state").and_then(Value::as_str) != Some("ReadyGrant")
                || grant_rec.get("binding") != Some(&binding)
                || !grant_rec
                    .get("expires")
                    .and_then(Value::as_u64)
                    .is_some_and(|expiry| expiry >= now())
            {
                return Err("grant-expired-or-invalid");
            }
            if presented_action(value, request.as_ref())? {
                if rec.get("state").and_then(Value::as_str) != Some("GrantPresented")
                    || rec.get("presented_turn_id").and_then(Value::as_str)
                        != binding.get("turn_id").and_then(Value::as_str)
                {
                    return Err("grant-not-presented");
                }
                rec["state"] = json!("ActionAllowed");
                write_record(&pending, &rec).map_err(|_| "grant-presentation-failed")?;
                return Ok(json!({"continue":true,"decision":"allow"}));
            }
            let _ = tool;
            Ok(json!({"continue":false,"decision":"deny","reason":"bootstrap-pending"}))
        }
        "Stop" => {
            let Some(mut rec) = read_record(&pending) else {
                if pending.exists() {
                    return Err("bootstrap-state-invalid");
                }
                return Ok(json!({"continue":true}));
            };
            if rec.get("binding") != Some(&binding) {
                return Err("bootstrap-binding-mismatch");
            }
            if rec.get("expires").and_then(Value::as_u64).unwrap_or(0) < now() {
                return Err("bootstrap-expired");
            }
            let stops = rec
                .get("stops")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(1);
            if stops > MAX_STOPS {
                return Err("bootstrap-no-progress-limit");
            }
            rec["stops"] = json!(stops);
            let request = request.as_ref().ok_or("missing-plan-scope")?;
            let action = continuation_action(&request.prompt_arg);
            let control_dir = control_dir(&worktree, &request.prompt_arg)?;
            fs::create_dir_all(&control_dir).map_err(|_| "control-directory-failed")?;
            let control = control_dir.join("codex-hook-grant.json");
            if matches!(
                rec.get("state").and_then(Value::as_str),
                Some("GrantPresented") | Some("ActionAllowed") | Some("ReadyGrant")
            ) {
                validate_grant(&grant, &binding)?;
                write_record(&pending, &rec).map_err(|_| "bootstrap-write-failed")?;
                return Ok(json!({"continue":false,"decision":"block","reason":action}));
            }
            if rec.get("state").and_then(Value::as_str) != Some("BootstrapPending") {
                return Err("bootstrap-invalid-state");
            }
            rec["state"] = json!("CanaryAck");
            write_record(&pending, &rec).map_err(|_| "canary-write-failed")?;
            write_record(&ack, &rec).map_err(|_| "canary-ack-write-failed")?;
            let expires = now().saturating_add(GRANT_TTL);
            let grant_record =
                json!({"state":"ReadyGrant","binding":binding,"expires":expires,"stops":stops});
            write_record(&grant, &grant_record).map_err(|_| "grant-write-failed")?;
            let control_record = json!({"nonce_digest":digest(nonce.as_bytes()),"session_id":session,"turn_id":turn,"worktree":worktree,"prompt_hash":prompt_hash,"generation":generation,"expires":expires,"handler_digest":handler_digest});
            write_record(&control, &control_record).map_err(|_| "control-write-failed")?;
            rec["state"] = json!("ReadyGrant");
            rec["expires"] = json!(expires);
            write_record(&pending, &rec).map_err(|_| "grant-publication-failed")?;
            Ok(json!({"continue":false,"decision":"block","reason":action}))
        }
        _ => Ok(json!({"continue":true})),
    }
}

fn validate_grant(path: &Path, binding: &Value) -> Result<(), &'static str> {
    let grant = read_record(path).ok_or("grant-missing-or-invalid")?;
    if grant.get("state").and_then(Value::as_str) != Some("ReadyGrant")
        || grant.get("binding") != Some(binding)
        || !grant
            .get("expires")
            .and_then(Value::as_u64)
            .is_some_and(|expiry| expiry >= now())
    {
        return Err("grant-expired-or-invalid");
    }
    Ok(())
}

fn present_grant(
    pending_path: &Path,
    grant_path: &Path,
    worktree: &str,
    request: &ResolvedPipelineRequest,
    presented_binding: &Value,
    turn: &str,
) -> Result<(), &'static str> {
    let mut pending = read_record(pending_path).ok_or("bootstrap-state-invalid")?;
    if pending.get("state").and_then(Value::as_str) != Some("ReadyGrant") {
        return Err("grant-replay-or-invalid-state");
    }
    let original_binding = pending
        .get("binding")
        .cloned()
        .ok_or("bootstrap-binding-mismatch")?;
    if !same_continuation_binding(&original_binding, presented_binding) {
        return Err("bootstrap-binding-mismatch");
    }
    validate_grant(grant_path, &original_binding)?;
    let mut grant = read_record(grant_path).ok_or("grant-missing-or-invalid")?;
    let control = control_dir(worktree, &request.prompt_arg)?.join("codex-hook-grant.json");
    let mut control_record = read_record(&control).ok_or("grant-control-invalid")?;
    if control_record.get("session_id") != original_binding.get("session_id")
        || control_record.get("turn_id") != original_binding.get("turn_id")
        || control_record.get("worktree") != original_binding.get("worktree")
        || control_record.get("prompt_hash") != original_binding.get("prompt_hash")
        || control_record.get("generation") != original_binding.get("generation")
        || control_record.get("handler_digest") != original_binding.get("handler_digest")
    {
        return Err("grant-control-invalid");
    }
    grant["binding"] = presented_binding.clone();
    grant["presented_turn_id"] = json!(turn);
    control_record["turn_id"] = json!(turn);
    pending["binding"] = presented_binding.clone();
    pending["state"] = json!("GrantPresented");
    pending["presented_turn_id"] = json!(turn);

    // Publish the presentation in dependency order. PreToolUse keys off the
    // pending record, so a partial write fails closed until the final write.
    write_record(grant_path, &grant).map_err(|_| "grant-presentation-failed")?;
    write_record(&control, &control_record).map_err(|_| "grant-presentation-failed")?;
    write_record(pending_path, &pending).map_err(|_| "grant-presentation-failed")
}

fn same_continuation_binding(original: &Value, presented: &Value) -> bool {
    [
        "session_id",
        "worktree",
        "prompt_hash",
        "generation",
        "nonce",
        "handler_digest",
    ]
    .iter()
    .all(|key| original.get(key) == presented.get(key))
}

fn control_dir(worktree: &str, prompt_arg: &str) -> Result<PathBuf, &'static str> {
    let path = Path::new(prompt_arg);
    let file = path
        .file_name()
        .and_then(|v| v.to_str())
        .ok_or("invalid-plan-scope")?;
    let scope = file
        .strip_suffix(".prompt.md")
        .or_else(|| file.strip_suffix(".md"))
        .ok_or("invalid-plan-scope")?;
    if scope.is_empty()
        || !scope
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("invalid-plan-scope");
    }
    Ok(Path::new(worktree).join(".dev/pipeline").join(scope))
}

fn presented_action(
    value: &Value,
    request: Option<&ResolvedPipelineRequest>,
) -> Result<bool, &'static str> {
    let request = request.ok_or("missing-plan-scope")?;
    let action = continuation_action(&request.prompt_arg);
    Ok(
        value.get("action").and_then(Value::as_str) == Some(action.as_str())
            || value
                .get("tool_input")
                .and_then(|input| input.get("command"))
                .and_then(Value::as_str)
                == Some(action.as_str()),
    )
}

fn resolve_pipeline_request(
    worktree: &Path,
    prompt: &str,
) -> Result<Option<ResolvedPipelineRequest>, &'static str> {
    let mut parts = prompt.split_whitespace();
    let Some(command) = parts.next() else {
        return Ok(None);
    };
    let Some(subcommand) = parts.next() else {
        return Ok(None);
    };
    if !matches!(command.to_ascii_lowercase().as_str(), "gal" | "/gal")
        || !subcommand.eq_ignore_ascii_case("pipeline")
    {
        return Ok(None);
    }
    let tokens = parts.collect::<Vec<_>>();
    let continuation = tokens.first().copied() == Some("--require-codex-stop-v1");
    if continuation && tokens.len() != 2 {
        return Err("invalid-continuation-action");
    }
    let mut explicit = None;
    let mut index = usize::from(continuation);
    if let Some(first) = tokens.get(index).copied() {
        if first != "from" && first != "stop-at" {
            explicit = Some(first);
            index += 1;
        }
    }
    let mut has_modifier = false;
    while index < tokens.len() {
        if !matches!(tokens[index], "from" | "stop-at") {
            return Err("ambiguous-pipeline-request");
        }
        let task = tokens
            .get(index + 1)
            .copied()
            .ok_or("invalid-pipeline-modifier")?;
        if !is_task_id(task) {
            return Err("invalid-pipeline-modifier");
        }
        has_modifier = true;
        index += 2;
    }
    if has_modifier {
        return Err("guarded-pipeline-modifiers-unsupported");
    }
    let prompt_arg = match explicit {
        Some(token) => resolve_prompt_path(worktree, token)?,
        None => resolve_active_prompt(worktree)?,
    };
    if continuation && prompt != continuation_action(&prompt_arg) {
        return Err("invalid-continuation-action");
    }
    let bytes = fs::read(worktree.join(&prompt_arg)).map_err(|_| "execution-prompt-unavailable")?;
    if bytes.len() > MAX_PROMPT {
        return Err("execution-prompt-too-large");
    }
    Ok(Some(ResolvedPipelineRequest {
        prompt_arg,
        prompt_hash: digest(&bytes),
        continuation,
    }))
}

fn is_task_id(value: &str) -> bool {
    value
        .strip_prefix("T-")
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()))
}

fn resolve_active_prompt(worktree: &Path) -> Result<String, &'static str> {
    let state = fs::read_to_string(worktree.join(".dev/state.md"))
        .map_err(|_| "active-plan-unavailable")?;
    let mut in_active = false;
    let mut file_column = None;
    let mut candidates = Vec::new();
    for line in state.lines() {
        if line.trim() == "## Active Plans" {
            in_active = true;
            continue;
        }
        if in_active && line.trim_start().starts_with("## ") {
            break;
        }
        if !in_active || !line.trim_start().starts_with('|') {
            continue;
        }
        let cells = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if file_column.is_none() {
            file_column = cells.iter().position(|cell| *cell == "File");
            continue;
        }
        let Some(cell) = file_column.and_then(|column| cells.get(column)) else {
            continue;
        };
        if cell.chars().all(|c| matches!(c, '-' | ':' | ' ')) {
            continue;
        }
        let candidate = cell.trim_matches('`').trim();
        if candidate.ends_with(".md") {
            candidates.push(candidate.to_string());
        }
    }
    match candidates.as_slice() {
        [] => Err("active-plan-unavailable"),
        [candidate] => resolve_prompt_path(worktree, candidate),
        _ => Err("active-plan-ambiguous"),
    }
}

fn resolve_prompt_path(worktree: &Path, token: &str) -> Result<String, &'static str> {
    let raw = token
        .strip_prefix("#file:")
        .or_else(|| token.strip_prefix('@'))
        .unwrap_or(token);
    if raw.is_empty()
        || !raw
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '\\' | '-' | '_' | ':'))
        || raw.ends_with(".en.md")
        || raw.ends_with(".equiv.md")
    {
        return Err("invalid-plan-scope");
    }
    let supplied = PathBuf::from(raw);
    let supplied = if supplied.is_absolute() {
        supplied
    } else {
        worktree.join(supplied)
    };
    let candidate = if raw.ends_with(".prompt.md") {
        supplied
    } else if raw.ends_with(".md") {
        fs::canonicalize(&supplied).map_err(|_| "source-plan-unavailable")?;
        let normalized = supplied.to_string_lossy().replace('\\', "/");
        PathBuf::from(format!(
            "{}.prompt.md",
            normalized.strip_suffix(".md").ok_or("invalid-plan-scope")?
        ))
    } else {
        return Err("invalid-plan-scope");
    };
    let plans = fs::canonicalize(worktree.join(".dev/plans"))
        .map_err(|_| "execution-prompt-unavailable")?;
    let canonical = fs::canonicalize(candidate).map_err(|_| "execution-prompt-unavailable")?;
    if !canonical.starts_with(&plans)
        || !canonical
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.ends_with(".prompt.md"))
    {
        return Err("execution-prompt-outside-worktree");
    }
    let relative = canonical
        .strip_prefix(worktree)
        .map_err(|_| "execution-prompt-outside-worktree")?
        .to_string_lossy()
        .replace('\\', "/");
    if !relative
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '/' | '-' | '_'))
    {
        return Err("unsafe-execution-prompt-path");
    }
    Ok(relative)
}

fn continuation_action(prompt_arg: &str) -> String {
    format!("{ACTION_PREFIX} {prompt_arg}")
}
