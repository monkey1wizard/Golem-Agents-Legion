//! `gal quickfix-run --open --host <host>` — open one bounded quickfix attempt.
//! `gal quickfix-run --close <attempt-id> --session <ref>` — close it.
//!
//! Opening checks the refusal conditions in a fixed order and, when none applies,
//! records the attempt under `.dev/pipeline/quickfix/<attempt-id>/`: a manifest
//! (the durable record of what the attempt started from) and a spec (the
//! instructions handed to the host subagent). Every refusal prints
//! `QUICKFIX: <status>` and creates nothing. The manifest records `headAtOpen`
//! and `openTree`, a tree object of the working tree outside `.dev` built in a
//! throwaway index file inside the attempt directory.
//!
//! Closing takes an exclusive lock, verifies the host session, refuses a moved
//! `HEAD`, builds `closeTree` the same way, takes the change set as the difference
//! between the two trees, runs the quickfix gate on that set, and records one
//! terminal status in the manifest. A closed attempt cannot be closed again.
//!
//! `gal quickfix-run --commit <attempt-id>` lands a `completed` attempt on its own
//! branch `quickfix/<attempt-id>`, committing only the attempt's changed files, and
//! returns to the original branch. `gal quickfix-run --merge <attempt-id>` merges
//! that branch into the original branch with a plain `git merge`.

use super::finalize_check::CheckState;
use super::quickfix_check::{
    collect_working_tree_changes, evaluate_change_set, load_quickfix_config,
    render_quickfix_receipt, run_git, run_git_env, split_nul_paths, QuickfixConfig,
    QuickfixEvaluation, QuickfixVerdict,
};
use dispatch::host_session::{
    observed_model_policy, verify, verify_model_policy, HostSessionError, HostSessionRef,
    VerifiedHostSession,
};
use dispatch::model_tier::{HostModelMap, ModelTier};
use gal_engine::ExitCode;
use pipeline::coordinator::CoordinatorState;
use projection::ORCHESTRATED_ONLY_GOLEM_NAMES;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const PIPELINE_DIR: &str = ".dev/pipeline";
const ATTEMPT_SCOPE: &str = "quickfix";
const MANIFEST_FILE: &str = "manifest.json";
const SPEC_FILE: &str = "spec.md";
const COORDINATOR_FILE: &str = "coordinator.json";
const PREFLIGHT_RECEIPT: &str = "preflight.receipt.md";
const PLANS_DIR: &str = ".dev/plans";
/// Subdirectory of the attempt scope that holds hook state, not an attempt.
const HOOK_DIR: &str = "hook";
const CLOSE_RECEIPT: &str = "close.receipt.md";
const TERMINAL_KEY: &str = "terminalStatus";
const ATTEMPT_PHASE: &str = "quickfix";
/// Exclusive marker held while one close runs.
const LOCK_FILE: &str = "close.lock";
/// Repository state under `.dev` is never part of a change set.
const DEV_PREFIX: &str = ".dev/";
const DEV_EXCLUDE_PATHSPEC: &str = ":(exclude).dev";
/// Scope checks whose failure makes a closed attempt `scope-fail`.
const SCOPE_CHECKS: [&str; 4] = [
    "scope-files",
    "scope-lines",
    "protected-paths",
    "dependency-files",
];
/// Lock markers a plan-scoped coordinator leaves in its scope directory.
const COORDINATOR_LOCKS: [&str; 2] = [".lock", ".coordinator-revision-lock"];
const SUPPORTED_HOSTS: [&str; 2] = ["claude", "codex"];
const MAX_OUTPUT_BYTES: usize = 16 * 1024;
const BRANCH_PREFIX: &str = "quickfix/";
const COMMITTED_STATUS: &str = "committed";
const MERGED_STATUS: &str = "merged";

/// Why an attempt was not opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Refusal {
    PipelineActive,
    /// An earlier attempt has no terminal status. Holds the attempt directory.
    AttemptOpen(String),
    HostSubagentUnsupported(String),
    NoChange,
    ChecksPass,
    AttemptLimit,
    ScopeExceeded(String),
    ChecksNotRun(String),
}

impl Refusal {
    fn status_line(&self) -> String {
        match self {
            Self::PipelineActive => "QUICKFIX: pipeline-active".to_string(),
            Self::AttemptOpen(dir) => format!(
                "QUICKFIX: attempt-open dir={dir} run `gal quickfix-run --close <attempt> --session <ref>` for that attempt, and if a stale {LOCK_FILE} remains from a killed process, remove that lock first"
            ),
            Self::HostSubagentUnsupported(host) => {
                format!("QUICKFIX: host-subagent-unsupported host={host}")
            }
            Self::NoChange => "QUICKFIX: no-change".to_string(),
            Self::ChecksPass => "QUICKFIX: checks-pass".to_string(),
            Self::AttemptLimit => "QUICKFIX: attempt-limit".to_string(),
            Self::ScopeExceeded(reason) => format!("QUICKFIX: scope-exceeded {reason}"),
            Self::ChecksNotRun(reason) => format!("QUICKFIX: checks-not-run {reason}"),
        }
    }
}

/// An attempt that was opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OpenedAttempt {
    pub(crate) attempt_id: String,
    pub(crate) spec_path: PathBuf,
    pub(crate) model: String,
}

/// Result of an open request.
#[derive(Debug)]
pub(crate) enum OpenOutcome {
    Opened(OpenedAttempt),
    Refused(Refusal),
}

fn pipeline_root(repo: &Path) -> PathBuf {
    repo.join(".dev").join("pipeline")
}

fn git_text(repo: &Path, args: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&run_git(repo, args)?)
        .trim()
        .to_string())
}

/// Value of the `prompt:` line of a preflight receipt, when it is a bare file name.
fn receipt_prompt_name(receipt: &Path) -> Option<String> {
    let text = std::fs::read_to_string(receipt).ok()?;
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix("prompt:"))?
        .trim();
    let bare = !value.is_empty() && !value.contains(['/', '\\']) && value != "..";
    bare.then(|| value.to_string())
}

