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
                    for expanded in expand_brace_alternatives(candidate) {
                        if !expanded.is_empty()
                            && (expanded.contains('/')
                                || expanded.contains('\\')
                                || looks_like_bare_filename(&expanded))
                            && seen.insert(expanded.to_lowercase())
                        {
                            out.push(expanded);
                        }
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

/// Expand one `{a,b,c}` brace-alternation group in `s` into its literal
/// variants, e.g. `crates/x/{a,b}.rs` becomes `crates/x/a.rs` and
/// `crates/x/b.rs`. Multiple non-nested groups expand via cross product
/// through recursion on each already-expanded variant. A malformed group
/// (unbalanced, empty, or nested braces) is left as one literal candidate,
/// unchanged and never dropped, so a plan author's typo degrades to the old
/// literal-string behavior instead of losing the file.
fn expand_brace_alternatives(s: &str) -> Vec<String> {
    let Some(open) = s.find('{') else {
        return vec![s.to_string()];
    };
    let after = &s[open + 1..];
    match after.find(['{', '}']) {
        Some(rel) if after.as_bytes()[rel] == b'}' => {
            let close = open + 1 + rel;
            let inner = &s[open + 1..close];
            if inner.is_empty() {
                return vec![s.to_string()];
            }
            let prefix = &s[..open];
            let suffix = &s[close + 1..];
            inner
                .split(',')
                .flat_map(|alt| expand_brace_alternatives(&format!("{prefix}{alt}{suffix}")))
                .collect()
        }
        _ => vec![s.to_string()],
    }
}

/// Backtick paths named in a `## Files to Create or Modify` bullet's own
/// subject clause only — the part before the ` — ` description, e.g. the two
/// leading paths in `` - `a.rs`, `b.rs` — moves X to `c.rs`. `` A path the
/// prose merely mentions after the dash (such as a pointer target) is not
/// part of what the bullet declares itself to be about, so it must not steal
/// a `.find()` match away from that path's own dedicated bullet further down
/// the section.
fn subject_backtick_paths(line: &str) -> Vec<String> {
    let subject = line.split('—').next().unwrap_or(line);
    backtick_paths(subject)
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
            fallback
                .iter()
                .find(|line| {
                    subject_backtick_paths(line)
                        .iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(path))
                })
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
            if !lower
                .lines()
                .any(|line| line.contains("evidence:") || line.contains("coverage:"))
            {
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

fn write_back_instruction(phase: &str, prompt_path: &str, receipt_path: Option<&str>) -> String {
    let phase_lc = phase.to_lowercase();
    if phase_lc == "investigate" {
        // The receipt is a liveness signal for `investigate`; the brief is the
        // work product, named `brief-<n>.md` beside the receipt so the
        // orchestrator can find it without a control-node write-back path.
        let brief = receipt_path
            .and_then(|receipt| {
                let (dir, filename) = receipt.rsplit_once('/')?;
                let stem = filename.strip_suffix(".receipt.md")?;
                let n = stem.rsplit_once('-').map_or(stem, |(_, n)| n);
                Some(format!("{dir}/brief-{n}.md"))
            })
            .unwrap_or_else(|| "brief-1.md".to_string());
        return match receipt_path {
            Some(receipt) => format!(
                "Write the research brief to the file: {brief}\n\
                 This file lives beside the dispatch receipt but is a separate work product.\n\n\
                 MANDATORY — you MUST write a one-line verdict plus a short evidence summary to the receipt file: {receipt}\n\
                 This is the completion signal. The pipeline accepts this phase ONLY if that file exists and is non-empty. \
                 If you do not write it, this phase is recorded as FAILED (no-receipt) no matter what else you did. \
                 Write the receipt file before you finish."
            ),
            None => format!("Write the research brief to the file: {brief}"),
        };
    }
    let in_scope = is_writeback_in_scope(phase);
    if in_scope {
        if let Some(receipt) = receipt_path {
            return format!(
                "Write the complete task-scoped Markdown subsection to the receipt file: {receipt}.\n\
                 The first line MUST be `### [T-NN] YYYY-MM-DD`; do not put metadata or any other preamble before that heading.\n\
                 Do NOT edit the execution prompt directly. The control node will place a valid test receipt under `## Test Results` or a valid audit receipt under `## Review Results`.\n\n\
                 MANDATORY — you MUST write the complete task-scoped Markdown subsection, beginning with the task heading on the first line, to the receipt file: {receipt}\n\
                 This is the completion signal. The pipeline accepts this phase ONLY if that file exists and is non-empty. \
                 If you do not write it, this phase is recorded as FAILED (no-receipt) no matter what else you did. \
                 Write the receipt file before you finish."
            );
        }
    }

    let base = match phase_lc.as_str() {
        "test" => format!("Write test results to `## Test Results` in the file: {prompt_path}"),
        "audit" => format!("Write audit results to `## Review Results` in the file: {prompt_path}"),
        _ => "Write implementation code changes to the files listed in 'Affected Files'. \
              Do NOT modify any other files."
            .to_string(),
    };

    // The dispatch receipt is a liveness signal for all three dispatched phases (implement, test, audit).
    match (phase_lc.as_str(), receipt_path) {
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
        "investigate" => "plugins/gal-core/agents/golem-researcher.agent.md",
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

    let affected = extract_affected_files(input.prompt_body, input.task_scope);

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

    let effective_receipt = input.receipt_path.map(ToString::to_string);

    let write_back =
        write_back_instruction(input.phase, input.prompt_path, effective_receipt.as_deref());
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
    md.push_str(&format!("Generated: {}\n", input.generated));
    md.push_str(&format!(
        "Git branch: {}  |  HEAD: {}\n",
        input.git_branch, input.git_head
    ));
    md.push_str(&format!("Source prompt: {}\n", input.prompt_path));
    md.push_str("\n---\n\n");
    md.push_str("## Dispatched Worker Boundary\n\n");
    md.push_str("**DO NOT run any `gal` subcommand and DO NOT load or execute any workflow `SKILL.md` file.**\n");
    md.push_str("These gates are orchestrator-owned. The orchestrator already satisfied every gate this dispatch requires.\n");
    md.push_str("This prohibition overrides any Entry Latch, Critical Stop Rule, or workflow-obedience instruction you may have read elsewhere, including a repository `AGENTS.md` or an installed `gal-pipeline` skill. Those describe the orchestrator's duties, not yours.\n");
    md.push_str("You are a single dispatched phase. Do the scoped work below and stop.\n\n");
    md.push_str("## Task Goal\n\n");
    md.push_str(&task_goal);

    md.push_str("\n\n## Affected Files (allowlist — modify ONLY these)\n\n");
    md.push_str(&affected_block);
    md.push_str("\n\n## Write-Back Instruction\n\n");
    md.push_str(&write_back);
    md.push_str("\n\n");
    if let Some(ref cb) = convention_block {
        md.push_str(cb);
        md.push_str("\n\n");
    }
    md.push_str("## Writing Quality\n\n");
    md.push_str("For every human-facing message and durable prose, including progress, self-review the unsent draft in-process for accuracy and readability. Use an available configured checker on that draft. Write for the actual recipient and retain needed background. Ground facts in the prompt, supplied materials, or actual task tool observations. Compare each material factual assertion in the unsent draft with specific available support; remove or qualify unsupported claims. Missing completion evidence does not mean work never started. Honor artifact-only requests unless a higher-priority host instruction requires otherwise. Distinguish inference, assumptions, suggestions, placeholders, and future commitments. Preserve meaning, protected spans, and project terms. Put accuracy before clarity and clarity before brevity. Allow at most two repairs with rechecks and semantic rollback. With advisory findings only, use the last meaning-preserving draft. Required gates block on hard findings or operational failure, not advisory findings alone unless explicitly required. Disclose optional checker failure once unless availability or delivery impact changes. Do not recursively lint checker-status messages or assign a reviewer per message. Read `plugins/gal-core/conventions/writing-quality.md` when that source file exists. Otherwise read the writing-quality.md section in `~/.gal/plugins/gal/rules/gal.md` for the full neutral contract. Expand `~` to the current user home.\n\n");
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

/// Returns whether the control node owns semantic placement for a dispatched phase.
pub fn is_writeback_in_scope(phase: &str) -> bool {
    matches!(phase.to_ascii_lowercase().as_str(), "audit" | "test")
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
        let text = "Targets: `CLAUDE.md`, `AGENTS.md`.";
        let paths = backtick_paths(text);
        assert!(paths.iter().any(|p| p == "CLAUDE.md"));
        assert!(paths.iter().any(|p| p == "AGENTS.md"));
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn backtick_paths_expands_one_brace_group_into_each_variant() {
        let text = "Files: `crates/cli/src/commands/{boundary_check,converge_check}.rs`";
        let paths = backtick_paths(text);
        assert!(paths
            .iter()
            .any(|p| p == "crates/cli/src/commands/boundary_check.rs"));
        assert!(paths
            .iter()
            .any(|p| p == "crates/cli/src/commands/converge_check.rs"));
        assert_eq!(paths.len(), 2);
    }

    #[test]
    fn backtick_paths_expands_two_sequential_brace_groups_as_cross_product() {
        let text = "Files: `crates/{cli,dispatch}/src/{a,b}.rs`";
        let paths = backtick_paths(text);
        for expected in [
            "crates/cli/src/a.rs",
            "crates/cli/src/b.rs",
            "crates/dispatch/src/a.rs",
            "crates/dispatch/src/b.rs",
        ] {
            assert!(paths.iter().any(|p| p == expected), "missing {expected}");
        }
        assert_eq!(paths.len(), 4);
    }

    #[test]
    fn backtick_paths_treats_unbalanced_brace_group_as_one_literal() {
        let text = "Files: `crates/cli/src/commands/{boundary_check.rs`";
        let paths = backtick_paths(text);
        assert_eq!(paths, vec!["crates/cli/src/commands/{boundary_check.rs"]);
    }

    #[test]
    fn backtick_paths_treats_nested_brace_group_as_one_literal() {
        let text = "Files: `crates/cli/{a,{b,c}}.rs`";
        let paths = backtick_paths(text);
        assert_eq!(paths, vec!["crates/cli/{a,{b,c}}.rs"]);
    }

    #[test]
    fn backtick_paths_treats_empty_brace_group_as_one_literal() {
        let text = "Files: `crates/cli/{}.rs`";
        let paths = backtick_paths(text);
        assert_eq!(paths, vec!["crates/cli/{}.rs"]);
    }

    #[test]
    fn backtick_paths_leaves_brace_free_paths_unaffected() {
        let text = "Files: `crates/cli/src/commands/dispatch.rs`";
        let paths = backtick_paths(text);
        assert_eq!(paths, vec!["crates/cli/src/commands/dispatch.rs"]);
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
        assert!(write_back_instruction("test", "P", None).contains("## Test Results"));
        assert!(write_back_instruction("audit", "P", None).contains("## Review Results"));
        assert!(write_back_instruction("implement", "P", None).contains("Affected Files"));
        // Unknown phase (incl. the removed `verify`) falls back to implement.
        assert!(write_back_instruction("verify", "P", None).contains("Affected Files"));
        assert!(write_back_instruction("deploy", "P", None).contains("Affected Files"));
    }

    #[test]
    fn write_back_instruction_names_receipt_for_test_and_audit() {
        let r = ".dev/pipeline/fix-x/T-XX/x.log";
        let test_wb = write_back_instruction("test", "P", Some(r));
        assert!(
            test_wb.contains(r),
            "test write-back must name the receipt path"
        );
        assert!(
            test_wb.contains("complete task-scoped Markdown subsection"),
            "in-scope test instruction demands subsection"
        );
        assert!(test_wb.contains("first line MUST be `### [T-NN] YYYY-MM-DD`"));
        assert!(test_wb.contains("under `## Test Results`"));
        let audit_wb = write_back_instruction("audit", "P", Some(r));
        assert!(
            audit_wb.contains(r),
            "audit write-back must name the receipt path"
        );
        assert!(
            audit_wb.contains("complete task-scoped Markdown subsection"),
            "in-scope audit instruction demands subsection"
        );
        assert!(audit_wb.contains("first line MUST be `### [T-NN] YYYY-MM-DD`"));
        assert!(audit_wb.contains("under `## Review Results`"));
        // The orchestrator's 2f correctness receipt stays the correctness authority while the dispatch receipt covers liveness.
        assert!(write_back_instruction("implement", "P", Some(r)).contains(r));
    }

    #[test]
    fn write_back_instruction_in_scope_demands_heading_first_subsection() {
        let r = ".dev/pipeline/fix-x/T-XX/x.log";
        let prompt_path = "path/to/exec.prompt.md";

        // Markerless test
        let test_wb = write_back_instruction("test", prompt_path, Some(&r));
        assert!(test_wb.contains("complete task-scoped Markdown subsection"));
        assert!(test_wb.contains("first line MUST be `### [T-NN] YYYY-MM-DD`"));
        assert!(!test_wb.contains(prompt_path));

        // Markerless audit
        let audit_wb_less = write_back_instruction("audit", prompt_path, Some(&r));
        assert!(audit_wb_less.contains("complete task-scoped Markdown subsection"));
        assert!(audit_wb_less.contains("first line MUST be `### [T-NN] YYYY-MM-DD`"));
        assert!(!audit_wb_less.contains(prompt_path));

        // Marked audit
        let audit_wb_marked = write_back_instruction("audit", prompt_path, Some(&r));
        assert!(audit_wb_marked.contains("complete task-scoped Markdown subsection"));
        assert!(audit_wb_marked.contains("first line MUST be `### [T-NN] YYYY-MM-DD`"));
        assert!(!audit_wb_marked.contains(prompt_path));
    }

    #[test]
    fn write_back_instruction_remaining_out_of_scope_byte_compatibility() {
        let r = ".dev/pipeline/fix-x/T-XX/x.log";
        let prompt_path = "path/to/exec.prompt.md";
        let expected_impl = "Write implementation code changes to the files listed in 'Affected Files'. Do NOT modify any other files.";

        // Implement markerless & marked with receipt path carries receipt clause
        let impl_less = write_back_instruction("implement", prompt_path, Some(&r));
        assert!(impl_less.starts_with(expected_impl));
        assert!(impl_less.contains(&r));

        let impl_marked = write_back_instruction("implement", prompt_path, Some(&r));
        assert!(impl_marked.starts_with(expected_impl));
        assert!(impl_marked.contains(&r));

        // Implement with None receipt returns expected base implementation instruction
        let impl_none = write_back_instruction("implement", prompt_path, None);
        assert_eq!(impl_none, expected_impl);

        // Unknown phases
        let ver_wb = write_back_instruction("verify", prompt_path, Some(&r));
        assert_eq!(ver_wb, expected_impl);
    }

    #[test]
    fn writeback_scope_covers_phase_and_marker_combinations() {
        let markerless_prompt = "# Plan\n\n## Tasks\n";
        let cases = [
            ("audit", markerless_prompt, true, "audit"),
            ("test", markerless_prompt, true, "test-markerless"),
            ("implement", markerless_prompt, false, "implement"),
            ("unknown", markerless_prompt, false, "unknown-phase"),
        ];

        for (phase, _prompt_body, expected, case_name) in cases {
            assert_eq!(is_writeback_in_scope(phase), expected, "case {case_name}");
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
    fn phase_writeback_rejects_metadata_before_heading_without_mutation() {
        let task_id = tid(2);
        let prompt = b"# Plan\n\n## Test Results\n\n## Review Results\n";
        let receipt = format!(
            "plan: fix-x\ntask: {task_id}\n\n### [{task_id}] 2026-08-26\n\n**Verdict:** PASS\n"
        );
        assert_eq!(
            render_phase_writeback(prompt, receipt.as_bytes(), "test", &task_id).unwrap_err(),
            PipelineError::InvalidHeading(task_id.to_string())
        );
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
        assert!(spec.markdown.contains("## Writing Quality"));
        assert!(spec
            .markdown
            .contains("`plugins/gal-core/conventions/writing-quality.md`"));
        assert!(spec.markdown.contains(
            "For every human-facing message and durable prose, including progress, self-review the unsent draft in-process for accuracy and readability. Use an available configured checker on that draft. Write for the actual recipient and retain needed background. Ground facts in the prompt, supplied materials, or actual task tool observations. Compare each material factual assertion in the unsent draft with specific available support; remove or qualify unsupported claims. Missing completion evidence does not mean work never started. Honor artifact-only requests unless a higher-priority host instruction requires otherwise. Distinguish inference, assumptions, suggestions, placeholders, and future commitments. Preserve meaning, protected spans, and project terms. Put accuracy before clarity and clarity before brevity. Allow at most two repairs with rechecks and semantic rollback. With advisory findings only, use the last meaning-preserving draft. Required gates block on hard findings or operational failure, not advisory findings alone unless explicitly required. Disclose optional checker failure once unless availability or delivery impact changes. Do not recursively lint checker-status messages or assign a reviewer per message. Read `plugins/gal-core/conventions/writing-quality.md` when that source file exists. Otherwise read the writing-quality.md section in `~/.gal/plugins/gal/rules/gal.md` for the full neutral contract. Expand `~` to the current user home."
        ));
    }

    #[test]
    fn assemble_includes_writing_quality_without_convention_hints_or_contract_body() {
        let body = prompt();
        let scope = tid(3);
        let mut input = base_input(&body, &scope, "implement");
        input.convention_hints = None;
        input.agent_contract_body = None;
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.markdown.matches("## Writing Quality").count(), 1);
        assert_eq!(
            spec.markdown
                .matches("`plugins/gal-core/conventions/writing-quality.md`")
                .count(),
            1
        );
        assert!(spec.markdown.contains(
            "For every human-facing message and durable prose, including progress, self-review the unsent draft in-process for accuracy and readability. Use an available configured checker on that draft. Write for the actual recipient and retain needed background. Ground facts in the prompt, supplied materials, or actual task tool observations. Compare each material factual assertion in the unsent draft with specific available support; remove or qualify unsupported claims. Missing completion evidence does not mean work never started. Honor artifact-only requests unless a higher-priority host instruction requires otherwise. Distinguish inference, assumptions, suggestions, placeholders, and future commitments. Preserve meaning, protected spans, and project terms. Put accuracy before clarity and clarity before brevity. Allow at most two repairs with rechecks and semantic rollback. With advisory findings only, use the last meaning-preserving draft. Required gates block on hard findings or operational failure, not advisory findings alone unless explicitly required. Disclose optional checker failure once unless availability or delivery impact changes. Do not recursively lint checker-status messages or assign a reviewer per message. Read `plugins/gal-core/conventions/writing-quality.md` when that source file exists. Otherwise read the writing-quality.md section in `~/.gal/plugins/gal/rules/gal.md` for the full neutral contract. Expand `~` to the current user home."
        ));
        assert!(!spec.markdown.contains("## Convention Files"));
        assert_eq!(spec.byte_len, spec.markdown.len());
    }

    #[test]
    fn assemble_writing_quality_hints_present_contract_present() {
        let body = prompt();
        let scope = tid(3);
        let mut input = base_input(&body, &scope, "implement");
        input.convention_hints = Some("plugins/gal-core/conventions/rust.md");
        input.agent_contract_body = Some("# Inlined Implementer Contract\n");
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.markdown.matches("## Writing Quality").count(), 1);
        assert_eq!(
            spec.markdown
                .matches("`plugins/gal-core/conventions/writing-quality.md`")
                .count(),
            1
        );
        assert!(spec.markdown.contains(
            "For every human-facing message and durable prose, including progress, self-review the unsent draft in-process for accuracy and readability. Use an available configured checker on that draft. Write for the actual recipient and retain needed background. Ground facts in the prompt, supplied materials, or actual task tool observations. Compare each material factual assertion in the unsent draft with specific available support; remove or qualify unsupported claims. Missing completion evidence does not mean work never started. Honor artifact-only requests unless a higher-priority host instruction requires otherwise. Distinguish inference, assumptions, suggestions, placeholders, and future commitments. Preserve meaning, protected spans, and project terms. Put accuracy before clarity and clarity before brevity. Allow at most two repairs with rechecks and semantic rollback. With advisory findings only, use the last meaning-preserving draft. Required gates block on hard findings or operational failure, not advisory findings alone unless explicitly required. Disclose optional checker failure once unless availability or delivery impact changes. Do not recursively lint checker-status messages or assign a reviewer per message. Read `plugins/gal-core/conventions/writing-quality.md` when that source file exists. Otherwise read the writing-quality.md section in `~/.gal/plugins/gal/rules/gal.md` for the full neutral contract. Expand `~` to the current user home."
        ));
        assert!(spec.markdown.contains("## Convention Files"));
        assert!(spec
            .markdown
            .contains("- plugins/gal-core/conventions/rust.md"));
        assert_eq!(spec.markdown.matches("## Agent Contract").count(), 1);
        assert!(spec.markdown.contains("# Inlined Implementer Contract"));
        assert!(!spec.markdown.contains("Follow the instructions in:"));
        assert_eq!(spec.byte_len, spec.markdown.len());
        assert_eq!(
            spec.exceeds_size_target,
            spec.byte_len > SPEC_SIZE_TARGET_BYTES
        );
    }

    #[test]
    fn assemble_writing_quality_hints_present_contract_absent() {
        let body = prompt();
        let scope = tid(3);
        let mut input = base_input(&body, &scope, "test");
        input.convention_hints = Some(
            "plugins/gal-core/conventions/rust.md; plugins/gal-core/conventions/token-budget.md",
        );
        input.agent_contract_body = None;
        let receipt = format!(".dev/pipeline/feat-x/{scope}/{scope}-test.receipt.md");
        input.receipt_path = Some(&receipt);
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.markdown.matches("## Writing Quality").count(), 1);
        assert_eq!(
            spec.markdown
                .matches("`plugins/gal-core/conventions/writing-quality.md`")
                .count(),
            1
        );
        assert!(spec.markdown.contains("## Convention Files"));
        assert!(spec
            .markdown
            .contains("- plugins/gal-core/conventions/rust.md"));
        assert!(spec
            .markdown
            .contains("- plugins/gal-core/conventions/token-budget.md"));
        assert_eq!(spec.markdown.matches("## Agent Contract").count(), 1);
        assert!(spec.markdown.contains(
            "Follow the instructions in: `plugins/gal-core/agents/golem-tester.agent.md`"
        ));
        assert!(spec.markdown.contains(&receipt));
        assert_eq!(spec.byte_len, spec.markdown.len());
        assert_eq!(
            spec.exceeds_size_target,
            spec.byte_len > SPEC_SIZE_TARGET_BYTES
        );
    }

    #[test]
    fn assemble_writing_quality_hints_absent_contract_present() {
        let body = prompt();
        let scope = tid(3);
        let mut input = base_input(&body, &scope, "audit");
        input.convention_hints = None;
        input.agent_contract_body = Some("# Auditor Contract Body\nAudit carefully.\n");
        let receipt = format!(".dev/pipeline/feat-x/{scope}/{scope}-audit.receipt.md");
        input.receipt_path = Some(&receipt);
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.markdown.matches("## Writing Quality").count(), 1);
        assert_eq!(
            spec.markdown
                .matches("`plugins/gal-core/conventions/writing-quality.md`")
                .count(),
            1
        );
        assert!(!spec.markdown.contains("## Convention Files"));
        assert_eq!(spec.markdown.matches("## Agent Contract").count(), 1);
        assert!(spec.markdown.contains("# Auditor Contract Body"));
        assert!(!spec.markdown.contains("Follow the instructions in:"));
        assert!(spec.markdown.contains(&receipt));
        assert_eq!(spec.byte_len, spec.markdown.len());
        assert_eq!(
            spec.exceeds_size_target,
            spec.byte_len > SPEC_SIZE_TARGET_BYTES
        );
    }

    #[test]
    fn assemble_writing_quality_hints_absent_contract_absent() {
        let body = prompt();
        let scope = tid(3);
        let mut input = base_input(&body, &scope, "audit");
        input.convention_hints = None;
        input.agent_contract_body = None;
        let receipt = format!(".dev/pipeline/feat-x/{scope}/{scope}-audit.receipt.md");
        input.receipt_path = Some(&receipt);
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.markdown.matches("## Writing Quality").count(), 1);
        assert_eq!(
            spec.markdown
                .matches("`plugins/gal-core/conventions/writing-quality.md`")
                .count(),
            1
        );
        assert!(!spec.markdown.contains("## Convention Files"));
        assert_eq!(spec.markdown.matches("## Agent Contract").count(), 1);
        assert!(spec.markdown.contains(
            "Follow the instructions in: `plugins/gal-core/agents/golem-auditor.agent.md`"
        ));
        assert!(spec.markdown.contains(&receipt));
        assert_eq!(spec.byte_len, spec.markdown.len());
        assert_eq!(
            spec.exceeds_size_target,
            spec.byte_len > SPEC_SIZE_TARGET_BYTES
        );
    }

    #[test]
    fn assemble_writing_quality_matrix_all_phases_and_combinations_preserves_boundaries() {
        let body = prompt();
        let scope = tid(3);
        let phases = ["implement", "test", "audit"];
        let hints_options = [Some("plugins/gal-core/conventions/rust.md"), None];
        let contract_options = [Some("# Custom Inlined Contract\n"), None];

        for phase in phases {
            for hints in hints_options {
                for contract in contract_options {
                    let receipt_file =
                        format!(".dev/pipeline/feat-x/{scope}/{scope}-{phase}.receipt.md");
                    let mut input = base_input(&body, &scope, phase);
                    input.convention_hints = hints;
                    input.agent_contract_body = contract;
                    if phase == "test" || phase == "audit" {
                        input.receipt_path = Some(&receipt_file);
                    }

                    let spec = assemble_task_spec(&input).unwrap();

                    // Exactly one Writing Quality header and canonical convention link
                    assert_eq!(
                        spec.markdown.matches("## Writing Quality").count(),
                        1,
                        "phase={phase}, hints={hints:?}, contract={contract:?}: must have exactly 1 Writing Quality header"
                    );
                    assert_eq!(
                        spec.markdown
                            .matches("`plugins/gal-core/conventions/writing-quality.md`")
                            .count(),
                        1,
                        "phase={phase}, hints={hints:?}, contract={contract:?}: must have exactly 1 canonical convention pointer"
                    );
                    assert!(spec.markdown.contains(
                        "Ground facts in the prompt, supplied materials, or actual task tool observations. Compare each material factual assertion in the unsent draft with specific available support; remove or qualify unsupported claims. Missing completion evidence does not mean work never started. Honor artifact-only requests unless a higher-priority host instruction requires otherwise. Distinguish inference, assumptions, suggestions, placeholders, and future commitments. Preserve meaning, protected spans, and project terms."
                    ));

                    for obligation in [
                        "including progress",
                        "when that source file exists. Otherwise",
                        "~/.gal/plugins/gal/rules/gal.md",
                        "Expand `~` to the current user home",
                        "self-review the unsent draft in-process",
                        "Use an available configured checker on that draft",
                        "actual recipient",
                        "at most two repairs with rechecks and semantic rollback",
                        "With advisory findings only, use the last meaning-preserving draft",
                        "hard findings or operational failure",
                        "not advisory findings alone unless explicitly required",
                        "Disclose optional checker failure once unless availability or delivery impact changes",
                        "Do not recursively lint checker-status messages or assign a reviewer per message",
                    ] {
                        assert!(
                            spec.markdown.contains(obligation),
                            "phase={phase}, hints={hints:?}, contract={contract:?}: missing {obligation}"
                        );
                    }

                    // Dispatched worker boundary intact
                    assert!(spec
                        .markdown
                        .starts_with(&format!("# Task Spec: {scope} ({phase})\n\n")));
                    assert!(spec.markdown.contains("## Dispatched Worker Boundary"));
                    assert!(spec.markdown.contains(
                        "**DO NOT run any `gal` subcommand and DO NOT load or execute any workflow `SKILL.md` file.**"
                    ));

                    // Task goal intact
                    assert!(spec.markdown.contains("## Task Goal"));
                    assert!(spec.markdown.contains("Port orchestration"));

                    // Affected files allowlist intact
                    assert!(spec
                        .markdown
                        .contains("## Affected Files (allowlist — modify ONLY these)"));
                    assert!(spec.markdown.contains("crates/pipeline/src/lib.rs"));

                    // Write-back instruction intact
                    assert!(spec.markdown.contains("## Write-Back Instruction"));
                    if phase == "test" || phase == "audit" {
                        assert!(spec.markdown.contains(&receipt_file));
                        assert!(spec
                            .markdown
                            .contains("first line MUST be `### [T-NN] YYYY-MM-DD`"));
                    } else {
                        assert!(spec.markdown.contains(
                            "Write implementation code changes to the files listed in 'Affected Files'."
                        ));
                    }

                    // Convention files block presence aligns with hints
                    if hints.is_some() {
                        assert!(spec.markdown.contains("## Convention Files"));
                        assert!(spec
                            .markdown
                            .contains("- plugins/gal-core/conventions/rust.md"));
                    } else {
                        assert!(!spec.markdown.contains("## Convention Files"));
                    }

                    // Agent contract block aligns with contract presence
                    assert_eq!(spec.markdown.matches("## Agent Contract").count(), 1);
                    if let Some(body_text) = contract {
                        assert!(spec.markdown.contains(body_text));
                        assert!(!spec.markdown.contains("Follow the instructions in:"));
                    } else {
                        assert!(spec.markdown.contains("Follow the instructions in:"));
                        assert!(spec.markdown.contains(agent_contract_rel(phase)));
                    }

                    // Commit boundary intact
                    assert!(spec.markdown.contains("## IMPORTANT: Commit Boundary"));
                    assert!(spec
                        .markdown
                        .contains("**DO NOT run `git commit` or `git push`.**"));
                    assert!(spec
                        .markdown
                        .contains("*This spec is transient — discard after use.*"));

                    // Byte size signaling intact
                    assert_eq!(spec.byte_len, spec.markdown.len());
                    assert_eq!(
                        spec.exceeds_size_target,
                        spec.byte_len > SPEC_SIZE_TARGET_BYTES
                    );
                }
            }
        }
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
            input.receipt_path = Some(".dev/pipeline/fix-x/T-XX/x.log");
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

    // Regression test for substring collision: when `docs/x/readme.md`
    // precedes `README.md` in the Files section and the task names only
    // `README.md`, the lookup must not match the earlier substring hit.
    #[test]
    fn affected_file_lines_avoids_substring_collision_when_longer_path_precedes() {
        let prompt_lines = [
            "## Files to Create or Modify",
            "",
            "- `docs/x/readme.md`",
            "- `README.md`",
        ];
        let task_block = ["- Update `README.md`."];
        let lines = affected_file_lines(&prompt_lines, &task_block);
        assert_eq!(
            lines,
            vec!["- `README.md`"],
            "collision fallback resolves to the wrong bullet line"
        );

        let prompt = format!(
            "## Files to Create or Modify\n\n- `docs/x/readme.md`\n- `README.md`\n\n## Tasks\n\n- [ ] **{}** — Update `README.md`.\n",
            tid(101)
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(101)),
            vec!["README.md"]
        );
    }

    // Mirror order: when `README.md` precedes `docs/x/readme.md` in the
    // Files section and the task names only `docs/x/readme.md`.
    #[test]
    fn affected_file_lines_resolves_exact_match_in_mirror_order() {
        let prompt_lines = [
            "## Files to Create or Modify",
            "",
            "- `README.md`",
            "- `docs/x/readme.md`",
        ];
        let task_block = ["- Update `docs/x/readme.md`."];
        let lines = affected_file_lines(&prompt_lines, &task_block);
        assert_eq!(
            lines,
            vec!["- `docs/x/readme.md`"],
            "mirror order resolves to correct bullet line"
        );

        let prompt = format!(
            "## Files to Create or Modify\n\n- `README.md`\n- `docs/x/readme.md`\n\n## Tasks\n\n- [ ] **{}** — Update `docs/x/readme.md`.\n",
            tid(102)
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(102)),
            vec!["docs/x/readme.md"]
        );
    }

    // An earlier bullet's OWN subject is a different file, but its post-dash
    // prose incidentally names the path a later task actually declares (e.g.
    // "pointers to `x.md`"). `.find()` must skip that incidental mention and
    // resolve to `x.md`'s own bullet.
    #[test]
    fn affected_file_lines_ignores_incidental_mention_in_earlier_bullets_prose() {
        let prompt_lines = [
            "## Files to Create or Modify",
            "",
            "- `skill.md` — rewritten with pointers to `manual.md` and to `architecture.md`.",
            "- `architecture.md` — one new subsection for the residual internals.",
        ];
        let task_block = ["- Absorb the residual internals. Files: `architecture.md`."];
        let lines = affected_file_lines(&prompt_lines, &task_block);
        assert_eq!(
            lines,
            vec!["- `architecture.md` — one new subsection for the residual internals."],
            "an incidental post-dash mention in an earlier bullet must not shadow the target's own bullet"
        );

        let prompt = format!(
            "## Files to Create or Modify\n\n- `skill.md` — rewritten with pointers to `manual.md` and to `architecture.md`.\n- `architecture.md` — one new subsection.\n\n## Tasks\n\n- [ ] **{}** — Absorb the residual internals.\n  - Files: `architecture.md`\n",
            tid(103)
        );
        assert_eq!(
            extract_affected_file_paths(&prompt, &tid(103)),
            vec!["architecture.md"]
        );
    }
}
