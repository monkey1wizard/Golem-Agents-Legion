//! `gal planning-check` - planning-stage structure receipt.

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use super::markdown_fence::{fence_run_length, MarkdownFence};
use gal_engine::ExitCode;
use std::path::{Path, PathBuf};

struct Args {
    target: PathBuf,
    receipt: PathBuf,
}

fn default_receipt_path(target: &Path) -> Result<PathBuf, String> {
    resolve_receipt_path(Some(target), None, "planning-check.receipt.md")
        .map_err(|error| format!("receipt scope resolution: {error}"))
}

fn parse_args(args: &[String]) -> Result<Args, String> {
    let mut target = None;
    let mut receipt = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--receipt" => {
                receipt = Some(PathBuf::from(it.next().ok_or("--receipt requires a path")?))
            }
            s if s.starts_with("--") => return Err(format!("unknown option '{s}'")),
            s => {
                if target.replace(PathBuf::from(s)).is_some() {
                    return Err(format!("unexpected extra argument '{s}'"));
                }
            }
        }
    }
    let target = target.ok_or("planning-check requires a <source-plan> path")?;
    let receipt = match receipt {
        Some(path) => path,
        None => default_receipt_path(&target)?,
    };
    Ok(Args { target, receipt })
}

const STANDARD_HEADINGS: &[&str] = &[
    "Approval",
    "Goal",
    "Open Questions",
    "Plan Chain",
    "Files to Create or Modify",
    "Requirements",
    "Diagrams",
    "Approach",
    "Preconditions",
    "Test Plan",
    "Tasks",
    "Verification Map",
    "Owner Acceptance",
    "Success Criteria",
    "Review Results",
    "Appendix",
];

const PLAN_TEMPLATE: &str = include_str!("../../../../plugins/gal-core/templates/plan.md");
const HISTORICAL_GUIDANCE: &[&str] = &[
    "PLAN CONTRACT A plan is a specification, not a changelog. Rewrite stale sections instead of adding process notes or correction history. Follow token-budget.md for language policy. Diagrams are optional for local changes and useful for structural, multi-file, or Protected-Path changes.",
    "Zone: Context | Diátaxis: explanation | Any producer may fill this zone. | Gate: gal planning-check validates required plan structure.",
];
const HISTORICAL_TASK_GUIDANCE: &str =
    "Each task's Change and Acceptance answer applicable questions in conventions/task-quality.md.";

#[derive(Default)]
struct MarkdownScan {
    lines: Vec<String>,
    headings: Vec<(usize, String)>,
    comments: Vec<(usize, String)>,
}

fn scan_markdown(text: &str) -> MarkdownScan {
    let mut scan = MarkdownScan::default();
    let mut fence: Option<(char, usize)> = None;
    let mut backtick_fence = MarkdownFence::new();
    let mut comment: Option<(usize, String)> = None;
    let lines: Vec<&str> = text.lines().collect();
    let mut inline_ticks = None;
    let mut indented_code_block = false;
    for (index, line) in lines.iter().enumerate() {
        let line = *line;
        let trimmed = line.trim_start();
        if let Some((ch, len)) = fence {
            if !is_indented_line(line) && markdown_fence_close(trimmed, ch, len) {
                if ch == '`' {
                    backtick_fence.feed(&"`".repeat(len));
                }
                fence = None;
            }
            debug_assert_eq!(
                backtick_fence.is_in_fence(),
                fence.is_some_and(|(open, _)| open == '`')
            );
            scan.lines.push(String::new());
            continue;
        }
        if comment.is_none() && !is_indented_line(line) {
            if let Some((ch, len, _)) = markdown_fence_open(trimmed) {
                if ch == '`' {
                    debug_assert_eq!(fence_run_length(trimmed), Some(len));
                    backtick_fence.feed(trimmed);
                }
                fence = Some((ch, len));
                debug_assert_eq!(backtick_fence.is_in_fence(), ch == '`');
                scan.lines.push(String::new());
                continue;
            }
        }
        let in_code_span_at_line_start = inline_ticks.is_some();
        let in_comment_at_line_start = comment.is_some();
        if indented_code_block {
            if line.trim().is_empty() || is_indented_line(line) {
                scan.lines.push(String::new());
                continue;
            }
            indented_code_block = false;
        }
        let indented_code = is_indented_code_block(&lines, index);
        if indented_code && !in_code_span_at_line_start && !in_comment_at_line_start {
            indented_code_block = true;
            scan.lines.push(String::new());
            continue;
        }
        let mut visible = String::new();
        let mut rest = line;
        while !rest.is_empty() {
            if let Some(len) = inline_ticks {
                if let Some(end) = matching_backtick_run(rest, len) {
                    rest = &rest[end..];
                    inline_ticks = None;
                } else {
                    rest = "";
                }
            } else if let Some((start, body)) = &mut comment {
                if let Some(end) = rest.find("-->") {
                    body.push_str(&rest[..end]);
                    scan.comments.push((*start, std::mem::take(body)));
                    comment = None;
                    rest = &rest[end + 3..];
                } else {
                    body.push_str(rest);
                    body.push('\n');
                    rest = "";
                }
            } else if rest.starts_with("<!--") && !is_escaped(line, line.len() - rest.len()) {
                comment = Some((index + 1, String::new()));
                rest = &rest[4..];
            } else {
                let Some(ch) = rest.chars().next() else {
                    break;
                };
                if ch == '`' {
                    let count = rest.bytes().take_while(|b| *b == b'`').count();
                    let offset = line.len() - rest.len();
                    if has_matching_backtick(&lines, index, offset, count) {
                        inline_ticks = Some(count);
                    } else {
                        visible.push_str(&rest[..count]);
                    }
                    rest = &rest[count..];
                } else {
                    visible.push(ch);
                    rest = &rest[ch.len_utf8()..];
                }
            }
        }
        if !in_code_span_at_line_start && !in_comment_at_line_start && !indented_code {
            if let Some(title) = h2_heading(line) {
                scan.headings.push((index, title));
            }
        }
        scan.lines.push(visible);
    }
    debug_assert_eq!(
        backtick_fence.is_unterminated(),
        fence.is_some_and(|(open, _)| open == '`')
    );
    scan
}

fn matching_backtick_run(text: &str, expected_len: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'`' {
            index += 1;
            continue;
        }
        let start = index;
        while index < bytes.len() && bytes[index] == b'`' {
            index += 1;
        }
        if index - start == expected_len {
            return Some(index);
        }
    }
    None
}