/// True when the plan prompt `name` exists and is not finished. A prompt without
/// a readable `Workflow:` line counts as unfinished.
fn prompt_is_unfinished(repo: &Path, name: &str) -> bool {
    let path = repo.join(PLANS_DIR).join(name);
    if !path.is_file() {
        return false;
    }
    let workflow = std::fs::read_to_string(&path).ok().and_then(|text| {
        text.lines()
            .find_map(|line| line.strip_prefix("Workflow:"))
            .map(|value| value.trim().to_string())
    });
    !matches!(workflow.as_deref(), Some("DONE"))
}

/// First attempt directory under `.dev/pipeline/quickfix` whose manifest has no
/// terminal status or cannot be parsed. A directory without a manifest is not an
/// attempt.
fn open_attempt_dir(repo: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(pipeline_root(repo).join(ATTEMPT_SCOPE)).ok()?;
    let mut dirs: Vec<PathBuf> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir() && entry.file_name() != HOOK_DIR)
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs.into_iter().find(|dir| {
        let manifest = dir.join(MANIFEST_FILE);
        manifest.exists()
            && match load_manifest(&manifest) {
                Ok(value) => value.get(TERMINAL_KEY).is_none(),
                Err(_) => true,
            }
    })
}

/// True when any plan scope under `.dev/pipeline` holds a coordinator lock, a
/// coordinator state with an active attempt, or a preflight receipt for a plan
/// prompt that is not finished. A coordinator state that cannot be read or
/// parsed counts as active.
fn pipeline_is_active(repo: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(pipeline_root(repo)) else {
        return false;
    };
    for entry in entries.flatten() {
        let scope_dir = entry.path();
        if !scope_dir.is_dir() || entry.file_name() == ATTEMPT_SCOPE {
            continue;
        }
        if COORDINATOR_LOCKS
            .iter()
            .any(|lock| scope_dir.join(lock).exists())
        {
            return true;
        }
        if receipt_prompt_name(&scope_dir.join(PREFLIGHT_RECEIPT))
            .is_some_and(|name| prompt_is_unfinished(repo, &name))
        {
            return true;
        }
        let state_path = scope_dir.join(COORDINATOR_FILE);
        if !state_path.exists() {
            continue;
        }
        let parsed = std::fs::read(&state_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CoordinatorState>(&bytes).ok());
        match parsed {
            Some(state) if state.active_attempt.is_none() => {}
            _ => return true,
        }
    }
    false
}

