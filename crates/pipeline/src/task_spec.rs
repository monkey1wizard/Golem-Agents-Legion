//! task_spec: port of `scripts/common/New-TaskSpec.ps1`.
//!
//! Assembles a compact, self-contained task spec from an execution prompt so a
//! secondary headless CLI has just enough context to act on one `T-NN` task
//! without reading the full codebase. The "small-context" upgrade is captured by
//! two behaviors, both ported here:
//! - multi-line task block extraction (the whole `T-NN` bullet incl. indented
//!   sub-bullets, up to the next top-level task), and
//! - per-task affected-file convergence (backtick-quoted paths named inside the
//!   task block, falling back to the full `## Files to Create or Modify` list).
//!
//! Extraction is pure (operates on the prompt text); git/clock/routing context is
//! passed in by the caller so the assembly stays deterministic and testable.

use std::collections::HashSet;

use crate::PipelineError;
use thiserror::Error;

#[path = "probe_evidence.rs"]
pub mod probe_evidence;

pub use probe_evidence::*;

/// 5 KB soft target for a task spec (legacy warns past this).
pub const SPEC_SIZE_TARGET_BYTES: usize = 5 * 1024;

// ── Markdown section / task-block extraction ────────────────────────────────────

fn is_h2_header(line: &str, header: &str) -> bool {
    line.trim_end()
        .strip_prefix("## ")
        .map(|rest| rest.trim() == header)
        .unwrap_or(false)
}

/// A level-2 boundary: `##` followed by whitespace (matches legacy `^##\s+`).
/// `### ...` is not a boundary because `##` is followed by `#`, not whitespace.
fn is_h2_boundary(line: &str) -> bool {
    line.strip_prefix("##")
        .map(|rest| rest.starts_with(char::is_whitespace))
        .unwrap_or(false)
}

fn section_lines<'a>(lines: &[&'a str], header: &str) -> Vec<&'a str> {
    let mut in_section = false;
    let mut out = Vec::new();
    for &line in lines {
        if !in_section {
            if is_h2_header(line, header) {
                in_section = true;
            }
            continue;
        }
        if is_h2_boundary(line) {
            break;
        }
        out.push(line);
    }
    out
}

/// Strip a markdown task-checkbox bullet prefix (`- [ ]` / `- [x]` / `- []`),
/// returning the remainder after the trailing whitespace. Mirrors legacy
/// `^\s*-\s*\[.?\]\s*`.
fn strip_checkbox_bullet(line: &str) -> Option<&str> {
    let s = line.trim_start().strip_prefix('-')?.trim_start();
    let s = s.strip_prefix('[')?;
    let s = if let Some(rest) = s.strip_prefix(']') {
        rest
    } else {
        let inner = s.chars().next()?;
        s[inner.len_utf8()..].strip_prefix(']')?
    };
    Some(s.trim_start())
}

/// Is this the opening bullet of `task_id`? Mirrors legacy task-start pattern
/// `^\s*-\s*\[.?\]\s*(?:\*\*)?<id>(?:\*\*)?(?:\b|\s|\()`.
fn is_task_start(line: &str, task_id: &str) -> bool {
    let rest = match strip_checkbox_bullet(line) {
        Some(r) => r,
        None => return false,
    };
    let rest = rest.strip_prefix("**").unwrap_or(rest);
    let after = match rest.strip_prefix(task_id) {
        Some(a) => a,
        None => return false,
    };
    let after = after.strip_prefix("**").unwrap_or(after);
    // \b word boundary: next char must not continue a word ([A-Za-z0-9_]).
    match after.chars().next() {
        None => true,
        Some(c) => !(c.is_ascii_alphanumeric() || c == '_'),
    }
}

/// Any top-level task bullet `- [.] **?T-<digits>` — ends the current block.
fn is_any_task_bullet(line: &str) -> bool {
    let rest = match strip_checkbox_bullet(line) {
        Some(r) => r,
        None => return false,
    };
    let rest = rest.strip_prefix("**").unwrap_or(rest);
    rest.strip_prefix("T-")
        .and_then(|a| a.chars().next())
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false)
}

/// A Markdown heading at the detail-section depth or deeper.
fn is_task_detail_heading(line: &str) -> bool {
    line.strip_prefix("###")
        .map(|rest| rest.starts_with('#') || rest.chars().next().is_some_and(char::is_whitespace))
        .unwrap_or(false)
}

fn task_block_lines<'a>(prompt_lines: &[&'a str], task_id: &str) -> Vec<&'a str> {
    let section = section_lines(prompt_lines, "Tasks");
    let start = match section.iter().position(|l| is_task_start(l, task_id)) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let mut block = Vec::new();
    for (i, &line) in section.iter().enumerate().skip(start) {
        if i > start && (is_any_task_bullet(line) || is_task_detail_heading(line)) {
            break;
        }
        block.push(line);
    }
    block
}

// ── Affected-file selection ─────────────────────────────────────────────────────

fn fallback_file_lines(prompt_lines: &[&str]) -> Vec<String> {
    section_lines(prompt_lines, "Files to Create or Modify")
        .iter()
        .filter(|l| {
            let t = l.trim_start();
            t.strip_prefix('-')
                .map(|r| r.starts_with(char::is_whitespace))
                .unwrap_or(false)
        })
        .map(|l| l.trim().to_string())
        .collect()
}

/// Known repo-file extensions that make a bare (no `/` or `\`) backtick token
/// count as a root-level path, e.g. `CLAUDE.md`. Kept short and specific to
/// avoid false-positiving on prose (a domain-like `example.com` still fails,
/// since `com` isn't in this list).
const BARE_FILENAME_EXTENSIONS: &[&str] = &[
    "md", "rs", "toml", "json", "ndjson", "yml", "yaml", "lock", "txt",
];

/// A root-level bare filename (no directory separator) with a recognized
/// extension, e.g. `CLAUDE.md` or `Cargo.toml`. Rejects anything containing
/// whitespace or with no dot, so generic backtick-quoted prose terms are
/// never mistaken for a path.
fn looks_like_bare_filename(candidate: &str) -> bool {
    if candidate.contains(char::is_whitespace) {
        return false;
    }
    if let Some(dotfile) = candidate.strip_prefix('.') {
        if !dotfile.is_empty()
            && !dotfile.contains('.')
            && !dotfile.contains(['/', '\\'])
            && !BARE_FILENAME_EXTENSIONS.contains(&dotfile.to_lowercase().as_str())
        {
            return true;
        }
    }
    match candidate.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty() && BARE_FILENAME_EXTENSIONS.contains(&ext.to_lowercase().as_str())
        }
        None => false,
    }
}

/// Backtick-quoted path-like tokens — containing `/` or `\`, or a bare
/// root-level filename with a recognized extension (see
/// [`looks_like_bare_filename`]) — deduped case-insensitively in first-seen
/// order. Only same-line quotes count (no newline between backticks).
fn backtick_paths(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after_open = &rest[open + 1..];
        match after_open.find(['`', '\n', '\r']) {
            Some(rel) => {
                let closing = after_open[rel..].chars().next().unwrap();
                if closing == '`' {
                    let candidate = after_open[..rel].trim();
                    if !candidate.is_empty()
                        && (candidate.contains('/')
                            || candidate.contains('\\')
                            || looks_like_bare_filename(candidate))
                        && seen.insert(candidate.to_lowercase())
                    {
                        out.push(candidate.to_string());
                    }
                    rest = &after_open[rel + 1..];
                } else {
                    rest = &after_open[rel..];
                }
            }
            None => break,
        }
    }
    out
}

fn affected_file_lines(prompt_lines: &[&str], task_block: &[&str]) -> Vec<String> {
    let fallback = fallback_file_lines(prompt_lines);
    if task_block.is_empty() {
        return fallback;
    }
    let paths = backtick_paths(&task_block.join("\n"));
    if paths.is_empty() {
        return fallback;
    }
    paths
        .iter()
        .map(|path| {
            let lc = path.to_lowercase();
            fallback
                .iter()
                .find(|line| line.to_lowercase().contains(&lc))
                .cloned()
                .unwrap_or_else(|| format!("- `{path}`"))
        })
        .collect()
}

// ── Phase → write-back / agent contract ─────────────────────────────────────────

fn unfenced_heading(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('#')
        && trimmed
            .as_bytes()
            .get(1)
            .is_some_and(|b| *b == b'#' || *b == b' ')
        && !trimmed.starts_with("####")
}

fn fenced_lines(text: &str) -> Vec<(usize, usize, &str)> {
    let mut lines = Vec::new();
    let mut start = 0;
    for part in text.split_inclusive('\n') {
        let end = start + part.len();
        lines.push((start, end, part.trim_end_matches(['\n', '\r'])));
        start = end;
    }
    if start < text.len() || text.is_empty() {
        lines.push((start, text.len(), &text[start..]));
    }
    lines
}

fn is_fence(line: &str) -> bool {
    line.trim_start().starts_with("```")
}

fn valid_payload_heading(line: &str, task_id: &str) -> bool {
    let heading = line.trim();
    let prefix = format!("### [{task_id}] ");
    let Some(date) = heading.strip_prefix(&prefix) else {
        return false;
    };
    date.len() == 10
        && date.as_bytes()[4] == b'-'
        && date.as_bytes()[7] == b'-'
        && date
            .bytes()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
}

fn validate_payload(receipt: &str, phase: &str, task_id: &str) -> Result<(), PipelineError> {
    let lines = fenced_lines(receipt);
    let Some((_, _, first)) = lines.first() else {
        return Err(PipelineError::InvalidHeading(task_id.to_string()));
    };
    if !valid_payload_heading(first, task_id) {
        if first.trim_start().starts_with("### [") {
            return Err(PipelineError::TaskMismatch(task_id.to_string()));
        }
        return Err(PipelineError::InvalidHeading(task_id.to_string()));
    }

    let mut in_fence = false;
    for (index, (_, _, line)) in lines.iter().enumerate() {
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if index != 0 && !in_fence && unfenced_heading(line) {
            return Err(PipelineError::UnfencedHeading);
        }
    }
    if in_fence {
        return Err(PipelineError::UnfencedHeading);
    }

    let lower = receipt.to_ascii_lowercase();
    match phase.to_ascii_lowercase().as_str() {
        "test" => {
            if !lower.lines().any(|line| line.contains("verdict:")) {
                return Err(PipelineError::MissingMarker("verdict"));
            }
            if !lower.lines().any(|line| {
                line.contains("evidence:")
                    || line.contains("coverage:")
                    || line.contains("probe_record_b64=")
            }) {
                return Err(PipelineError::MissingMarker("evidence"));
            }
        }
        "audit"
            if !receipt.contains("<!-- AUDIT_REVIEW: CLEAR -->")
                && !receipt.contains("<!-- AUDIT_REVIEW: FINDINGS-OPEN -->") =>
        {
            return Err(PipelineError::MissingMarker("AUDIT_REVIEW"));
        }
        _ => {}
    }
    Ok(())
}

fn destination_section(
    prompt: &str,
    header: &'static str,
) -> Result<(usize, usize), PipelineError> {
    let lines = fenced_lines(prompt);
    let mut in_fence = false;
    let mut matches = Vec::new();
    for (start, end, line) in &lines {
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence && is_h2_header(line, header) {
            matches.push((*start, *end));
        }
    }
    match matches.as_slice() {
        [] => Err(PipelineError::MissingSection(header)),
        [_] => {
            let (_, header_end) = matches[0];
            let mut section_end = prompt.len();
            in_fence = false;
            for (start, _, line) in &lines {
                if *start < header_end {
                    if is_fence(line) {
                        in_fence = !in_fence;
                    }
                    continue;
                }
                if is_fence(line) {
                    in_fence = !in_fence;
                } else if !in_fence && is_h2_boundary(line) {
                    section_end = *start;
                    break;
                }
            }
            Ok((header_end, section_end))
        }
        _ => Err(PipelineError::DuplicateSection(header)),
    }
}

