//! Deterministic continuation authority for an active pipeline invocation.
//!
//! This is deliberately a small, internal checker.  It consumes durable prompt
//! state and receipts; it never treats conversational claims as evidence.
//!
//! Classification success is independent from terminal response authorization.
#![allow(dead_code, clippy::unnecessary_map_or)]

use super::converge_check::{
    check_cursor_cleared, check_task_commit, check_three_surface_and_task, classify_phase,
    current_task_value, dispatch_phases_for, scan_plan_log_dir, CurrentTaskValue, PhaseOutcome,
    PhaseVerdict,
};
use super::dispatch::{resolve_log_dir_override, resolve_receipt_path};
use super::finalize_check::{checked_task_ids, CheckState};
use super::pipeline_execution::{binding_path_text, resolve_worktree_root};
use gal_engine::ExitCode;
use pipeline::coordinator::{
    CheckpointKind, CheckpointReceipt, ContinuationProfile, CoordinatorState,
};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DECISIONS: [&str; 6] = [
    "continue",
    "goal-verified",
    "design-defect",
    "human-required",
    "retry-ceiling",
    "stop-at",
];
const HUMAN_REASONS: [&str; 5] = [
    "security-protected-path",
    "goal-gaps-blocked",
    "head-drift",
    "boundary-scope-decision",
    "convergence-human-repair",
];
const RUN_GOAL_BACKWARD_VERIFICATION: &str = "run-goal-backward-verification";
const NO_CONTINUE_ACTION: &str = "none";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HandbackTuple {
    pub(crate) decision: String,
    pub(crate) reason: String,
    pub(crate) voluntary_response_authorized: bool,
    pub(crate) continue_action: String,
}

#[derive(Debug, Clone)]
struct Args {
    prompt: PathBuf,
    stop_at: Option<String>,
    receipt: PathBuf,
}

#[derive(Debug, Clone, Default)]
struct GoalRecord {
    values: BTreeMap<String, String>,
    generic_evidence: Vec<String>,
    must_haves: BTreeMap<usize, String>,
    commands: Vec<String>,
    schema_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GoalBindingResult {
    prompt_path: bool,
    prompt_hash: bool,
    head: bool,
    checked_task_projection: bool,
    cursor: bool,
    must_haves: bool,
    commands: bool,
    verdict: bool,
}

#[derive(Debug, Clone)]
struct BindingIdentity {
    prompt_hash: String,
    head: String,
    executable_path: String,
    executable_sha256: String,
    execution_digest: String,
    execution_binding_sha256: String,
}

/// Resolve the runtime receipt identity shared by pipeline evidence commands.
/// A coordinator becomes authoritative only when it carries activation evidence.
pub(crate) fn runtime_receipt_identity(prompt: &Path, repo: &Path) -> Result<String, String> {
    let scope = prompt
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".prompt.md"))
        .ok_or("invalid prompt scope")?;
    let repo = resolve_worktree_root(repo);
    let coordinator = repo
        .join(".dev/pipeline")
        .join(scope)
        .join("coordinator.json");
    let state = if coordinator.exists() {
        Some(
            serde_json::from_slice::<CoordinatorState>(
                &std::fs::read(&coordinator).map_err(|e| format!("coordinator unreadable: {e}"))?,
            )
            .map_err(|_| "coordinator malformed")?,
        )
    } else {
        None
    };
    let guarded = state.as_ref().is_some_and(|state| {
        state
            .activation_digest
            .as_deref()
            .is_some_and(|digest| !digest.trim().is_empty())
    });
    let executable = std::fs::canonicalize(
        std::env::current_exe().map_err(|e| format!("executable unavailable: {e}"))?,
    )
    .map_err(|e| format!("executable path unavailable: {e}"))?;
    let executable_path = binding_path_text(&executable);
    let executable_sha256 = format!(
        "{:x}",
        Sha256::digest(
            std::fs::read(&executable).map_err(|e| format!("executable unreadable: {e}"))?
        )
    );
    let prompt_sha256 = format!(
        "{:x}",
        Sha256::digest(std::fs::read(prompt).map_err(|e| format!("prompt unreadable: {e}"))?)
    );
    if !guarded {
        let fields = [
            "standalone",
            executable_path.as_str(),
            executable_sha256.as_str(),
            prompt_sha256.as_str(),
        ];
        let mut input = Vec::new();
        for field in fields {
            input.extend_from_slice(&(field.len() as u64).to_be_bytes());
            input.extend_from_slice(field.as_bytes());
        }
        let digest = format!("{:x}", Sha256::digest(input));
        let envelope = super::finalize_check::receipt_envelope(Some("standalone"), Some(prompt))?;
        return Ok(format!(
            "binding_scope: standalone\nexecution_digest: {digest}\nexecution_binding_sha256: {}\nprompt_sha256: {prompt_sha256}\nexecutable_path: {executable_path}\nexecutable_sha256: {executable_sha256}\n",
            envelope.execution_binding_sha256
        ));
    }
    let state = state.expect("guarded coordinator state");
    if state.profile != ContinuationProfile::CodexStopV1 {
        return Err("coordinator is not guarded".into());
    }
    let binding = state
        .execution_binding
        .as_ref()
        .ok_or("coordinator binding missing")?;
    let root = std::fs::canonicalize(&repo).map_err(|e| e.to_string())?;
    if binding.binding_scope != format!("coordinator:{scope}")
        || binding.worktree_root != binding_path_text(&root)
        || binding.executable_path != executable_path
        || binding.executable_sha256 != executable_sha256
    {
        return Err("coordinator execution identity mismatch".into());
    }
    let digest = binding.digest().map_err(|e| e.to_string())?;
    let coordinator = std::fs::canonicalize(coordinator).map_err(|e| e.to_string())?;
    Ok(format!(
        "binding_scope: coordinator:{scope}\nplan_scope: {scope}\ncoordinator_path: {}\nexecution_digest: {digest}\nexecution_binding_sha256: {digest}\nprompt_sha256: {prompt_sha256}\nexecutable_path: {executable_path}\nexecutable_sha256: {executable_sha256}\n",
        coordinator.to_string_lossy().replace('\\', "/")
    ))
}

impl GoalBindingResult {
    pub(crate) fn predicate_states(
        &self,
    ) -> impl Iterator<Item = (&'static str, Option<bool>)> + '_ {
        [
            ("prompt-path", Some(self.prompt_path)),
            ("prompt-hash", Some(self.prompt_hash)),
            ("head", Some(self.head)),
            (
                "checked-task-projection",
                Some(self.checked_task_projection),
            ),
            ("cursor", Some(self.cursor)),
            ("must-haves", Some(self.must_haves)),
            ("commands", Some(self.commands)),
            ("verdict", Some(self.verdict)),
        ]
        .into_iter()
    }

    pub(crate) fn passed(&self) -> bool {
        self.predicate_states()
            .all(|(_, state)| state.unwrap_or(true))
    }

    pub(crate) fn failed_predicates(&self) -> Vec<&'static str> {
        self.predicate_states()
            .filter_map(|(name, state)| (state == Some(false)).then_some(name))
            .collect()
    }
}

pub(crate) trait GoalBindingEvaluation: Sized {
    fn from_goal_binding(result: GoalBindingResult) -> Self;
    fn from_goal_binding_failure() -> Self;
}

impl GoalBindingEvaluation for GoalBindingResult {
    fn from_goal_binding(result: GoalBindingResult) -> Self {
        result
    }

    fn from_goal_binding_failure() -> Self {
        Self {
            prompt_path: false,
            prompt_hash: false,
            head: false,
            checked_task_projection: false,
            cursor: false,
            must_haves: false,
            commands: false,
            verdict: false,
        }
    }
}

impl GoalBindingEvaluation for bool {
    fn from_goal_binding(result: GoalBindingResult) -> Self {
        result.passed()
    }

    fn from_goal_binding_failure() -> Self {
        false
    }
}

#[derive(Debug, Clone, Default)]
struct HumanHandback {
    reason: String,
    status: String,
    task: String,
    phase: String,
    producer: String,
    producer_state: String,
    next_human_step: String,
    what_to_check: String,
    expected_result: String,
    pass_fail_rule: String,
    head: String,
    baseline_head: Option<String>,
    observed_head: Option<String>,
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut prompt = None;
    let mut stop_at = None;
    let mut receipt = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--stop-at" => stop_at = Some(it.next().ok_or("--stop-at requires a task id")?.clone()),
            "--receipt" => {
                receipt = Some(PathBuf::from(it.next().ok_or("--receipt requires a path")?))
            }
            s if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            s if prompt.is_none() => prompt = Some(PathBuf::from(s)),
            _ => return Err("unexpected extra argument".to_string()),
        }
    }
    let prompt = prompt.ok_or("pipeline-handback-check requires a <prompt> path")?;
    let receipt = match receipt {
        Some(path) => path,
        None => resolve_receipt_path(Some(&prompt), None, "pipeline-handback-check.receipt.md")
            .map_err(|error| format!("receipt scope resolution: {error}"))?,
    };
    Ok(Args {
        prompt,
        stop_at,
        receipt,
    })
}

fn canonical_prompt(path: &Path, repo: &Path) -> Result<PathBuf, String> {
    let candidate = if path.extension().and_then(|x| x.to_str()) == Some("md")
        && path
            .file_name()
            .and_then(|x| x.to_str())
            .map_or(false, |x| !x.ends_with(".prompt.md"))
    {
        let prompt = path.with_file_name(format!(
            "{}.prompt.md",
            path.file_stem()
                .and_then(|x| x.to_str())
                .unwrap_or_default()
        ));
        if prompt.exists() {
            prompt
        } else {
            path.to_path_buf()
        }
    } else {
        path.to_path_buf()
    };
    let absolute = if candidate.is_absolute() {
        candidate
    } else {
        repo.join(candidate)
    };
    std::fs::canonicalize(&absolute).map_err(|e| format!("prompt not readable: {e}"))
}

fn sha256(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("prompt not readable: {e}"))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// A recorded digest must be 64 hex characters before it is compared. Without
/// this an empty recorded value matched an empty computed value, mirroring the
/// `valid_head` guard the sibling predicate already had.
fn valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.chars().all(|c| c.is_ascii_hexdigit())
}

fn git_head(repo: &Path) -> Result<String, String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|e| format!("git spawn failed: {e}"))?;
    if !out.status.success() {
        return Err("git HEAD unavailable".to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn parse_key_values(text: &str) -> GoalRecord {
    let mut record = GoalRecord::default();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some((key, value)) = trimmed.split_once(':') {
            let exact_key = key.trim();
            let key = exact_key.to_ascii_lowercase().replace([' ', '-'], "_");
            let value = value.trim().trim_matches('`').to_string();
            if key == "command" || key == "commands" {
                if !value.is_empty() {
                    record.commands.push(value);
                }
            } else if key == "evidence" {
                if !value.is_empty() {
                    record.generic_evidence.push(value);
                }
            } else if exact_key.starts_with("must_have") {
                match must_have_index(exact_key) {
                    Some(index) if !value.is_empty() => {
                        if record.must_haves.insert(index, value).is_some() {
                            record.schema_error = true;
                        }
                    }
                    _ => record.schema_error = true,
                }
            } else if key.starts_with("must_have") {
                record.schema_error = true;
            } else {
                record.values.insert(key, value);
            }
        }
    }
    record
}

fn must_have_index(key: &str) -> Option<usize> {
    let suffix = key.strip_prefix("must_have_")?;
    let index = suffix.parse::<usize>().ok()?;
    (index > 0 && suffix == index.to_string()).then_some(index)
}

fn complete_must_haves(record: &GoalRecord) -> bool {
    !record.schema_error
        && !record.must_haves.is_empty()
        && record
            .must_haves
            .keys()
            .copied()
            .eq(1..=record.must_haves.len())
}

fn value<'a>(record: &'a GoalRecord, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| record.values.get(*name).map(String::as_str))
}