/// SHA-256 over the sorted failing commands and the commit the attempt started at.
fn failure_fingerprint(failing_commands: &[String], head_at_open: &str) -> String {
    let mut sorted = failing_commands.to_vec();
    sorted.sort();
    let mut hasher = Sha256::new();
    hasher.update(sorted.join("\n").as_bytes());
    hasher.update([0u8]);
    hasher.update(head_at_open.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Number of earlier attempts recorded with `fingerprint`.
fn count_earlier_attempts(repo: &Path, fingerprint: &str) -> usize {
    let Ok(entries) = std::fs::read_dir(pipeline_root(repo).join(ATTEMPT_SCOPE)) else {
        return 0;
    };
    entries
        .flatten()
        .filter_map(|entry| std::fs::read(entry.path().join(MANIFEST_FILE)).ok())
        .filter_map(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .filter(|manifest| manifest.get("fingerprint").and_then(Value::as_str) == Some(fingerprint))
        .count()
}

fn bound_output(text: &str) -> String {
    if text.len() <= MAX_OUTPUT_BYTES {
        return text.to_string();
    }
    let mut end = MAX_OUTPUT_BYTES;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_string()
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
///
/// The tree is built in a temporary index file with a unique name inside
/// `scratch_dir`, so the repository's own index is never touched.
fn snapshot_tree(repo: &Path, scratch_dir: &Path) -> Result<String, String> {
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

/// Paths that differ between two tree objects.
fn tree_changes(repo: &Path, from: &str, to: &str) -> Result<BTreeSet<String>, String> {
    let bytes = run_git(
        repo,
        &[
            "diff-tree",
            "-r",
            "--name-only",
            "--no-renames",
            "-z",
            from,
            to,
        ],
    )?;
    Ok(split_nul_paths(&bytes).into_iter().collect())
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = (day_of_year - (153 * month_index + 2) / 5 + 1) as u32;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    } as u32;
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn format_utc(epoch_seconds: u64) -> String {
    let days = (epoch_seconds / 86_400) as i64;
    let remainder = epoch_seconds % 86_400;
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        remainder / 3_600,
        remainder % 3_600 / 60,
        remainder % 60
    )
}

fn render_spec(attempt_id: &str, config: &QuickfixConfig, failing: &[(String, String)]) -> String {
    let mut out = format!("# Quickfix attempt {attempt_id}\n\n");
    out.push_str("Fix only the failing findings listed below.\n\n");
    out.push_str("## Rules\n\n");
    out.push_str(&format!(
        "- Stay within {} file(s) and {} changed line(s) in total.\n",
        config.max_files, config.max_lines
    ));
    out.push_str("- Do not commit and do not push.\n");
    out.push_str(&format!(
        "- Do not edit anything under `{PIPELINE_DIR}/{ATTEMPT_SCOPE}/{attempt_id}/`.\n"
    ));
    out.push_str("- Do not touch Protected Paths or dependency manifests.\n\n");
    out.push_str("## Failing commands\n\n");
    for (command, output) in failing {
        out.push_str(&format!("### `{command}`\n\n```text\n{output}\n```\n\n"));
    }
    out
}

fn write_atomically(path: &Path, content: &[u8]) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let temporary = path.with_file_name(format!(".{name}.{}.tmp", unique_suffix()));
    std::fs::write(&temporary, content)
        .map_err(|error| format!("cannot write {}: {error}", temporary.display()))?;
    std::fs::rename(&temporary, path).map_err(|error| {
        let _ = std::fs::remove_file(&temporary);
        format!("cannot publish {}: {error}", path.display())
    })
}

fn new_attempt_id() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("qf_{millis}_{}", std::process::id())
}

/// Open one quickfix attempt for the repository at `repo`.
///
/// Check commands run in the process working directory, so this sets the process
/// working directory to `repo` before evaluating them.
pub(crate) fn open_attempt(repo: &Path, host: &str) -> Result<OpenOutcome, String> {
    if pipeline_is_active(repo) {
        return Ok(OpenOutcome::Refused(Refusal::PipelineActive));
    }
    if let Some(dir) = open_attempt_dir(repo) {
        let name = dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        return Ok(OpenOutcome::Refused(Refusal::AttemptOpen(format!(
            "{PIPELINE_DIR}/{ATTEMPT_SCOPE}/{name}"
        ))));
    }
    if !SUPPORTED_HOSTS.contains(&host) {
        return Ok(OpenOutcome::Refused(Refusal::HostSubagentUnsupported(
            host.to_string(),
        )));
    }
    let changed = collect_working_tree_changes(repo)?;
    if changed.is_empty() {
        return Ok(OpenOutcome::Refused(Refusal::NoChange));
    }

    let config = load_quickfix_config()?;
    std::env::set_current_dir(repo)
        .map_err(|error| format!("cannot enter {}: {error}", repo.display()))?;
    let evaluation = evaluate_change_set(repo, &config, &changed);
    let failing: Vec<(String, String)> = evaluation
        .receipt
        .checks
        .iter()
        .filter(|check| check.state == CheckState::Fail)
        .filter_map(|check| {
            check
                .command
                .as_ref()
                .map(|command| (command.clone(), bound_output(&check.summary)))
        })
        .collect();
    if failing.is_empty() {
        let refusal = match evaluation.verdict_kind() {
            QuickfixVerdict::Pass => Refusal::ChecksPass,
            QuickfixVerdict::Fail | QuickfixVerdict::NotRun => {
                let reasons: Vec<String> = evaluation
                    .receipt
                    .checks
                    .iter()
                    .filter(|check| check.state != CheckState::Pass)
                    .map(|check| format!("{}: {}", check.name, check.summary))
                    .collect();
                if evaluation
                    .receipt
                    .checks
                    .iter()
                    .any(|check| check.state == CheckState::Fail)
                {
                    Refusal::ScopeExceeded(reasons.join("; "))
                } else {
                    Refusal::ChecksNotRun(reasons.join("; "))
                }
            }
        };
        return Ok(OpenOutcome::Refused(refusal));
    }

    let failing_commands: Vec<String> =
        failing.iter().map(|(command, _)| command.clone()).collect();
    let head_at_open = git_text(repo, &["rev-parse", "HEAD"])?;
    let fingerprint = failure_fingerprint(&failing_commands, &head_at_open);
    if count_earlier_attempts(repo, &fingerprint) as u64 >= config.max_attempts {
        return Ok(OpenOutcome::Refused(Refusal::AttemptLimit));
    }

    let model = HostModelMap::load_default()
        .and_then(|map| map.resolve(host, ModelTier::Light))
        .map_err(|error| format!("cannot resolve the {host} light model: {error}"))?;
    let original_branch = git_text(repo, &["rev-parse", "--abbrev-ref", "HEAD"])?;
    let started = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let attempt_id = new_attempt_id();
    let attempt_dir = repo
        .join(".dev")
        .join("pipeline")
        .join(ATTEMPT_SCOPE)
        .join(&attempt_id);
    std::fs::create_dir_all(&attempt_dir)
        .map_err(|error| format!("cannot create {}: {error}", attempt_dir.display()))?;
    let open_tree = match snapshot_tree(repo, &attempt_dir) {
        Ok(tree) => tree,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&attempt_dir);
            return Err(error);
        }
    };

    let manifest = json!({
        "attemptId": attempt_id,
        "host": host,
        "originalBranch": original_branch,
        "startedAt": format_utc(started),
        "startedAtEpochSeconds": started,
        "model": model.model,
        "effort": model.effort,
        "fingerprint": fingerprint,
        "failingCommands": failing
            .iter()
            .map(|(command, output)| json!({"command": command, "output": output}))
            .collect::<Vec<_>>(),
        "limits": {
            "maxFiles": config.max_files,
            "maxLines": config.max_lines,
            "maxAttempts": config.max_attempts,
        },
        "headAtOpen": head_at_open,
        "openTree": open_tree,
    });
    let spec_path = attempt_dir.join(SPEC_FILE);
    let written = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("cannot encode manifest: {error}"))
        .and_then(|bytes| write_atomically(&attempt_dir.join(MANIFEST_FILE), &bytes))
        .and_then(|()| {
            write_atomically(
                &spec_path,
                render_spec(&attempt_id, &config, &failing).as_bytes(),
            )
        });
    if let Err(error) = written {
        let _ = std::fs::remove_dir_all(&attempt_dir);
        return Err(error);
    }
    Ok(OpenOutcome::Opened(OpenedAttempt {
        attempt_id,
        spec_path,
        model: model.model,
    }))
}

/// Terminal status recorded on a closed attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TerminalStatus {
    Completed,
    SessionInvalid,
    ModelTierMismatch,
    HeadMoved,
    NoChange,
    ScopeFail,
    CheckFail,
}

impl TerminalStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::SessionInvalid => "session-invalid",
            Self::ModelTierMismatch => "model-tier-mismatch",
            Self::HeadMoved => "head-moved",
            Self::NoChange => "no-change",
            Self::ScopeFail => "scope-fail",
            Self::CheckFail => "check-fail",
        }
    }
}

/// What a close established: the status, the change set, and the working-tree
/// changes that are outside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClosedAttempt {
    pub(crate) status: TerminalStatus,
    pub(crate) changed: Vec<String>,
    pub(crate) outside: Vec<String>,
}

impl ClosedAttempt {
    fn without_changes(status: TerminalStatus) -> Self {
        Self {
            status,
            changed: Vec::new(),
            outside: Vec::new(),
        }
    }
}