fn has_matching_backtick(lines: &[&str], line_index: usize, offset: usize, run_len: usize) -> bool {
    matching_backtick_run(&lines[line_index][offset + run_len..], run_len).is_some()
        || lines
            .iter()
            .skip(line_index + 1)
            .take_while(|line| !starts_markdown_block(line))
            .any(|line| matching_backtick_run(line, run_len).is_some())
}

fn starts_markdown_block(line: &str) -> bool {
    let trimmed = line.trim_start();
    line.trim().is_empty()
        || h2_heading(line).is_some()
        || (!is_indented_line(line) && markdown_fence_open(trimmed).is_some())
        || (line.len() - trimmed.len() <= 3 && trimmed.starts_with("<!--"))
}

fn is_indented_code_block(lines: &[&str], line_index: usize) -> bool {
    let line = lines[line_index];
    if !is_indented_line(line) {
        return false;
    }
    line_index == 0
        || lines[line_index - 1].trim().is_empty()
        || h2_heading(lines[line_index - 1]).is_some()
}

fn is_indented_line(line: &str) -> bool {
    let indentation = line
        .chars()
        .take_while(|character| matches!(character, ' ' | '\t'))
        .fold(0, |columns, character| match character {
            ' ' => columns + 1,
            '\t' => (columns + 4) & !3,
            _ => columns,
        });
    indentation >= 4
}

fn is_escaped(line: &str, offset: usize) -> bool {
    line.as_bytes()[..offset]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count()
        % 2
        == 1
}

fn h2_heading(line: &str) -> Option<String> {
    let indent = line.bytes().take_while(|byte| *byte == b' ').count();
    if indent > 3 {
        return None;
    }
    let content = &line[indent..];
    let hashes = content.bytes().take_while(|byte| *byte == b'#').count();
    if hashes != 2
        || content
            .as_bytes()
            .get(hashes)
            .is_some_and(|byte| !matches!(byte, b' ' | b'\t'))
    {
        return None;
    }

    let title = content[hashes..].trim_matches([' ', '\t']);
    let closing_hashes = title
        .char_indices()
        .rev()
        .take_while(|(_, ch)| *ch == '#')
        .last()
        .map(|(index, _)| index);
    let title = match closing_hashes {
        Some(index)
            if index > 0 && matches!(title[..index].chars().next_back(), Some(' ' | '\t')) =>
        {
            title[..index].trim_end_matches(|ch| matches!(ch, ' ' | '\t'))
        }
        _ => title,
    };
    Some(title.to_string())
}

fn section_text(text: &str, heading: &str) -> Option<String> {
    let scan = scan_markdown(text);
    let title = heading.strip_prefix("## ")?;
    let pos = scan.headings.iter().position(|(_, name)| name == title)?;
    let start = scan.headings[pos].0 + 1;
    let end = scan.headings.get(pos + 1).map_or(scan.lines.len(), |h| h.0);
    Some(scan.lines[start..end].join("\n"))
}