/// Validate and append one complete phase payload to its control-node section.
/// This function performs no I/O and does not mutate the input on failure.
pub fn render_phase_writeback(
    prompt_bytes: &[u8],
    receipt_bytes: &[u8],
    phase: &str,
    task_id: &str,
) -> Result<Vec<u8>, PipelineError> {
    let prompt = std::str::from_utf8(prompt_bytes)
        .map_err(|error| PipelineError::PromptUtf8(error.to_string()))?;
    let receipt = std::str::from_utf8(receipt_bytes)
        .map_err(|error| PipelineError::ReceiptUtf8(error.to_string()))?;
    validate_payload(receipt, phase, task_id)?;
    let header = match phase.to_ascii_lowercase().as_str() {
        "test" => "Test Results",
        "audit" => "Review Results",
        _ => {
            return Err(PipelineError::MissingSection(
                "Test Results or Review Results",
            ))
        }
    };
    let (section_start, section_end) = destination_section(prompt, header)?;
    let section = &prompt[section_start..section_end];
    if let Some(last) = section.lines().rev().find(|line| !line.trim().is_empty()) {
        if last.trim_start().starts_with('|') && !last.trim_end().ends_with('|') {
            return Err(PipelineError::MalformedTableBoundary);
        }
    }
    let mut rendered = Vec::with_capacity(prompt.len() + receipt.len() + 2);
    rendered.extend_from_slice(&prompt.as_bytes()[..section_end]);
    if !rendered.ends_with(b"\n\n") {
        if rendered.ends_with(b"\n") {
            rendered.push(b'\n');
        } else {
            rendered.extend_from_slice(b"\n\n");
        }
    }
    rendered.extend_from_slice(receipt.as_bytes());
    if !rendered.ends_with(b"\n") {
        rendered.push(b'\n');
    }
    rendered.extend_from_slice(&prompt.as_bytes()[section_end..]);
    Ok(rendered)
}

fn write_back_instruction(
    phase: &str,
    prompt_path: &str,
    receipt_path: Option<&str>,
    is_test_first: bool,
) -> String {
    let phase_lc = phase.to_lowercase();
    let in_scope = is_writeback_in_scope(phase, is_test_first);
    if in_scope {
        if let Some(receipt) = receipt_path {
            return format!(
                "Write the complete task-scoped Markdown subsection to the receipt file: {receipt}\n\
                 Do NOT edit the execution prompt directly.\n\n\
                 MANDATORY — you MUST write the complete task-scoped Markdown subsection to the receipt file: {receipt}\n\
                 This is the completion signal. The pipeline accepts this phase ONLY if that file exists and is non-empty. \
                 If you do not write it, this phase is recorded as FAILED (no-receipt) no matter what else you did. \
                 Write the receipt file before you finish."
            );
        }
    }

    let base = match phase_lc.as_str() {
        "test" if !is_test_first => {
            format!("Write test results to `## Test Results` in the file: {prompt_path}")
        }
        "test" if is_test_first => "Write test results.".to_string(),
        "audit" => format!("Write audit results to `## Review Results` in the file: {prompt_path}"),
        _ => "Write implementation code changes to the files listed in 'Affected Files'. \
              Do NOT modify any other files."
            .to_string(),
    };

    // The dispatch receipt is a liveness signal for all three dispatched phases (implement, test, audit).
    match (phase_lc.as_str(), receipt_path) {
        ("test", Some(receipt)) if is_test_first => format!(
            "MANDATORY — you MUST write a one-line verdict plus a short evidence summary to the receipt file: {receipt}\n\
             This is the completion signal. The pipeline accepts this phase ONLY if that file exists and is non-empty. \
             If you do not write it, this phase is recorded as FAILED (no-receipt) no matter what else you did. \
             Write the receipt file before you finish."
        ),
        ("test" | "audit" | "implement", Some(receipt)) => format!(
            "{base}\n\n\
             MANDATORY — you MUST write a one-line verdict plus a short evidence summary to the receipt file: {receipt}\n\
             This is the completion signal. The pipeline accepts this phase ONLY if that file exists and is non-empty. \
             If you do not write it, this phase is recorded as FAILED (no-receipt) no matter what else you did. \
             Write the receipt file before you finish."
        ),
        _ => base,
    }
}

/// Repo-relative agent contract path for a phase.
pub fn agent_contract_rel(phase: &str) -> &'static str {
    match phase.to_lowercase().as_str() {
        "test" => "plugins/gal-core/agents/golem-tester.agent.md",
        "audit" => "plugins/gal-core/agents/golem-auditor.agent.md",
        _ => "plugins/gal-core/agents/golem-implementer.agent.md",
    }
}

// ── Public extraction API ───────────────────────────────────────────────────────

/// The full multi-line task goal block for `task_id`, trimmed.
/// `Err(TaskNotFound)` when the task bullet is absent from `## Tasks`.
pub fn extract_task_goal(prompt_body: &str, task_id: &str) -> Result<String, PipelineError> {
    let lines: Vec<&str> = prompt_body.lines().collect();
    let block = task_block_lines(&lines, task_id);
    if block.is_empty() {
        return Err(PipelineError::TaskNotFound(task_id.to_string()));
    }
    Ok(block.join("\n").trim().to_string())
}

/// Per-task affected files (falls back to the full Files section when the task
/// block names no paths).
pub fn extract_affected_files(prompt_body: &str, task_id: &str) -> Vec<String> {
    let lines: Vec<&str> = prompt_body.lines().collect();
    let block = task_block_lines(&lines, task_id);
    affected_file_lines(&lines, &block)
}

/// Bare path tokens for a task's affected files, derived by extracting the
/// backtick paths over the SAME display-line set [`extract_affected_files`]
/// returns. Deriving from that identical set guarantees, by construction, that
/// a boundary-check allowlist built from this function equals the allowlist the
/// dispatch spec shows the executor — closing the parser-drift seam.
///
/// Returns an empty `Vec` when neither the task block nor the
/// `## Files to Create or Modify` section names any path (the caller maps that
/// to a "no allowlist" / `NotRun` decision — never allow-all).
pub fn extract_affected_file_paths(prompt_body: &str, task_id: &str) -> Vec<String> {
    let lines = extract_affected_files(prompt_body, task_id);
    backtick_paths(&lines.join("\n"))
}

/// Strict per-task declared paths for the `declared-vs-touched` lint only.
/// Unlike [`extract_affected_file_paths`], this reads only the task block;
/// `extract_affected_file_paths` remains the boundary-check allowlist source.
pub fn extract_declared_file_paths_strict(prompt_body: &str, task_id: &str) -> Vec<String> {
    let lines: Vec<&str> = prompt_body.lines().collect();
    let block = task_block_lines(&lines, task_id);
    backtick_paths(&block.join("\n"))
}

// ── Retry-handoff extraction ───────────────────────────────────────────────────

/// A unique, still-open remediation instruction owned by the orchestrator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenRetryHandoff {
    pub heading: String,
    pub origin_phase: String,
    pub problem: String,
    pub next_human_step: String,
}

/// The exact stable authority fields used to detect a replayed fix attempt.
/// Volatile spec metadata is deliberately absent from this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixAuthorityInputs {
    pub task_id: String,
    pub executor_phase: String,
    pub handoff_heading: String,
    pub task_goal: String,
    pub affected_files: Vec<String>,
    pub problem: String,
    pub next_human_step: String,
    pub agent_contract: String,
}

/// Typed failure shape for a fix-mode handoff. The CLI adds prompt-path context
/// before reporting these errors to an operator.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RetryHandoffError {
    #[error("no OPEN Retry Handoff found for task '{task_id}'")]
    NotFound { task_id: String },
    #[error("multiple OPEN Retry Handoffs found for task '{task_id}': {headings:?}")]
    Ambiguous {
        task_id: String,
        headings: Vec<String>,
    },
    #[error("malformed Retry Handoff '{heading}' for task '{task_id}': {reason}")]
    Malformed {
        task_id: String,
        heading: String,
        reason: String,
    },
}

fn retry_handoff_heading(line: &str, task_id: &str) -> Option<(String, String)> {
    let heading = line.trim();
    let rest = heading.strip_prefix("#### Retry Handoff — ")?;
    let (id, phase) = rest.rsplit_once(" / ")?;
    if id != task_id {
        return None;
    }
    let phase = phase.trim();
    if !matches!(phase, "IMPLEMENT" | "TEST" | "AUDIT") {
        return None;
    }
    Some((heading.to_string(), phase.to_string()))
}

fn handoff_field(lines: &[&str], label: &str) -> Option<String> {
    let prefix = format!("- {label}:");
    let start = lines
        .iter()
        .position(|line| line.trim_start().starts_with(&prefix))?;
    let mut value = lines[start].trim_start()[prefix.len()..].trim().to_string();
    for line in &lines[start + 1..] {
        let trimmed = line.trim();
        if trimmed.starts_with("- ") && trimmed.contains(':') {
            break;
        }
        if !value.is_empty() {
            value.push('\n');
        }
        value.push_str(trimmed);
    }
    (!value.trim().is_empty()).then_some(value.trim().to_string())
}

/// Extract the task's sole OPEN `#### Retry Handoff — <task> / <PHASE>` block
/// from `## Status > ### Handoff Notes`. Resolved blocks are intentionally
/// ignored; malformed matching blocks fail closed instead of being guessed at.
pub fn extract_open_retry_handoff(
    prompt_body: &str,
    task_id: &str,
) -> Result<OpenRetryHandoff, RetryHandoffError> {
    let lines: Vec<&str> = prompt_body.lines().collect();
    let status = section_lines(&lines, "Status");
    let notes_start = status
        .iter()
        .position(|line| line.trim() == "### Handoff Notes")
        .ok_or_else(|| RetryHandoffError::NotFound {
            task_id: task_id.to_string(),
        })?;
    let notes = &status[notes_start + 1..];
    let mut open = Vec::new();

    for index in 0..notes.len() {
        let raw_heading = notes[index].trim();
        if let Some(rest) = raw_heading.strip_prefix("#### Retry Handoff — ") {
            if let Some((id, phase)) = rest.rsplit_once(" / ") {
                if id == task_id && !matches!(phase.trim(), "IMPLEMENT" | "TEST" | "AUDIT") {
                    return Err(RetryHandoffError::Malformed {
                        task_id: task_id.to_string(),
                        heading: raw_heading.to_string(),
                        reason: "origin phase must be IMPLEMENT, TEST, or AUDIT".to_string(),
                    });
                }
            } else if rest == task_id || rest.starts_with(&format!("{task_id} ")) {
                return Err(RetryHandoffError::Malformed {
                    task_id: task_id.to_string(),
                    heading: raw_heading.to_string(),
                    reason: "heading must end with ' / IMPLEMENT', ' / TEST', or ' / AUDIT'"
                        .to_string(),
                });
            }
        }
        let Some((heading, origin_phase)) = retry_handoff_heading(notes[index], task_id) else {
            continue;
        };
        let end = notes[index + 1..]
            .iter()
            .position(|line| line.trim_start().starts_with("#### "))
            .map(|offset| index + 1 + offset)
            .unwrap_or(notes.len());
        let block = &notes[index + 1..end];
        let status =
            handoff_field(block, "Status").ok_or_else(|| RetryHandoffError::Malformed {
                task_id: task_id.to_string(),
                heading: heading.clone(),
                reason: "missing Status field".to_string(),
            })?;
        if status == "RESOLVED" {
            continue;
        }
        if status != "OPEN" {
            return Err(RetryHandoffError::Malformed {
                task_id: task_id.to_string(),
                heading,
                reason: "Status must be OPEN or RESOLVED".to_string(),
            });
        }
        let problem =
            handoff_field(block, "Problem").ok_or_else(|| RetryHandoffError::Malformed {
                task_id: task_id.to_string(),
                heading: heading.clone(),
                reason: "missing Problem field".to_string(),
            })?;
        let next_human_step = handoff_field(block, "Next human step").ok_or_else(|| {
            RetryHandoffError::Malformed {
                task_id: task_id.to_string(),
                heading: heading.clone(),
                reason: "missing Next human step field".to_string(),
            }
        })?;
        open.push(OpenRetryHandoff {
            heading,
            origin_phase,
            problem,
            next_human_step,
        });
    }

    match open.len() {
        0 => Err(RetryHandoffError::NotFound {
            task_id: task_id.to_string(),
        }),
        1 => Ok(open.remove(0)),
        _ => Err(RetryHandoffError::Ambiguous {
            task_id: task_id.to_string(),
            headings: open.into_iter().map(|handoff| handoff.heading).collect(),
        }),
    }
}