/// Result of a close request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CloseOutcome {
    Closed(ClosedAttempt),
    AlreadyClosed,
}

/// Exclusive close marker. Created with `create_new`, removed on every exit path.
struct CloseLock {
    path: PathBuf,
}

impl CloseLock {
    fn acquire(attempt_dir: &Path) -> Result<Self, String> {
        let path = attempt_dir.join(LOCK_FILE);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(_) => Ok(Self { path }),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Err(format!(
                "another close of this attempt holds {}",
                path.display()
            )),
            Err(error) => Err(format!(
                "cannot create close lock {}: {error}",
                path.display()
            )),
        }
    }
}

impl Drop for CloseLock {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn manifest_text(manifest: &Value, key: &str) -> Result<String, String> {
    manifest
        .get(key)
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
        .ok_or_else(|| format!("manifest field {key} is missing or malformed"))
}

fn manifest_number(object: &Value, key: &str) -> Result<u64, String> {
    object
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("manifest field {key} is missing or malformed"))
}

fn manifest_object_id(manifest: &Value, key: &str) -> Result<String, String> {
    let text = manifest_text(manifest, key)?;
    if is_object_id(&text) {
        Ok(text)
    } else {
        Err(format!("manifest field {key} is not a git object id"))
    }
}

/// The facts the open step recorded, all validated.
struct OpenRecord {
    host: String,
    model: String,
    effort: Option<String>,
    started: u64,
    head_at_open: String,
    open_tree: String,
}

fn read_open_record(manifest: &Value, attempt_id: &str) -> Result<OpenRecord, String> {
    if manifest_text(manifest, "attemptId")? != attempt_id {
        return Err("manifest attemptId does not match the attempt".to_string());
    }
    manifest_text(manifest, "originalBranch")?;
    manifest_text(manifest, "startedAt")?;
    let limits = manifest
        .get("limits")
        .filter(|limits| limits.is_object())
        .ok_or("manifest field limits is missing or malformed")?;
    for key in ["maxFiles", "maxLines", "maxAttempts"] {
        manifest_number(limits, key)?;
    }
    let failing = manifest
        .get("failingCommands")
        .and_then(Value::as_array)
        .filter(|commands| !commands.is_empty())
        .ok_or("manifest field failingCommands is missing or malformed")?;
    for entry in failing {
        manifest_text(entry, "command")?;
    }
    let effort = match manifest.get("effort") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => return Err("manifest field effort is malformed".to_string()),
    };
    Ok(OpenRecord {
        host: manifest_text(manifest, "host")?,
        model: manifest_text(manifest, "model")?,
        effort,
        started: manifest_number(manifest, "startedAtEpochSeconds")?,
        head_at_open: manifest_object_id(manifest, "headAtOpen")?,
        open_tree: manifest_object_id(manifest, "openTree")?,
    })
}

fn load_manifest(path: &Path) -> Result<Value, String> {
    let bytes = std::fs::read(path)
        .map_err(|error| format!("cannot read attempt manifest {}: {error}", path.display()))?;
    let manifest: Value = serde_json::from_slice(&bytes)
        .map_err(|error| format!("attempt manifest is not valid JSON: {error}"))?;
    if manifest.is_object() {
        Ok(manifest)
    } else {
        Err("attempt manifest is not a JSON object".to_string())
    }
}

/// Why the host session did not verify.
enum SessionFailure {
    /// The session record is not there yet. The attempt stays open.
    NotReady(String),
    /// The session record exists and does not support the attempt.
    Invalid(String),
}

/// Verify the host session against the attempt, as the pipeline close does.
fn verify_session(
    repo: &Path,
    home: &Path,
    record: &OpenRecord,
    attempt_id: &str,
    session: &str,
) -> Result<VerifiedHostSession, SessionFailure> {
    let reference = HostSessionRef::parse(session)
        .map_err(|error| SessionFailure::Invalid(error.to_string()))?;
    if reference.host.as_str() != record.host {
        return Err(SessionFailure::Invalid(format!(
            "session host {} does not match attempt host {}",
            reference.host.as_str(),
            record.host
        )));
    }
    verify(
        &reference,
        repo,
        home,
        &format!("epoch+{}s", record.started),
        attempt_id,
        ATTEMPT_PHASE,
        ORCHESTRATED_ONLY_GOLEM_NAMES,
    )
    .map_err(|error| match error {
        HostSessionError::RecordMissing => SessionFailure::NotReady(error.to_string()),
        other => SessionFailure::Invalid(other.to_string()),
    })
}

