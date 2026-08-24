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

fn write_back_instruction(phase: &str, prompt_path: &str, receipt_path: Option<&str>) -> String {
    let phase_lc = phase.to_lowercase();
    let base = match phase_lc.as_str() {
        "test" => format!("Write test results to `## Test Results` in the file: {prompt_path}"),
        "audit" => format!("Write audit results to `## Review Results` in the file: {prompt_path}"),
        _ => "Write implementation code changes to the files listed in 'Affected Files'. \
              Do NOT modify any other files."
            .to_string(),
    };
    // test/audit also write a verifiable receipt the pipeline checks (exists +
    // non-empty) to confirm the phase actually ran — the prompt-section append
    // alone is not a reliable completion signal.
    match (phase_lc.as_str(), receipt_path) {
        ("test" | "audit", Some(receipt)) => format!(
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
    /// or `None` to omit the receipt clause (e.g. implement, which the orchestrator
    /// verifies via a code-file receipt). For test/audit this is what the pipeline
    /// checks (`verify_receipt`: exists + non-empty) to confirm the phase ran.
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
    let write_back = write_back_instruction(input.phase, input.prompt_path, input.receipt_path);
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
    md.push_str(&format!("Source prompt: {}\n\n", input.prompt_path));
    md.push_str("---\n\n");
    md.push_str("## Task Goal\n\n");
    md.push_str(&task_goal);
    md.push_str("\n\n## Affected Files (allowlist — modify ONLY these)\n\n");
    md.push_str(&affected_block);
    md.push_str("\n\n## Write-Back Instruction\n\n");
    md.push_str(&write_back);
    md.push_str("\n\n");
    if let Some(cb) = convention_block {
        md.push_str(&cb);
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
        assert!(write_back_instruction("test", "P", None).contains("## Test Results"));
        assert!(write_back_instruction("audit", "P", None).contains("## Review Results"));
        assert!(write_back_instruction("implement", "P", None).contains("Affected Files"));
        // Unknown phase (incl. the removed `verify`) falls back to implement.
        assert!(write_back_instruction("verify", "P", None).contains("Affected Files"));
        assert!(write_back_instruction("deploy", "P", None).contains("Affected Files"));
    }

    #[test]
    fn write_back_instruction_names_receipt_for_test_and_audit() {
        let r = format!(".dev/pipeline/receipts/{}-test.receipt.md", tid(4));
        let r = r.as_str();
        let test_wb = write_back_instruction("test", "P", Some(r));
        assert!(test_wb.contains("## Test Results"));
        assert!(
            test_wb.contains(r),
            "test write-back must name the receipt path"
        );
        let audit_wb = write_back_instruction("audit", "P", Some(r));
        assert!(
            audit_wb.contains(r),
            "audit write-back must name the receipt path"
        );
        // implement ignores the receipt clause (orchestrator uses a code-file receipt).
        assert!(!write_back_instruction("implement", "P", Some(r)).contains(r));
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