/// Extract only the stable authority fields that may distinguish one fix
/// attempt from another.
pub fn extract_fix_authority_inputs(
    prompt_body: &str,
    task_id: &str,
    executor_phase: &str,
    agent_contract: &str,
) -> Result<FixAuthorityInputs, PipelineError> {
    let handoff = extract_open_retry_handoff(prompt_body, task_id)
        .map_err(|error| PipelineError::Dispatch(error.to_string()))?;
    Ok(FixAuthorityInputs {
        task_id: task_id.to_string(),
        executor_phase: executor_phase.to_string(),
        handoff_heading: handoff.heading,
        task_goal: extract_task_goal(prompt_body, task_id)?,
        affected_files: extract_affected_file_paths(prompt_body, task_id),
        problem: handoff.problem,
        next_human_step: handoff.next_human_step,
        agent_contract: agent_contract.to_string(),
    })
}

/// Extract the plan slug from a prompt path (e.g. `.dev/plans/<slug>.prompt.md` -> `<slug>`).
pub fn extract_plan_slug(prompt_path: &str) -> String {
    let filename = prompt_path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(prompt_path);
    let slug = filename
        .strip_suffix(".prompt.md")
        .or_else(|| filename.strip_suffix(".md"))
        .unwrap_or(filename);
    slug.to_string()
}

/// A record from the `### Test-First Generations` table under `## Status`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GenerationRecord {
    pub task_id: String,
    pub generation: u32,
    pub contract_digest: String,
    pub reason: String,
}

/// Extracts all records from `## Status > ### Test-First Generations` table in `prompt_body`.
pub fn extract_generation_ledger(prompt_body: &str) -> Vec<GenerationRecord> {
    let lines: Vec<&str> = prompt_body.lines().collect();
    let status = section_lines(&lines, "Status");
    let gen_start = match status
        .iter()
        .position(|l| l.trim() == "### Test-First Generations")
    {
        Some(pos) => pos,
        None => return Vec::new(),
    };

    let mut records = Vec::new();
    for line in &status[gen_start + 1..] {
        let trimmed = line.trim();
        if trimmed.starts_with("### ") || is_h2_boundary(line) {
            break;
        }
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let cols: Vec<&str> = trimmed.split('|').map(str::trim).collect();
            if cols.len() >= 5 {
                let task_id = cols[1];
                if let Ok(gen_val) = cols[2].parse::<u32>() {
                    records.push(GenerationRecord {
                        task_id: task_id.to_string(),
                        generation: gen_val,
                        contract_digest: cols[3].to_string(),
                        reason: cols[4].to_string(),
                    });
                }
            }
        }
    }
    records
}

/// Extract the latest generation for `task_id` from `prompt_body`.
pub fn extract_latest_generation(prompt_body: &str, task_id: &str) -> Option<u32> {
    extract_generation_ledger(prompt_body)
        .into_iter()
        .filter(|r| r.task_id == task_id)
        .map(|r| r.generation)
        .max()
}

/// Plan-scoped receipt path helper for test-first evidence:
/// `.dev/pipeline/receipts/<plan-slug>/<task_id>/g<generation>-c<contract_digest>/probe-<expectation>.receipt.md`
pub fn test_first_receipt_rel(
    plan_slug: &str,
    task_id: &str,
    generation: u32,
    contract_digest: &str,
    expectation: &str,
) -> String {
    format!(
        ".dev/pipeline/receipts/{plan_slug}/{task_id}/g{generation}-c{contract_digest}/probe-{expectation}.receipt.md"
    )
}

/// Plan-scoped snapshot path helper for test-first path capture:
/// `.dev/pipeline/snapshots/<plan-slug>/<task_id>/g<generation>-c<contract_digest>/<kind>.snapshot.tsv`
pub fn test_first_snapshot_rel(
    plan_slug: &str,
    task_id: &str,
    generation: u32,
    contract_digest: &str,
    kind: &str,
) -> String {
    format!(
        ".dev/pipeline/snapshots/{plan_slug}/{task_id}/g{generation}-c{contract_digest}/{kind}.snapshot.tsv"
    )
}

fn map_paths_to_display_lines(paths: &[String], prompt_lines: &[&str]) -> Vec<String> {
    let fallback = fallback_file_lines(prompt_lines);
    paths
        .iter()
        .map(|path| {
            let lc = path.to_lowercase();
            fallback
                .iter()
                .find(|line| line.to_lowercase().contains(&lc))
                .cloned()
                .unwrap_or_else(|| format!("- `{path}`"))
        })
        .collect()
}

/// Per-phase affected files for a task (display formatted bullet lines).
/// Respects `test-first-v1` phase allowlists for `scaffold`, `test`, `implement`, and `audit`.
/// Markerless prompts fall back to standard `extract_affected_files`.
pub fn extract_phase_affected_files(
    prompt_body: &str,
    plan_slug: &str,
    task_id: &str,
    phase: &str,
) -> Vec<String> {
    let lines: Vec<&str> = prompt_body.lines().collect();
    if !has_test_first_marker(prompt_body) {
        return extract_affected_files(prompt_body, task_id);
    }
    match parse_task_contract(prompt_body, plan_slug, task_id) {
        Ok(contract) if contract.applicability == Applicability::Required => {
            match phase.to_lowercase().as_str() {
                "scaffold" | "implement" => {
                    map_paths_to_display_lines(&contract.production_paths, &lines)
                }
                "test" => map_paths_to_display_lines(&contract.test_paths, &lines),
                "audit" => {
                    let mut combined = contract.production_paths.clone();
                    for p in &contract.test_paths {
                        if !combined.contains(p) {
                            combined.push(p.clone());
                        }
                    }
                    map_paths_to_display_lines(&combined, &lines)
                }
                _ => extract_affected_files(prompt_body, task_id),
            }
        }
        Ok(_) => extract_affected_files(prompt_body, task_id),
        Err(_) => Vec::new(),
    }
}

/// Bare path tokens for a task's per-phase affected files.
pub fn extract_phase_affected_file_paths(
    prompt_body: &str,
    plan_slug: &str,
    task_id: &str,
    phase: &str,
) -> Vec<String> {
    if !has_test_first_marker(prompt_body) {
        return extract_affected_file_paths(prompt_body, task_id);
    }
    match parse_task_contract(prompt_body, plan_slug, task_id) {
        Ok(contract) if contract.applicability == Applicability::Required => {
            match phase.to_lowercase().as_str() {
                "scaffold" | "implement" => contract.production_paths,
                "test" => contract.test_paths,
                "audit" => {
                    let mut combined = contract.production_paths.clone();
                    for p in &contract.test_paths {
                        if !combined.contains(p) {
                            combined.push(p.clone());
                        }
                    }
                    combined
                }
                _ => extract_affected_file_paths(prompt_body, task_id),
            }
        }
        Ok(_) => extract_affected_file_paths(prompt_body, task_id),
        Err(_) => Vec::new(),
    }
}

// ── Spec assembly ───────────────────────────────────────────────────────────────

/// Caller-supplied context for [`assemble_task_spec`] (git/clock are inputs so the
/// assembly is pure).
#[derive(Debug, Clone)]
pub struct TaskSpecInput<'a> {
    pub task_scope: &'a str,
    pub phase: &'a str,
    pub prompt_path: &'a str,
    pub prompt_body: &'a str,
    pub generated: &'a str,
    pub git_branch: &'a str,
    pub git_head: &'a str,
    /// Semicolon-separated convention file paths, or `None` to omit the block.
    pub convention_hints: Option<&'a str>,
    /// Deterministic receipt file the executor must write its phase summary to,
    /// or `None` to omit the receipt clause. The dispatch receipt is a liveness
    /// signal for all three dispatched phases (implement, test, audit).
    pub receipt_path: Option<&'a str>,
    /// Caller-read agent contract body (the exact bytes of the resolved
    /// `agent_contract_rel(phase)` file), rendered verbatim under
    /// `## Agent Contract` when present. `None` keeps the legacy control-node
    /// file-reference rendering.
    pub agent_contract_body: Option<&'a str>,
    /// A retry round injects the orchestrator's unique OPEN handoff into the
    /// task goal. `false` preserves the existing spec byte-for-byte.
    pub fix_mode: bool,
}

/// The assembled spec plus the transient output filename and size signal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskSpec {
    pub markdown: String,
    pub out_file_name: String,
    pub byte_len: usize,
    pub exceeds_size_target: bool,
}