/// Judge the attempt and return what it established, recording the facts in
/// `manifest`. Errors leave the attempt open.
fn run_close(
    repo: &Path,
    attempt_dir: &Path,
    attempt_id: &str,
    session: &str,
    home: &Path,
    manifest: &mut Value,
) -> Result<ClosedAttempt, String> {
    let record = match read_open_record(manifest, attempt_id) {
        Ok(record) => record,
        Err(reason) => {
            eprintln!("gal quickfix-run: attempt manifest: {reason}");
            return Ok(ClosedAttempt::without_changes(
                TerminalStatus::SessionInvalid,
            ));
        }
    };
    let verified = match verify_session(repo, home, &record, attempt_id, session) {
        Ok(verified) => verified,
        Err(SessionFailure::NotReady(reason)) => {
            return Err(format!(
                "session evidence is not ready ({reason}); the attempt stays open"
            ));
        }
        Err(SessionFailure::Invalid(reason)) => {
            eprintln!("gal quickfix-run: session verification: {reason}");
            return Ok(ClosedAttempt::without_changes(
                TerminalStatus::SessionInvalid,
            ));
        }
    };
    manifest["sessionId"] = json!(verified.session_id);
    if let Some(observed) = observed_model_policy(&verified) {
        manifest["observedModel"] = json!(observed.model);
        manifest["observedEffort"] = json!(observed.effort);
    }
    match verify_model_policy(&verified, &record.model, record.effort.as_deref()) {
        Ok(_) => {}
        Err(HostSessionError::ModelMismatch | HostSessionError::EffortMismatch) => {
            return Ok(ClosedAttempt::without_changes(
                TerminalStatus::ModelTierMismatch,
            ))
        }
        Err(error) => {
            eprintln!("gal quickfix-run: model evidence: {error}");
            return Ok(ClosedAttempt::without_changes(
                TerminalStatus::SessionInvalid,
            ));
        }
    }

    let head_now = git_text(repo, &["rev-parse", "HEAD"])?;
    if head_now != record.head_at_open {
        eprintln!(
            "gal quickfix-run: HEAD moved from {} to {head_now} during the attempt",
            record.head_at_open
        );
        return Ok(ClosedAttempt::without_changes(TerminalStatus::HeadMoved));
    }

    let close_tree = snapshot_tree(repo, attempt_dir)?;
    manifest["closeTree"] = json!(close_tree);
    let change_set = tree_changes(repo, &record.open_tree, &close_tree)?;
    let changed: Vec<String> = change_set.iter().cloned().collect();
    manifest["changedFiles"] = json!(changed);
    let outside: Vec<String> = collect_working_tree_changes(repo)?
        .into_iter()
        .filter(|path| !path.starts_with(DEV_PREFIX) && !change_set.contains(path))
        .collect();
    let closed = |status| ClosedAttempt {
        status,
        changed: changed.clone(),
        outside: outside.clone(),
    };
    if change_set.is_empty() {
        return Ok(closed(TerminalStatus::NoChange));
    }

    let config = load_quickfix_config()?;
    std::env::set_current_dir(repo)
        .map_err(|error| format!("cannot enter {}: {error}", repo.display()))?;
    let evaluation = evaluate_change_set(repo, &config, &change_set);
    write_atomically(
        &attempt_dir.join(CLOSE_RECEIPT),
        render_quickfix_receipt(&evaluation).as_bytes(),
    )?;
    manifest["closeReceipt"] = json!(format!(
        "{PIPELINE_DIR}/{ATTEMPT_SCOPE}/{attempt_id}/{CLOSE_RECEIPT}"
    ));
    Ok(closed(terminal_for(&evaluation)))
}

/// Map a gate evaluation to a terminal status. A `not-run` gate, such as a
/// project without an authoritative check fence, is not a pass.
fn terminal_for(evaluation: &QuickfixEvaluation) -> TerminalStatus {
    match evaluation.verdict_kind() {
        QuickfixVerdict::Pass => TerminalStatus::Completed,
        QuickfixVerdict::Fail => {
            let scope_failed = evaluation.receipt.checks.iter().any(|check| {
                check.state == CheckState::Fail && SCOPE_CHECKS.contains(&check.name.as_str())
            });
            if scope_failed {
                TerminalStatus::ScopeFail
            } else {
                TerminalStatus::CheckFail
            }
        }
        QuickfixVerdict::NotRun => TerminalStatus::CheckFail,
    }
}

/// Directory of the attempt `attempt_id`, after validating the id.
fn locate_attempt(repo: &Path, attempt_id: &str) -> Result<PathBuf, String> {
    if attempt_id.is_empty()
        || !attempt_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(format!("invalid attempt id '{attempt_id}'"));
    }
    let attempt_dir = pipeline_root(repo).join(ATTEMPT_SCOPE).join(attempt_id);
    if !attempt_dir.is_dir() {
        return Err(format!("no such attempt: {}", attempt_dir.display()));
    }
    Ok(attempt_dir)
}

/// Close one quickfix attempt. A repeated close is refused and changes nothing.
pub(crate) fn close_attempt(
    repo: &Path,
    attempt_id: &str,
    session: &str,
    home: &Path,
) -> Result<CloseOutcome, String> {
    let attempt_dir = locate_attempt(repo, attempt_id)?;
    let _lock = CloseLock::acquire(&attempt_dir)?;
    let manifest_path = attempt_dir.join(MANIFEST_FILE);
    let mut manifest = load_manifest(&manifest_path)?;
    if manifest.get(TERMINAL_KEY).is_some() {
        return Ok(CloseOutcome::AlreadyClosed);
    }
    let closed = run_close(repo, &attempt_dir, attempt_id, session, home, &mut manifest)?;
    let closed_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    manifest[TERMINAL_KEY] = json!(closed.status.as_str());
    manifest["closedAt"] = json!(format_utc(closed_at));
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("cannot encode manifest: {error}"))?;
    write_atomically(&manifest_path, &bytes)?;
    Ok(CloseOutcome::Closed(closed))
}

/// Why a commit or merge request was refused. Nothing changes on a refusal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LandRefusal {
    /// The attempt is not `completed`. Holds the recorded status, or `open`.
    NotCompleted(String),
    ReceiptNotPass,
    HeadMoved,
    SnapshotChanged,
    /// The original branch is not the checked-out branch.
    WrongBranch {
        expected: String,
        actual: String,
    },
    /// The attempt is not `committed`. Holds the recorded status, or `open`.
    NotCommitted(String),
    /// A merge is already in progress in the repository.
    MergeInProgress,
}

impl LandRefusal {
    fn status_line(&self) -> String {
        match self {
            Self::NotCompleted(status) => format!("QUICKFIX: not-completed status={status}"),
            Self::ReceiptNotPass => "QUICKFIX: receipt-not-pass".to_string(),
            Self::HeadMoved => "QUICKFIX: head-moved".to_string(),
            Self::SnapshotChanged => "QUICKFIX: snapshot-changed".to_string(),
            Self::WrongBranch { expected, actual } => {
                format!("QUICKFIX: wrong-branch expected={expected} actual={actual}")
            }
            Self::NotCommitted(status) => format!("QUICKFIX: not-committed status={status}"),
            Self::MergeInProgress => "QUICKFIX: merge-in-progress".to_string(),
        }
    }
}