fn normalized_comment(body: &str) -> String {
    body.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn source_plan_shape_outcome(text: &str) -> CheckOutcome {
    let scan = scan_markdown(text);
    let guidance: Vec<String> = scan_markdown(PLAN_TEMPLATE)
        .comments
        .iter()
        .map(|(_, body)| normalized_comment(body))
        .chain(
            HISTORICAL_GUIDANCE
                .iter()
                .map(|body| normalized_comment(body)),
        )
        .chain(std::iter::once(normalized_comment(
            HISTORICAL_TASK_GUIDANCE,
        )))
        .collect();
    let mut problem: Option<(usize, String)> = None;
    let mut last = None;
    let mut seen = std::collections::HashSet::new();
    for (index, (line, title)) in scan.headings.iter().enumerate() {
        let reason = if index == 0 && title != "Approval" {
            Some("first heading must be Approval")
        } else if let Some(order) = STANDARD_HEADINGS.iter().position(|name| *name == title) {
            if !seen.insert(order) {
                Some("duplicate standard heading")
            } else if last.is_some_and(|previous| order < previous) {
                Some("standard heading out of template order")
            } else {
                last = Some(order);
                None
            }
        } else {
            None
        };
        if let Some(reason) = reason {
            problem = Some((line + 1, format!("heading `{title}`: {reason}")));
            break;
        }
    }
    for (line, body) in &scan.comments {
        let normalized = normalized_comment(body);
        if guidance.contains(&normalized) && problem.as_ref().is_none_or(|(first, _)| line < first)
        {
            let label = normalized.chars().take(60).collect::<String>();
            problem = Some((
                *line,
                format!("author guidance comment `{label}` must be removed"),
            ));
        }
    }
    match problem {
        Some((line, reason)) => outcome(
            "source-plan-shape",
            CheckState::Fail,
            format!("line {line}: {reason}"),
        ),
        None => outcome(
            "source-plan-shape",
            CheckState::Pass,
            "source headings and comments conform".to_string(),
        ),
    }
}

fn outcome(name: &str, state: CheckState, summary: String) -> CheckOutcome {
    CheckOutcome {
        name: name.to_string(),
        command: None,
        state,
        summary,
    }
}

fn naming_gate_outcome(repo_root: &Path) -> CheckOutcome {
    use crate::gal::naming_gate::{load_retired_terms, NamingGate};
    let retired = load_retired_terms(repo_root);
    match NamingGate::new(&retired) {
        Ok(gate) => match gate.scan_tree(repo_root) {
            Ok(hits) => {
                let total: usize = hits.iter().map(|f| f.violations.len()).sum();
                outcome(
                    "naming-gate",
                    if total == 0 {
                        CheckState::Pass
                    } else {
                        CheckState::Fail
                    },
                    format!("{total} violation(s) across {} file(s)", hits.len()),
                )
            }
            Err(err) => outcome(
                "naming-gate",
                CheckState::Fail,
                format!("scan failed: {err}"),
            ),
        },
        Err(err) => outcome(
            "naming-gate",
            CheckState::Fail,
            format!("scanner build failed: {err}"),
        ),
    }
}

pub(crate) fn required_sections_outcome(plan_text: &str) -> CheckOutcome {
    let required = [
        "## Goal",
        "## Requirements",
        "## Approach",
        "## Open Questions",
        "## Approval",
        "## Review Results",
    ];
    let missing: Vec<&str> = required
        .into_iter()
        .filter(|h| {
            !scan_markdown(plan_text)
                .headings
                .iter()
                .any(|(_, title)| *h == format!("## {title}"))
        })
        .collect();
    outcome(
        "required-sections",
        if missing.is_empty() {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        if missing.is_empty() {
            "all required planning sections are present".to_string()
        } else {
            format!("missing section(s): {}", missing.join(", "))
        },
    )
}

fn diagram_orientation_outcome(plan_text: &str) -> CheckOutcome {
    let lines: Vec<&str> = plan_text.lines().collect();
    let Some(section_start) = lines.iter().position(|line| line.trim() == "## Diagrams") else {
        return outcome(
            "diagram-orientation",
            CheckState::Pass,
            "no flowchart found in ## Diagrams".to_string(),
        );
    };
    let content_start = section_start + 1;
    let section_end = lines[content_start..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map(|offset| content_start + offset)
        .unwrap_or(lines.len());

    let mut mermaid_block_index = 0;
    let mut flowchart_count = 0;
    let mut failures = Vec::new();
    let mut index = content_start;
    while index < section_end {
        let trimmed = lines[index].trim_start();
        let Some((fence_char, fence_len, info)) = markdown_fence_open(trimmed) else {
            index += 1;
            continue;
        };
        if !info.eq_ignore_ascii_case("mermaid") {
            index += 1;
            while index < section_end {
                if markdown_fence_close(lines[index].trim_start(), fence_char, fence_len) {
                    index += 1;
                    break;
                }
                index += 1;
            }
            continue;
        }

        mermaid_block_index += 1;
        index += 1;
        while index < section_end
            && !markdown_fence_close(lines[index].trim_start(), fence_char, fence_len)
        {
            let declaration = lines[index].trim();
            if !declaration.is_empty() && !declaration.starts_with("%%") {
                let mut parts = declaration.split_whitespace();
                let kind = parts.next().unwrap_or_default();
                if kind == "flowchart" || kind == "graph" {
                    flowchart_count += 1;
                    if let Some(direction) = parts.next().map(|token| token.trim_end_matches(';')) {
                        if matches!(direction, "LR" | "RL" | "BT") {
                            failures.push(format!(
                                "block {mermaid_block_index}, declaration line {} uses {direction}",
                                index + 1
                            ));
                        }
                    }
                }
                break;
            }
            index += 1;
        }
        if index < section_end {
            index += 1;
        }
    }

    outcome(
        "diagram-orientation",
        if failures.is_empty() {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        if failures.is_empty() {
            if flowchart_count == 0 {
                "no flowchart found in ## Diagrams".to_string()
            } else {
                "no flowchart with a disallowed orientation found in ## Diagrams".to_string()
            }
        } else {
            format!("{}; use `flowchart TD`", failures.join("; "))
        },
    )
}

fn markdown_fence_open(line: &str) -> Option<(char, usize, &str)> {
    let fence_char = line.chars().next()?;
    if fence_char != '`' && fence_char != '~' {
        return None;
    }
    let fence_len = line.chars().take_while(|ch| *ch == fence_char).count();
    if fence_len < 3 {
        return None;
    }
    let info = line[fence_len..].trim();
    let language = info.split_whitespace().next().unwrap_or_default();
    Some((fence_char, fence_len, language))
}

fn markdown_fence_close(line: &str, fence_char: char, fence_len: usize) -> bool {
    let count = line.chars().take_while(|ch| *ch == fence_char).count();
    count >= fence_len && line[count..].trim().is_empty()
}

fn open_questions_outcome(plan_text: &str) -> CheckOutcome {
    let section = section_text(plan_text, "## Open Questions").unwrap_or_default();
    let pass = section.trim().starts_with("None");
    outcome(
        "open-questions-cleared",
        if pass {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        if pass {
            "Open Questions is `None`".to_string()
        } else {
            "Open Questions must be resolved before handoff".to_string()
        },
    )
}

/// True only when `marker` appears as a whole line (trimmed) outside any fenced
/// code block, so a prose mention or a `code-span` of the marker cannot spoof it.
fn has_standalone_marker(text: &str, marker: &str) -> bool {
    let scan = scan_markdown(text);
    let lines: Vec<&str> = text.lines().collect();
    scan.comments.iter().any(|(line, body)| {
        lines[*line - 1].trim() == marker
            && normalized_comment(body)
                == normalized_comment(marker.trim_start_matches("<!--").trim_end_matches("-->"))
    })
}

fn arch_review_outcome(plan_text: &str) -> CheckOutcome {
    let marker = has_standalone_marker(plan_text, "<!-- ARCH_REVIEW: CLEAR -->");
    let section = approval_section_text(plan_text).unwrap_or_default();
    let exemption = section.lines().map(str::trim).any(|line| {
        parse_approval_line(line).is_some_and(|(label, token)| {
            label == "Architect review"
                && token == "not-required"
                && line
                    .split_once(" — ")
                    .is_some_and(|(_, reason)| !reason.trim().is_empty())
        })
    });
    let clear = architect_review_token(plan_text).as_deref() == Some("clear") && marker;
    let pass = clear || (exemption && !marker);
    outcome("arch-review-clear", if pass { CheckState::Pass } else { CheckState::Fail },
        if clear { "ARCH_REVIEW clear marker present (standalone line)" } else if pass { "recorded architecture review exemption with reason" } else { "architecture review requires clear with a standalone marker or a reasoned exemption without a marker" }.to_string())
}

/// Fixed Approval field order with each field's closed token vocabulary.
const APPROVAL_FIELDS: [(&str, &[&str]); 4] = [
    ("Human approval", &["pending", "approved"]),
    (
        "Architect review",
        &["pending", "clear", "blocked", "not-required"],
    ),
    ("Design review", &["not-requested", "clear", "blocked"]),
    ("Business review", &["not-requested", "clear", "blocked"]),
];

/// Parse one Approval line as `- <label>: [<token>]` optionally followed by
/// ` — <non-empty prose>`. Returns `None` on any grammar deviation: missing
/// brackets, empty token, or a suffix that is not a well-formed reason tail.
fn parse_approval_line(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("- ")?;
    let (label, rest) = rest.split_once(": ")?;
    let rest = rest.strip_prefix('[')?;
    let close = rest.find(']')?;
    let token = &rest[..close];
    if token.is_empty() {
        return None;
    }
    let after = &rest[close + 1..];
    if !after.is_empty() {
        let reason = after.strip_prefix(" — ")?;
        if reason.trim().is_empty() {
            return None;
        }
    }
    Some((label.to_string(), token.to_string()))
}

/// Like `section_text(plan_text, "## Approval")`, but also stops at a
/// standalone `---` thematic-break line. Execution prompts place this
/// divider immediately after `## Approval` to separate stable planning
/// content from mutable execution state (see `## Status`); without this,
/// the divider itself is misread as a fifth Approval line.
fn approval_section_text(plan_text: &str) -> Option<String> {
    let section = section_text(plan_text, "## Approval")?;
    let end = section
        .lines()
        .position(|line| line.trim() == "---")
        .unwrap_or_else(|| section.lines().count());
    Some(section.lines().take(end).collect::<Vec<_>>().join("\n"))
}

fn approval_shape_outcome(plan_text: &str) -> CheckOutcome {
    let section = approval_section_text(plan_text).unwrap_or_default();
    let lines: Vec<&str> = section
        .lines()
        .map(str::trim)
        .filter(|l| l.starts_with("- "))
        .collect();

    let expected_order = APPROVAL_FIELDS
        .iter()
        .map(|(label, _)| *label)
        .collect::<Vec<_>>()
        .join(", ");

    if lines.len() != APPROVAL_FIELDS.len() {
        return outcome(
            "approval-shape",
            CheckState::Fail,
            format!(
                "expected exactly {} Approval line(s) in order [{expected_order}]; found {}",
                APPROVAL_FIELDS.len(),
                lines.len()
            ),
        );
    }

    for (line, (label, allowed)) in lines.iter().zip(APPROVAL_FIELDS.iter()) {
        let Some((parsed_label, token)) = parse_approval_line(line) else {
            return outcome(
                "approval-shape",
                CheckState::Fail,
                format!(
                    "`{label}` line does not match grammar `- <label>: [<token>]` (optionally ` — <reason>`): `{line}`"
                ),
            );
        };
        if parsed_label != *label {
            return outcome(
                "approval-shape",
                CheckState::Fail,
                format!(
                    "expected `{label}` at this position (fixed order: {expected_order}); found `{parsed_label}`"
                ),
            );
        }
        if !allowed.contains(&token.as_str()) {
            return outcome(
                "approval-shape",
                CheckState::Fail,
                format!(
                    "`{label}` token `{token}` is not in its closed set {{{}}}",
                    allowed.join("|")
                ),
            );
        }
    }

    outcome(
        "approval-shape",
        CheckState::Pass,
        "four ordered Approval fields conform to grammar and closed sets".to_string(),
    )
}

/// Extract the parsed `Architect review` token from `## Approval`, tolerating
/// any grammar or ordering defect (those are `approval-shape`'s job) by
/// returning `None` rather than panicking.
fn architect_review_token(plan_text: &str) -> Option<String> {
    let section = approval_section_text(plan_text)?;
    section
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with("<!--") && !l.starts_with("-->"))
        .find_map(|line| {
            let (label, token) = parse_approval_line(line)?;
            (label == "Architect review").then_some(token)
        })
}

/// Biconditional: `Architect review: [clear]` iff a standalone
/// `<!-- ARCH_REVIEW: CLEAR -->` marker exists. Independent of `arch-review-clear`,
/// which only checks the marker; this row catches the token and marker drifting
/// apart in either direction.
fn arch_review_consistency_outcome(plan_text: &str) -> CheckOutcome {
    let marker_present = has_standalone_marker(plan_text, "<!-- ARCH_REVIEW: CLEAR -->");
    let Some(token) = architect_review_token(plan_text) else {
        return outcome(
            "arch-review-consistency",
            CheckState::Fail,
            "`Architect review` token not found in `## Approval`".to_string(),
        );
    };
    let is_clear = token == "clear";
    let consistent = is_clear == marker_present;
    outcome(
        "arch-review-consistency",
        if consistent {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        if consistent {
            format!("Architect review token `{token}` and ARCH_REVIEW marker agree")
        } else if is_clear {
            "Architect review is `[clear]` but no standalone ARCH_REVIEW clear marker is present"
                .to_string()
        } else {
            format!(
                "Architect review token `{token}` is not `[clear]` but a standalone ARCH_REVIEW clear marker is present"
            )
        },
    )
}

fn run_checks(plan_text: &str, repo_root: &Path) -> Vec<CheckOutcome> {
    vec![
        source_plan_shape_outcome(plan_text),
        open_questions_outcome(plan_text),
        required_sections_outcome(plan_text),
        diagram_orientation_outcome(plan_text),
        approval_shape_outcome(plan_text),
        arch_review_outcome(plan_text),
        arch_review_consistency_outcome(plan_text),
        naming_gate_outcome(repo_root),
    ]
}

fn render_receipt(receipt: &Receipt) -> String {
    let mut out = String::from("# planning-check receipt\n\n");
    out.push_str(&format!(
        "overall: {}\n\n",
        if receipt.passed() { "pass" } else { "fail" }
    ));
    out.push_str("| check | state | command | summary |\n| --- | --- | --- | --- |\n");
    for check in &receipt.checks {
        out.push_str(&format!(
            "| {} | {} | {} | {} |\n",
            check.name,
            check.state.as_str(),
            check.command.as_deref().unwrap_or("-"),
            check.summary
        ));
    }
    out
}

pub(crate) fn cmd_planning_check(args: &[String]) -> ExitCode {
    let rest: Vec<String> = args.iter().skip(1).cloned().collect();
    let parsed = match parse_args(&rest) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("gal planning-check: {err}");
            return if err.starts_with("receipt scope resolution:") {
                ExitCode::Error
            } else {
                ExitCode::Usage
            };
        }
    };
    if !parsed.target.exists() {
        eprintln!(
            "gal planning-check: source plan not found: {}",
            parsed.target.display()
        );
        return ExitCode::Usage;
    }
    let repo_root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let raw = std::fs::read(&parsed.target).unwrap_or_default();
    let plan_text = String::from_utf8(raw.clone()).unwrap_or_default();
    let receipt = Receipt {
        checks: run_checks(&plan_text, &repo_root),
    };

    let body = match super::finalize_check::receipt_envelope(None, Some(&parsed.target)) {
        Ok(envelope) => {
            let rendered = format!(
                "{}{}",
                render_receipt(&receipt),
                super::finalize_check::render_repair_dispositions_for_target(
                    &receipt.checks,
                    "planning",
                    "default",
                    &parsed.target,
                    &raw,
                    None
                )
            );
            match super::finalize_check::bind_receipt(rendered, &envelope) {
                Ok(body) => body,
                Err(err) => {
                    eprintln!("gal planning-check: invalid execution binding: {err}");
                    return ExitCode::Error;
                }
            }
        }
        Err(err) => {
            eprintln!("gal planning-check: cannot establish execution identity: {err}");
            return ExitCode::Error;
        }
    };
    if let Err(err) = super::report_capture::write_gate_receipt(
        "planning",
        "default",
        &parsed.receipt,
        &body,
        || {
            if let Some(parent) = parsed.receipt.parent() {
                if let Err(err) = std::fs::create_dir_all(parent) {
                    eprintln!(
                        "gal planning-check: cannot create receipt dir {}: {err}",
                        parent.display()
                    );
                    return Err(err);
                }
            }
            std::fs::write(&parsed.receipt, &body)
        },
    ) {
        eprintln!(
            "gal planning-check: cannot write receipt {}: {err}",
            parsed.receipt.display()
        );
        return ExitCode::Error;
    }
    println!(
        "gal planning-check: {} ({} check(s)) -> {}",
        if receipt.passed() { "pass" } else { "fail" },
        receipt.checks.len(),
        parsed.receipt.display()
    );
    receipt.exit_code()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_orientation_passes(plan: &str) -> CheckOutcome {
        let result = diagram_orientation_outcome(plan);
        assert_eq!(result.name, "diagram-orientation", "{result:?}");
        assert_eq!(result.state, CheckState::Pass, "{result:?}");
        result
    }

    fn assert_orientation_fails(plan: &str) -> CheckOutcome {
        let result = diagram_orientation_outcome(plan);
        assert_eq!(result.name, "diagram-orientation", "{result:?}");
        assert_eq!(result.state, CheckState::Fail, "{result:?}");
        assert!(result.summary.contains("block 1"), "{result:?}");
        assert!(result.summary.contains("declaration line 4"), "{result:?}");
        assert!(result.summary.contains("flowchart TD"), "{result:?}");
        result
    }

    #[test]
    fn diagram_orientation_accepts_top_down_flowchart() {
        assert_orientation_passes("## Diagrams\n\n```mermaid\nflowchart TD\n```");
    }

    #[test]
    fn diagram_orientation_accepts_top_bottom_flowchart() {
        assert_orientation_passes("## Diagrams\n\n```mermaid\nflowchart TB\n```");
    }

    #[test]
    fn diagram_orientation_accepts_flowchart_without_direction() {
        assert_orientation_passes("## Diagrams\n\n```mermaid\nflowchart\n```");
    }

    #[test]
    fn diagram_orientation_rejects_left_right_flowchart() {
        assert_orientation_fails("## Diagrams\n\n```mermaid\nflowchart LR\n```");
    }

    #[test]
    fn diagram_orientation_rejects_bottom_top_graph() {
        assert_orientation_fails("## Diagrams\n\n```mermaid\ngraph BT\n```");
    }

    #[test]
    fn diagram_orientation_accepts_plan_without_diagrams_section() {
        let result = assert_orientation_passes("## Goal\nA plan without diagrams.\n");
        assert!(result.summary.contains("no flowchart"), "{result:?}");
    }

    #[test]
    fn diagram_orientation_ignores_non_flowchart_mermaid() {
        assert_orientation_passes("## Diagrams\n\n```mermaid\nsequenceDiagram\n```");
    }

    #[test]
    fn diagram_orientation_ignores_flowchart_outside_diagrams_section() {
        assert_orientation_passes(
            "```mermaid\nflowchart LR\n```\n\n## Diagrams\nNo diagram here.\n",
        );
    }

    #[test]
    fn diagram_orientation_rejects_semicolon_after_direction() {
        assert_orientation_fails("## Diagrams\n\n```mermaid\nflowchart LR;\n```");
    }

    #[test]
    fn default_receipt_path_is_plan_scoped_and_explicit_override_wins() {
        let target = Path::new(".dev/plans/planning-scope.md");
        assert_eq!(
            default_receipt_path(target).unwrap(),
            default_receipt_path(target).unwrap(),
        );
        let parsed = parse_args(&[
            target.display().to_string(),
            "--receipt".to_string(),
            "custom/receipt.md".to_string(),
        ])
        .unwrap();
        assert_eq!(parsed.receipt, PathBuf::from("custom/receipt.md"));
    }
    use tempfile::TempDir;

    fn temp_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(
            tmp.path().join("docs").join("glossary.md"),
            "# RETIRED-TERMS:BEGIN\n# RETIRED-TERMS:END\n",
        )
        .unwrap();
        // naming-gate's scan_tree is git-proven: it must be a git repo, or the
        // scan fails closed instead of reporting an empty-clean result.
        let status = std::process::Command::new("git")
            .arg("-C")
            .arg(tmp.path())
            .arg("init")
            .arg("--quiet")
            .status()
            .expect("git init spawns");
        assert!(status.success(), "git init must succeed");
        tmp
    }

    fn clean_plan() -> String {
        "# Plan\n\n## Approval\n- Human approval: [approved]\n- Architect review: [clear]\n- Design review: [not-requested]\n- Business review: [not-requested]\n\n## Goal\n目標\n\n## Open Questions\nNone.\n\n## Requirements\n- 條件\n\n## Approach\n做法\n\n## Review Results\n### Architecture Review\nAPPROVE\n\n<!-- ARCH_REVIEW: CLEAR -->\n".to_string()
    }

    #[test]
    fn source_shape_rejects_order_duplicates_and_guidance() {
        for (plan, expected) in [
            ("## Goal\n## Approval\n", "first heading"),
            (
                "## Approval\n## Requirements\n## Goal\n",
                "out of template order",
            ),
            ("## Approval\n## Goal\n## Goal\n", "duplicate"),
        ] {
            let result = source_plan_shape_outcome(plan);
            assert_eq!(result.state, CheckState::Fail, "{result:?}");
            assert!(result.summary.contains(expected), "{result:?}");
            assert!(result.summary.contains("line "), "{result:?}");
        }
        for (_, body) in scan_markdown(PLAN_TEMPLATE).comments {
            let plan = format!("## Approval\n<!-- {body} -->\n");
            let result = source_plan_shape_outcome(&plan);
            assert_eq!(result.state, CheckState::Fail, "{result:?}");
            assert!(result.summary.contains("line 2"), "{result:?}");
        }
        let historical = format!("## Approval\n<!-- {HISTORICAL_TASK_GUIDANCE} -->");
        assert_eq!(
            source_plan_shape_outcome(&historical).state,
            CheckState::Fail
        );
    }

    #[test]
    fn source_shape_recognizes_only_real_atx_headings() {
        for heading in ["## Approval#", "<!-- ordinary -->## Approval"] {
            let result = source_plan_shape_outcome(&format!("{heading}\n## Goal\n"));
            assert_eq!(result.state, CheckState::Fail, "{result:?}");
            assert!(result.summary.contains("first heading"), "{result:?}");
        }

        assert_eq!(h2_heading("##\tApproval").as_deref(), Some("Approval"));
        assert_eq!(h2_heading("  ## Approval ###").as_deref(), Some("Approval"));
        assert_eq!(h2_heading("## Approval#").as_deref(), Some("Approval#"));
        assert_eq!(h2_heading("\t## Approval"), None);
    }

    #[test]
    fn source_shape_ignores_literal_comment_examples() {
        let guidance = HISTORICAL_TASK_GUIDANCE;
        let unmatched = format!("## Approval\n`literal\n<!-- {guidance} -->\n");
        assert_eq!(
            source_plan_shape_outcome(&unmatched).state,
            CheckState::Fail
        );

        let paragraph_boundary = "## Approval\nUnmatched ` delimiter\n\n## Goal\n";
        assert_eq!(
            source_plan_shape_outcome(paragraph_boundary).state,
            CheckState::Pass
        );

        let multiline = format!("## Approval\nExample ``\nliteral <!-- {guidance} -->\n``.\n");
        assert_eq!(
            source_plan_shape_outcome(&multiline).state,
            CheckState::Pass
        );

        let html_block = format!("## Approval\nExample ``\n<!-- {guidance} -->\n``.\n");
        assert_eq!(
            source_plan_shape_outcome(&html_block).state,
            CheckState::Fail
        );

        let indented = format!("## Approval\n\n    <!-- {guidance} -->\n");
        assert_eq!(source_plan_shape_outcome(&indented).state, CheckState::Pass);

        let indented_continuation =
            format!("## Approval\n\n    example\n    <!-- {guidance} -->\n");
        assert_eq!(
            source_plan_shape_outcome(&indented_continuation).state,
            CheckState::Pass
        );
        assert!(scan_markdown(&indented_continuation).comments.is_empty());

        let paragraph_indentation = format!("## Approval\nExample text\n    <!-- {guidance} -->\n");
        assert_eq!(
            source_plan_shape_outcome(&paragraph_indentation).state,
            CheckState::Fail
        );

        let escaped = format!("## Approval\n\\<!-- {guidance} -->\n");
        assert_eq!(source_plan_shape_outcome(&escaped).state, CheckState::Pass);

        let heading_boundary = "## Approval\nUnmatched ` delimiter\n## Goal\ntext `\n";
        assert_eq!(
            source_plan_shape_outcome(heading_boundary).state,
            CheckState::Pass
        );
    }

    #[test]
    fn indented_fence_markers_do_not_hide_source_violations() {
        for marker in ["```", "~~~"] {
            for indent in ["    ", "\t", "  \t"] {
                let prefix = format!("## Approval\n## Goal\n\n{indent}{marker}\n");
                let duplicate = format!("{prefix}## Goal\n{marker}\n");
                let result = source_plan_shape_outcome(&duplicate);
                assert_eq!(result.state, CheckState::Fail, "{result:?}");
                assert!(
                    result.summary.contains("line 5: heading `Goal`: duplicate"),
                    "{result:?}"
                );

                let guidance = format!("{prefix}<!-- {HISTORICAL_TASK_GUIDANCE} -->\n{marker}\n");
                let result = source_plan_shape_outcome(&guidance);
                assert_eq!(result.state, CheckState::Fail, "{result:?}");
                assert!(
                    result.summary.contains("line 5: author guidance"),
                    "{result:?}"
                );
            }
        }
    }

    #[test]
    fn indented_fence_marker_does_not_close_a_real_fence() {
        for marker in ["```", "~~~"] {
            let plan = format!("## Approval\n## Goal\n{marker}\n    {marker}\n## Goal\n<!-- {HISTORICAL_TASK_GUIDANCE} -->\n{marker}\n");
            assert_eq!(source_plan_shape_outcome(&plan).state, CheckState::Pass);
        }
    }

    #[test]
    fn source_shape_accepts_examples_custom_headings_and_optional_omissions() {
        let plan = format!(
            "## Approval\r\n<!-- Zone: UTC -->\r\n<!-- PLAN CONTRACT commentary -->\r\n## Custom\r\n`<!-- {HISTORICAL_TASK_GUIDANCE} -->`\r\n~~~\r\n## Goal\r\n<!-- {HISTORICAL_TASK_GUIDANCE} -->\r\n~~~\r\n## Goal\r\n## Requirements\r\n## Approach\r\n"
        );
        assert_eq!(source_plan_shape_outcome(&plan).state, CheckState::Pass);
    }

    #[test]
    fn fenced_sections_and_approval_examples_are_inert() {
        let plan = clean_plan().replace("## Goal\n目標", "~~~\n## Goal\n目標\n~~~");
        assert_eq!(required_sections_outcome(&plan).state, CheckState::Fail);
        let plan = format!("```\n## Approval\n- invalid\n```\n{}", clean_plan());
        assert_eq!(approval_shape_outcome(&plan).state, CheckState::Pass);
        assert_eq!(
            arch_review_consistency_outcome(&plan).state,
            CheckState::Pass
        );
    }

    #[test]
    fn architecture_disposition_requires_marker_or_reasoned_exemption() {
        let clear = clean_plan();
        assert_eq!(arch_review_outcome(&clear).state, CheckState::Pass);
        let exempt = clear
            .replace("[clear]", "[not-required] — local change")
            .replace("<!-- ARCH_REVIEW: CLEAR -->", "");
        let result = arch_review_outcome(&exempt);
        assert_eq!(result.state, CheckState::Pass);
        assert!(result.summary.contains("exemption"));
        for plan in [
            exempt.replace(" — local change", ""),
            clear.replace("<!-- ARCH_REVIEW: CLEAR -->", ""),
            clear.replace("[clear]", "[not-required] — local change"),
            exempt.replace("[not-required] — local change", "[pending]"),
            exempt.replace("[not-required] — local change", "[blocked]"),
        ] {
            assert_eq!(arch_review_outcome(&plan).state, CheckState::Fail, "{plan}");
        }
        assert!(!has_standalone_marker(
            "~~~\n<!-- ARCH_REVIEW: CLEAR -->\n~~~",
            "<!-- ARCH_REVIEW: CLEAR -->"
        ));
        assert!(!has_standalone_marker(
            "<!-- outer\n<!-- ARCH_REVIEW: CLEAR -->\n",
            "<!-- ARCH_REVIEW: CLEAR -->"
        ));
    }

    #[test]
    fn author_guidance_inventory_matches_template() {
        let comments = scan_markdown(PLAN_TEMPLATE).comments;
        assert_eq!(
            comments.len(),
            19,
            "update the guidance inventory when the template changes"
        );
        let inventory = comments
            .iter()
            .map(|(_, body)| format!("{}\n", normalized_comment(body)))
            .collect::<String>();
        let hash = inventory
            .bytes()
            .fold(14695981039346656037u64, |value, byte| {
                (value ^ u64::from(byte)).wrapping_mul(1099511628211)
            });
        assert_eq!(hash, 16937721696681563785);
        for body in HISTORICAL_GUIDANCE {
            let plan = format!("## Approval\n<!-- {body} -->");
            assert_eq!(source_plan_shape_outcome(&plan).state, CheckState::Fail);
        }
        assert_eq!(h2_headings(PLAN_TEMPLATE), STANDARD_HEADINGS);
    }

    #[test]
    fn clean_plan_passes() {
        let repo = temp_repo();
        let receipt = Receipt {
            checks: run_checks(&clean_plan(), repo.path()),
        };
        assert!(receipt.passed(), "{:#?}", receipt.checks);
    }

    #[test]
    fn naming_gate_outcome_fails_closed_outside_git_repo() {
        // No `git init`: scan_tree cannot prove a clean result and must fail
        // the check, not report a false Pass.
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("docs")).unwrap();
        std::fs::write(
            tmp.path().join("docs").join("glossary.md"),
            "# RETIRED-TERMS:BEGIN\n# RETIRED-TERMS:END\n",
        )
        .unwrap();
        let result = naming_gate_outcome(tmp.path());
        assert_eq!(result.state, CheckState::Fail);
        assert!(
            result.summary.contains("scan failed"),
            "{:?}",
            result.summary
        );
    }
    #[test]
    fn open_questions_not_none_fails() {
        let repo = temp_repo();
        let plan = clean_plan().replace("None.", "- [ ] OQ-01 [H] - unresolved");
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "open-questions-cleared" && c.state == CheckState::Fail));
    }
    #[test]
    fn missing_section_fails() {
        let repo = temp_repo();
        let plan = clean_plan().replace("## Requirements\n- 條件\n\n", "");
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "required-sections" && c.state == CheckState::Fail));
    }
    #[test]
    fn missing_arch_review_marker_fails() {
        let repo = temp_repo();
        let plan = clean_plan().replace("\n<!-- ARCH_REVIEW: CLEAR -->\n", "\n");
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path()),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "arch-review-clear" && c.state == CheckState::Fail));
    }
    fn valid_approval_block() -> String {
        "## Approval\n\n- Human approval: [approved]\n- Architect review: [clear]\n- Design review: [not-requested]\n- Business review: [not-requested]\n\n## Review Results\n".to_string()
    }

    #[test]
    fn approval_shape_valid_block_passes() {
        let out = approval_shape_outcome(&valid_approval_block());
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn approval_shape_not_required_with_reason_passes() {
        let plan = valid_approval_block().replace(
            "- Architect review: [clear]",
            "- Architect review: [not-required] — no code changes in this plan",
        );
        let out = approval_shape_outcome(&plan);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn approval_shape_unknown_token_fails() {
        let plan = valid_approval_block().replace(
            "- Architect review: [clear]",
            "- Architect review: [approved]",
        );
        let out = approval_shape_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
        assert!(out.summary.contains("closed set"), "{out:?}");
    }

    #[test]
    fn approval_shape_human_clear_fails() {
        let plan = valid_approval_block()
            .replace("- Human approval: [approved]", "- Human approval: [clear]");
        let out = approval_shape_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
        assert!(out.summary.contains("closed set"), "{out:?}");
    }

    #[test]
    fn approval_shape_old_spaced_spelling_fails() {
        let plan = valid_approval_block().replace(
            "- Design review: [not-requested]",
            "- Design review: [not requested]",
        );
        let out = approval_shape_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
        assert!(out.summary.contains("closed set"), "{out:?}");
    }

    #[test]
    fn approval_shape_old_three_line_shape_fails() {
        let plan = "## Approval\n\n- Human approval: [approved]\n- Architect review: [clear]\n- Additional domain review: [not requested]\n\n## Review Results\n";
        let out = approval_shape_outcome(plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn approval_shape_wrong_order_fails() {
        let plan = "## Approval\n\n- Architect review: [clear]\n- Human approval: [approved]\n- Design review: [not-requested]\n- Business review: [not-requested]\n\n## Review Results\n";
        let out = approval_shape_outcome(plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
        assert!(out.summary.contains("fixed order"), "{out:?}");
    }

    #[test]
    fn approval_shape_missing_section_fails() {
        let out = approval_shape_outcome("## Goal\nNo approval section here.\n");
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn approval_shape_bare_date_suffix_fails() {
        let plan = valid_approval_block().replace(
            "- Human approval: [approved]",
            "- Human approval: [approved] 2026-07-29",
        );
        let out = approval_shape_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn approval_shape_unbracketed_verdict_fails() {
        let plan = valid_approval_block()
            .replace("- Architect review: [clear]", "- Architect review: clear");
        let out = approval_shape_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn approval_shape_em_dash_reason_passes() {
        let plan = valid_approval_block().replace(
            "- Business review: [not-requested]",
            "- Business review: [not-requested] — no business rule changes",
        );
        let out = approval_shape_outcome(&plan);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    fn read_template(relative_path: &str) -> String {
        let manifest_dir = env!("CARGO_MANIFEST_DIR");
        std::fs::read_to_string(format!("{manifest_dir}/../../{relative_path}"))
            .unwrap_or_else(|e| panic!("template {relative_path} must be readable: {e}"))
    }

    fn h2_headings(text: &str) -> Vec<&str> {
        text.lines()
            .filter_map(|line| line.strip_prefix("## "))
            .map(|heading| {
                let two_spaces = heading.find("  ");
                let annotation = heading.find(" (");
                let end = match (two_spaces, annotation) {
                    (Some(left), Some(right)) => left.min(right),
                    (Some(index), None) | (None, Some(index)) => index,
                    (None, None) => heading.len(),
                };
                heading[..end].trim_end()
            })
            .collect()
    }

    #[test]
    fn plan_template_uses_five_zone_heading_order() {
        let text = read_template("plugins/gal-core/templates/plan.md");
        assert_eq!(
            h2_headings(&text),
            vec![
                "Approval",
                "Goal",
                "Open Questions",
                "Plan Chain",
                "Files to Create or Modify",
                "Requirements",
                "Diagrams",
                "Approach",
                "Preconditions",
                "Test Plan",
                "Tasks",
                "Verification Map",
                "Owner Acceptance",
                "Success Criteria",
                "Review Results",
                "Appendix",
            ]
        );
    }

    #[test]
    fn plan_prompt_h2_sequence_matches_embedded_skeleton() {
        let prompt = read_template("plugins/gal-core/templates/plan-prompt.md");
        let skill = read_template("plugins/gal-core/commands/plan-to-prompt/SKILL.template.md");
        let template = skill
            .split("### Execution-Prompt Template")
            .nth(1)
            .and_then(|section| section.split("```markdown").nth(1))
            .and_then(|section| section.split("```").next())
            .expect("embedded execution-prompt skeleton must be present");

        assert_eq!(h2_headings(&prompt), h2_headings(template));
    }

    #[test]
    fn template_pin_plan_md_passes_approval_shape() {
        let text = read_template("plugins/gal-core/templates/plan.md");
        let out = approval_shape_outcome(&text);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn template_pin_plan_prompt_md_passes_approval_shape() {
        let text = read_template("plugins/gal-core/templates/plan-prompt.md");
        assert!(
            scan_markdown(&text)
                .headings
                .iter()
                .any(|(_, heading)| heading == "Approval"),
            "parsed headings: {:#?}",
            scan_markdown(&text).headings
        );
        let out = approval_shape_outcome(&text);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn template_pin_corrupted_plan_md_fails_approval_shape() {
        let text = read_template("plugins/gal-core/templates/plan.md");
        let corrupted = text.replacen(
            "- Human approval: [pending]",
            "- Human approval: [pendin]",
            1,
        );
        assert_ne!(corrupted, text, "corruption must actually change the text");
        let out = approval_shape_outcome(&corrupted);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    fn plan_with_marker(approval_block: &str, marker_present: bool) -> String {
        let marker = if marker_present {
            "\n<!-- ARCH_REVIEW: CLEAR -->\n"
        } else {
            "\n"
        };
        format!("{approval_block}{marker}")
    }

    #[test]
    fn arch_review_consistency_clear_with_marker_passes() {
        let plan = plan_with_marker(&valid_approval_block(), true);
        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn arch_review_consistency_clear_without_marker_fails() {
        let plan = plan_with_marker(&valid_approval_block(), false);
        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn arch_review_marker_in_indented_code_continuations_fails() {
        let marker = "<!-- ARCH_REVIEW: CLEAR -->";
        let continuations = [
            ("four spaces", format!("    {marker}")),
            ("after blank line", format!("\n    {marker}")),
            ("tab", format!("\t{marker}")),
            ("two spaces and tab", format!("  \t{marker}")),
        ];

        for (label, continuation) in continuations {
            let plan = format!(
                "{}\n## Example\n\n    literal example\n{continuation}\n",
                valid_approval_block()
            );
            let out = arch_review_consistency_outcome(&plan);
            assert_eq!(out.state, CheckState::Fail, "{label}: {out:?}");
        }
    }

    #[test]
    fn arch_review_marker_after_indented_code_block_passes() {
        let plan = format!(
            "{}\n## Example\n\n    literal example\n\n<!-- ARCH_REVIEW: CLEAR -->\n",
            valid_approval_block()
        );

        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn arch_review_consistency_marker_without_clear_fails() {
        let block = valid_approval_block().replace(
            "- Architect review: [clear]",
            "- Architect review: [blocked]",
        );
        let plan = plan_with_marker(&block, true);
        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    // `[blocked]` without the marker passes.
    #[test]
    fn arch_review_consistency_blocked_without_marker_passes() {
        let block = valid_approval_block().replace(
            "- Architect review: [clear]",
            "- Architect review: [blocked]",
        );
        let plan = plan_with_marker(&block, false);
        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    // `[not-required]` without the marker passes.
    #[test]
    fn arch_review_consistency_not_required_without_marker_passes() {
        let block = valid_approval_block().replace(
            "- Architect review: [clear]",
            "- Architect review: [not-required] — no code changes in this plan",
        );
        let plan = plan_with_marker(&block, false);
        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn standalone_marker_line_passes() {
        assert!(has_standalone_marker(
            "intro\n<!-- ARCH_REVIEW: CLEAR -->\nmore",
            "<!-- ARCH_REVIEW: CLEAR -->"
        ));
    }

    #[test]
    fn marker_in_prose_does_not_pass() {
        // Mentioned mid-sentence, or inside a fenced block, must not pass.
        let prose = "We already wrote the <!-- ARCH_REVIEW: CLEAR --> marker last week.\n";
        assert!(!has_standalone_marker(prose, "<!-- ARCH_REVIEW: CLEAR -->"));
        let fenced = "```\n<!-- ARCH_REVIEW: CLEAR -->\n```\n";
        assert!(!has_standalone_marker(
            fenced,
            "<!-- ARCH_REVIEW: CLEAR -->"
        ));
    }

    #[test]
    fn has_standalone_marker_fence_nesting() {
        let marker = "<!-- ARCH_REVIEW: CLEAR -->";
        let inside_nested = format!("````\n```\n{marker}\n````\n");
        assert!(
            !has_standalone_marker(&inside_nested, marker),
            "planning_check fence sites flag an unterminated fence: inner marker in nested fence must be ignored"
        );
        let post_nested = format!("````\n```\ninner\n````\n{marker}\n");
        assert!(
            has_standalone_marker(&post_nested, marker),
            "planning_check fence sites flag an unterminated fence: post-fence marker must not be dropped"
        );
    }

    /// Covers a genuinely odd fence-marker count, distinct from the 4-over-3
    /// nesting `_fence_nesting` already covers. By CommonMark semantics a
    /// single unclosed fence extends to EOF, so the marker inside it is
    /// correctly never seen — this locks that as intended, not a silent drop.
    #[test]
    fn has_standalone_marker_unterminated_fence() {
        let marker = "<!-- ARCH_REVIEW: CLEAR -->";
        let doc = format!("```\n{marker}\nmore content after the never-closed fence\n");
        assert!(
            !has_standalone_marker(&doc, marker),
            "a marker inside a genuinely unterminated fence stays fenced (extends to EOF), never matched"
        );
    }
}