fn valid_head(value: &str) -> bool {
    (7..=40).contains(&value.len()) && value.chars().all(|c| c.is_ascii_hexdigit())
}
fn task_id(value: &str) -> bool {
    value.len() > 2 && value.starts_with("T-") && value[2..].chars().all(|c| c.is_ascii_digit())
}

fn checked_projection(text: &str) -> String {
    let checked = checked_task_ids(text).into_iter().collect::<Vec<_>>();
    checked.join(",")
}

fn normalize_path_str(s: &str) -> String {
    let s = s.strip_prefix(r"\\?\").unwrap_or(s);
    s.replace('\\', "/")
}

fn normalize_path(p: &Path) -> String {
    normalize_path_str(&p.to_string_lossy())
}

fn path_matches(recorded: &str, expected_path: &Path, repo: &Path) -> bool {
    let norm_expected = normalize_path(expected_path);
    let norm_recorded = normalize_path_str(recorded);
    if norm_recorded == norm_expected {
        return true;
    }
    let norm_abs_recorded = normalize_path(&repo.join(recorded));
    norm_abs_recorded == norm_expected
}

pub(crate) fn evaluate_goal_binding<T: GoalBindingEvaluation>(
    prompt_path: &Path,
    prompt: &str,
    repo: &Path,
) -> T {
    // Hash the bytes the caller already validated. The previous second read
    // went to disk unvalidated, so the attested hash could describe bytes that
    // were never the bytes evaluated, and a read failure degraded to an empty
    // string that an empty recorded value then matched.
    let hash = format!("{:x}", Sha256::digest(prompt.as_bytes()));
    let head = git_head(repo).unwrap_or_default();
    let goal_path =
        match resolve_receipt_path(Some(prompt_path), None, "goal-verification.receipt.md") {
            Ok(path) => path,
            Err(_) => return T::from_goal_binding_failure(),
        };
    let goal = std::fs::read_to_string(&goal_path)
        .ok()
        .map(|x| parse_key_values(&x));
    let record = goal.as_ref();
    let recorded_head = record
        .and_then(|record| value(record, &["head", "git_head"]))
        .unwrap_or("");
    let result = GoalBindingResult {
        prompt_path: record
            .and_then(|record| value(record, &["prompt_path"]))
            .is_some_and(|val| path_matches(val, prompt_path, repo)),
        prompt_hash: record
            .and_then(|record| value(record, &["prompt_sha256", "prompt_hash"]))
            .is_some_and(|value| valid_sha256(value) && value.eq_ignore_ascii_case(&hash)),
        head: valid_head(recorded_head)
            && (recorded_head == head || head.starts_with(recorded_head)),
        checked_task_projection: record
            .and_then(|record| value(record, &["checked_tasks", "task_projection"]))
            .is_some_and(|value| value == checked_projection(prompt)),
        cursor: matches!(current_task_value(prompt), CurrentTaskValue::Cleared),
        must_haves: record.is_some_and(complete_must_haves),
        commands: record.is_some_and(|record| !record.commands.is_empty()),
        verdict: record
            .and_then(|record| value(record, &["verdict"]))
            .is_some_and(|value| value == "VERIFIED"),
    };
    T::from_goal_binding(result)
}

fn parse_human_blocks(prompt: &str) -> Vec<HumanHandback> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = prompt.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let Some(reason) = line.trim().strip_prefix("#### Human Handback — ") else {
            continue;
        };
        let mut block = HumanHandback {
            reason: reason.trim().to_string(),
            ..Default::default()
        };
        for row in lines
            .iter()
            .skip(index + 1)
            .take_while(|row| !row.trim().starts_with("#### "))
            .take(16)
        {
            let row = row.trim();
            let row = row.strip_prefix("- ").unwrap_or(row);
            let Some((key, value)) = row.split_once(':') else {
                continue;
            };
            let value = value.trim().to_string();
            match key.trim() {
                "Status" => block.status = value,
                "Reason" => block.reason = value,
                "Task" => block.task = value,
                "Phase" => block.phase = value,
                "Producer" => block.producer = value,
                "Producer state" => block.producer_state = value,
                "Next human step" => block.next_human_step = value,
                "What to check" => block.what_to_check = value,
                "Expected result" => block.expected_result = value,
                "Pass/fail rule" => block.pass_fail_rule = value,
                "Git HEAD" => block.head = value,
                "Baseline HEAD" => block.baseline_head = Some(value),
                "Observed HEAD" => block.observed_head = Some(value),
                _ => {}
            }
        }
        blocks.push(block);
    }
    blocks
}

fn expected_producer(reason: &str) -> &'static str {
    match reason {
        "security-protected-path" => "AUDITOR",
        "goal-gaps-blocked" => "VERIFY",
        "head-drift" => "PIPELINE",
        "boundary-scope-decision" => "BOUNDARY",
        "convergence-human-repair" => "CONVERGE",
        _ => "",
    }
}

fn valid_human(prompt: &str, reason: &str, task: &str, phase: &str, head: &str) -> bool {
    if !HUMAN_REASONS.contains(&reason) {
        return false;
    }
    let open_blocks = parse_human_blocks(prompt)
        .into_iter()
        .filter(|block| block.status == "OPEN")
        .collect::<Vec<_>>();
    if open_blocks.len() != 1 {
        return false;
    }
    let block = &open_blocks[0];
    if block.status != "OPEN"
        || block.reason != reason
        || block.task != task
        || block.phase != phase
        || block.producer != expected_producer(reason)
        || block.producer_state.is_empty()
        || block.next_human_step.is_empty()
        || block.what_to_check.is_empty()
        || block.expected_result.is_empty()
        || block.pass_fail_rule.is_empty()
        || block.head != head
    {
        return false;
    }
    if !valid_head(&block.head) {
        return false;
    }
    if reason == "head-drift" {
        match (&block.baseline_head, &block.observed_head) {
            (Some(base), Some(observed)) => {
                valid_head(base) && valid_head(observed) && base != observed && observed == head
            }
            _ => false,
        }
    } else {
        true
    }
}

fn convergence(prompt_path: &Path, prompt: &str, repo: &Path, task: &str) -> bool {
    let surface = check_three_surface_and_task(task, prompt_path, prompt, repo);
    let commit = check_task_commit(task, prompt, repo);
    let cursor = check_cursor_cleared(task, prompt);
    if [surface.state, commit.state, cursor.state]
        .iter()
        .any(|s| *s != CheckState::Pass)
    {
        return false;
    }
    let dir = match resolve_log_dir_override(repo, Some(prompt_path), Some(task)) {
        Ok(Some(dir)) => dir,
        Ok(None) | Err(_) => return false,
    };
    let scan = scan_plan_log_dir(&dir, task);
    if scan.directory_unreadable.is_some() || !scan.malformed_files.is_empty() {
        return false;
    }
    dispatch_phases_for(prompt_path, prompt, task)
        .iter()
        .all(|phase| {
            scan.attempts_by_phase
                .get(*phase)
                .map_or(false, |attempts| {
                    matches!(
                        classify_phase(attempts, true, false),
                        PhaseVerdict::Outcome(
                            PhaseOutcome::DispatchOffload | PhaseOutcome::RecoveredInConversation
                        )
                    )
                })
        })
}

/// A routed attempt must name an executor other than `host-subagent`. The log
/// header parser drops empty header values, so a present executor is never empty.
fn has_routed_executor(headers: &BTreeMap<String, String>) -> bool {
    headers
        .get("executor")
        .is_some_and(|executor| executor != "host-subagent")
}

/// Whether an attempt is valid evidence under the preflight pin, judged from the
/// pin, the phase, and the record alone. The live route is never consulted.
///
/// - A routed attempt has no `host` header and a routed executor.
/// - A host attempt has the `host-subagent` executor and the pinned host.
/// - Under a routed pin, a pinned host also admits a host attempt, because
///   dispatch turns an absent route into a host-subagent attempt there.
/// - Under a host-subagent pin, `finalize-review` also admits a routed attempt,
///   because dispatch keeps a present auditor route for that phase.
/// - With no pin, only a routed attempt is valid.
fn attempt_matches_pin(
    headers: &BTreeMap<String, String>,
    phase: dispatch::stage::Phase,
    pin: Option<&super::pipeline_preflight::PreflightPin>,
) -> bool {
    use super::pipeline_preflight::DispatchMode;
    let routed = !headers.contains_key("host") && has_routed_executor(headers);
    let Some(pin) = pin else {
        return routed;
    };
    let pinned_host_attempt = pin.host.as_deref().is_some_and(|host| {
        headers.get("executor").map(String::as_str) == Some("host-subagent")
            && headers.get("host").map(String::as_str) == Some(host)
    });
    match pin.dispatch_mode {
        DispatchMode::Routed => routed || pinned_host_attempt,
        DispatchMode::HostSubagent => {
            pinned_host_attempt || (phase == dispatch::stage::Phase::FinalizeReview && routed)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct HostDispatchIssue {
    task: String,
    phase: String,
    reason: HostDispatchReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum HostDispatchReason {
    ExecutorMismatch,
    ModeMismatch,
    NotCompleted,
    NoWritebackWithoutWitness,
    MalformedLog,
    UnreadableLogDirectory,
}

impl HostDispatchReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::ExecutorMismatch => "executor-mismatch",
            Self::ModeMismatch => "mode-mismatch",
            Self::NotCompleted => "not-completed",
            Self::NoWritebackWithoutWitness => "no-writeback-without-witness",
            Self::MalformedLog => "malformed-log",
            Self::UnreadableLogDirectory => "unreadable-log-directory",
        }
    }
}

/// Every dispatch phase, longest name first so a hyphenated phase is matched
/// as a whole before any phase whose name it contains.
fn dispatch_phase_names_longest_first() -> Vec<&'static str> {
    use dispatch::stage::Phase;
    let mut names: Vec<&'static str> = [
        Phase::Implement,
        Phase::Test,
        Phase::Audit,
        Phase::Investigate,
        Phase::FinalizeReview,
    ]
    .into_iter()
    .map(Phase::as_str)
    .collect();
    names.sort_by_key(|name| std::cmp::Reverse(name.len()));
    names
}

/// The dispatch phase a log file name carries as a whole `-phase-` token, or
/// `unknown` when the name carries none.
fn log_file_phase(file: &str) -> &'static str {
    dispatch_phase_names_longest_first()
        .into_iter()
        .find(|phase| {
            file.contains(&format!("-{phase}-")) || file.ends_with(&format!("-{phase}.log"))
        })
        .unwrap_or("unknown")
}

/// A phase name read from a log keeps its value in the issue line. A byte that
/// cannot stand in one issue token is percent-encoded, so the line grammar
/// holds and the read name stays recoverable. An empty name establishes no
/// phase and is attributed as `unknown`.
fn attributable_phase(name: &str) -> String {
    if name.is_empty() {
        return "unknown".to_string();
    }
    let mut token = String::with_capacity(name.len());
    for byte in name.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_' {
            token.push(char::from(byte));
        } else {
            token.push_str(&format!("%{byte:02X}"));
        }
    }
    token
}

/// Order issues by task, phase, then reason text, and drop exact repeats.
fn sort_host_issues(issues: &mut Vec<HostDispatchIssue>) {
    issues.sort_by(|left, right| {
        (&left.task, &left.phase, left.reason.as_str()).cmp(&(
            &right.task,
            &right.phase,
            right.reason.as_str(),
        ))
    });
    issues.dedup();
}