/// An attempt that was committed to its own branch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CommittedAttempt {
    pub(crate) commit_id: String,
    pub(crate) branch: String,
    /// Working-tree changes the commit left alone, outside `.dev`.
    pub(crate) outside: Vec<String>,
}

#[derive(Debug)]
pub(crate) enum CommitOutcome {
    Committed(CommittedAttempt),
    Refused(LandRefusal),
}

#[derive(Debug)]
pub(crate) enum MergeOutcome {
    Merged { branch: String },
    Refused(LandRefusal),
}

fn recorded_status(manifest: &Value) -> String {
    manifest
        .get(TERMINAL_KEY)
        .and_then(Value::as_str)
        .unwrap_or("open")
        .to_string()
}

fn manifest_changed_files(manifest: &Value) -> Result<Vec<String>, String> {
    let entries = manifest
        .get("changedFiles")
        .and_then(Value::as_array)
        .filter(|entries| !entries.is_empty())
        .ok_or("manifest field changedFiles is missing or malformed")?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .as_str()
            .filter(|path| !path.is_empty())
            .ok_or("manifest field changedFiles is missing or malformed")?;
        if path.starts_with(DEV_PREFIX) {
            return Err(format!("changedFiles names a path under .dev: {path}"));
        }
        paths.push(path.to_string());
    }
    Ok(paths)
}

fn receipt_passes(attempt_dir: &Path) -> bool {
    std::fs::read_to_string(attempt_dir.join(CLOSE_RECEIPT))
        .map(|text| text.lines().any(|line| line.trim() == "overall: pass"))
        .unwrap_or(false)
}

fn commit_message(manifest: &Value) -> String {
    let commands: Vec<String> = manifest
        .get("failingCommands")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.get("command").and_then(Value::as_str))
        .map(|command| format!("- {command}"))
        .collect();
    format!(
        "fix: repair failing authoritative checks\n\nFailing commands:\n{}",
        commands.join("\n")
    )
}

/// Run git with pathspecs taken literally, so file names never act as patterns.
fn run_git_literal(repo: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let mut full = vec!["--literal-pathspecs"];
    full.extend_from_slice(args);
    run_git(repo, &full)
}

/// Commit exactly `paths` on the checked-out branch and return the commit id.
///
/// Files that are not tracked yet are added with intent-to-add first, because
/// `git commit --only` accepts tracked paths only. A failure removes those
/// intent-to-add entries again and leaves any other staged file as it was.
fn commit_only(repo: &Path, paths: &[String], message: &str) -> Result<String, String> {
    let mut listing = vec!["ls-files", "-z", "--"];
    listing.extend(paths.iter().map(String::as_str));
    let tracked: BTreeSet<String> = split_nul_paths(&run_git_literal(repo, &listing)?)
        .into_iter()
        .collect();
    let mut head_listing = vec!["ls-tree", "-z", "--name-only", "HEAD", "--"];
    head_listing.extend(paths.iter().map(String::as_str));
    let in_head: BTreeSet<String> = split_nul_paths(&run_git_literal(repo, &head_listing)?)
        .into_iter()
        .collect();
    let mut new_files: Vec<&str> = Vec::new();
    let mut commit_paths: Vec<&str> = Vec::new();
    let mut dropped: Vec<&str> = Vec::new();
    for path in paths.iter().map(String::as_str) {
        if tracked.contains(path) || in_head.contains(path) {
            // A path removed with `git rm` is in HEAD only, and the commit records its deletion.
            commit_paths.push(path);
        } else if repo.join(path).exists() {
            new_files.push(path);
            commit_paths.push(path);
        } else {
            dropped.push(path);
        }
    }
    if !dropped.is_empty() {
        return Err(format!(
            "changed files cannot be committed: {}",
            dropped.join(", ")
        ));
    }
    if commit_paths.is_empty() {
        return Err("none of the attempt's changed files can be committed".to_string());
    }
    if !new_files.is_empty() {
        let mut add = vec!["add", "-N", "--"];
        add.extend(new_files.iter().copied());
        run_git_literal(repo, &add)?;
    }
    let mut commit = vec!["commit", "--only", "-q", "-m", message, "--"];
    commit.extend(commit_paths.iter().copied());
    let committed =
        run_git_literal(repo, &commit).and_then(|_| git_text(repo, &["rev-parse", "HEAD"]));
    if committed.is_err() && !new_files.is_empty() {
        let mut reset = vec!["reset", "-q", "--"];
        reset.extend(new_files.iter().copied());
        let _ = run_git_literal(repo, &reset);
    }
    committed
}