/// Assemble the task spec markdown for one task+phase. Mirrors the legacy
/// `New-TaskSpec.ps1` template.
pub fn assemble_task_spec(input: &TaskSpecInput<'_>) -> Result<TaskSpec, PipelineError> {
    let mut task_goal = extract_task_goal(input.prompt_body, input.task_scope)?;
    if input.fix_mode {
        let handoff = extract_open_retry_handoff(input.prompt_body, input.task_scope)
            .map_err(|error| PipelineError::Dispatch(error.to_string()))?;
        task_goal.push_str("\n\n### Fix Round — Binding Acceptance\n\n");
        task_goal.push_str(&format!(
            "This is a binding remediation acceptance from the OPEN {} handoff.\n\n- Problem: {}\n- Next human step: {}",
            handoff.origin_phase, handoff.problem, handoff.next_human_step
        ));
    }

    let plan_slug = extract_plan_slug(input.prompt_path);
    let is_test_first = has_test_first_marker(input.prompt_body);

    let contract_opt = if is_test_first {
        parse_task_contract(input.prompt_body, &plan_slug, input.task_scope).ok()
    } else {
        None
    };

    let generation = if is_test_first {
        extract_latest_generation(input.prompt_body, input.task_scope).unwrap_or(1)
    } else {
        1
    };

    let affected = if is_test_first {
        extract_phase_affected_files(input.prompt_body, &plan_slug, input.task_scope, input.phase)
    } else {
        extract_affected_files(input.prompt_body, input.task_scope)
    };

    let affected_block = if affected.is_empty() {
        "(none parsed from the task) — modify ONLY the file(s) named in the Task Goal above. \
         Do NOT create or edit any other file."
            .to_string()
    } else {
        format!(
            "You may modify ONLY the files listed below. Editing, creating, or deleting any \
             other file is an out-of-scope boundary violation and will be rejected:\n\n{}",
            affected.join("\n")
        )
    };

    let effective_receipt = input.receipt_path.map(ToString::to_string).or_else(|| {
        if is_test_first {
            contract_opt.as_ref().and_then(|c| {
                let expectation = match (c.applicability, input.phase.to_lowercase().as_str()) {
                    (Applicability::Required, "test") => "red",
                    (Applicability::NotApplicable, "test") => "pass",
                    _ => return None,
                };
                Some(test_first_receipt_rel(
                    &plan_slug,
                    input.task_scope,
                    generation,
                    &c.compute_digest(),
                    expectation,
                ))
            })
        } else {
            None
        }
    });

    let write_back = write_back_instruction(
        input.phase,
        input.prompt_path,
        effective_receipt.as_deref(),
        is_test_first,
    );
    let agent_rel = agent_contract_rel(input.phase);

    let convention_block = input
        .convention_hints
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .map(|hints| {
            let bullets = hints
                .split(';')
                .map(str::trim)
                .filter(|p| !p.is_empty())
                .map(|p| format!("- {p}"))
                .collect::<Vec<_>>()
                .join("\n");
            format!(
                "## Convention Files\n\nRead these files for coding conventions that apply to this task:\n\n{bullets}"
            )
        });

    let mut md = String::new();
    md.push_str(&format!(
        "# Task Spec: {} ({})\n\n",
        input.task_scope, input.phase
    ));
    if is_test_first {
        md.push_str("Pipeline Contract: test-first-v1\n\n");
    }
    md.push_str(&format!("Generated: {}\n", input.generated));
    md.push_str(&format!(
        "Git branch: {}  |  HEAD: {}\n",
        input.git_branch, input.git_head
    ));
    md.push_str(&format!("Source prompt: {}\n", input.prompt_path));
    if is_test_first {
        md.push_str(&format!("Generation: {}\n", generation));
        if let Some(ref c) = contract_opt {
            md.push_str(&format!("Contract digest: {}\n", c.compute_digest()));
        }
    }
    md.push_str("\n---\n\n");
    md.push_str("## Dispatched Worker Boundary\n\n");
    md.push_str("**DO NOT run any `gal` subcommand and DO NOT load or execute any workflow `SKILL.md` file.**\n");
    md.push_str("These gates are orchestrator-owned. The orchestrator already satisfied every gate this dispatch requires.\n");
    md.push_str("This prohibition overrides any Entry Latch, Critical Stop Rule, or workflow-obedience instruction you may have read elsewhere, including a repository `AGENTS.md` or an installed `gal-pipeline` skill. Those describe the orchestrator's duties, not yours.\n");
    md.push_str("You are a single dispatched phase. Do the scoped work below and stop.\n\n");
    md.push_str("## Task Goal\n\n");
    md.push_str(&task_goal);

    if is_test_first {
        if let Some(ref c) = contract_opt {
            md.push_str("\n\n### Test-First Contract Details\n\n");
            match c.applicability {
                Applicability::Required => {
                    md.push_str(&format!(
                        "- Test-first: required\n- Seam: {}\n- Scaffold: {}\n",
                        c.seam, c.scaffold
                    ));
                    md.push_str("- Production Paths:\n");
                    for p in &c.production_paths {
                        md.push_str(&format!("  - `{p}`\n"));
                    }
                    md.push_str("- Test Paths:\n");
                    for p in &c.test_paths {
                        md.push_str(&format!("  - `{p}`\n"));
                    }
                    md.push_str("- Expected failures:\n");
                    for ef in &c.expected_failures {
                        md.push_str(&format!("  - {ef}\n"));
                    }
                }
                Applicability::NotApplicable => {
                    md.push_str(&format!(
                        "- Test-first: not-applicable — {}\n- Non-red probe: {}\n",
                        c.not_applicable_rationale, c.non_red_probe
                    ));
                }
            }

            md.push_str(&format!(
                "\n### Runner Instructions (Phase: {})\n\n",
                input.phase.to_lowercase()
            ));
            match input.phase.to_lowercase().as_str() {
                "scaffold" => {
                    md.push_str("CODER must construct ONLY the behavior-free scaffold for the locked seam on Production Paths. Contain zero domain or behavioral logic. Test paths are frozen at the locked seam (CODER must not add, remove, or modify test items at the locked seam, including when those items live in a file that is also a production path).\n");
                }
                "test" => {
                    if c.applicability == Applicability::Required {
                        md.push_str("TESTER must author acceptance probes in Test Paths and collect expected red evidence. Production paths are frozen at the locked seam (TESTER must not add, remove, or modify production code at the locked seam, including when test items live in a file that is also a production path).\n");
                    } else {
                        md.push_str("TESTER must run the non-red probe and record pass evidence. Production paths are frozen at the locked seam (TESTER must not add, remove, or modify production code at the locked seam, including when test items live in a file that is also a production path).\n");
                    }
                }
                "implement" => {
                    md.push_str("CODER must write production code in Production Paths to satisfy exact green duty for expected red probes. Test paths are frozen at the locked seam (CODER must not add, remove, or modify test items at the locked seam, including when those items live in a file that is also a production path).\n");
                }
                "audit" => {
                    md.push_str("AUDITOR must review the bounded dirty tree (Production Paths and Test Paths) and write review findings.\n");
                }
                _ => {}
            }
        }
    }

    md.push_str("\n\n## Affected Files (allowlist — modify ONLY these)\n\n");
    md.push_str(&affected_block);
    md.push_str("\n\n## Write-Back Instruction\n\n");
    md.push_str(&write_back);
    md.push_str("\n\n");
    if let Some(ref cb) = convention_block {
        md.push_str(cb);
        md.push_str("\n\n");
    }
    match input.agent_contract_body {
        Some(body) => {
            md.push_str("## Agent Contract\n\n");
            md.push_str(body.trim_end());
            md.push_str("\n\n");
        }
        None => {
            md.push_str(&format!(
                "## Agent Contract\n\nFollow the instructions in: `{agent_rel}`\n\n"
            ));
        }
    }
    md.push_str("## IMPORTANT: Commit Boundary\n\n");
    md.push_str("**DO NOT run `git commit` or `git push`.**\n");
    md.push_str("The orchestrator holds the commit boundary.\n");
    md.push_str("Write changes to files only; the calling pipeline will commit after verifying write-back.\n\n");
    md.push_str("---\n*This spec is transient — discard after use.*\n");

    let byte_len = md.len();
    Ok(TaskSpec {
        out_file_name: format!("{}-{}.md", input.task_scope, input.phase.to_lowercase()),
        exceeds_size_target: byte_len > SPEC_SIZE_TARGET_BYTES,
        byte_len,
        markdown: md,
    })
}

// ── Test-First Contract & Canonical Digest (R4) ───────────────────────────────

pub const TEST_FIRST_MARKER: &str = "Pipeline Contract: test-first-v1";

/// Checks if `prompt_body` contains the exact standalone marker line
/// `Pipeline Contract: test-first-v1` in the region after the H1 title
/// (`# ...`) and before the first H2 heading (`## ...`), ignoring code blocks.
pub fn has_test_first_marker(prompt_body: &str) -> bool {
    let mut in_code_fence = false;
    for line in prompt_body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            continue;
        }
        if in_code_fence {
            continue;
        }
        if line
            .strip_prefix("##")
            .is_some_and(|r| r.starts_with(char::is_whitespace))
        {
            break;
        }
        if line == TEST_FIRST_MARKER {
            return true;
        }
    }
    false
}

/// Returns whether the control node owns semantic placement for a dispatched
/// phase. Audit payloads are staged for every prompt; legacy markerless test
/// payloads are staged as well. Marked test phases retain probe-receipt-only
/// behavior.
pub fn is_writeback_in_scope(phase: &str, is_test_first: bool) -> bool {
    match phase.to_ascii_lowercase().as_str() {
        "audit" => true,
        "test" => !is_test_first,
        _ => false,
    }
}

/// Length-prefixed binary framing: `u64be(len) || bytes`.
pub fn lp(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(8 + data.len());
    out.extend_from_slice(&(data.len() as u64).to_be_bytes());
    out.extend_from_slice(data);
    out
}

/// List binary framing: `u64be(count) || LP(item_1) || ... || LP(item_N)`.
pub fn list<T: AsRef<[u8]>>(items: &[T]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&(items.len() as u64).to_be_bytes());
    for item in items {
        out.extend_from_slice(&lp(item.as_ref()));
    }
    out
}

/// Converts `\r\n` to `\n` and applies Unicode NFC normalization.
pub fn normalize_canonical_text(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let lf = text.replace("\r\n", "\n");
    lf.nfc().collect::<String>()
}

/// Encode raw bytes to unpadded base64url string (RFC 4648 URL safe, no padding).
pub fn encode_base64url_unpadded(data: &[u8]) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.encode(data)
}