fn host_dispatch_evidence(prompt_path: &Path, prompt: &str, repo: &Path) -> Vec<HostDispatchIssue> {
    use super::pipeline_preflight::{read_pin, PinError};
    let (pin, invalid_pin) = match read_pin(prompt_path) {
        Ok(pin) => (Some(pin), false),
        Err(PinError::Missing) => (None, false),
        Err(PinError::Unparseable) => (None, true),
    };
    let scope = match resolve_log_dir_override(repo, Some(prompt_path), None) {
        Ok(Some(scope)) => scope,
        _ => {
            return vec![host_issue(
                "unknown",
                "unknown",
                HostDispatchReason::UnreadableLogDirectory,
            )];
        }
    };
    let entries = match std::fs::read_dir(scope) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Vec::new(),
        Err(_) => {
            return vec![host_issue(
                "unknown",
                "unknown",
                HostDispatchReason::UnreadableLogDirectory,
            )];
        }
    };
    let mut issues = Vec::new();
    let independent = host_verification_independence(prompt);
    let mut any_host_attempt = false;
    for entry in entries {
        let Ok(entry) = entry else {
            issues.push(host_issue(
                "unknown",
                "unknown",
                HostDispatchReason::UnreadableLogDirectory,
            ));
            continue;
        };
        let Ok(kind) = entry.file_type() else {
            // The entry name is known even when its type is not.
            let name = entry.file_name().to_string_lossy().into_owned();
            issues.push(host_issue(
                &name,
                "unknown",
                HostDispatchReason::UnreadableLogDirectory,
            ));
            continue;
        };
        if !kind.is_dir() {
            continue;
        }
        let task = entry.file_name().to_string_lossy().into_owned();
        let scan = scan_plan_log_dir(&entry.path(), &task);
        if scan.directory_unreadable.is_some() {
            issues.push(host_issue(
                &task,
                "unknown",
                HostDispatchReason::UnreadableLogDirectory,
            ));
        }
        for file in &scan.malformed_files {
            let phase = log_file_phase(file);
            issues.push(host_issue(&task, phase, HostDispatchReason::MalformedLog));
        }
        for (phase, attempts) in &scan.attempts_by_phase {
            let Ok(dispatch_phase) = dispatch::stage::Phase::from_str(phase) else {
                issues.push(host_issue(
                    &task,
                    &attributable_phase(phase),
                    HostDispatchReason::ModeMismatch,
                ));
                continue;
            };
            // Host evidence is judged per task phase so the issue set never
            // depends on directory iteration order.
            if attempts
                .iter()
                .any(|attempt| !attempt.headers.contains_key("executor"))
            {
                issues.push(host_issue(&task, phase, HostDispatchReason::MalformedLog));
            }
            let phase_has_host = attempts.iter().any(|attempt| {
                attempt.headers.get("executor").map(String::as_str) == Some("host-subagent")
                    || attempt.headers.contains_key("host")
            });
            any_host_attempt |= phase_has_host;
            if phase_has_host && (pin.is_none() || !independent) {
                issues.push(host_issue(&task, phase, HostDispatchReason::ModeMismatch));
            }
            let Some(effective) = attempts.last() else {
                continue;
            };
            let effective_attempt = std::slice::from_ref(effective);
            let effective_verdict = classify_phase(effective_attempt, true, false);
            if matches!(&effective_verdict, PhaseVerdict::FailClosed(reason)
                if !reason.contains(" is 'no-writeback': phase exited cleanly"))
            {
                issues.push(host_issue(&task, phase, HostDispatchReason::NotCompleted));
            }
            let effective_host =
                effective.headers.get("executor").map(String::as_str) == Some("host-subagent");
            // Legacy routed evidence predates preflight pins. A pin applies to
            // the effective record only, so superseded modes cannot veto it. An
            // effective record with no executor is already a malformed log and
            // is not judged against the pin a second time.
            if (pin.is_some() || effective_host || effective.headers.contains_key("host"))
                && effective.headers.contains_key("executor")
                && !attempt_matches_pin(&effective.headers, dispatch_phase, pin.as_ref())
            {
                let reason = if pin.as_ref().is_some_and(|pin| {
                    pin.dispatch_mode == super::pipeline_preflight::DispatchMode::HostSubagent
                }) {
                    HostDispatchReason::ExecutorMismatch
                } else {
                    HostDispatchReason::ModeMismatch
                };
                issues.push(host_issue(&task, phase, reason));
            }
            if effective.headers.get("terminal_state").map(String::as_str) == Some("no-writeback")
                && !effective_host
            {
                let has_witness = attempts[..attempts.len() - 1].iter().any(|candidate| {
                    candidate.headers.get("terminal_state").map(String::as_str) == Some("completed")
                        && !candidate.headers.contains_key("host")
                        && has_routed_executor(&candidate.headers)
                        && attempt_matches_pin(&candidate.headers, dispatch_phase, pin.as_ref())
                        && matches!(
                            classify_phase(std::slice::from_ref(candidate), true, false),
                            PhaseVerdict::Outcome(PhaseOutcome::DispatchOffload)
                        )
                });
                if !has_witness {
                    issues.push(host_issue(
                        &task,
                        phase,
                        HostDispatchReason::NoWritebackWithoutWitness,
                    ));
                }
            }
        }
    }
    // A malformed pin is attributed to the host attempts that needed it. With
    // no such attempt, no task or phase can be established.
    if invalid_pin && !any_host_attempt {
        issues.push(host_issue(
            "unknown",
            "unknown",
            HostDispatchReason::ModeMismatch,
        ));
    }
    sort_host_issues(&mut issues);
    issues
}

fn host_issue(task: &str, phase: &str, reason: HostDispatchReason) -> HostDispatchIssue {
    HostDispatchIssue {
        task: task.to_string(),
        phase: phase.to_string(),
        reason,
    }
}

fn host_verification_independence(prompt: &str) -> bool {
    let markers = prompt
        .lines()
        .filter_map(|line| {
            line.trim()
                .trim_start_matches("- ")
                .strip_prefix("Verification Independence:")
        })
        .collect::<Vec<_>>();
    markers.len() == 1 && markers[0].trim() == "DEGRADED_HOST_SUBAGENT"
}

/// A handoff block ends at the next peer or enclosing heading, which is any
/// Markdown heading from level one to level four.
fn closes_handoff_block(line: &str) -> bool {
    let line = line.trim_start();
    let level = line.bytes().take_while(|byte| *byte == b'#').count();
    (1..=4).contains(&level) && line[level..].starts_with(' ')
}

fn retry_ceiling(prompt: &str, task: &str) -> bool {
    let lines: Vec<&str> = prompt.lines().collect();
    let Some(status_start) = lines.iter().position(|line| line.trim() == "## Status") else {
        return false;
    };
    let status_end = lines[status_start + 1..]
        .iter()
        .position(|line| line.trim_start().starts_with("## "))
        .map(|offset| status_start + 1 + offset)
        .unwrap_or(lines.len());
    let status = &lines[status_start + 1..status_end];
    let current_task = status
        .iter()
        .find_map(|line| line.trim().strip_prefix("Current Task:").map(str::trim));
    if current_task != Some(task) {
        return false;
    }
    let retry_ceiling_reached = ["Test Retry Count", "Review Retry Count"]
        .iter()
        .any(|name| {
            status.iter().find_map(|line| {
                line.trim()
                    .strip_prefix(&format!("{name}:"))
                    .and_then(|value| value.trim().parse::<u32>().ok())
            }) == Some(3)
        });
    if !retry_ceiling_reached {
        return false;
    }
    lines.iter().enumerate().any(|(index, line)| {
        let Some(_phase) = line
            .trim()
            .strip_prefix(&format!("#### Retry Handoff — {task} /"))
            .map(str::trim)
        else {
            return false;
        };
        let end = lines[index + 1..]
            .iter()
            .position(|next| closes_handoff_block(next))
            .map(|offset| index + 1 + offset)
            .unwrap_or(lines.len());
        let block = lines[index..end].join("\n");
        block.lines().any(|entry| {
            let entry = entry.trim();
            entry.eq_ignore_ascii_case("Status: OPEN")
                || entry.eq_ignore_ascii_case("- Status: OPEN")
        })
    })
}

fn annotate_host_dispatch_evidence(mut receipt: String, issues: &[HostDispatchIssue]) -> String {
    if !issues.is_empty() {
        if !receipt.is_empty() && !receipt.ends_with('\n') {
            receipt.push('\n');
        }
        receipt.push_str("host_dispatch_evidence: fail");
        for issue in issues {
            receipt.push_str(&format!(
                "\nhost_dispatch_evidence_issue: task={} phase={} reason={}",
                issue.task,
                issue.phase,
                issue.reason.as_str()
            ));
        }
    }
    receipt
}