fn epoch_seconds_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Commit a `completed` attempt on `quickfix/<attempt-id>` and return to the
/// original branch. A git failure leaves the recorded status unchanged.
pub(crate) fn commit_attempt(repo: &Path, attempt_id: &str) -> Result<CommitOutcome, String> {
    let attempt_dir = locate_attempt(repo, attempt_id)?;
    let _lock = CloseLock::acquire(&attempt_dir)?;
    let manifest_path = attempt_dir.join(MANIFEST_FILE);
    let mut manifest = load_manifest(&manifest_path)?;
    let status = recorded_status(&manifest);
    if status != TerminalStatus::Completed.as_str() {
        return Ok(CommitOutcome::Refused(LandRefusal::NotCompleted(status)));
    }
    if !receipt_passes(&attempt_dir) {
        return Ok(CommitOutcome::Refused(LandRefusal::ReceiptNotPass));
    }
    let head_at_open = manifest_object_id(&manifest, "headAtOpen")?;
    let close_tree = manifest_object_id(&manifest, "closeTree")?;
    let original = manifest_text(&manifest, "originalBranch")?;
    let changed = manifest_changed_files(&manifest)?;
    if git_text(repo, &["rev-parse", "HEAD"])? != head_at_open {
        return Ok(CommitOutcome::Refused(LandRefusal::HeadMoved));
    }
    let actual = git_text(repo, &["branch", "--show-current"])?;
    if actual != original {
        return Ok(CommitOutcome::Refused(LandRefusal::WrongBranch {
            expected: original,
            actual,
        }));
    }
    if snapshot_tree(repo, &attempt_dir)? != close_tree {
        return Ok(CommitOutcome::Refused(LandRefusal::SnapshotChanged));
    }

    let branch = format!("{BRANCH_PREFIX}{attempt_id}");
    run_git(repo, &["switch", "-c", &branch])?;
    let commit_id = match commit_only(repo, &changed, &commit_message(&manifest)) {
        Ok(commit_id) => commit_id,
        Err(error) => {
            return Err(match run_git(repo, &["switch", &original]) {
                Ok(_) => {
                    let _ = run_git(repo, &["branch", "-d", &branch]);
                    format!("{error}; returned to {original} and removed {branch}")
                }
                Err(back) => format!(
                    "{error}; {back}; the repository is still on {branch}, run `git switch {original}` to leave it"
                ),
            });
        }
    };
    if let Err(error) = run_git(repo, &["switch", &original]) {
        return Err(format!(
            "{error}; commit {commit_id} is on {branch} and the attempt is not recorded as committed, run `git switch {original}` to leave the branch"
        ));
    }

    let outside: Vec<String> = collect_working_tree_changes(repo)?
        .into_iter()
        .filter(|path| !path.starts_with(DEV_PREFIX))
        .collect();
    manifest[TERMINAL_KEY] = json!(COMMITTED_STATUS);
    manifest["commitId"] = json!(commit_id);
    manifest["quickfixBranch"] = json!(branch);
    manifest["committedAt"] = json!(format_utc(epoch_seconds_now()));
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("cannot encode manifest: {error}"))?;
    write_atomically(&manifest_path, &bytes).map_err(|error| {
        format!(
            "{error}; commit {commit_id} is on {branch} but the attempt is not recorded as committed"
        )
    })?;
    Ok(CommitOutcome::Committed(CommittedAttempt {
        commit_id,
        branch,
        outside,
    }))
}

/// Merge the branch of a `committed` attempt into the original branch with a
/// plain `git merge`. The branch stays. A failed merge is aborted and the
/// recorded status stays `committed`.
pub(crate) fn merge_attempt(repo: &Path, attempt_id: &str) -> Result<MergeOutcome, String> {
    let attempt_dir = locate_attempt(repo, attempt_id)?;
    let _lock = CloseLock::acquire(&attempt_dir)?;
    let manifest_path = attempt_dir.join(MANIFEST_FILE);
    let mut manifest = load_manifest(&manifest_path)?;
    let status = recorded_status(&manifest);
    if status != COMMITTED_STATUS {
        return Ok(MergeOutcome::Refused(LandRefusal::NotCommitted(status)));
    }
    let original = manifest_text(&manifest, "originalBranch")?;
    let actual = git_text(repo, &["branch", "--show-current"])?;
    if actual != original {
        return Ok(MergeOutcome::Refused(LandRefusal::WrongBranch {
            expected: original,
            actual,
        }));
    }
    if run_git(repo, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok() {
        return Ok(MergeOutcome::Refused(LandRefusal::MergeInProgress));
    }
    let branch = format!("{BRANCH_PREFIX}{attempt_id}");
    // No merge was in progress before this call, so a `MERGE_HEAD` now is this call's own.
    if let Err(error) = run_git(repo, &["merge", "--no-edit", &branch]) {
        if run_git(repo, &["rev-parse", "-q", "--verify", "MERGE_HEAD"]).is_ok() {
            let _ = run_git(repo, &["merge", "--abort"]);
        }
        return Err(error);
    }
    manifest[TERMINAL_KEY] = json!(MERGED_STATUS);
    manifest["mergedAt"] = json!(format_utc(epoch_seconds_now()));
    let bytes = serde_json::to_vec_pretty(&manifest)
        .map_err(|error| format!("cannot encode manifest: {error}"))?;
    write_atomically(&manifest_path, &bytes).map_err(|error| {
        format!("{error}; {branch} is merged but the attempt is not recorded as merged")
    })?;
    Ok(MergeOutcome::Merged { branch })
}

/// Parsed `gal quickfix-run` request.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Request {
    Open { host: String },
    Close { attempt: String, session: String },
    Commit { attempt: String },
    Merge { attempt: String },
}

/// Parse `<flag> <attempt>` with no other argument.
fn parse_attempt_flag(args: &[String], flag: &str) -> Result<String, String> {
    let mut attempt: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            attempt = Some(
                iter.next()
                    .ok_or_else(|| format!("{flag} requires an attempt id"))?
                    .clone(),
            );
        } else {
            return Err(format!("unexpected argument '{arg}'"));
        }
    }
    attempt.ok_or_else(|| format!("{flag} requires an attempt id"))
}

fn parse_close_arguments(args: &[String]) -> Result<(String, String), String> {
    let mut attempt: Option<String> = None;
    let mut session: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--close" => {
                attempt = Some(iter.next().ok_or("--close requires an attempt id")?.clone())
            }
            "--session" => {
                session = Some(iter.next().ok_or("--session requires a reference")?.clone())
            }
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    Ok((
        attempt.ok_or("--close requires an attempt id")?,
        session.ok_or("--session is required")?,
    ))
}

fn parse_request(args: &[String]) -> Result<Request, String> {
    if args.iter().any(|arg| arg == "--commit") {
        parse_attempt_flag(args, "--commit").map(|attempt| Request::Commit { attempt })
    } else if args.iter().any(|arg| arg == "--merge") {
        parse_attempt_flag(args, "--merge").map(|attempt| Request::Merge { attempt })
    } else if args.iter().any(|arg| arg == "--close") {
        let (attempt, session) = parse_close_arguments(args)?;
        Ok(Request::Close { attempt, session })
    } else {
        parse_open_arguments(args).map(|host| Request::Open { host })
    }
}

