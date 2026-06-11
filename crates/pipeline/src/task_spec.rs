//! task_spec (R-02): port of `scripts/common/New-TaskSpec.ps1`.
//!
//! Assembles a compact, self-contained task spec from an execution prompt so a
//! secondary headless CLI has just enough context to act on one `T-NNN` task
//! without reading the full codebase. The "small-context" upgrade is captured by
//! two behaviors, both ported here:
//! - multi-line task block extraction (the whole `T-NNN` bullet incl. indented
//!   sub-bullets, up to the next top-level task), and
//! - per-task affected-file convergence (backtick-quoted paths named inside the
//!   task block, falling back to the full `## Files to Create or Modify` list).
//!
//! Extraction is pure (operates on the prompt text); git/clock/routing context is
//! passed in by the caller so the assembly stays deterministic and testable.

use std::collections::HashSet;

use crate::PipelineError;

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

fn task_block_lines<'a>(prompt_lines: &[&'a str], task_id: &str) -> Vec<&'a str> {
    let section = section_lines(prompt_lines, "Tasks");
    let start = match section.iter().position(|l| is_task_start(l, task_id)) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let mut block = Vec::new();
    for (i, &line) in section.iter().enumerate().skip(start) {
        if i > start && is_any_task_bullet(line) {
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

/// Backtick-quoted path-like tokens (contain `/` or `\`), deduped case-insensitively
/// in first-seen order. Only same-line quotes count (no newline between backticks).
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
                        && (candidate.contains('/') || candidate.contains('\\'))
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

fn write_back_instruction(phase: &str, prompt_path: &str) -> String {
    match phase.to_lowercase().as_str() {
        "test" => format!("Write test results to `## Test Results` in the file: {prompt_path}"),
        "audit" => format!("Write audit results to `## Review Results` in the file: {prompt_path}"),
        "verify" => format!("Write verification results to `## Analyze` in the file: {prompt_path}"),
        _ => "Write implementation code changes to the files listed in 'Affected Files'. \
              Do NOT modify any other files."
            .to_string(),
    }
}

/// Repo-relative agent contract path for a phase.
pub fn agent_contract_rel(phase: &str) -> &'static str {
    match phase.to_lowercase().as_str() {
        "test" => "plugins/gal-core/agents/golem-tester.agent.md",
        "audit" => "plugins/gal-core/agents/golem-auditor.agent.md",
        "verify" => "plugins/gal-core/agents/golem-verifier.agent.md",
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
    let task_goal = extract_task_goal(input.prompt_body, input.task_scope)?;
    let affected = extract_affected_files(input.prompt_body, input.task_scope);
    let affected_block = if affected.is_empty() {
        "(none specified)".to_string()
    } else {
        affected.join("\n")
    };
    let write_back = write_back_instruction(input.phase, input.prompt_path);
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
    md.push_str(&format!("# Task Spec: {} ({})\n\n", input.task_scope, input.phase));
    md.push_str(&format!("Generated: {}\n", input.generated));
    md.push_str(&format!("Git branch: {}  |  HEAD: {}\n", input.git_branch, input.git_head));
    md.push_str(&format!("Source prompt: {}\n\n", input.prompt_path));
    md.push_str("---\n\n");
    md.push_str("## Task Goal\n\n");
    md.push_str(&task_goal);
    md.push_str("\n\n## Affected Files\n\n");
    md.push_str(&affected_block);
    md.push_str("\n\n## Write-Back Instruction\n\n");
    md.push_str(&write_back);
    md.push_str("\n\n");
    if let Some(cb) = convention_block {
        md.push_str(&cb);
        md.push_str("\n\n");
    }
    md.push_str(&format!(
        "## Agent Contract\n\nFollow the instructions in: `{agent_rel}`\n\n"
    ));
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

    const PROMPT: &str = "\
# Plan Prompt

## Files to Create or Modify

- [CREATE] `crates/pipeline/src/lib.rs`: orchestration scaffold.
- [MODIFY] `crates/cli/src/main.rs`: wire `gal pipeline`.
- [MODIFY] `plugins/gal-core/agents/agents.md`: contract surface.

## Tasks

P0
- [x] **T-001** — Freeze fixtures.

- [ ] **T-003** — Port orchestration into `crates/pipeline/src/lib.rs`; parity.
  - sub-detail: compose `dispatch` and do not rebuild stage.
  - also touch `crates/cli/src/main.rs` later.
- [ ] **T-004** — Port task-spec (`New-TaskSpec`) into `pipeline`.

## Test Plan

| ID | Type |
";

    #[test]
    fn extracts_multiline_task_block_until_next_task() {
        let goal = extract_task_goal(PROMPT, "T-003").unwrap();
        assert!(goal.starts_with("- [ ] **T-003** — Port orchestration"));
        // Indented sub-bullets stay inside the block...
        assert!(goal.contains("sub-detail: compose `dispatch`"));
        assert!(goal.contains("also touch `crates/cli/src/main.rs` later."));
        // ...and the next top-level task is excluded.
        assert!(!goal.contains("T-004"));
    }

    #[test]
    fn per_task_affected_files_use_backtick_paths_from_block() {
        let files = extract_affected_files(PROMPT, "T-003");
        // Paths named in the T-003 block, deduped, matched to the Files section line.
        assert_eq!(files.len(), 2);
        assert!(files[0].contains("crates/pipeline/src/lib.rs"));
        assert!(files[1].contains("crates/cli/src/main.rs"));
        // agents.md is in Files section but NOT named by T-003 → excluded.
        assert!(files.iter().all(|f| !f.contains("agents.md")));
    }

    #[test]
    fn affected_files_fall_back_to_full_section_when_block_names_none() {
        let prompt = "\
## Files to Create or Modify

- `crates/base/src/lib.rs`
- `crates/dispatch/src/lib.rs`

## Tasks

- [ ] **T-009** — A task that names no backtick paths at all.
";
        let files = extract_affected_files(prompt, "T-009");
        assert_eq!(files.len(), 2);
        assert!(files[0].contains("crates/base/src/lib.rs"));
        assert!(files[1].contains("crates/dispatch/src/lib.rs"));
    }

    #[test]
    fn missing_task_is_task_not_found() {
        let err = extract_task_goal(PROMPT, "T-999").unwrap_err();
        assert_eq!(err, PipelineError::TaskNotFound("T-999".to_string()));
    }

    #[test]
    fn write_back_instruction_is_phase_specific() {
        assert!(write_back_instruction("test", "P").contains("## Test Results"));
        assert!(write_back_instruction("audit", "P").contains("## Review Results"));
        assert!(write_back_instruction("verify", "P").contains("## Analyze"));
        assert!(write_back_instruction("implement", "P").contains("Affected Files"));
        // Unknown phase falls back to the implement instruction.
        assert!(write_back_instruction("deploy", "P").contains("Affected Files"));
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
        let input = TaskSpecInput {
            task_scope: "T-003",
            phase: "implement",
            prompt_path: ".dev/plans/refactor-gal-xmachine-rust-port.prompt.md",
            prompt_body: PROMPT,
            generated: "2026-06-11 12:00",
            git_branch: "main",
            git_head: "deadbee",
            convention_hints: Some("conventions/rust.md; conventions/token-budget.md"),
        };
        let spec = assemble_task_spec(&input).unwrap();

        assert_eq!(spec.out_file_name, "T-003-implement.md");
        assert!(spec.markdown.starts_with("# Task Spec: T-003 (implement)"));
        assert!(spec.markdown.contains("## Task Goal"));
        assert!(spec.markdown.contains("Port orchestration"));
        assert!(spec.markdown.contains("## Affected Files"));
        assert!(spec.markdown.contains("crates/pipeline/src/lib.rs"));
        assert!(spec.markdown.contains("## Convention Files"));
        assert!(spec.markdown.contains("- conventions/rust.md"));
        assert!(spec.markdown.contains("golem-implementer.agent.md"));
        assert!(spec.markdown.contains("**DO NOT run `git commit` or `git push`.**"));
        assert!(!spec.exceeds_size_target);
    }

    #[test]
    fn assemble_omits_convention_block_when_no_hints() {
        let input = TaskSpecInput {
            task_scope: "T-003",
            phase: "audit",
            prompt_path: "P",
            prompt_body: PROMPT,
            generated: "G",
            git_branch: "main",
            git_head: "h",
            convention_hints: None,
        };
        let spec = assemble_task_spec(&input).unwrap();
        assert_eq!(spec.out_file_name, "T-003-audit.md");
        assert!(!spec.markdown.contains("## Convention Files"));
        // audit phase → review write-back + auditor contract.
        assert!(spec.markdown.contains("## Review Results"));
        assert!(spec.markdown.contains("golem-auditor.agent.md"));
    }
}