/// Decode an unpadded base64url string to raw bytes. Fails if invalid or padded.
pub fn decode_base64url_unpadded(encoded: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    URL_SAFE_NO_PAD.decode(encoded.as_bytes())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Applicability {
    Required,
    NotApplicable,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TaskContractError {
    #[error("task '{0}' not found in prompt")]
    TaskNotFound(String),
    #[error("task '{0}' missing required field: {1}")]
    MissingField(String, &'static str),
    #[error("task '{0}' has invalid path: {1}")]
    InvalidPath(String, String),
    #[error(
        "task '{0}' has invalid scaffold value: '{1}' (expected 'required' or 'not-required')"
    )]
    InvalidScaffold(String, String),
    #[error("missing contract section: {0}")]
    MissingSection(String),
    #[error("duplicate contract section: {0}")]
    DuplicateSection(String),
}

/// Required contract region section headers for contract-region digest.
pub const REQUIRED_CONTRACT_SECTIONS: &[&str] = &["Goal", "Requirements", "Tasks", "Test Plan"];

/// Computes the SHA-256 digest of the four locked contract regions (Goal, Requirements,
/// Tasks, and Test Plan) in `prompt_body`, while excluding mutable execution state (Status,
/// Handoff Notes, Test Results). Rejects prompts with missing or duplicate contract sections.
pub fn compute_contract_region_digest(prompt_body: &str) -> Result<String, TaskContractError> {
    let mut in_code_fence = false;
    let mut sections: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
    let mut section_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    let mut current_section: Option<&str> = None;

    for line in prompt_body.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_code_fence = !in_code_fence;
            if let Some(sec) = current_section {
                sections.entry(sec).or_default().push(line);
            }
            continue;
        }
        if !in_code_fence && is_h2_boundary(line) {
            let sec_name = line.strip_prefix("##").unwrap().trim();
            current_section = Some(sec_name);
            *section_counts.entry(sec_name).or_default() += 1;
            sections.entry(sec_name).or_default().push(line);
            continue;
        }
        if let Some(sec) = current_section {
            sections.entry(sec).or_default().push(line);
        }
    }

    for &sec in REQUIRED_CONTRACT_SECTIONS {
        let count = section_counts.get(sec).copied().unwrap_or(0);
        if count == 0 {
            return Err(TaskContractError::MissingSection(sec.to_string()));
        }
        if count > 1 {
            return Err(TaskContractError::DuplicateSection(sec.to_string()));
        }
    }

    let mut out = Vec::new();
    out.extend_from_slice(&lp(b"contract-region-digest-v1"));
    for &sec in REQUIRED_CONTRACT_SECTIONS {
        let sec_lines = sections.get(sec).unwrap();
        let sec_text = sec_lines.join("\n");
        let norm_text = normalize_canonical_text(&sec_text);
        out.extend_from_slice(&lp(norm_text.as_bytes()));
    }

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&out);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Validates that a path is a repository-relative, regular-file-or-missing path
/// using forward slashes with no absolute, dotted (`.` / `..`), glob, or directory markers.
pub fn validate_repo_path(path: &str) -> Result<String, String> {
    let p = path.trim();
    if p.is_empty() {
        return Err("path cannot be empty".to_string());
    }
    if p.contains('\\') {
        return Err(format!("path '{p}' contains backslashes"));
    }
    if p.starts_with('/') || (p.len() >= 2 && p.chars().nth(1) == Some(':')) {
        return Err(format!("path '{p}' is absolute"));
    }
    if p.ends_with('/') {
        return Err(format!("path '{p}' is a directory"));
    }
    if p.contains('*') || p.contains('?') {
        return Err(format!("path '{p}' contains glob characters"));
    }
    for comp in p.split('/') {
        if comp == "." || comp == ".." {
            return Err(format!("path '{p}' contains '.' or '..' components"));
        }
    }
    Ok(p.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TaskContract {
    pub plan_slug: String,
    pub task_id: String,
    pub applicability: Applicability,
    pub seam: String,
    pub expected_failures: Vec<String>,
    pub production_paths: Vec<String>,
    pub test_paths: Vec<String>,
    pub scaffold: String,
    pub not_applicable_rationale: String,
    pub non_red_probe: String,
    pub tp_rows: Vec<String>,
}

impl TaskContract {
    /// Computes the exact R4 binary-canonical bytes sequence:
    /// `sha256(LP("test-first-contract-v1") || LP(plan_slug) || LP(task_id) || LP(applicability) || LP(seam_or_empty) || LIST(expected_failure_lines_in_source_order) || LIST(production_paths_bytewise_sorted) || LIST(test_paths_bytewise_sorted) || LP(scaffold) || LP(not_applicable_rationale_or_empty) || LP(non_red_probe_or_empty) || LIST(full_TP_rows_covering_the_task_in_source_order))`
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut prod_paths = self.production_paths.clone();
        prod_paths.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

        let mut test_paths = self.test_paths.clone();
        test_paths.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));

        let mut out = Vec::new();

        out.extend_from_slice(&lp(b"test-first-contract-v1"));
        out.extend_from_slice(&lp(normalize_canonical_text(&self.plan_slug).as_bytes()));
        out.extend_from_slice(&lp(normalize_canonical_text(&self.task_id).as_bytes()));

        let app_str = match self.applicability {
            Applicability::Required => "required",
            Applicability::NotApplicable => "not-applicable",
        };
        out.extend_from_slice(&lp(app_str.as_bytes()));
        out.extend_from_slice(&lp(normalize_canonical_text(&self.seam).as_bytes()));

        let ef_lps: Vec<Vec<u8>> = self
            .expected_failures
            .iter()
            .map(|s| normalize_canonical_text(s).into_bytes())
            .collect();
        out.extend_from_slice(&list(&ef_lps));

        let prod_lps: Vec<Vec<u8>> = prod_paths
            .iter()
            .map(|s| normalize_canonical_text(s).into_bytes())
            .collect();
        out.extend_from_slice(&list(&prod_lps));

        let test_lps: Vec<Vec<u8>> = test_paths
            .iter()
            .map(|s| normalize_canonical_text(s).into_bytes())
            .collect();
        out.extend_from_slice(&list(&test_lps));

        out.extend_from_slice(&lp(normalize_canonical_text(&self.scaffold).as_bytes()));
        out.extend_from_slice(&lp(normalize_canonical_text(
            &self.not_applicable_rationale,
        )
        .as_bytes()));
        out.extend_from_slice(&lp(normalize_canonical_text(&self.non_red_probe).as_bytes()));

        let tp_lps: Vec<Vec<u8>> = self
            .tp_rows
            .iter()
            .map(|s| normalize_canonical_text(s).into_bytes())
            .collect();
        out.extend_from_slice(&list(&tp_lps));

        out
    }

    /// Computes the lower-hex SHA-256 digest of the binary-canonical bytes.
    pub fn compute_digest(&self) -> String {
        use sha2::{Digest, Sha256};
        let bytes = self.canonical_bytes();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    }
}