fn classify(
    prompt_path: &Path,
    prompt: &str,
    repo: &Path,
    stop_at: Option<&str>,
    execution_digest: &str,
    execution_binding_sha256: &str,
) -> (HandbackTuple, String) {
    let executable_path = std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::canonicalize(path).ok())
        .map(|path| binding_path_text(&path))
        .unwrap_or_default();
    let executable_sha256 = std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::canonicalize(path).ok())
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        .unwrap_or_default();
    let identity = BindingIdentity {
        prompt_hash: sha256(prompt_path).unwrap_or_default(),
        head: git_head(repo).unwrap_or_default(),
        executable_path,
        executable_sha256,
        execution_digest: execution_digest.to_string(),
        execution_binding_sha256: execution_binding_sha256.to_string(),
    };
    let goal_binding: GoalBindingResult = evaluate_goal_binding(prompt_path, prompt, repo);
    let checked = checked_task_ids(prompt);
    let unchecked = super::finalize_check::task_checkbox_projection(prompt).unchecked;
    let current = match current_task_value(prompt) {
        CurrentTaskValue::Active(x) => x,
        CurrentTaskValue::Cleared => "—".to_string(),
        CurrentTaskValue::Missing => "<missing>".to_string(),
    };
    let last = match &current[..] {
        "—" | "<missing>" => checked
            .iter()
            .next_back()
            .cloned()
            .unwrap_or_else(|| "none".to_string()),
        active => active.to_string(),
    };
    let task_for_action = unchecked
        .first()
        .cloned()
        .unwrap_or_else(|| "none".to_string());
    let all_tasks = checked
        .iter()
        .chain(unchecked.iter())
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    let host_evidence_issues = host_dispatch_evidence(prompt_path, prompt, repo);
    let finish = |tuple, receipt| {
        (
            tuple,
            annotate_host_dispatch_evidence(receipt, &host_evidence_issues),
        )
    };
    let stop_at_state = stop_at.map(|target| {
        if !task_id(target) || !all_tasks.contains(target) {
            StopAtState::Invalid
        } else if !checked.contains(target) {
            StopAtState::Pending
        } else if !convergence(prompt_path, prompt, repo, target)
            || !matches!(current_task_value(prompt), CurrentTaskValue::Cleared)
        {
            StopAtState::Stale
        } else {
            StopAtState::Reached
        }
    });
    if stop_at_state == Some(StopAtState::Reached) {
        return finish(
            ("stop-at", "none", true, NO_CONTINUE_ACTION).into(),
            binding_text(
                repo,
                prompt_path,
                &identity,
                prompt,
                &unchecked,
                &current,
                &goal_binding,
            ),
        );
    }
    if retry_ceiling(prompt, &last) {
        return finish(
            ("retry-ceiling", "none", true, NO_CONTINUE_ACTION).into(),
            format!(
                "prompt_sha256: {}\nhead: {}",
                identity.prompt_hash, identity.head
            ),
        );
    }
    for reason in HUMAN_REASONS {
        let phase = match reason {
            "security-protected-path" => "AUDIT",
            "goal-gaps-blocked" => "VERIFY",
            "head-drift" => "CONVERGE",
            "boundary-scope-decision" => "BOUNDARY",
            _ => "CONVERGE",
        };
        if valid_human(prompt, reason, &last, phase, &identity.head) {
            return finish(
                ("human-required", reason, true, NO_CONTINUE_ACTION).into(),
                binding_text(
                    repo,
                    prompt_path,
                    &identity,
                    prompt,
                    &unchecked,
                    &current,
                    &goal_binding,
                ),
            );
        }
    }
    if let (Some(target), Some(StopAtState::Invalid)) = (stop_at, stop_at_state) {
        return finish(
            (
                "continue",
                "none",
                false,
                format!("repair-stop-at-target {target}"),
            )
                .into(),
            binding_text(
                repo,
                prompt_path,
                &identity,
                prompt,
                &unchecked,
                &current,
                &goal_binding,
            ),
        );
    }
    if let (Some(target), Some(StopAtState::Stale)) = (stop_at, stop_at_state) {
        return finish(
            (
                "continue",
                "none",
                false,
                format!("repair-stop-at-convergence {target}"),
            )
                .into(),
            binding_text(
                repo,
                prompt_path,
                &identity,
                prompt,
                &unchecked,
                &current,
                &goal_binding,
            ),
        );
    }
    if !unchecked.is_empty() {
        let action = if task_for_action == "none" {
            RUN_GOAL_BACKWARD_VERIFICATION.to_string()
        } else if let (Some(target), Some(StopAtState::Pending)) = (stop_at, stop_at_state) {
            format!("/gal pipeline <prompt> from {task_for_action} stop-at {target}")
        } else {
            format!("/gal pipeline <prompt> from {task_for_action}")
        };
        return finish(
            ("continue", "none", false, action).into(),
            binding_text(
                repo,
                prompt_path,
                &identity,
                prompt,
                &unchecked,
                &current,
                &goal_binding,
            ),
        );
    }
    let bound_goal = goal_binding.passed() && host_evidence_issues.is_empty();
    let decision = if bound_goal {
        "goal-verified"
    } else {
        "continue"
    };
    let action = if bound_goal {
        NO_CONTINUE_ACTION
    } else {
        RUN_GOAL_BACKWARD_VERIFICATION
    };
    finish(
        (decision, "none", bound_goal, action).into(),
        binding_text(
            repo,
            prompt_path,
            &identity,
            prompt,
            &unchecked,
            &current,
            &goal_binding,
        ),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StopAtState {
    Invalid,
    Pending,
    Stale,
    Reached,
}

fn binding_text(
    repo: &Path,
    prompt_path: &Path,
    identity: &BindingIdentity,
    prompt: &str,
    unchecked: &[String],
    current: &str,
    goal_binding: &GoalBindingResult,
) -> String {
    let checked = checked_projection(prompt);
    let scope = match resolve_log_dir_override(repo, Some(prompt_path), None) {
        Ok(Some(path)) => path.to_string_lossy().to_string(),
        Ok(None) => "not-run".to_string(),
        Err(error) => format!("refused: {error}"),
    };
    let failed_predicates = goal_binding.failed_predicates();
    let failed_predicates = if failed_predicates.is_empty() {
        "none".to_string()
    } else {
        failed_predicates.join(",")
    };
    let plan_scope = prompt_path
        .file_name()
        .and_then(|x| x.to_str())
        .and_then(|x| x.strip_suffix(".prompt.md"))
        .unwrap_or("unknown");
    let coordinator = repo
        .join(".dev/pipeline")
        .join(plan_scope)
        .join("coordinator.json");
    let guarded = std::fs::read(&coordinator)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<CoordinatorState>(&bytes).ok())
        .is_some_and(|state| {
            state
                .activation_digest
                .as_ref()
                .is_some_and(|digest| !digest.trim().is_empty())
        });
    let coordinator_fields = if guarded {
        format!(
            "plan_scope: {plan_scope}\ncoordinator_path: {}\n",
            std::fs::canonicalize(&coordinator)
                .map(|path| path.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default()
        )
    } else {
        String::new()
    };
    format!(
        "binding_scope: {}\n{}execution_digest: {}\nexecution_binding_sha256: {}\nprompt_path: {}\nprompt_sha256: {}\nhead: {}\nchecked_tasks: {checked}\nunchecked_tasks: {}\ncurrent_task: {current}\nevidence_path: {scope}\nexecutable_path: {}\nexecutable_sha256: {}\ngoal_binding: {}\ngoal_binding_failed_predicates: {failed_predicates}",
        if guarded {
            format!("coordinator:{plan_scope}")
        } else {
            "standalone".to_string()
        },
        coordinator_fields,
        identity.execution_digest,
        identity.execution_binding_sha256,
        prompt_path.display(),
        identity.prompt_hash,
        identity.head,
        unchecked.join(","),
        identity.executable_path,
        identity.executable_sha256,
        if goal_binding.passed() {
            "pass"
        } else {
            "fail"
        }
    )
}

fn valid_continue_action(decision: &str, action: &str) -> bool {
    if decision != "continue" {
        return action == NO_CONTINUE_ACTION;
    }
    if action == RUN_GOAL_BACKWARD_VERIFICATION {
        return true;
    }
    for prefix in ["repair-stop-at-target ", "repair-stop-at-convergence "] {
        if let Some(target) = action.strip_prefix(prefix) {
            return task_id(target);
        }
    }
    let Some(rest) = action.strip_prefix("/gal pipeline <prompt> from ") else {
        return false;
    };
    let parts = rest.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        [task] => task_id(task),
        [task, "stop-at", target] => task_id(task) && task_id(target),
        _ => false,
    }
}

fn design_stop_can_override(tuple: &HandbackTuple) -> bool {
    tuple.decision == "continue" && !tuple.voluntary_response_authorized
}

impl From<(&str, &str, bool, &str)> for HandbackTuple {
    fn from(value: (&str, &str, bool, &str)) -> Self {
        Self {
            decision: value.0.to_string(),
            reason: value.1.to_string(),
            voluntary_response_authorized: value.2,
            continue_action: value.3.to_string(),
        }
    }
}

impl From<(&str, &str, bool, String)> for HandbackTuple {
    fn from(value: (&str, &str, bool, String)) -> Self {
        Self {
            decision: value.0.to_string(),
            reason: value.1.to_string(),
            voluntary_response_authorized: value.2,
            continue_action: value.3,
        }
    }
}

fn render(
    tuple: &HandbackTuple,
    bindings: &str,
    action: Option<&str>,
    revision: Option<u64>,
    commit: Option<&str>,
) -> String {
    let fields = match revision {
        Some(revision) => format!(
            "coordinator_revision: {revision}\ncoordinator_commit: {}\ntyped_action: {}\n",
            commit.unwrap_or("none"),
            action.unwrap_or("none")
        ),
        None => String::new(),
    };
    format!(
        "# pipeline-handback-check receipt\n\ndecision: {}\nreason: {}\nvoluntary_response_authorized: {}\ncontinue_action: {}\nrepair_disposition: owner\nrepair_context: unavailable\nrepair_context_authority: none\n{}\n{}\n",
        tuple.decision,
        tuple.reason,
        tuple.voluntary_response_authorized,
        tuple.continue_action,
        fields,
        bindings
    )
}

