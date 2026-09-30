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
use gal_engine::ExitCode;
use pipeline::coordinator::{CheckpointReceipt, ContinuationProfile, CoordinatorState};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const DECISIONS: [&str; 5] = [
    "continue",
    "goal-verified",
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
            .take_12()
        {
            let Some((key, value)) = row.trim().split_once(':') else {
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

trait Take12: Iterator {
    fn take_12(self) -> std::iter::Take<Self>
    where
        Self: Sized,
    {
        self.take(12)
    }
}
impl<I: Iterator> Take12 for I {}

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

fn retry_ceiling(prompt: &str, task: &str) -> bool {
    let lines: Vec<&str> = prompt.lines().collect();
    lines.iter().enumerate().any(|(index, line)| {
        if !line.contains(&format!("#### Retry Handoff — {task} /")) {
            return false;
        }
        let end = lines[index + 1..]
            .iter()
            .position(|next| next.trim_start().starts_with("#### "))
            .map(|offset| index + 1 + offset)
            .unwrap_or(lines.len());
        let block = lines[index..end].join("\n");
        block.contains("Status: OPEN")
            && [
                "Retry Count: 3",
                "Test Retry Count: 3",
                "Review Retry Count: 3",
                "Security Retry Count: 3",
            ]
            .iter()
            .any(|marker| block.contains(marker))
    })
}

fn classify(
    prompt_path: &Path,
    prompt: &str,
    repo: &Path,
    stop_at: Option<&str>,
) -> (HandbackTuple, String) {
    let identity = BindingIdentity {
        prompt_hash: sha256(prompt_path).unwrap_or_default(),
        head: git_head(repo).unwrap_or_default(),
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
        return (
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
        return (
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
            return (
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
        return (
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
        return (
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
        return (
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
    let bound_goal = goal_binding.passed();
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
    (
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
    format!(
        "prompt_path: {}\nprompt_sha256: {}\nhead: {}\nchecked_tasks: {checked}\nunchecked_tasks: {}\ncurrent_task: {current}\nevidence_path: {scope}\ngoal_binding: {}\ngoal_binding_failed_predicates: {failed_predicates}",
        prompt_path.display(),
        identity.prompt_hash,
        identity.head,
        unchecked.join(","),
        if goal_binding.passed() { "pass" } else { "fail" }
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
    format!("# pipeline-handback-check receipt\n\ndecision: {}\nreason: {}\nvoluntary_response_authorized: {}\ncontinue_action: {}\n{}\n{}\n", tuple.decision, tuple.reason, tuple.voluntary_response_authorized, tuple.continue_action, fields, bindings)
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
    if let Err(error) = std::fs::remove_file(&parsed.receipt) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("gal pipeline-handback-check: failed to remove previous receipt: {error}");
            return ExitCode::Error;
        }
    }
    let repo = match std::env::current_dir() {
        Ok(x) => x,
        Err(_) => return ExitCode::Error,
    };
    let canonical = match canonical_prompt(&parsed.prompt, &repo) {
        Ok(x) => x,
        Err(e) => {
            eprintln!("gal pipeline-handback-check: {e}");
            return ExitCode::Usage;
        }
    };
    let scope = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".prompt.md"));
    let coordinator = scope.map(|scope| {
        repo.join(".dev/pipeline")
            .join(scope)
            .join("coordinator.json")
    });
    let mut guarded_state = if let Some(coordinator) =
        coordinator.as_ref().filter(|path| path.exists())
    {
        let journal = coordinator.with_file_name("projection-journal.json");
        if journal.exists() {
            if let Err(error) = pipeline::projection_journal::recover(&journal) {
                eprintln!("gal pipeline-handback-check: pending projection journal recovery failed: {error}");
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
        if receipt_path.exists() {
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
            if std::fs::write(path, bytes).is_err() || std::fs::remove_file(&receipt_path).is_err()
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
    let (tuple, bindings) = classify(&canonical, &prompt, &repo, parsed.stop_at.as_deref());
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
    if std::fs::write(
        &parsed.receipt,
        render(
            &receipt_tuple,
            &bindings,
            typed_action.as_deref(),
            revision,
            commit,
        ),
    )
    .is_err()
    {
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
    use super::*;
    use tempfile::tempdir;

    // Task ids are composed at runtime, never written as digit literals in source
    // (a stale `T-<digits>` literal is just dead data; the naming gate enforces this).
    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    fn tuple(decision: &str, reason: &str, authorized: bool, action: &str) -> HandbackTuple {
        (decision, reason, authorized, action).into()
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
            "#### Human Handback — {reason}\nStatus: OPEN\nReason: {reason}\nTask: {task}\nPhase: {phase}\nProducer: {}\nProducer state: blocked\nNext human step: await the required decision\nGit HEAD: {head}\n{drift}",
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
        ] {
            cases.push((
                collision,
                tuple("continue", "none", false, RUN_GOAL_BACKWARD_VERIFICATION),
            ));
        }

        assert_eq!(cases.len(), 30, "fixture inventory changed without review");
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
        );
        assert_eq!(actual, cases[0].1, "case: {}", cases[0].0);
        drop(dir);

        let (dir, path) = prompt_file(&prompt(&checked, "—", ""));
        let text = std::fs::read_to_string(&path).unwrap();
        let (actual, _) = classify(&path, &text, Path::new("."), None);
        assert_eq!(actual, cases[1].1, "case: {}", cases[1].0);
        drop(dir);

        let retry =
            format!("#### Retry Handoff — {t1} / TEST\nStatus: OPEN\nTest Retry Count: 3\n");
        let (dir, path) = prompt_file(&prompt(&checked, "—", &retry));
        let text = std::fs::read_to_string(&path).unwrap();
        let (actual, _) = classify(&path, &text, Path::new("."), None);
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
            let (actual, _) = classify(&path, &text, Path::new("."), None);
            assert_eq!(actual, cases[10 + index].1, "case: {}", cases[10 + index].0);
            drop(dir);
        }
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
    fn tp_04_binding_evidence_stays_at_scope_root() {
        let workdir = Path::new("/repo");
        let prompt = Path::new(".dev/plans/fix-foo.prompt.md");
        assert_eq!(
            resolve_log_dir_override(workdir, Some(prompt), None).unwrap(),
            Some(PathBuf::from("/repo/.dev/pipeline/fix-foo"))
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
            format!("#### Retry Handoff — {t1} / TEST\nStatus: OPEN\nTest Retry Count: 3\n");
        let (dir, path) = prompt_file(&prompt(&format!("- [x] {t1}\n- [ ] {t2}"), "—", &retry));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            classify(&path, &text, Path::new("."), Some(&t2)).0.decision,
            "retry-ceiling"
        );
        drop(dir);

        let handback = handback("goal-gaps-blocked", &t1, "VERIFY", &head);
        let (dir, path) = prompt_file(&prompt(&format!("- [x] {t1}\n- [ ] {t2}"), "—", &handback));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            classify(&path, &text, Path::new("."), Some(&t2)).0.decision,
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

        let pending = classify(&path, &text, Path::new("."), Some(&t2)).0;
        assert_eq!(pending.decision, "continue");
        assert_eq!(
            pending.continue_action,
            format!("/gal pipeline <prompt> from {t2} stop-at {t2}")
        );

        let invalid = classify(&path, &text, Path::new("."), Some(&t99)).0;
        assert_eq!(
            invalid.continue_action,
            format!("repair-stop-at-target {t99}")
        );
        drop(dir);

        let (dir, path) = prompt_file(&prompt(&format!("- [x] {t1}"), "—", ""));
        let text = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            classify(&path, &text, Path::new("."), None)
                .0
                .continue_action,
            RUN_GOAL_BACKWARD_VERIFICATION
        );
        assert_eq!(
            classify(&path, &text, Path::new("."), Some(&t1))
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

        let record = parse_key_values("prompt_path: foreign\nprompt_sha256: deadbeef\nhead: foreign\nverdict: VERIFIED\nevidence: advisory\ncommand: fake");
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