fn parse_open_arguments(args: &[String]) -> Result<String, String> {
    let mut open = false;
    let mut host: Option<String> = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--open" => open = true,
            "--host" => host = Some(iter.next().ok_or("--host requires a value")?.clone()),
            other => return Err(format!("unexpected argument '{other}'")),
        }
    }
    if !open {
        return Err("only --open is supported".to_string());
    }
    host.ok_or_else(|| "--host is required".to_string())
}

fn repository_root() -> Result<PathBuf, String> {
    let here = std::env::current_dir().map_err(|error| format!("cannot read cwd: {error}"))?;
    Ok(PathBuf::from(git_text(
        &here,
        &["rev-parse", "--show-toplevel"],
    )?))
}

/// Run `gal quickfix-run`.
pub(crate) fn cmd_quickfix_run(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let request = match parse_request(&rest) {
        Ok(request) => request,
        Err(error) => {
            eprintln!("gal quickfix-run: {error}");
            return ExitCode::Usage;
        }
    };
    match request {
        Request::Open { host } => {
            let outcome = repository_root().and_then(|repo| open_attempt(&repo, &host));
            match outcome {
                Ok(OpenOutcome::Opened(opened)) => {
                    println!(
                        "QUICKFIX_ATTEMPT: {} spec={} model={}",
                        opened.attempt_id,
                        opened.spec_path.display(),
                        opened.model
                    );
                    ExitCode::Success
                }
                Ok(OpenOutcome::Refused(refusal)) => {
                    println!("{}", refusal.status_line());
                    ExitCode::Error
                }
                Err(error) => {
                    eprintln!("gal quickfix-run: {error}");
                    ExitCode::Error
                }
            }
        }
        Request::Close { attempt, session } => {
            let outcome = repository_root().and_then(|repo| {
                let home = gal_foundation::paths::user_home()
                    .ok_or_else(|| "cannot determine the user home".to_string())?;
                close_attempt(&repo, &attempt, &session, &home)
            });
            match outcome {
                Ok(CloseOutcome::Closed(closed)) => {
                    println!("QUICKFIX: {}", closed.status.as_str());
                    for path in &closed.changed {
                        println!("CHANGED: {path}");
                    }
                    for path in &closed.outside {
                        println!("OUTSIDE: {path}");
                    }
                    if closed.status == TerminalStatus::Completed {
                        ExitCode::Success
                    } else {
                        ExitCode::Error
                    }
                }
                Ok(CloseOutcome::AlreadyClosed) => {
                    println!("QUICKFIX: attempt-closed");
                    ExitCode::Error
                }
                Err(error) => {
                    eprintln!("gal quickfix-run: {error}");
                    ExitCode::Error
                }
            }
        }
        Request::Commit { attempt } => {
            match repository_root().and_then(|repo| commit_attempt(&repo, &attempt)) {
                Ok(CommitOutcome::Committed(done)) => {
                    println!("QUICKFIX: {COMMITTED_STATUS}");
                    println!("COMMIT: {}", done.commit_id);
                    println!("BRANCH: {}", done.branch);
                    for path in &done.outside {
                        println!("OUTSIDE: {path}");
                    }
                    ExitCode::Success
                }
                Ok(CommitOutcome::Refused(refusal)) => {
                    println!("{}", refusal.status_line());
                    ExitCode::Error
                }
                Err(error) => {
                    eprintln!("gal quickfix-run: {error}");
                    ExitCode::Error
                }
            }
        }
        Request::Merge { attempt } => {
            match repository_root().and_then(|repo| merge_attempt(&repo, &attempt)) {
                Ok(MergeOutcome::Merged { branch }) => {
                    println!("QUICKFIX: {MERGED_STATUS}");
                    println!("BRANCH: {branch}");
                    ExitCode::Success
                }
                Ok(MergeOutcome::Refused(refusal)) => {
                    println!("{}", refusal.status_line());
                    ExitCode::Error
                }
                Err(error) => {
                    eprintln!("gal quickfix-run: {error}");
                    ExitCode::Error
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_ignores_command_order_and_tracks_head() {
        let a = failure_fingerprint(&["x".into(), "y".into()], "head-one");
        let b = failure_fingerprint(&["y".into(), "x".into()], "head-one");
        let c = failure_fingerprint(&["x".into(), "y".into()], "head-two");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn output_is_bounded_on_a_char_boundary() {
        let text = "é".repeat(MAX_OUTPUT_BYTES);
        let bounded = bound_output(&text);
        assert!(bounded.len() <= MAX_OUTPUT_BYTES);
        assert!(text.starts_with(&bounded));
    }

    #[test]
    fn utc_formatting_matches_known_instants() {
        assert_eq!(format_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(format_utc(1_792_659_490), "2026-10-22T08:58:10Z");
    }

    #[test]
    fn open_arguments_require_open_and_host() {
        let args = |parts: &[&str]| parts.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_open_arguments(&args(&["--open", "--host", "claude"])).unwrap(),
            "claude"
        );
        assert!(parse_open_arguments(&args(&["--host", "claude"])).is_err());
        assert!(parse_open_arguments(&args(&["--open"])).is_err());
    }

    #[test]
    fn commit_and_merge_arguments_take_one_attempt() {
        let args = |parts: &[&str]| parts.iter().map(|p| p.to_string()).collect::<Vec<_>>();
        assert_eq!(
            parse_request(&args(&["--commit", "qf_1"])).unwrap(),
            Request::Commit {
                attempt: "qf_1".into()
            }
        );
        assert_eq!(
            parse_request(&args(&["--merge", "qf_1"])).unwrap(),
            Request::Merge {
                attempt: "qf_1".into()
            }
        );
        assert!(parse_request(&args(&["--commit"])).is_err());
        assert!(parse_request(&args(&["--merge", "qf_1", "--host", "x"])).is_err());
    }

    #[test]
    fn object_ids_are_hex_of_the_right_length() {
        assert!(is_object_id(&"a".repeat(40)));
        assert!(is_object_id(&"0".repeat(64)));
        assert!(!is_object_id(&"a".repeat(39)));
        assert!(!is_object_id(&"g".repeat(40)));
    }
}