/// Extracts and validates the task contract for `task_id` from `prompt_body`.
pub fn parse_task_contract(
    prompt_body: &str,
    plan_slug: &str,
    task_id: &str,
) -> Result<TaskContract, TaskContractError> {
    let lines: Vec<&str> = prompt_body.lines().collect();
    let block = task_block_lines(&lines, task_id);
    if block.is_empty() {
        return Err(TaskContractError::TaskNotFound(task_id.to_string()));
    }

    let block_str = block.join("\n");
    let is_marked = has_test_first_marker(prompt_body);

    let is_req = block_str.contains("Test-first: required");
    let is_na = block_str.contains("Test-first: not-applicable");

    let applicability = if is_req {
        Applicability::Required
    } else if is_na {
        Applicability::NotApplicable
    } else if is_marked {
        return Err(TaskContractError::MissingField(
            task_id.to_string(),
            "Test-first",
        ));
    } else {
        Applicability::NotApplicable
    };

    let mut seam = String::new();
    let mut expected_failures = Vec::new();
    let mut production_paths = Vec::new();
    let mut test_paths = Vec::new();
    let mut scaffold = String::new();
    let mut not_applicable_rationale = String::new();
    let mut non_red_probe = String::new();

    if applicability == Applicability::Required {
        // Parse Seam
        if let Some(line) = block.iter().find(|l| l.contains("Seam:")) {
            let after = line.split_once("Seam:").map(|x| x.1.trim()).unwrap_or("");
            seam = after.to_string();
        } else {
            return Err(TaskContractError::MissingField(task_id.to_string(), "Seam"));
        }

        // Parse Expected failures
        let mut in_ef = false;
        for line in &block {
            let trimmed = line.trim();
            if trimmed.starts_with("- Expected failures:")
                || trimmed.starts_with("Expected failures:")
            {
                in_ef = true;
                let after = trimmed
                    .strip_prefix("- Expected failures:")
                    .or_else(|| trimmed.strip_prefix("Expected failures:"))
                    .unwrap_or("")
                    .trim();
                if !after.is_empty() {
                    expected_failures.push(after.to_string());
                }
                continue;
            }
            if in_ef {
                if trimmed.starts_with("- Production Paths:")
                    || trimmed.starts_with("- Test Paths:")
                    || trimmed.starts_with("- Scaffold:")
                    || trimmed.starts_with("- Seam:")
                    || trimmed.starts_with("- Test-first:")
                {
                    in_ef = false;
                } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
                    let content = trimmed.trim_start_matches(['-', '*', ' ']).trim();
                    if !content.is_empty() {
                        expected_failures.push(content.to_string());
                    }
                } else if !trimmed.is_empty() {
                    expected_failures.push(trimmed.to_string());
                }
            }
        }

        // Parse Production Paths
        let mut in_prod = false;
        for line in &block {
            let trimmed = line.trim();
            if trimmed.starts_with("- Production Paths:")
                || trimmed.starts_with("Production Paths:")
            {
                in_prod = true;
                let paths = backtick_paths(trimmed);
                production_paths.extend(paths);
                continue;
            }
            if in_prod {
                if trimmed.starts_with("- Test Paths:")
                    || trimmed.starts_with("- Scaffold:")
                    || trimmed.starts_with("- Seam:")
                    || trimmed.starts_with("- Test-first:")
                    || trimmed.starts_with("- Expected failures:")
                {
                    in_prod = false;
                } else {
                    let paths = backtick_paths(trimmed);
                    production_paths.extend(paths);
                }
            }
        }

        // Parse Test Paths
        let mut in_test = false;
        for line in &block {
            let trimmed = line.trim();
            if trimmed.starts_with("- Test Paths:") || trimmed.starts_with("Test Paths:") {
                in_test = true;
                let paths = backtick_paths(trimmed);
                test_paths.extend(paths);
                continue;
            }
            if in_test {
                if trimmed.starts_with("- Scaffold:")
                    || trimmed.starts_with("- Production Paths:")
                    || trimmed.starts_with("- Seam:")
                    || trimmed.starts_with("- Test-first:")
                    || trimmed.starts_with("- Expected failures:")
                {
                    in_test = false;
                } else {
                    let paths = backtick_paths(trimmed);
                    test_paths.extend(paths);
                }
            }
        }

        // Parse Scaffold
        if let Some(line) = block.iter().find(|l| l.contains("Scaffold:")) {
            let after = line
                .split_once("Scaffold:")
                .map(|x| x.1.trim())
                .unwrap_or("");
            if after == "required" || after == "not-required" {
                scaffold = after.to_string();
            } else {
                return Err(TaskContractError::InvalidScaffold(
                    task_id.to_string(),
                    after.to_string(),
                ));
            }
        } else {
            return Err(TaskContractError::MissingField(
                task_id.to_string(),
                "Scaffold",
            ));
        }

        // Path validation & disjoint check
        if production_paths.is_empty() {
            return Err(TaskContractError::MissingField(
                task_id.to_string(),
                "Production Paths",
            ));
        }
        if test_paths.is_empty() {
            return Err(TaskContractError::MissingField(
                task_id.to_string(),
                "Test Paths",
            ));
        }

        for p in &production_paths {
            validate_repo_path(p)
                .map_err(|e| TaskContractError::InvalidPath(task_id.to_string(), e))?;
        }
        for p in &test_paths {
            validate_repo_path(p)
                .map_err(|e| TaskContractError::InvalidPath(task_id.to_string(), e))?;
        }
    } else {
        // NotApplicable
        if let Some(line) = block
            .iter()
            .find(|l| l.contains("Test-first: not-applicable"))
        {
            let after = line
                .split_once("Test-first: not-applicable")
                .map(|x| x.1.trim_start_matches([' ', '—', '-']).trim())
                .unwrap_or("");
            not_applicable_rationale = after.to_string();
        }
        if let Some(line) = block.iter().find(|l| l.contains("Non-red probe:")) {
            let after = line
                .split_once("Non-red probe:")
                .map(|x| x.1.trim())
                .unwrap_or("");
            non_red_probe = after.to_string();
        }
    }

    // Extract TP rows from ## Test Plan
    let mut tp_rows = Vec::new();
    let tp_section = section_lines(&lines, "Test Plan");
    for line in tp_section {
        let trimmed = line.trim();
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            let cols: Vec<&str> = trimmed.split('|').collect();
            if cols.iter().any(|c| {
                let cell = c.trim();
                cell == task_id || cell.contains(task_id)
            }) {
                tp_rows.push(trimmed.to_string());
            }
        }
    }

    Ok(TaskContract {
        plan_slug: plan_slug.to_string(),
        task_id: task_id.to_string(),
        applicability,
        seam,
        expected_failures,
        production_paths,
        test_paths,
        scaffold,
        not_applicable_rationale,
        non_red_probe,
        tp_rows,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Task IDs are composed at runtime, never written as `T-NN` literals in
    // source: a `T-NN` literal with no corresponding live plan task is just stale
    // data. The pipeline parses `T-<digit>` bullets, so these composed ids still
    // exercise the real format.
    fn tid(n: u32) -> String {
        format!("T-{:03}", n)
    }

    fn prompt() -> String {
        format!(
            "\
# Plan Prompt

## Files to Create or Modify

- [CREATE] `crates/pipeline/src/lib.rs`: orchestration scaffold.
- [MODIFY] `crates/cli/src/main.rs`: wire `gal pipeline`.
- [MODIFY] `plugins/gal-core/agents/agents.md`: contract surface.

## Tasks

P0
- [x] **{freeze}** — Freeze fixtures.

- [ ] **{orch}** — Port orchestration into `crates/pipeline/src/lib.rs`; parity.
  - sub-detail: compose `dispatch` and do not rebuild stage.
  - also touch `crates/cli/src/main.rs` later.
- [ ] **{spec}** — Port task-spec (`New-TaskSpec`) into `pipeline`.

## Test Plan

| ID | Type |
",
            freeze = tid(1),
            orch = tid(3),
            spec = tid(4),
        )
    }

    #[test]
    fn extracts_multiline_task_block_until_next_task() {
        let goal = extract_task_goal(&prompt(), &tid(3)).unwrap();
        assert!(goal.starts_with(&format!("- [ ] **{}** — Port orchestration", tid(3))));
        // Indented sub-bullets stay inside the block...
        assert!(goal.contains("sub-detail: compose `dispatch`"));
        assert!(goal.contains("also touch `crates/cli/src/main.rs` later."));
        // ...and the next top-level task is excluded.
        assert!(!goal.contains(&tid(4)));
    }

    #[test]
    fn per_task_affected_files_use_backtick_paths_from_block() {
        let files = extract_affected_files(&prompt(), &tid(3));
        // Paths named in the task block, deduped, matched to the Files section line.
        assert_eq!(files.len(), 2);
        assert!(files[0].contains("crates/pipeline/src/lib.rs"));
        assert!(files[1].contains("crates/cli/src/main.rs"));
        // agents.md is in Files section but NOT named by the task block → excluded.
        assert!(files.iter().all(|f| !f.contains("agents.md")));
    }

    #[test]
    fn affected_files_fall_back_to_full_section_when_block_names_none() {
        let prompt = format!(
            "\
## Files to Create or Modify

- `crates/base/src/lib.rs`
- `crates/dispatch/src/lib.rs`

## Tasks

- [ ] **{}** — A task that names no backtick paths at all.
",
            tid(9)
        );
        let files = extract_affected_files(&prompt, &tid(9));
        assert_eq!(files.len(), 2);
        assert!(files[0].contains("crates/base/src/lib.rs"));
        assert!(files[1].contains("crates/dispatch/src/lib.rs"));
    }

    // template-shape prompt (task-block backtick paths + Files section, no
    // `Affected:`, no `## Affected Files`) → non-empty bare-path set.
    #[test]
    fn extract_affected_file_paths_returns_bare_paths_from_task_block() {
        let paths = extract_affected_file_paths(&prompt(), &tid(3));
        assert!(paths.iter().any(|p| p == "crates/pipeline/src/lib.rs"));
        assert!(paths.iter().any(|p| p == "crates/cli/src/main.rs"));
        // bare tokens, not display lines
        assert!(paths
            .iter()
            .all(|p| !p.contains('`') && !p.starts_with('-')));
        // agents.md is in the Files section but NOT named by the task block.
        assert!(paths.iter().all(|p| !p.contains("agents.md")));
    }

    // Regression: `backtick_paths` originally required `/` or `\`, so a
    // root-level generated file like `CLAUDE.md` (no directory separator) was
    // silently dropped from the allowlist -- a task naming only root-level
    // bare filenames would extract 0 of them.
    #[test]
    fn bare_root_level_filenames_with_known_extensions_are_recognized() {
        assert!(looks_like_bare_filename("CLAUDE.md"));
        assert!(looks_like_bare_filename("AGENTS.md"));
        assert!(looks_like_bare_filename("Cargo.toml"));
        assert!(looks_like_bare_filename("structure-map.ndjson"));
    }

    #[test]
    fn bare_tokens_without_a_recognized_extension_are_not_paths() {
        assert!(!looks_like_bare_filename("example.com"));
        assert!(!looks_like_bare_filename("cargo test"));
        assert!(!looks_like_bare_filename("finalize_hygiene"));
        assert!(!looks_like_bare_filename(""));
        assert!(!looks_like_bare_filename(".md"));
    }

    #[test]
    fn backtick_paths_extracts_root_level_bare_filenames() {
        let text = "Targets: `CLAUDE.md`, `AGENTS.md`, `GEMINI.md`, `.github/copilot-instructions.md`, `.agents/rules/gal.md`.";
        let paths = backtick_paths(text);
        assert!(paths.iter().any(|p| p == "CLAUDE.md"));
        assert!(paths.iter().any(|p| p == "AGENTS.md"));
        assert!(paths.iter().any(|p| p == "GEMINI.md"));
        assert!(paths.iter().any(|p| p == ".github/copilot-instructions.md"));
        assert!(paths.iter().any(|p| p == ".agents/rules/gal.md"));
        assert_eq!(paths.len(), 5);
    }

    // task block names no backtick path → fall back to the Files section bare
    // paths.
    #[test]
    fn extract_affected_file_paths_falls_back_to_files_section() {
        let prompt = format!(
            "\
## Files to Create or Modify

- `crates/base/src/lib.rs`
- `crates/dispatch/src/lib.rs`

## Tasks

- [ ] **{}** — A task that names no backtick paths at all.
",
            tid(9)
        );
        let paths = extract_affected_file_paths(&prompt, &tid(9));
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|p| p == "crates/base/src/lib.rs"));
        assert!(paths.iter().any(|p| p == "crates/dispatch/src/lib.rs"));
    }

    // neither task block nor Files section names a path → empty Vec (pipeline
    // half of the "empty allowlist never allow-all" invariant).
    #[test]
    fn extract_affected_file_paths_empty_when_no_paths_anywhere() {
        let prompt = format!("## Tasks\n\n- [ ] **{}** — A task with no paths.\n", tid(9));
        let paths = extract_affected_file_paths(&prompt, &tid(9));
        assert!(paths.is_empty());
    }

    // multi-line task block (indented sub-bullet backtick path) → path is in the
    // set. `prompt()` tid(3) names `crates/cli/src/main.rs` only in an indented
    // sub-bullet.
    #[test]
    fn extract_affected_file_paths_includes_multiline_subbullet_path() {
        let paths = extract_affected_file_paths(&prompt(), &tid(3));
        assert!(
            paths.iter().any(|p| p == "crates/cli/src/main.rs"),
            "sub-bullet backtick path must be captured"
        );
    }

    #[test]
    fn strict_declared_paths_exclude_unrelated_file_list_paths() {
        let prompt = format!(
            "## Files to Create or Modify\n\n- `docs/guide.md` and `crates/cli/src/main.rs`\n\n## Tasks\n\n- [ ] **{}** — Update `docs/guide.md`.\n",
            tid(10)
        );
        assert_eq!(
            extract_declared_file_paths_strict(&prompt, &tid(10)),
            vec!["docs/guide.md"]
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(10)),
            vec!["docs/guide.md", "crates/cli/src/main.rs"]
        );
    }

    #[test]
    fn strict_declared_paths_are_empty_for_pathless_tasks() {
        let prompt = format!(
            "## Files to Create or Modify\n\n- `crates/base/src/lib.rs`\n- `crates/dispatch/src/lib.rs`\n\n## Tasks\n\n- [ ] **{}** — A task with no backtick paths.\n",
            tid(11)
        );
        assert!(extract_declared_file_paths_strict(&prompt, &tid(11)).is_empty());
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(11)),
            vec!["crates/base/src/lib.rs", "crates/dispatch/src/lib.rs"]
        );
    }

    #[test]
    fn strict_declared_paths_preserve_documentation_only_declarations() {
        let prompt = format!(
            "## Files to Create or Modify\n\n- `docs/architecture.md`\n\n## Tasks\n\n- [ ] **{}** — Document the architecture in `docs/architecture.md`.\n",
            tid(12)
        );
        assert_eq!(
            extract_declared_file_paths_strict(&prompt, &tid(12)),
            vec!["docs/architecture.md"]
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(12)),
            vec!["docs/architecture.md"]
        );
    }

    #[test]
    fn strict_declared_paths_stop_before_two_tier_task_details() {
        let prompt = format!(
            "## Tasks\n\n- [ ] **{}** — First task in `crates/first.rs`.\n- [ ] **{}** — Last task in `crates/last.rs`.\n\n### {}\nDetails for the first task in `crates/first-detail.rs`.\n\n### {}\nDetails for the last task in `crates/last-detail.rs`.\n",
            tid(13),
            tid(14),
            tid(13),
            tid(14)
        );
        assert_eq!(
            extract_declared_file_paths_strict(&prompt, &tid(14)),
            vec!["crates/last.rs"]
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(14)),
            vec!["crates/last.rs"]
        );
    }

    #[test]
    fn task_block_keeps_rust_attribute_lines() {
        let prompt = format!(
            "## Tasks\n\n- [ ] **{}** — Update the parser.\n#[allow(dead_code)]\nMore detail in `crates/pipeline/src/task_spec.rs`.\n",
            tid(15)
        );
        assert_eq!(
            extract_declared_file_paths_strict(&prompt, &tid(15)),
            vec!["crates/pipeline/src/task_spec.rs"]
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(15)),
            vec!["crates/pipeline/src/task_spec.rs"]
        );
    }

    #[test]
    fn single_tier_task_blocks_keep_their_previous_boundaries() {
        let prompt = format!(
            "## Tasks\n\n- [ ] **{}** — First task in `crates/first.rs`.\n- [ ] **{}** — Second task in `crates/second.rs`.\n",
            tid(16),
            tid(17)
        );
        assert_eq!(
            extract_declared_file_paths_strict(&prompt, &tid(16)),
            vec!["crates/first.rs"]
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(16)),
            vec!["crates/first.rs"]
        );
        assert_eq!(
            extract_declared_file_paths_strict(&prompt, &tid(17)),
            vec!["crates/second.rs"]
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(17)),
            vec!["crates/second.rs"]
        );
    }

    #[test]
    fn missing_task_is_task_not_found() {
        let err = extract_task_goal(&prompt(), &tid(999)).unwrap_err();
        assert_eq!(err, PipelineError::TaskNotFound(tid(999)));
    }

    #[test]
    fn write_back_instruction_is_phase_specific() {
        assert!(write_back_instruction("test", "P", None, false).contains("## Test Results"));
        assert!(write_back_instruction("audit", "P", None, false).contains("## Review Results"));
        assert!(write_back_instruction("implement", "P", None, false).contains("Affected Files"));
        // Unknown phase (incl. the removed `verify`) falls back to implement.
        assert!(write_back_instruction("verify", "P", None, false).contains("Affected Files"));
        assert!(write_back_instruction("deploy", "P", None, false).contains("Affected Files"));
    }

    #[test]
    fn write_back_instruction_names_receipt_for_test_and_audit() {
        let r = format!(".dev/pipeline/receipts/{}-test.receipt.md", tid(4));
        let r = r.as_str();
        let test_wb = write_back_instruction("test", "P", Some(r), false);
        assert!(
            test_wb.contains(r),
            "test write-back must name the receipt path"
        );
        assert!(
            test_wb.contains("complete task-scoped Markdown subsection"),
            "in-scope test instruction demands subsection"
        );
        let audit_wb = write_back_instruction("audit", "P", Some(r), false);
        assert!(
            audit_wb.contains(r),
            "audit write-back must name the receipt path"
        );
        assert!(
            audit_wb.contains("complete task-scoped Markdown subsection"),
            "in-scope audit instruction demands subsection"
        );
        // The orchestrator's 2f correctness receipt stays the correctness authority while the dispatch receipt covers liveness.
        assert!(write_back_instruction("implement", "P", Some(r), false).contains(r));
    }

    #[test]
    fn write_back_instruction_marked_test_omits_prompt_path_and_keeps_receipt() {
        let r = format!(".dev/pipeline/receipts/{}-test.receipt.md", tid(4));
        // Test marked test dispatches (is_test_first=true, covering both required and not-applicable classifications)
        let marked_wb_some = write_back_instruction("test", "P", Some(&r), true);
        assert!(
            !marked_wb_some.contains("P"),
            "marked test instruction must omit execution prompt path"
        );
        assert!(
            marked_wb_some.contains(&r),
            "marked test instruction must keep resolved receipt path"
        );
        assert!(
            !marked_wb_some.contains("complete task-scoped Markdown subsection"),
            "marked test instruction must not demand a subsection write-back"
        );
        assert!(
            marked_wb_some.contains("one-line verdict plus a short evidence summary"),
            "marked test instruction keeps one-line verdict requirement"
        );

        let marked_wb_none = write_back_instruction("test", "P", None, true);
        assert!(
            !marked_wb_none.contains("P"),
            "marked test instruction without receipt must omit execution prompt path"
        );
        assert_eq!(
            marked_wb_none, "Write test results.",
            "marked test instruction without receipt must match expected base text"
        );
    }

    #[test]
    fn write_back_instruction_in_scope_demands_subsection_not_one_line_verdict() {
        let r = format!(".dev/pipeline/receipts/{}-audit.receipt.md", tid(4));
        let prompt_path = "path/to/exec.prompt.md";

        // Markerless test
        let test_wb = write_back_instruction("test", prompt_path, Some(&r), false);
        assert!(test_wb.contains("complete task-scoped Markdown subsection"));
        assert!(!test_wb.contains("one-line verdict"));
        assert!(!test_wb.contains(prompt_path));

        // Markerless audit
        let audit_wb_less = write_back_instruction("audit", prompt_path, Some(&r), false);
        assert!(audit_wb_less.contains("complete task-scoped Markdown subsection"));
        assert!(!audit_wb_less.contains("one-line verdict"));
        assert!(!audit_wb_less.contains(prompt_path));

        // Marked audit
        let audit_wb_marked = write_back_instruction("audit", prompt_path, Some(&r), true);
        assert!(audit_wb_marked.contains("complete task-scoped Markdown subsection"));
        assert!(!audit_wb_marked.contains("one-line verdict"));
        assert!(!audit_wb_marked.contains(prompt_path));
    }

    #[test]
    fn write_back_instruction_remaining_out_of_scope_byte_compatibility() {
        let r = format!(".dev/pipeline/receipts/{}-impl.receipt.md", tid(4));
        let prompt_path = "path/to/exec.prompt.md";
        let expected_impl = "Write implementation code changes to the files listed in 'Affected Files'. Do NOT modify any other files.";

        // Implement markerless & marked with receipt path carries receipt clause
        let impl_less = write_back_instruction("implement", prompt_path, Some(&r), false);
        assert!(impl_less.starts_with(expected_impl));
        assert!(impl_less.contains(&r));

        let impl_marked = write_back_instruction("implement", prompt_path, Some(&r), true);
        assert!(impl_marked.starts_with(expected_impl));
        assert!(impl_marked.contains(&r));

        // Implement with None receipt returns expected base implementation instruction
        let impl_none = write_back_instruction("implement", prompt_path, None, false);
        assert_eq!(impl_none, expected_impl);

        // Scaffold marked
        let scaf_marked = write_back_instruction("scaffold", prompt_path, Some(&r), true);
        assert_eq!(scaf_marked, expected_impl);

        // Unknown phases
        let ver_wb = write_back_instruction("verify", prompt_path, Some(&r), false);
        assert_eq!(ver_wb, expected_impl);
    }

    #[test]
    fn writeback_scope_covers_phase_and_marker_combinations() {
        let markerless_prompt = "# Plan\n\n## Tasks\n";
        let marked_required_prompt =
            "# Plan\n\nPipeline Contract: test-first-v1\n\n## Tasks\n- Test-first: required\n";
        let marked_not_applicable_prompt =
            "# Plan\n\nPipeline Contract: test-first-v1\n\n## Tasks\n- Test-first: not-applicable\n";
        let cases = [
            ("audit", markerless_prompt, true, "audit-markerless"),
            ("audit", marked_required_prompt, true, "audit-marked"),
            ("test", markerless_prompt, true, "test-markerless"),
            (
                "test",
                marked_required_prompt,
                false,
                "test-marked-required",
            ),
            (
                "test",
                marked_not_applicable_prompt,
                false,
                "test-marked-not-applicable",
            ),
            ("implement", markerless_prompt, false, "implement"),
            ("scaffold", marked_required_prompt, false, "scaffold"),
            ("unknown", markerless_prompt, false, "unknown-phase"),
        ];

        for (phase, prompt_body, expected, case_name) in cases {
            let is_test_first = has_test_first_marker(prompt_body);
            assert_eq!(
                is_writeback_in_scope(phase, is_test_first),
                expected,
                "case {case_name}"
            );
        }
    }

    #[test]
    fn phase_writeback_appends_to_the_matching_section() {
        let task_id = tid(2);
        let prompt = b"# Plan\n\n## Test Results\n\nold test\n\n## Review Results\n\nold review\n\n## Tasks\n";
        let receipt = format!(
            "### [{task_id}] 2026-08-26\n\n**Verdict:** PASS\n**Evidence:** 3 tests passed; coverage: complete\n"
        );
        let rendered =
            render_phase_writeback(prompt, receipt.as_bytes(), "test", &task_id).unwrap();
        let text = String::from_utf8(rendered).unwrap();
        assert!(text.contains(&format!("old test\n\n### [{task_id}] 2026-08-26")));
        assert!(text.contains("## Review Results\n\nold review"));
    }

    #[test]
    fn phase_writeback_accepts_same_task_heading_in_sibling_section() {
        let task_id = tid(2);
        let prompt = format!(
            "# Plan\n\n## Test Results\n\n### [{task_id}] 2026-08-26\n\n**Verdict:** PASS\n**Evidence:** coverage: complete\n\n## Review Results\n"
        );
        let receipt = format!("### [{task_id}] 2026-08-26\n\n<!-- AUDIT_REVIEW: CLEAR -->\n");
        let rendered =
            render_phase_writeback(prompt.as_bytes(), receipt.as_bytes(), "audit", &task_id)
                .unwrap();
        assert!(String::from_utf8(rendered).unwrap().ends_with(&format!(
            "## Review Results\n\n### [{task_id}] 2026-08-26\n\n<!-- AUDIT_REVIEW: CLEAR -->\n"
        )));
    }

    #[test]
    fn phase_writeback_rejects_invalid_payload_without_changing_input() {
        let task_id = tid(2);
        let prompt = b"# Plan\n\n## Test Results\n\n## Review Results\n";
        let receipt = format!("### [{}] 2026-08-26\n\n**Verdict:** PASS\n", tid(3));
        let error =
            render_phase_writeback(prompt, receipt.as_bytes(), "test", &task_id).unwrap_err();
        assert_eq!(error, PipelineError::TaskMismatch(task_id));
        assert_eq!(prompt, b"# Plan\n\n## Test Results\n\n## Review Results\n");
    }

    #[test]
    fn phase_writeback_rejects_duplicate_destination_sections_and_bad_utf8() {
        let task_id = tid(2);
        let duplicate = b"# Plan\n\n## Review Results\n\n## Review Results\n";
        let receipt = format!("### [{task_id}] 2026-08-26\n\n<!-- AUDIT_REVIEW: CLEAR -->\n");
        assert_eq!(
            render_phase_writeback(duplicate, receipt.as_bytes(), "audit", &task_id).unwrap_err(),
            PipelineError::DuplicateSection("Review Results")
        );
        assert!(matches!(
            render_phase_writeback(b"# Plan\n", &[0xff], "audit", &task_id),
            Err(PipelineError::ReceiptUtf8(_))
        ));
    }

    #[test]
    fn phase_writeback_accepts_valid_audit_payload() {
        let task_id = tid(2);
        let prompt = b"# Plan\n\n## Review Results\n\n## Tasks\n";
        let receipt = format!(
            "### [{task_id}] 2026-08-26\n\n<!-- AUDIT_REVIEW: FINDINGS-OPEN -->\n\nEvidence: finding recorded\n"
        );
        let rendered =
            render_phase_writeback(prompt, receipt.as_bytes(), "audit", &task_id).unwrap();
        assert!(String::from_utf8(rendered)
            .unwrap()
            .contains(&format!("## Review Results\n\n### [{task_id}] 2026-08-26")));
    }

    #[test]
    fn phase_writeback_appends_retry_after_existing_attempt() {
        let task_id = tid(2);
        let prompt = format!(
            "# Plan\n\n## Test Results\n\n### [{task_id}] 2026-08-25\n\n**Verdict:** FAIL\n**Evidence:** first attempt\n\n## Tasks\n"
        );
        let receipt = format!(
            "### [{task_id}] 2026-08-26\n\n**Verdict:** PASS\n**Evidence:** retry attempt\n"
        );
        let rendered = String::from_utf8(
            render_phase_writeback(prompt.as_bytes(), receipt.as_bytes(), "test", &task_id)
                .unwrap(),
        )
        .unwrap();
        assert!(rendered.find("first attempt") < rendered.find("retry attempt"));
        assert!(rendered.contains("**Evidence:** retry attempt\n## Tasks\n"));
    }

    #[test]
    fn phase_writeback_accepts_headings_inside_fenced_payload() {
        let task_id = tid(2);
        let prompt = b"# Plan\n\n## Test Results\n\n## Tasks\n";
        let receipt = format!(
            "### [{task_id}] 2026-08-26\n\n**Verdict:** PASS\n**Evidence:** coverage: complete\n\n```markdown\n## Example\n### Nested example\n```\n"
        );
        assert!(render_phase_writeback(prompt, receipt.as_bytes(), "test", &task_id).is_ok());
    }

    #[test]
    fn phase_writeback_rejects_missing_destination_section() {
        let task_id = tid(2);
        let prompt = b"# Plan\n\n## Test Results\n";
        let receipt = format!("### [{task_id}] 2026-08-26\n\n<!-- AUDIT_REVIEW: CLEAR -->\n");
        assert_eq!(
            render_phase_writeback(prompt, receipt.as_bytes(), "audit", &task_id).unwrap_err(),
            PipelineError::MissingSection("Review Results")
        );
    }

    #[test]
    fn phase_writeback_rejects_malformed_table_boundary_without_mutation() {
        let task_id = tid(2);
        let prompt = b"# Plan\n\n## Review Results\n\n| prior result\n\n## Tasks\n";
        let receipt = format!("### [{task_id}] 2026-08-26\n\n<!-- AUDIT_REVIEW: CLEAR -->\n");
        assert_eq!(
            render_phase_writeback(prompt, receipt.as_bytes(), "audit", &task_id).unwrap_err(),
            PipelineError::MalformedTableBoundary
        );
        assert_eq!(
            prompt,
            b"# Plan\n\n## Review Results\n\n| prior result\n\n## Tasks\n"
        );
    }

    #[test]
    fn agent_contract_maps_audit_to_auditor() {
        assert_eq!(
            agent_contract_rel("audit"),
            "plugins/gal-core/agents/golem-auditor.agent.md"
        );
        assert_eq!(
            agent_contract_rel("implement"),
            "plugins/gal-core/agents/golem-implementer.agent.md"
        );
    }

    #[test]
    fn assemble_produces_self_contained_spec() {
        let body = prompt();
        let scope = tid(3);
        let input = TaskSpecInput {
            task_scope: &scope,
            phase: "implement",
            prompt_path: ".dev/plans/refactor-gal-xmachine-rust-port.prompt.md",
            prompt_body: &body,
            generated: "2026-06-11 12:00",
            git_branch: "main",
            git_head: "deadbee",
            convention_hints: Some("conventions/rust.md; conventions/token-budget.md"),
            receipt_path: None,
            agent_contract_body: None,
            fix_mode: false,
        };
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.out_file_name, format!("{}-implement.md", tid(3)));
        assert!(spec
            .markdown
            .starts_with(&format!("# Task Spec: {} (implement)", tid(3))));
        assert!(spec.markdown.contains("## Task Goal"));
        assert!(spec.markdown.contains("Port orchestration"));
        assert!(spec
            .markdown
            .contains("## Affected Files (allowlist — modify ONLY these)"));
        assert!(spec.markdown.contains("crates/pipeline/src/lib.rs"));
        // Allowlist directive present when files are parsed.
        assert!(spec
            .markdown
            .contains("You may modify ONLY the files listed below"));
        assert!(spec.markdown.contains("## Convention Files"));
        assert!(spec.markdown.contains("- conventions/rust.md"));
        assert!(spec.markdown.contains("golem-implementer.agent.md"));
        assert!(spec
            .markdown
            .contains("**DO NOT run `git commit` or `git push`.**"));
        assert!(!spec.exceeds_size_target);
    }

    #[test]
    fn assemble_forbids_orchestrator_gate_commands_for_every_phase() {
        // A dispatched executor reads the repository `AGENTS.md` and any installed
        // `gal-pipeline` skill, both of which instruct the *orchestrator* to run the
        // Entry Latch before the first implementation edit. Without an explicit
        // counter-instruction in the spec, an executor obeys that higher-precedence
        // text, runs the orchestrator's gates itself, and stalls the phase. The spec
        // must name each gate and override that instruction for all three phases.
        let body = prompt();
        let scope = tid(3);
        for phase in ["implement", "test", "audit"] {
            let input = TaskSpecInput {
                task_scope: &scope,
                phase,
                prompt_path: "P",
                prompt_body: &body,
                generated: "G",
                git_branch: "main",
                git_head: "deadbee",
                convention_hints: None,
                receipt_path: None,
                agent_contract_body: None,
                fix_mode: false,
            };
            let spec = assemble_task_spec(&input).unwrap();

            let boundary_pos = spec
                .markdown
                .find("## IMPORTANT: Orchestrator Gate Boundary")
                .or_else(|| spec.markdown.find("## Dispatched Worker Boundary"))
                .or_else(|| {
                    spec.markdown
                        .find("## IMPORTANT: Dispatched Worker Boundary")
                });
            let goal_pos = spec.markdown.find("## Task Goal");

            let boundary_before_goal = match (boundary_pos, goal_pos) {
                (Some(b), Some(g)) => b < g,
                _ => false,
            };
            let bans_skill = spec.markdown.contains("SKILL.md");
            let bans_gal_subcommand = spec.markdown.contains("DO NOT run any `gal` subcommand")
                || spec.markdown.contains("do not run any `gal` subcommand")
                || spec.markdown.contains("DO NOT run any gal subcommand")
                || spec.markdown.contains("do not run any gal subcommand");

            assert!(
                boundary_before_goal && bans_skill && bans_gal_subcommand,
                "{phase} spec: dispatched worker boundary must sit before Task Goal, ban SKILL.md loading, and ban every gal subcommand"
            );
            assert!(
                spec.markdown.contains("overrides any Entry Latch"),
                "{phase} spec must override the Entry Latch instruction the executor reads elsewhere"
            );
            assert!(
                !spec.exceeds_size_target,
                "{phase} spec must stay within the size target after the added block"
            );
        }
    }

    #[test]
    fn assemble_omits_convention_block_when_no_hints() {
        let body = prompt();
        let scope = tid(3);
        let input = TaskSpecInput {
            task_scope: &scope,
            phase: "audit",
            prompt_path: "P",
            prompt_body: &body,
            generated: "G",
            git_branch: "main",
            git_head: "h",
            convention_hints: None,
            receipt_path: None,
            agent_contract_body: None,
            fix_mode: false,
        };
        let spec = assemble_task_spec(&input).unwrap();
        assert_eq!(spec.out_file_name, format!("{}-audit.md", tid(3)));
        assert!(!spec.markdown.contains("## Convention Files"));
        // audit phase → review write-back + auditor contract.
        assert!(spec.markdown.contains("## Review Results"));
        assert!(spec.markdown.contains("golem-auditor.agent.md"));
    }

    fn base_input<'a>(body: &'a str, scope: &'a str, phase: &'a str) -> TaskSpecInput<'a> {
        TaskSpecInput {
            task_scope: scope,
            phase,
            prompt_path: "P",
            prompt_body: body,
            generated: "G",
            git_branch: "main",
            git_head: "h",
            convention_hints: None,
            receipt_path: None,
            agent_contract_body: None,
            fix_mode: false,
        }
    }

    fn prompt_with_handoff(task_id: &str, phase: &str) -> String {
        format!(
            "## Tasks\n\n- [ ] **{task_id}** — Repair `crates/pipeline/src/task_spec.rs`.\n\n## Status\n\n### Handoff Notes\n\n#### Retry Handoff — {task_id} / {phase}\n\n- Status: OPEN\n- Problem: The prior round did not satisfy the parser contract.\n- Next human step: Repair the parser and run its focused tests.\n"
        )
    }

    #[test]
    fn fix_mode_renders_each_allowed_handoff_origin_as_binding_acceptance() {
        let scope = tid(21);
        for phase in ["IMPLEMENT", "TEST", "AUDIT"] {
            let body = prompt_with_handoff(&scope, phase);
            let mut input = base_input(&body, &scope, "implement");
            input.fix_mode = true;
            let spec = assemble_task_spec(&input).unwrap();
            assert!(spec.markdown.contains("### Fix Round — Binding Acceptance"));
            assert!(spec.markdown.contains(&format!("OPEN {phase} handoff")));
            assert!(spec.markdown.contains("Problem: The prior round"));
            assert!(spec.markdown.contains("Next human step: Repair the parser"));
        }
    }

    #[test]
    fn retry_handoff_errors_are_typed_and_fail_closed() {
        let scope = tid(22);
        let absent = prompt();
        assert!(matches!(
            extract_open_retry_handoff(&absent, &scope),
            Err(RetryHandoffError::NotFound { .. })
        ));

        let first = prompt_with_handoff(&scope, "TEST");
        let ambiguous = format!(
            "{first}\n#### Retry Handoff — {scope} / AUDIT\n\n- Status: OPEN\n- Problem: A second open handoff.\n- Next human step: Resolve the ambiguity.\n"
        );
        assert!(matches!(
            extract_open_retry_handoff(&ambiguous, &scope),
            Err(RetryHandoffError::Ambiguous { .. })
        ));

        let malformed = format!(
            "## Status\n\n### Handoff Notes\n\n#### Retry Handoff — {scope} / IMPLEMENT\n\n- Status: OPEN\n- Problem: Missing the next step.\n"
        );
        assert!(matches!(
            extract_open_retry_handoff(&malformed, &scope),
            Err(RetryHandoffError::Malformed { .. })
        ));

        let invalid_phase = prompt_with_handoff(&scope, "VERIFY");
        assert!(matches!(
            extract_open_retry_handoff(&invalid_phase, &scope),
            Err(RetryHandoffError::Malformed { .. })
        ));

        let invalid_heading = format!(
            "## Status\n\n### Handoff Notes\n\n#### Retry Handoff — {scope}\n\n- Status: OPEN\n"
        );
        assert!(matches!(
            extract_open_retry_handoff(&invalid_heading, &scope),
            Err(RetryHandoffError::Malformed { .. })
        ));
    }

    #[test]
    fn non_fix_mode_ignores_handoffs_and_preserves_rendering() {
        let scope = tid(23);
        let ordinary =
            format!("## Tasks\n\n- [ ] **{scope}** — Repair `crates/pipeline/src/task_spec.rs`.\n");
        let with_handoff = prompt_with_handoff(&scope, "TEST");
        let ordinary_spec =
            assemble_task_spec(&base_input(&ordinary, &scope, "implement")).unwrap();
        let handoff_spec =
            assemble_task_spec(&base_input(&with_handoff, &scope, "implement")).unwrap();
        assert_eq!(ordinary_spec, handoff_spec);
    }

    // Explicit contract body appears exactly once under
    // `## Agent Contract`, verbatim, and the legacy control-node file
    // reference is absent.
    #[test]
    fn assemble_renders_explicit_contract_body_inline_exactly_once() {
        let body = prompt();
        let scope = tid(3);
        let contract = "# Golem Implementer\n\nDo the thing carefully.\n";
        let mut input = base_input(&body, &scope, "implement");
        input.agent_contract_body = Some(contract);
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(
            spec.markdown.matches("## Agent Contract").count(),
            1,
            "Agent Contract header must appear exactly once"
        );
        assert_eq!(
            spec.markdown.matches("Golem Implementer").count(),
            1,
            "contract body must be inlined exactly once"
        );
        assert!(spec.markdown.contains("Do the thing carefully."));
        assert!(!spec.markdown.contains("Follow the instructions in:"));
        assert!(!spec.markdown.contains("golem-implementer.agent.md"));
    }

    // No body supplied → legacy file-reference rendering is
    // preserved unchanged.
    #[test]
    fn assemble_falls_back_to_file_reference_when_no_body_supplied() {
        let body = prompt();
        let scope = tid(3);
        let input = base_input(&body, &scope, "implement");
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.markdown.matches("## Agent Contract").count(), 1);
        assert!(spec.markdown.contains(
            "Follow the instructions in: `plugins/gal-core/agents/golem-implementer.agent.md`"
        ));
    }

    // Size reporting never truncates the inlined body for any
    // phase — a large contract still appears in full and the byte_len /
    // exceeds_size_target signal reflects the actual assembled length.
    #[test]
    fn assemble_inlines_large_contract_body_without_truncation_for_all_phases() {
        let body = prompt();
        let scope = tid(3);
        let large_contract = "X".repeat(SPEC_SIZE_TARGET_BYTES * 2);
        for phase in ["implement", "test", "audit"] {
            let mut input = base_input(&body, &scope, phase);
            input.receipt_path = Some(".dev/pipeline/receipts/r.md");
            input.agent_contract_body = Some(&large_contract);
            let spec = assemble_task_spec(&input).unwrap();

            assert!(
                spec.markdown.contains(&large_contract),
                "phase {phase}: full contract body must be present, no truncation"
            );
            assert_eq!(spec.byte_len, spec.markdown.len());
            assert!(spec.exceeds_size_target);
        }
    }
}