pub(crate) fn cmd_pipeline_handback_check(args: &[String]) -> ExitCode {
    let rest = args.iter().skip(1).cloned().collect::<Vec<_>>();
    let parsed = match parse_args(&rest) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("gal pipeline-handback-check: {e}");
            return if e.starts_with("receipt scope resolution:") {
                ExitCode::Error
            } else {
                ExitCode::Usage
            };
        }
    };
    let cwd = match std::env::current_dir() {
        Ok(x) => x,
        Err(_) => return ExitCode::Error,
    };
    let repo = resolve_worktree_root(&cwd);
    let canonical = match canonical_prompt(&parsed.prompt, &cwd) {
        Ok(x) => x,
        Err(e) => {
            if let Err(remove_error) = std::fs::remove_file(&parsed.receipt) {
                if remove_error.kind() != std::io::ErrorKind::NotFound {
                    eprintln!(
                        "gal pipeline-handback-check: failed to remove previous receipt: {remove_error}"
                    );
                    return ExitCode::Error;
                }
            }
            eprintln!("gal pipeline-handback-check: {e}");
            return ExitCode::Usage;
        }
    };
    if let Err(error) = std::fs::remove_file(&parsed.receipt) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("gal pipeline-handback-check: failed to remove previous receipt: {error}");
            return ExitCode::Error;
        }
    }
    let scope = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".prompt.md"));
    let coordinator = scope.map(|scope| {
        repo.join(".dev/pipeline")
            .join(scope)
            .join("coordinator.json")
    });
    let coordinator_state = match coordinator.as_ref().filter(|path| path.exists()) {
        Some(path) => match std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CoordinatorState>(&bytes).ok())
        {
            Some(state) => Some(state),
            None => {
                eprintln!("gal pipeline-handback-check: coordinator malformed");
                return ExitCode::Error;
            }
        },
        None => None,
    };
    let guarded = coordinator_state.as_ref().is_some_and(|state| {
        state
            .activation_digest
            .as_ref()
            .is_some_and(|digest| !digest.trim().is_empty())
    });
    if guarded
        && coordinator_state
            .as_ref()
            .is_some_and(|state| state.profile != ContinuationProfile::CodexStopV1)
    {
        eprintln!("gal pipeline-handback-check: guarded coordinator profile mismatch");
        return ExitCode::Error;
    }
    let executable = std::env::current_exe()
        .ok()
        .and_then(|path| std::fs::canonicalize(path).ok());
    let executable_hash = executable
        .as_ref()
        .and_then(|path| std::fs::read(path).ok())
        .map(|bytes| format!("{:x}", Sha256::digest(bytes)));
    let worktree = std::fs::canonicalize(&repo).ok();
    let execution_digest = if guarded {
        let binding = match coordinator_state
            .as_ref()
            .and_then(|state| state.execution_binding.as_ref())
        {
            Some(binding) => binding,
            None => {
                eprintln!("gal pipeline-handback-check: coordinator binding missing");
                return ExitCode::Error;
            }
        };
        let expected_scope = format!("coordinator:{}", scope.unwrap_or_default());
        if binding.binding_scope != expected_scope
            || Some(binding.executable_path.as_str())
                != executable
                    .as_ref()
                    .map(|path| binding_path_text(path))
                    .as_deref()
            || Some(binding.executable_sha256.as_str()) != executable_hash.as_deref()
            || Some(binding.worktree_root.as_str())
                != worktree
                    .as_ref()
                    .map(|path| binding_path_text(path))
                    .as_deref()
        {
            eprintln!("gal pipeline-handback-check: coordinator execution identity mismatch");
            return ExitCode::Error;
        }
        match binding.digest() {
            Ok(digest) => digest,
            Err(_) => return ExitCode::Error,
        }
    } else {
        match (scope, executable_hash.as_deref()) {
            (Some(scope), Some(executable_hash)) => format!(
                "{:x}",
                Sha256::digest(format!("standalone:{scope}\n{executable_hash}").as_bytes())
            ),
            _ => {
                eprintln!("gal pipeline-handback-check: executable identity unavailable");
                return ExitCode::Error;
            }
        }
    };
    let binding_scope = if guarded {
        format!("coordinator:{}", scope.unwrap_or_default())
    } else {
        "standalone".to_string()
    };
    let execution_binding_sha256 =
        match super::finalize_check::receipt_envelope(Some(&binding_scope), Some(&canonical)) {
            Ok(envelope) => envelope.execution_binding_sha256,
            Err(error) => {
                eprintln!("gal pipeline-handback-check: {error}");
                return ExitCode::Error;
            }
        };
    let mut guarded_state = if guarded {
        let coordinator = coordinator.as_ref().expect("guarded coordinator path");
        let journal = coordinator.with_file_name("projection-journal.json");
        if journal.exists() {
            if let Err(error) = pipeline::projection_journal::recover(&journal) {
                eprintln!(
                    "gal pipeline-handback-check: pending projection journal recovery failed: {error}"
                );
                return ExitCode::Error;
            }
        }
        let state: CoordinatorState = match std::fs::read(coordinator)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        {
            Some(state) => state,
            None => return ExitCode::Error,
        };
        if state.profile == ContinuationProfile::CodexStopV1 {
            Some(state)
        } else {
            None
        }
    } else {
        None
    };
    if let (Some(state), Some(path)) = (guarded_state.as_mut(), coordinator.as_ref()) {
        let receipt_path = path.with_file_name("orchestrator-receipt.json");
        // Only a pending GoalBackward checkpoint consumes its receipt here.
        // Any other kind leaves the receipt on disk and the coordinator unwritten.
        let pending_goal_backward = state
            .checkpoint
            .as_ref()
            .is_some_and(|checkpoint| checkpoint.kind == CheckpointKind::GoalBackward);
        if receipt_path.exists() && pending_goal_backward {
            let receipt: CheckpointReceipt = match std::fs::read(&receipt_path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
            {
                Some(receipt) => receipt,
                None => return ExitCode::Error,
            };
            let current_head = match git_head(&repo) {
                Ok(head) => head,
                Err(_) => return ExitCode::Error,
            };
            if receipt.revision != state.revision
                || receipt.commit.as_deref() != Some(current_head.as_str())
            {
                return ExitCode::Error;
            }
            if state.consume_checkpoint_receipt(receipt).is_err() {
                return ExitCode::Error;
            }
            let bytes = match serde_json::to_vec_pretty(state) {
                Ok(bytes) => bytes,
                Err(_) => return ExitCode::Error,
            };
            if super::pipeline_driver::atomic_write(path, &bytes).is_err()
                || std::fs::remove_file(&receipt_path).is_err()
            {
                return ExitCode::Error;
            }
        }
    }
    let prompt = match std::fs::read_to_string(&canonical) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("gal pipeline-handback-check: {e}");
            return ExitCode::Usage;
        }
    };
    let (mut tuple, bindings) = classify(
        &canonical,
        &prompt,
        &repo,
        parsed.stop_at.as_deref(),
        &execution_digest,
        &execution_binding_sha256,
    );
    if guarded_state.as_ref().is_some_and(|state| {
        matches!(&state.next_action, pipeline::coordinator::NextAction::Blocked { reason } if reason.starts_with("design-defect:"))
    }) && design_stop_can_override(&tuple)
    {
        tuple = ("design-defect", "same-premise-gate-repair", true, NO_CONTINUE_ACTION).into();
    }
    if !DECISIONS.contains(&tuple.decision.as_str())
        || (tuple.voluntary_response_authorized && tuple.decision == "continue")
        || !valid_continue_action(&tuple.decision, &tuple.continue_action)
    {
        return ExitCode::Error;
    }
    if let Some(parent) = parsed.receipt.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let typed_action = guarded_state
        .as_ref()
        .and_then(|state| serde_json::to_string(&state.next_action).ok());
    let revision = guarded_state.as_ref().map(|state| state.revision);
    let commit = guarded_state
        .as_ref()
        .and_then(|state| state.checkpoint.as_ref())
        .and_then(|checkpoint| checkpoint.commit.as_deref());
    let mut receipt_tuple = tuple.clone();
    if let Some(action) = typed_action.as_deref() {
        receipt_tuple.continue_action = action.to_string();
    }
    let raw = std::fs::read(&canonical).unwrap_or_default();
    let handback = render(
        &receipt_tuple,
        &bindings,
        typed_action.as_deref(),
        revision,
        commit,
    );
    let handback = format!(
        "{}{}",
        handback,
        super::finalize_check::render_repair_dispositions_for_target(
            &[],
            "handback",
            "continuation",
            &canonical,
            &raw,
            None
        )
    );
    if std::fs::write(&parsed.receipt, handback).is_err() {
        return ExitCode::Error;
    }
    // LegacyInteractive keeps its v1 authorization-based exit contract.
    // CodexStopV1 reports successful classification independently.
    if guarded_state.is_some() || tuple.voluntary_response_authorized {
        ExitCode::Success
    } else {
        ExitCode::Error
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn log_file_phase_covers_every_dispatch_phase() {
        use dispatch::stage::Phase;
        for phase in [
            Phase::Implement,
            Phase::Test,
            Phase::Audit,
            Phase::Investigate,
            Phase::FinalizeReview,
        ] {
            // Adding a variant breaks this exhaustive match until the list is updated.
            match phase {
                Phase::Implement
                | Phase::Test
                | Phase::Audit
                | Phase::Investigate
                | Phase::FinalizeReview => {}
            }
            let name = phase.as_str();
            assert!(super::dispatch_phase_names_longest_first().contains(&name));
            assert_eq!(
                super::log_file_phase(&format!("000001-000001-000001-task-{name}.log")),
                name
            );
            assert_eq!(
                super::log_file_phase(&format!("000001-000001-000001-task-{name}-codex.log")),
                name
            );
        }
        assert_eq!(super::dispatch_phase_names_longest_first().len(), 5);
        assert_eq!(super::log_file_phase("garbled.log"), "unknown");
    }

    use super::*;
    use tempfile::tempdir;

    fn host_pin(
        mode: super::super::pipeline_preflight::DispatchMode,
    ) -> super::super::pipeline_preflight::PreflightPin {
        super::super::pipeline_preflight::PreflightPin {
            host: Some("codex".into()),
            dispatch_mode: mode,
            run_id: "fixture-run".into(),
        }
    }

    fn host_headers() -> BTreeMap<String, String> {
        [("executor", "host-subagent"), ("host", "codex")]
            .into_iter()
            .map(|(key, value)| (key.into(), value.into()))
            .collect()
    }

    fn routed_headers() -> BTreeMap<String, String> {
        [("executor".to_string(), "codex".to_string())]
            .into_iter()
            .collect()
    }

    const ALL_PHASES: [dispatch::stage::Phase; 5] = [
        dispatch::stage::Phase::Implement,
        dispatch::stage::Phase::Test,
        dispatch::stage::Phase::Audit,
        dispatch::stage::Phase::Investigate,
        dispatch::stage::Phase::FinalizeReview,
    ];

    /// Dispatch turns an absent route into a host-subagent attempt when the
    /// routed pin names a host, so that host attempt is valid evidence.
    #[test]
    fn handback_host_resolves_absent_route_despite_raw_routed_pin() {
        let pin = host_pin(super::super::pipeline_preflight::DispatchMode::Routed);
        for phase in ALL_PHASES {
            assert!(attempt_matches_pin(&host_headers(), phase, Some(&pin)));
            assert!(!attempt_matches_pin(&host_headers(), phase, None));
        }
    }

    #[test]
    fn handback_routed_pin_rejects_host_attempts_that_do_not_match_the_pinned_host() {
        let pin = host_pin(super::super::pipeline_preflight::DispatchMode::Routed);
        let unpinned = super::super::pipeline_preflight::PreflightPin {
            host: None,
            ..host_pin(super::super::pipeline_preflight::DispatchMode::Routed)
        };
        for phase in ALL_PHASES {
            // A routed pin without a host cannot own a host attempt.
            assert!(!attempt_matches_pin(
                &host_headers(),
                phase,
                Some(&unpinned)
            ));
            // The host header must equal the pinned host.
            for host in [None, Some("claude"), Some("unknown")] {
                let mut headers = host_headers();
                headers.remove("host");
                if let Some(host) = host {
                    headers.insert("host".into(), host.into());
                }
                assert!(!attempt_matches_pin(&headers, phase, Some(&pin)));
            }
            // A host header on a routed executor contradicts the routed pin.
            let mut routed_with_host = routed_headers();
            routed_with_host.insert("host".into(), "codex".into());
            assert!(!attempt_matches_pin(&routed_with_host, phase, Some(&pin)));
            assert!(attempt_matches_pin(&routed_headers(), phase, Some(&pin)));
        }
    }

    #[test]
    fn handback_host_pin_accepts_routed_attempts_only_for_finalize_review() {
        let requested = host_pin(super::super::pipeline_preflight::DispatchMode::HostSubagent);
        for phase in ALL_PHASES {
            assert!(attempt_matches_pin(
                &host_headers(),
                phase,
                Some(&requested)
            ));
            assert_eq!(
                attempt_matches_pin(&routed_headers(), phase, Some(&requested)),
                phase == dispatch::stage::Phase::FinalizeReview,
                "routed attempt in {}",
                phase.as_str()
            );
            let mut other_host = host_headers();
            other_host.insert("host".into(), "claude".into());
            assert!(!attempt_matches_pin(&other_host, phase, Some(&requested)));
        }
        let finalize = dispatch::stage::Phase::FinalizeReview;
        let mut routed_with_host = routed_headers();
        routed_with_host.insert("host".into(), "codex".into());
        assert!(!attempt_matches_pin(
            &routed_with_host,
            finalize,
            Some(&requested)
        ));
        let mut host_executor_only = host_headers();
        host_executor_only.remove("host");
        assert!(!attempt_matches_pin(
            &host_executor_only,
            finalize,
            Some(&requested)
        ));
        assert!(!attempt_matches_pin(
            &BTreeMap::new(),
            finalize,
            Some(&requested)
        ));
    }

    #[test]
    fn handback_without_a_pin_accepts_only_routed_attempts() {
        for phase in ALL_PHASES {
            assert!(attempt_matches_pin(&routed_headers(), phase, None));
            assert!(!attempt_matches_pin(&host_headers(), phase, None));
            assert!(!attempt_matches_pin(&BTreeMap::new(), phase, None));
        }
    }

    #[test]
    fn phase_names_attribute_only_as_one_issue_token() {
        assert_eq!(attributable_phase("scaffold"), "scaffold");
        assert_eq!(attributable_phase("finalize-review"), "finalize-review");
        assert_eq!(attributable_phase(""), "unknown");
        for (name, token) in [
            ("two words", "two%20words"),
            ("phase=x", "phase%3Dx"),
            ("tab\tname", "tab%09name"),
            ("newline\nname", "newline%0Aname"),
            ("100%", "100%25"),
        ] {
            assert_eq!(attributable_phase(name), token, "{name:?}");
        }
    }

    #[test]
    fn handback_host_requires_exact_verification_independence_separately_from_review() {
        let valid =
            "Verification Independence: DEGRADED_HOST_SUBAGENT\nReview Independence: full\n";
        assert!(host_verification_independence(valid));
        for invalid in [
            "",
            "Review Independence: DEGRADED_HOST_SUBAGENT",
            "Verification Independence: full",
            "Verification Independence: DEGRADED_BUNDLED",
            "Verification Independence: DEGRADED_SAME_RUNTIME",
            "Verification Independence: DEGRADED_HOST_SUBAGENT extra",
        ] {
            assert!(!host_verification_independence(invalid));
        }
        assert!(!host_verification_independence(&format!("{valid}{valid}")));
    }

    #[test]
    fn handback_host_scan_rejects_failed_evidence_even_with_passing_convergence() {
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let prompt_path = dir.path().join(".dev/plans/fixture.prompt.md");
        std::fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
        std::fs::write(&prompt_path, "fixture").unwrap();
        let scope = dir.path().join(".dev/pipeline/fixture");
        std::fs::create_dir_all(&scope).unwrap();
        std::fs::write(
            scope.join("preflight.receipt.md"),
            "host: codex\ndispatch_mode: host-subagent\nrun_id: fixture-run\n",
        )
        .unwrap();
        let task = tid(1);
        let task_dir = scope.join(&task);
        std::fs::create_dir_all(&task_dir).unwrap();
        let log = task_dir.join(format!("000002-000001-000001-{task}-implement.log"));
        let valid_prompt =
            "Verification Independence: DEGRADED_HOST_SUBAGENT\nReview Independence: full\n";
        std::fs::write(
            task_dir.join(format!("000001-000001-000001-{task}-implement.log")),
            format!("GAL-DISPATCH-LOG v1\nexecutor: codex\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-14T00:00:00Z\nterminal_state: completed\nattempt_id: routed-attempt\nsession_id: routed-session\n---STDOUT---\n"),
        )
        .unwrap();
        for terminal in [
            "completed",
            "started",
            "host-session-invalid",
            "model-tier-mismatch",
            "no-writeback",
        ] {
            std::fs::write(&log, format!("GAL-DISPATCH-LOG v1\nexecutor: host-subagent\nhost: codex\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:00Z\nterminal_state: {terminal}\nattempt_id: fixture-attempt\nsession_id: fixture-session\nactual_model: gpt-6.1-sol\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n")).unwrap();
            assert_eq!(
                host_dispatch_evidence(&prompt_path, valid_prompt, dir.path()).is_empty(),
                terminal == "completed",
                "{terminal}"
            );
            assert!(
                !host_dispatch_evidence(&prompt_path, "Review Independence: full", dir.path())
                    .is_empty()
            );
        }
        let valid_attempt = format!(
            "GAL-DISPATCH-LOG v1\nexecutor: host-subagent\nhost: codex\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:00Z\nterminal_state: completed\nattempt_id: fixture-attempt\nsession_id: fixture-session\nactual_model: gpt-6.1-sol\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"
        );
        for invalid_field in [
            "session_id: fixture-session\n",
            "actual_model: gpt-6.1-sol\n",
            "tier: standard\n",
            "model_tier_applied: yes\n",
        ] {
            std::fs::write(&log, valid_attempt.replace(invalid_field, "")).unwrap();
            assert!(
                !host_dispatch_evidence(&prompt_path, valid_prompt, dir.path()).is_empty(),
                "host completion missing {invalid_field:?} must fail closed"
            );
        }
        std::fs::write(&log, valid_attempt).unwrap();
        assert!(host_dispatch_evidence(&prompt_path, valid_prompt, dir.path()).is_empty());
        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn handback_host_header_in_superseded_attempt_requires_pin() {
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let dir = tempdir().unwrap();
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let task = tid(1);
        let prompt_path = dir.path().join(".dev/plans/fixture.prompt.md");
        std::fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
        std::fs::write(&prompt_path, "fixture").unwrap();
        let scope = dir.path().join(".dev/pipeline/fixture");
        let task_dir = scope.join(&task);
        std::fs::create_dir_all(&task_dir).unwrap();
        let first = task_dir.join(format!("000001-000001-000001-{task}-implement.log"));
        let latest = task_dir.join(format!("000002-000001-000001-{task}-implement.log"));
        std::fs::write(
            &first,
            format!("GAL-DISPATCH-LOG v1\nexecutor: codex\nhost: codex\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-14T00:00:00Z\nterminal_state: completed\nattempt_id: host-header-attempt\nsession_id: fixture-session\nactual_model: gpt-6.1-sol\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"),
        )
        .unwrap();
        std::fs::write(
            &latest,
            format!("GAL-DISPATCH-LOG v1\nexecutor: codex\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:00Z\nterminal_state: completed\nattempt_id: routed-attempt\nsession_id: fixture-session\nactual_model: gpt-6.1-sol\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"),
        )
        .unwrap();

        let passed = host_dispatch_evidence(&prompt_path, "", dir.path()).is_empty();
        std::env::set_current_dir(original).unwrap();
        assert!(
            !passed,
            "a retained host header must require a pin even when the latest attempt is routed"
        );
    }

    #[test]
    fn host_evidence_issue_set_is_independent_of_directory_order() {
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let original = std::env::current_dir().unwrap();
        let attempt = |executor: &str, host: &str, task: &str| {
            format!(
                "GAL-DISPATCH-LOG v1\nexecutor: {executor}\n{host}task_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:00Z\nterminal_state: completed\nattempt_id: fixture-attempt\nsession_id: fixture-session\nactual_model: gpt-6.1-sol\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"
            )
        };
        // Directory listings are usually name-ordered, so placing the host task
        // first and then last covers both visiting orders.
        for host_position in [1, 3] {
            let dir = tempdir().unwrap();
            std::env::set_current_dir(dir.path()).unwrap();
            let prompt_path = dir.path().join(".dev/plans/fixture.prompt.md");
            std::fs::create_dir_all(prompt_path.parent().unwrap()).unwrap();
            std::fs::write(&prompt_path, "fixture").unwrap();
            let scope = dir.path().join(".dev/pipeline/fixture");
            for number in 1..=3 {
                let task = tid(number);
                std::fs::create_dir_all(scope.join(&task)).unwrap();
                let log = if number == host_position {
                    attempt("host-subagent", "host: codex\n", &task)
                } else {
                    attempt("codex", "", &task)
                };
                std::fs::write(
                    scope
                        .join(&task)
                        .join(format!("000001-000001-000001-{task}-implement.log")),
                    log,
                )
                .unwrap();
            }
            let issues = host_dispatch_evidence(&prompt_path, "fixture", dir.path());
            std::env::set_current_dir(&original).unwrap();
            assert_eq!(
                issues,
                vec![host_issue(
                    &tid(host_position),
                    "implement",
                    HostDispatchReason::ModeMismatch
                )],
                "host task at position {host_position}"
            );
        }
    }

    // Task ids are composed at runtime, never written as digit literals in source
    // (a stale `T-<digits>` literal is just dead data; the naming gate enforces this).
    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    fn tuple(decision: &str, reason: &str, authorized: bool, action: &str) -> HandbackTuple {
        (decision, reason, authorized, action).into()
    }

    #[test]
    fn design_stop_preserves_safety_and_completion_decisions() {
        for decision in [
            "human-required",
            "retry-ceiling",
            "stop-at",
            "goal-verified",
        ] {
            assert!(
                !design_stop_can_override(&tuple(decision, "existing", true, NO_CONTINUE_ACTION)),
                "design stop must preserve {decision}"
            );
        }
        assert!(design_stop_can_override(&tuple(
            "continue",
            "none",
            false,
            RUN_GOAL_BACKWARD_VERIFICATION
        )));
    }

    fn prompt(tasks: &str, status: &str, extra: &str) -> String {
        format!("# fixture\n\n## Tasks\n{tasks}\n\n## Status\nCurrent Task: {status}\n\n{extra}\n")
    }

    fn prompt_file(text: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().expect("fixture directory");
        let path = dir.path().join("fixture.prompt.md");
        std::fs::write(&path, text).expect("fixture prompt");
        (dir, path)
    }

    fn host_evidence_fixture(root: &Path, task: &str, phase: &str, terminal_state: &str) {
        let scope = root.join(".dev/pipeline/fixture");
        std::fs::create_dir_all(scope.join(task)).unwrap();
        std::fs::write(
            scope.join("preflight.receipt.md"),
            "host: codex\ndispatch_mode: host-subagent\nrun_id: fixture-run\n",
        )
        .unwrap();
        let log = scope
            .join(task)
            .join(format!("000001-000001-000001-{task}-{phase}.log"));
        std::fs::write(
            log,
            format!(
                "GAL-DISPATCH-LOG v1\nexecutor: host-subagent\nhost: codex\ntask_id: {task}\nphase: {phase}\ntimestamp_start: 2026-10-15T00:00:00Z\nterminal_state: {terminal_state}\nattempt_id: fixture-attempt\nsession_id: fixture-session\nactual_model: gpt-6.1-sol\nactual_effort: high\ntier: standard\ntier_source: standard\nmodel_tier_applied: yes\n---STDOUT---\n"
            ),
        )
        .unwrap();
    }

    fn bound_goal_receipt(prompt_path: &Path, prompt: &str, head: &str) -> String {
        format!(
            "prompt_path: {}\nprompt_sha256: {:x}\nhead: {head}\nchecked_tasks: {}\ncurrent_task: —\nmust_have_1: expected\ncommand: cargo test -p gal-cli pipeline_handback_check\nverdict: VERIFIED\n",
            prompt_path.to_string_lossy().replace('\\', "/"),
            Sha256::digest(prompt.as_bytes()),
            checked_projection(prompt)
        )
    }

    fn initialize_fixture_repo(root: &Path) {
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "user.email", "fixture@example.invalid"],
            vec!["config", "user.name", "Fixture"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "fixture"],
        ] {
            assert!(std::process::Command::new("git")
                .current_dir(root)
                .args(args)
                .status()
                .unwrap()
                .success());
        }
    }

    /// Reads the test repository's HEAD through the process CWD. Every caller
    /// must already hold the binary-wide `ENV_GUARD` for its whole body, since
    /// the code under test resolves the repo from the CWD too — taking the lock
    /// here instead would leave that second read unprotected, and would
    /// deadlock against a caller that already holds it.
    fn current_head() -> String {
        git_head(Path::new(".")).expect("test repository HEAD")
    }

    fn handback(reason: &str, task: &str, phase: &str, head: &str) -> String {
        let drift = if reason == "head-drift" {
            format!(
                "Baseline HEAD: {}\nObserved HEAD: {head}\n",
                "0123456789abcdef0123456789abcdef01234567"
            )
        } else {
            String::new()
        };
        format!(
            "#### Human Handback — {reason}\nStatus: OPEN\nReason: {reason}\nTask: {task}\nPhase: {phase}\nProducer: {}\nProducer state: blocked\nNext human step: await the required decision\nWhat to check: the blocked task output\nExpected result: the output matches the task goal\nPass/fail rule: pass when every goal item is met\nGit HEAD: {head}\n{drift}",
            expected_producer(reason)
        )
    }

    #[test]
    fn pipeline_handback_check_table_covers_all_decisions_reasons_and_collisions() {
        // The code under test resolves the repo from the process CWD, so this
        // test must hold the binary-wide lock every CWD-mutating test takes.
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let head = current_head();
        let (t1, t2, t99) = (tid(1), tid(2), tid(99));
        let checked = format!("- [x] {t1} — complete *({head})*");
        let unchecked = format!("- [x] {t1} — complete *({head})*\n- [ ] {t2} — pending");

        let mut cases: Vec<(&str, HandbackTuple)> = vec![
            (
                "T1 checked/T2 unchecked",
                tuple(
                    "continue",
                    "none",
                    false,
                    &format!("/gal pipeline <prompt> from {t2}"),
                ),
            ),
            (
                "all checked without goal",
                tuple("continue", "none", false, RUN_GOAL_BACKWARD_VERIFICATION),
            ),
            (
                "retry below ceiling",
                tuple("continue", "none", false, RUN_GOAL_BACKWARD_VERIFICATION),
            ),
            (
                "retry at ceiling",
                tuple("retry-ceiling", "none", true, NO_CONTINUE_ACTION),
            ),
            (
                "fresh bound goal",
                tuple("goal-verified", "none", true, NO_CONTINUE_ACTION),
            ),
            (
                "proven stop-at",
                tuple("stop-at", "none", true, NO_CONTINUE_ACTION),
            ),
            (
                "stop-at nonexistent",
                tuple(
                    "continue",
                    "none",
                    false,
                    &format!("repair-stop-at-target {t99}"),
                ),
            ),
            (
                "stop-at unchecked",
                tuple(
                    "continue",
                    "none",
                    false,
                    &format!("/gal pipeline <prompt> from {t2} stop-at {t2}"),
                ),
            ),
            (
                "stop-at wrong target",
                tuple(
                    "continue",
                    "none",
                    false,
                    &format!("/gal pipeline <prompt> from {t2} stop-at {t2}"),
                ),
            ),
            (
                "stop-at stale convergence",
                tuple(
                    "continue",
                    "none",
                    false,
                    &format!("repair-stop-at-convergence {t1}"),
                ),
            ),
        ];

        for reason in HUMAN_REASONS {
            cases.push((
                reason,
                tuple("human-required", reason, true, NO_CONTINUE_ACTION),
            ));
        }

        for collision in [
            "human handback missing",
            "human handback duplicate",
            "human handback wrong reason",
            "human handback wrong task",
            "human handback wrong phase",
            "human handback stale HEAD",
            "head-drift missing baseline",
            "head-drift equal baseline",
            "foreign goal",
            "earlier HEAD goal",
            "changed prompt goal",
            "malformed goal",
            "forged VERIFIED prose",
            "incomplete phase evidence",
            "invented interruption marker",
            "malformed pin evidence",
        ] {
            cases.push((
                collision,
                tuple("continue", "none", false, RUN_GOAL_BACKWARD_VERIFICATION),
            ));
        }

        cases.push((
            "design-defect stop",
            tuple(
                "design-defect",
                "same-premise-gate-repair",
                true,
                NO_CONTINUE_ACTION,
            ),
        ));
        assert_eq!(cases.len(), 32, "fixture inventory changed without review");
        assert!(DECISIONS.iter().all(|decision| {
            cases
                .iter()
                .any(|(_, expected)| expected.decision == *decision)
        }));

        let (dir, path) = prompt_file(&prompt(&unchecked, "—", ""));
        let (actual, _) = classify(
            &path,
            &std::fs::read_to_string(&path).unwrap(),
            Path::new("."),
            None,
            "",
            "",
        );
        assert_eq!(actual, cases[0].1, "case: {}", cases[0].0);
        drop(dir);

        let (dir, path) = prompt_file(&prompt(&checked, "—", ""));
        let text = std::fs::read_to_string(&path).unwrap();
        let (actual, _) = classify(&path, &text, Path::new("."), None, "", "");
        assert_eq!(actual, cases[1].1, "case: {}", cases[1].0);
        drop(dir);

        let retry =
            format!("Test Retry Count: 3\n#### Retry Handoff — {t1} / IMPLEMENT\nStatus: OPEN\n");
        let (dir, path) = prompt_file(&prompt(&checked, &t1, &retry));
        let text = std::fs::read_to_string(&path).unwrap();
        let (actual, _) = classify(&path, &text, Path::new("."), None, "", "");
        assert_eq!(actual, cases[3].1, "case: {}", cases[3].0);
        drop(dir);

        for (index, reason) in HUMAN_REASONS.iter().enumerate() {
            let phase = match *reason {
                "security-protected-path" => "AUDIT",
                "goal-gaps-blocked" => "VERIFY",
                "head-drift" => "CONVERGE",
                "boundary-scope-decision" => "BOUNDARY",
                _ => "CONVERGE",
            };
            let text = prompt(&checked, "—", &handback(reason, &t1, phase, &head));
            let (dir, path) = prompt_file(&text);
            let (actual, _) = classify(&path, &text, Path::new("."), None, "", "");
            assert_eq!(actual, cases[10 + index].1, "case: {}", cases[10 + index].0);
            drop(dir);
        }
    }

    #[test]
    fn host_evidence_pairs_preserve_human_and_unchecked_routing() {
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let original = std::env::current_dir().unwrap();
        let root = tempdir().unwrap();
        initialize_fixture_repo(root.path());
        std::env::set_current_dir(root.path()).unwrap();
        std::fs::create_dir_all(root.path().join(".dev/plans")).unwrap();
        let (t1, t2) = (tid(1), tid(2));
        let head = current_head();
        let tasks = format!("- [x] {t1}\n- [ ] {t2}");
        let handback_text = prompt(
            &tasks,
            "—",
            &format!(
                "{}\nVerification Independence: DEGRADED_HOST_SUBAGENT\n",
                handback("goal-gaps-blocked", &t1, "VERIFY", &head)
            ),
        );
        let prompt_path = root.path().join(".dev/plans/fixture.prompt.md");
        std::fs::write(&prompt_path, &handback_text).unwrap();

        host_evidence_fixture(root.path(), &t1, "implement", "completed");
        let passing = classify(&prompt_path, &handback_text, Path::new("."), None, "", "");
        assert_eq!(passing.0.decision, "human-required");
        assert_eq!(passing.0.continue_action, NO_CONTINUE_ACTION);
        assert!(!passing.1.contains("host_dispatch_evidence: fail"));

        host_evidence_fixture(root.path(), &t1, "implement", "started");
        let failing = classify(&prompt_path, &handback_text, Path::new("."), None, "", "");
        assert_eq!(failing.0, passing.0);
        assert_eq!(failing.0.continue_action, NO_CONTINUE_ACTION);
        assert!(failing.1.contains("host_dispatch_evidence: fail"));

        let unchecked_text = prompt(
            &tasks,
            "—",
            "Verification Independence: DEGRADED_HOST_SUBAGENT\n",
        );
        std::fs::write(&prompt_path, &unchecked_text).unwrap();
        host_evidence_fixture(root.path(), &t1, "implement", "completed");
        let passing = classify(&prompt_path, &unchecked_text, Path::new("."), None, "", "");
        assert_eq!(passing.0.decision, "continue");
        assert_eq!(
            passing.0.continue_action,
            format!("/gal pipeline <prompt> from {t2}")
        );
        assert!(!passing.1.contains("host_dispatch_evidence: fail"));

        host_evidence_fixture(root.path(), &t1, "implement", "started");
        let failing = classify(&prompt_path, &unchecked_text, Path::new("."), None, "", "");
        assert_eq!(failing.0, passing.0);
        assert_eq!(failing.0.continue_action, passing.0.continue_action);
        assert!(!failing
            .0
            .continue_action
            .contains(RUN_GOAL_BACKWARD_VERIFICATION));
        assert!(failing.1.contains("host_dispatch_evidence: fail"));
        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn failed_host_evidence_blocks_fresh_goal_and_annotates_terminal_receipts() {
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let original = std::env::current_dir().unwrap();
        let root = tempdir().unwrap();
        initialize_fixture_repo(root.path());
        std::env::set_current_dir(root.path()).unwrap();
        std::fs::create_dir_all(root.path().join(".dev/plans")).unwrap();
        let task = tid(1);
        let head = current_head();
        let tasks = format!("- [x] {task}");
        let goal_text = prompt(
            &tasks,
            "—",
            "Verification Independence: DEGRADED_HOST_SUBAGENT\n",
        );
        let prompt_path = root.path().join(".dev/plans/fixture.prompt.md");
        std::fs::write(&prompt_path, &goal_text).unwrap();
        let scope = root.path().join(".dev/pipeline/fixture");
        std::fs::create_dir_all(&scope).unwrap();
        std::fs::write(
            scope.join("goal-verification.receipt.md"),
            bound_goal_receipt(&prompt_path, &goal_text, &head),
        )
        .unwrap();

        host_evidence_fixture(root.path(), &task, "implement", "completed");
        assert!(host_dispatch_evidence(
            &prompt_path,
            "Verification Independence: DEGRADED_HOST_SUBAGENT\n",
            root.path()
        )
        .is_empty());
        host_evidence_fixture(root.path(), &task, "implement", "started");
        let (tuple, receipt) = classify(&prompt_path, &goal_text, Path::new("."), None, "", "");
        assert_eq!(tuple.decision, "continue");
        assert_eq!(tuple.continue_action, RUN_GOAL_BACKWARD_VERIFICATION);
        assert_ne!(tuple.decision, "goal-verified");
        assert!(receipt.contains("host_dispatch_evidence: fail"));

        let retry =
            format!("Test Retry Count: 3\n#### Retry Handoff — {task} / IMPLEMENT\nStatus: OPEN\n");
        let retry_text = prompt(&tasks, &task, &retry);
        std::fs::write(&prompt_path, &retry_text).unwrap();
        let (retry_tuple, retry_receipt) =
            classify(&prompt_path, &retry_text, Path::new("."), None, "", "");
        assert_eq!(retry_tuple.decision, "retry-ceiling");
        assert!(retry_receipt.contains("host_dispatch_evidence: fail"));

        let stop_at_text = prompt(&format!("- [x] {task} — complete *({head})*"), "—", "");
        std::fs::write(&prompt_path, &stop_at_text).unwrap();
        std::fs::write(root.path().join(".dev/plans/fixture.md"), &stop_at_text).unwrap();
        for phase in ["implement", "test", "audit"] {
            host_evidence_fixture(root.path(), &task, phase, "completed");
        }
        std::fs::write(scope.join("preflight.receipt.md"), "invalid pin\n").unwrap();
        let surface = check_three_surface_and_task(&task, &prompt_path, &stop_at_text, root.path());
        assert_eq!(surface.state, CheckState::Pass, "{}", surface.summary);
        assert_eq!(
            check_task_commit(&task, &stop_at_text, root.path()).state,
            CheckState::Pass
        );
        assert_eq!(
            check_cursor_cleared(&task, &stop_at_text).state,
            CheckState::Pass
        );
        assert!(convergence(&prompt_path, &stop_at_text, root.path(), &task));
        let (stop_tuple, stop_receipt) = classify(
            &prompt_path,
            &stop_at_text,
            root.path(),
            Some(&task),
            "",
            "",
        );
        assert_eq!(stop_tuple.decision, "stop-at");
        assert!(stop_receipt.contains("host_dispatch_evidence: fail"));
        std::env::set_current_dir(original).unwrap();
    }

    #[test]
    fn human_handback_collisions_fail_closed() {
        // The code under test resolves the repo from the process CWD, so this
        // test must hold the binary-wide lock every CWD-mutating test takes.
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let head = current_head();
        let (t1, t2) = (tid(1), tid(2));
        let valid = handback("goal-gaps-blocked", &t1, "VERIFY", &head);
        assert!(valid_human(
            &valid,
            "goal-gaps-blocked",
            &t1,
            "VERIFY",
            &head
        ));
        assert!(!valid_human("", "goal-gaps-blocked", &t1, "VERIFY", &head));
        assert!(!valid_human(
            &format!("{valid}\n{valid}"),
            "goal-gaps-blocked",
            &t1,
            "VERIFY",
            &head
        ));
        let resolved = valid.replace("Status: OPEN", "Status: RESOLVED");
        assert!(valid_human(
            &format!("{resolved}\n{valid}"),
            "goal-gaps-blocked",
            &t1,
            "VERIFY",
            &head
        ));
        assert!(!valid_human(
            &valid.replace(
                "Reason: goal-gaps-blocked",
                "Reason: security-protected-path"
            ),
            "goal-gaps-blocked",
            &t1,
            "VERIFY",
            &head
        ));
        assert!(!valid_human(
            &valid,
            "goal-gaps-blocked",
            &t2,
            "VERIFY",
            &head
        ));
        assert!(!valid_human(
            &valid,
            "goal-gaps-blocked",
            &t1,
            "AUDIT",
            &head
        ));
        assert!(!valid_human(
            &valid,
            "goal-gaps-blocked",
            &t1,
            "VERIFY",
            "0".repeat(40).as_str()
        ));
    }

    #[test]
    fn human_handback_parser_accepts_contract_bullets() {
        let task = format!("T-{:02}", 1);
        let head = "0".repeat(40);
        let prompt = format!(
            "#### Human Handback — convergence-human-repair\n\n\
- Status: OPEN\n\
- Reason: convergence-human-repair\n\
- Task: {task}\n\
- Phase: CONVERGE\n\
- Producer: CONVERGE\n\
- Producer state: blocked\n\
- Git HEAD: {head}\n\
- Next human step: inspect output\n\
- What to check: preserved task diff\n\
- Expected result: explicit owner decision\n\
- Pass/fail rule: pass only when accepted\n"
        );
        assert!(valid_human(
            &prompt,
            "convergence-human-repair",
            &task,
            "CONVERGE",
            &head,
        ));
    }

    #[test]
    fn retry_ceiling_open_status_ignores_ascii_case_and_needs_the_current_task() {
        let task = tid(9);
        let text = |current: &str, status: &str| {
            prompt(
                &format!("- [ ] {task}"),
                current,
                &format!("Test Retry Count: 3\n#### Retry Handoff — {task} / TEST\n{status}\n"),
            )
        };
        for status in [
            "Status: OPEN",
            "- Status: OPEN",
            "status: open",
            "- STATUS: Open",
        ] {
            assert!(retry_ceiling(&text(&task, status), &task), "{status}");
        }
        for status in ["Status: RESOLVED", "Status: OPENED", "- Status:"] {
            assert!(!retry_ceiling(&text(&task, status), &task), "{status}");
        }
        // A cleared cursor never names the task, so it never triggers the rule.
        assert!(!retry_ceiling(&text("—", "Status: OPEN"), &task));
    }

    #[test]
    fn tp_04_binding_evidence_stays_at_scope_root() {
        let workdir = Path::new("/repo");
        let prompt = Path::new(".dev/plans/fix-foo.prompt.md");
        assert_eq!(
            resolve_log_dir_override(workdir, Some(prompt), None).unwrap(),
            Some(PathBuf::from("/repo/.dev/pipeline/fix-foo"))
        );
    }

    #[test]
    fn host_dispatch_issue_receipt_lines_are_sorted_and_keep_summary() {
        let mut issues = vec![
            host_issue("task-b", "test", HostDispatchReason::NotCompleted),
            host_issue(
                "task-a",
                "test",
                HostDispatchReason::NoWritebackWithoutWitness,
            ),
            host_issue("task-a", "implement", HostDispatchReason::MalformedLog),
            host_issue("task-a", "implement", HostDispatchReason::ModeMismatch),
            host_issue("task-a", "audit", HostDispatchReason::ExecutorMismatch),
            host_issue(
                "unknown",
                "unknown",
                HostDispatchReason::UnreadableLogDirectory,
            ),
            host_issue("task-a", "implement", HostDispatchReason::ModeMismatch),
        ];
        // The production sort orders by reason text, not by enum declaration order.
        sort_host_issues(&mut issues);
        assert_eq!(issues.len(), 6, "an exact repeat is dropped");
        let receipt = annotate_host_dispatch_evidence("receipt".into(), &issues);
        assert!(receipt.starts_with("receipt\nhost_dispatch_evidence: fail\n"));
        assert!(receipt.contains("host_dispatch_evidence_issue: task=task-a phase=audit reason=executor-mismatch\nhost_dispatch_evidence_issue: task=task-a phase=implement reason=malformed-log\nhost_dispatch_evidence_issue: task=task-a phase=implement reason=mode-mismatch\nhost_dispatch_evidence_issue: task=task-a phase=test reason=no-writeback-without-witness\nhost_dispatch_evidence_issue: task=task-b phase=test reason=not-completed\nhost_dispatch_evidence_issue: task=unknown phase=unknown reason=unreadable-log-directory"));
        assert_eq!(
            annotate_host_dispatch_evidence("receipt".into(), &[]),
            "receipt"
        );
    }

    #[test]
    fn terminal_decisions_emit_no_continue_action() {
        for decision in [
            "goal-verified",
            "human-required",
            "retry-ceiling",
            "stop-at",
        ] {
            let tuple = tuple(decision, "none", true, NO_CONTINUE_ACTION);
            assert_eq!(
                tuple.continue_action, NO_CONTINUE_ACTION,
                "decision: {decision}"
            );
            assert!(annotate_host_dispatch_evidence(
                "receipt".into(),
                &[host_issue(
                    "unknown",
                    "unknown",
                    HostDispatchReason::ModeMismatch
                )]
            )
            .contains("host_dispatch_evidence: fail"));
        }
    }

    #[test]
    fn terminal_decisions_reject_executable_continue_actions() {
        let t1 = tid(1);
        for decision in [
            "goal-verified",
            "human-required",
            "retry-ceiling",
            "stop-at",
        ] {
            assert!(!valid_continue_action(
                decision,
                RUN_GOAL_BACKWARD_VERIFICATION
            ));
            assert!(!valid_continue_action(
                decision,
                &format!("/gal pipeline <prompt> from {t1}")
            ));
            assert!(valid_continue_action(decision, NO_CONTINUE_ACTION));
        }
    }

    #[test]
    fn decisions_are_ordered() {
        assert_eq!(
            DECISIONS,
            [
                "continue",
                "goal-verified",
                "design-defect",
                "human-required",
                "retry-ceiling",
                "stop-at",
            ]
        );
    }

    #[test]
    fn command_removes_stale_receipt_before_rejection_or_prompt_resolution() {
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let original = std::env::current_dir().expect("current directory");
        let fixture = tempdir().expect("fixture directory");
        std::env::set_current_dir(fixture.path()).expect("fixture cwd");

        let prompt_path = fixture.path().join("fixture.prompt.md");
        let receipt = fixture.path().join("receipt.md");
        std::fs::write(&prompt_path, prompt(&format!("- [x] {}", tid(1)), "—", ""))
            .expect("fixture prompt");
        std::fs::write(&receipt, "stale").expect("stale receipt");
        assert_eq!(
            cmd_pipeline_handback_check(&[
                "pipeline-handback-check".into(),
                prompt_path.to_string_lossy().into_owned(),
                "--receipt".into(),
                receipt.to_string_lossy().into_owned(),
                "--stop-at".into(),
                "foo".into(),
            ]),
            ExitCode::Error
        );
        assert!(!receipt.exists());

        std::fs::write(&receipt, "stale").expect("stale receipt");
        let missing = fixture.path().join("missing.prompt.md");
        assert_eq!(
            cmd_pipeline_handback_check(&[
                "pipeline-handback-check".into(),
                missing.to_string_lossy().into_owned(),
                "--receipt".into(),
                receipt.to_string_lossy().into_owned(),
            ]),
            ExitCode::Usage
        );
        assert!(!receipt.exists());

        std::env::set_current_dir(original).expect("restore current directory");
        drop(fixture);
    }

    #[test]
    fn pending_stop_at_does_not_suppress_retry_or_human_handback() {
        // The code under test resolves the repo from the process CWD, so this
        // test must hold the binary-wide lock every CWD-mutating test takes.
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let head = current_head();
        let t1 = tid(1);
        let t2 = tid(2);
        let retry =
            format!("Test Retry Count: 3\n\n#### Retry Handoff — {t1} / IMPLEMENT\nStatus: OPEN\n");
        let (dir, path) = prompt_file(&prompt(&format!("- [x] {t1}\n- [ ] {t2}"), &t1, &retry));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            classify(&path, &text, Path::new("."), Some(&t2), "", "")
                .0
                .decision,
            "retry-ceiling"
        );
        drop(dir);

        let handback = handback("goal-gaps-blocked", &t1, "VERIFY", &head);
        let (dir, path) = prompt_file(&prompt(&format!("- [x] {t1}\n- [ ] {t2}"), "—", &handback));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            classify(&path, &text, Path::new("."), Some(&t2), "", "")
                .0
                .decision,
            "human-required"
        );
        drop(dir);
    }

    #[test]
    fn continuation_actions_advance_preserve_stop_target_and_select_verification() {
        // The code under test resolves the repo from the process CWD, so this
        // test must hold the binary-wide lock every CWD-mutating test takes.
        let _env_guard = crate::commands::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let head = current_head();
        let (t1, t2, t99) = (tid(1), tid(2), tid(99));
        let tasks = format!("- [x] {t1} — complete *({head})*\n- [ ] {t2} — pending");
        let (dir, path) = prompt_file(&prompt(&tasks, "—", ""));
        let text = std::fs::read_to_string(&path).unwrap();

        let pending = classify(&path, &text, Path::new("."), Some(&t2), "", "").0;
        assert_eq!(pending.decision, "continue");
        assert_eq!(
            pending.continue_action,
            format!("/gal pipeline <prompt> from {t2} stop-at {t2}")
        );

        let invalid = classify(&path, &text, Path::new("."), Some(&t99), "", "").0;
        assert_eq!(
            invalid.continue_action,
            format!("repair-stop-at-target {t99}")
        );
        drop(dir);

        let (dir, path) = prompt_file(&prompt(&format!("- [x] {t1}"), "—", ""));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            classify(&path, &text, Path::new("."), None, "", "")
                .0
                .continue_action,
            RUN_GOAL_BACKWARD_VERIFICATION
        );
        assert_eq!(
            classify(&path, &text, Path::new("."), Some(&t1), "", "")
                .0
                .continue_action,
            format!("repair-stop-at-convergence {t1}")
        );
        drop(dir);
    }

    #[test]
    fn parser_and_binding_primitives_reject_malformed_or_foreign_records() {
        let (t1, t2) = (tid(1), tid(2));
        assert_eq!(
            parse_args(&[]).unwrap_err(),
            "pipeline-handback-check requires a <prompt> path"
        );
        assert_eq!(
            parse_args(&["--unknown".into(), "x.md".into()]).unwrap_err(),
            "unknown option '--unknown'"
        );
        assert!(task_id(&t1));
        assert!(!task_id("T-x"));
        assert!(valid_head("0123456789abcdef0123456789abcdef01234567"));
        assert!(!valid_head("foreign"));
        assert!(valid_continue_action(
            "continue",
            RUN_GOAL_BACKWARD_VERIFICATION
        ));
        assert!(valid_continue_action(
            "continue",
            &format!("/gal pipeline <prompt> from {t1} stop-at {t2}")
        ));
        assert!(!valid_continue_action(
            "continue",
            &format!("gal.exe pipeline <prompt> from {t1} stop-at {t2}")
        ));
        assert!(!valid_continue_action(
            "continue",
            "gal.exe pipeline-handback-check <prompt>"
        ));
        assert_eq!(
            checked_projection(&format!("## Tasks\n- [x] {t2}\n- [x] {t1}")),
            format!("{t1},{t2}")
        );

        let record = parse_key_values(
            "prompt_path: foreign\nprompt_sha256: deadbeef\nhead: foreign\nverdict: VERIFIED\nevidence: advisory\ncommand: fake",
        );
        assert_ne!(value(&record, &["prompt_path"]), Some("current"));
        assert_eq!(value(&record, &["verdict"]), Some("VERIFIED"));
        assert_eq!(record.generic_evidence.len(), 1);
        assert!(!complete_must_haves(&record));
        assert_eq!(record.commands.len(), 1);

        let complete = parse_key_values("must_have_1: first\nmust_have_2: second");
        assert!(complete_must_haves(&complete));
        for malformed in [
            "must_have_2: gap",
            "must_have_1: first\nmust_have_1: duplicate",
            "must_have_x: malformed",
            "must_have_0: zero",
            "must_have_01: non-canonical",
            "MUST_HAVE_1: wrong-case",
            "must-have-1: wrong-separator",
            "must_have_1:",
        ] {
            assert!(
                !complete_must_haves(&parse_key_values(malformed)),
                "malformed must-have schema passed: {malformed}"
            );
        }
    }

    #[test]
    fn compatibility_pipeline_contract_generated_action_grammar_and_stop_at() {
        let task = tid(7);
        let generated = format!("/gal pipeline <prompt> from {task} stop-at {task}");
        assert!(valid_continue_action("continue", &generated));
        assert!(valid_continue_action(
            "continue",
            &format!("/gal pipeline <prompt> from {task}")
        ));
        assert!(!valid_continue_action(
            "continue",
            &format!("/gal pipeline <prompt> from {task} stop-at T-x")
        ));
        assert!(!valid_continue_action(
            "continue",
            &format!("/gal pipeline <prompt> from {task} --phase test")
        ));
    }

    #[test]
    fn goal_binding_predicate_projection_is_ordered() {
        let mut result = GoalBindingResult {
            prompt_path: true,
            prompt_hash: true,
            head: true,
            checked_task_projection: true,
            cursor: true,
            must_haves: true,
            commands: true,
            verdict: true,
        };
        assert_eq!(
            result.predicate_states().collect::<Vec<_>>(),
            vec![
                ("prompt-path", Some(true)),
                ("prompt-hash", Some(true)),
                ("head", Some(true)),
                ("checked-task-projection", Some(true)),
                ("cursor", Some(true)),
                ("must-haves", Some(true)),
                ("commands", Some(true)),
                ("verdict", Some(true)),
            ]
        );
        assert!(result.passed());
        assert!(result.failed_predicates().is_empty());

        result.prompt_hash = false;
        assert!(!result.passed());
        assert_eq!(result.failed_predicates(), vec!["prompt-hash"]);
    }
}
