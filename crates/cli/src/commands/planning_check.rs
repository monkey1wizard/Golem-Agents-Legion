//! `gal planning-check` - planning-stage structure receipt.

use super::dispatch::resolve_receipt_path;
use super::finalize_check::{CheckOutcome, CheckState, Receipt};
use super::planning_authority::{parse_localized_meta, rendered_source_hash, LocalizedMeta};
use gal_engine::ExitCode;
use serde_json::Value;
use std::path::{Path, PathBuf};

struct Args {
    target: PathBuf,
    receipt: PathBuf,
}

fn default_receipt_path(target: &Path) -> PathBuf {
    resolve_receipt_path(Some(target), "planning-check.receipt.md")
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
    let receipt = receipt.unwrap_or_else(|| default_receipt_path(&target));
    Ok(Args { target, receipt })
}

fn section_text(text: &str, heading: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let idx = lines.iter().position(|line| line.trim() == heading)?;
    let start = idx + 1;
    let end = lines[start..]
        .iter()
        .position(|line| line.starts_with("## "))
        .map(|offset| start + offset)
        .unwrap_or(lines.len());
    Some(lines[start..end].join("\n"))
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

fn required_sections_outcome(plan_text: &str) -> CheckOutcome {
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
        .filter(|h| !plan_text.contains(h))
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
    let mut in_fence = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if !in_fence && trimmed == marker {
            return true;
        }
    }
    false
}

fn arch_review_outcome(plan_text: &str) -> CheckOutcome {
    let pass = has_standalone_marker(plan_text, "<!-- ARCH_REVIEW: CLEAR -->");
    outcome(
        "arch-review-clear",
        if pass {
            CheckState::Pass
        } else {
            CheckState::Fail
        },
        if pass {
            "ARCH_REVIEW clear marker present (standalone line)".to_string()
        } else {
            "missing standalone ARCH_REVIEW clear marker".to_string()
        },
    )
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

fn configured_plan_language(config_path: Option<&Path>) -> Option<String> {
    let path = config_path?;
    let text = std::fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    value
        .get("planLanguage")
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn is_cjk(ch: char) -> bool {
    ('\u{3400}'..='\u{4DBF}').contains(&ch)
        || ('\u{4E00}'..='\u{9FFF}').contains(&ch)
        || ('\u{F900}'..='\u{FAFF}').contains(&ch)
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk)
}

// Language gate calibration. A whole-plan Latin-letter ratio is unusable: a
// genuine, heavily-technical zh-TW plan sits near 0.76 Latin at the letter level
// (dense inline English terms like reconcile/hash/metadata), so a 60% letter
// threshold false-positives the very plans it must protect. The robust signal
// for "a section reverted to English" is instead the fraction of substantial
// narrative lines that contain NO CJK at all: a real zh-TW plan sits near 0.05
// (only isolated English lines such as a metadata sample), while an Englishized
// plan climbs well above it. The threshold is a starting point and tunable.
const MAX_ENGLISH_PROSE_LINE_RATIO: f64 = 0.30;

// Minimum (letters + CJK) weight for a masked narrative line to count as prose;
// below this a line is scaffolding (a bare file-list bullet, a short marker).
const MIN_PROSE_LINE_WEIGHT: usize = 8;

/// Strip inline `code spans` from a line.
fn strip_inline_code(line: &str) -> String {
    let mut out = String::new();
    let mut in_code = false;
    for ch in line.chars() {
        if ch == '`' {
            in_code = !in_code;
            continue;
        }
        if !in_code {
            out.push(ch);
        }
    }
    out
}

/// Strip a leading markdown list / checkbox marker, keeping the content.
fn strip_list_marker(line: &str) -> &str {
    let s = line.trim_start();
    let s = s
        .strip_prefix("- [ ] ")
        .or_else(|| s.strip_prefix("- [x] "))
        .unwrap_or(s);
    s.strip_prefix("- ")
        .or_else(|| s.strip_prefix("* "))
        .unwrap_or(s)
}

/// A path, command, identifier, or plan/machine id token — masked so file lists
/// and commands never trip the language gate.
fn is_technical_token(tok: &str) -> bool {
    tok.contains('/')
        || tok.contains('\\')
        || is_upper_dash_number_id(tok)
        || (tok.contains('.') && tok.chars().any(|c| c.is_ascii_alphabetic()))
        || (tok.contains('_') && tok.chars().any(|c| c.is_ascii_alphabetic()))
}

/// Match an uppercase-prefix dash-number identifier (a plan/test/machine id).
fn is_upper_dash_number_id(tok: &str) -> bool {
    let tok = tok.trim_end_matches([',', '.', ':', ';', ')', '(']);
    let mut parts = tok.splitn(2, '-');
    matches!((parts.next(), parts.next()),
        (Some(a), Some(b))
            if !a.is_empty() && a.chars().all(|c| c.is_ascii_uppercase())
                && !b.is_empty() && b.chars().all(|c| c.is_ascii_digit()))
}

/// Residual narrative of a line after removing code spans, the list marker, and
/// technical tokens (paths / commands / identifiers).
fn line_narrative(line: &str) -> String {
    let no_code = strip_inline_code(line);
    let body = strip_list_marker(&no_code);
    body.split_whitespace()
        .filter(|tok| !is_technical_token(tok))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Count (pure-English prose lines, total substantial prose lines) after masking
/// headings, fenced blocks, HTML comments, table rows, and technical tokens.
fn english_prose_line_counts(text: &str) -> (usize, usize) {
    let mut english = 0;
    let mut total = 0;
    let mut in_fence = false;
    let mut skip_section = false;
    for raw in text.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(heading) = trimmed.strip_prefix("## ") {
            // Approval + Review Results carry machine-anchored verdict/marker
            // lines that are English by design (kept, no rename) — exclude them.
            let name = heading.trim();
            skip_section = name.eq_ignore_ascii_case("Approval")
                || name.eq_ignore_ascii_case("Review Results");
            continue;
        }
        if trimmed.starts_with('#')
            || skip_section
            || trimmed.is_empty()
            || trimmed.starts_with("<!--")
            || trimmed.starts_with("-->")
            || trimmed.starts_with('|')
        {
            continue;
        }
        let narrative = line_narrative(raw);
        let weight = narrative
            .chars()
            .filter(|c| c.is_ascii_alphabetic() || is_cjk(*c))
            .count();
        if weight < MIN_PROSE_LINE_WEIGHT {
            continue;
        }
        total += 1;
        if !contains_cjk(&narrative) {
            english += 1;
        }
    }
    (english, total)
}

fn plan_language_outcome(plan_text: &str, config_path: Option<&Path>) -> CheckOutcome {
    match configured_plan_language(config_path) {
        Some(language) if language.eq_ignore_ascii_case("zh-TW") => {
            let (english, total) = english_prose_line_counts(plan_text);
            if total == 0 {
                return outcome(
                    "plan-language",
                    CheckState::Pass,
                    "planLanguage=zh-TW; no substantial prose to check".to_string(),
                );
            }
            let ratio = english as f64 / total as f64;
            let pct = (MAX_ENGLISH_PROSE_LINE_RATIO * 100.0) as u32;
            let pass = ratio <= MAX_ENGLISH_PROSE_LINE_RATIO;
            outcome(
                "plan-language",
                if pass {
                    CheckState::Pass
                } else {
                    CheckState::Fail
                },
                if pass {
                    format!("planLanguage=zh-TW; {english}/{total} prose lines pure-English (<= {pct}% ok)")
                } else {
                    format!("planLanguage=zh-TW but {english}/{total} prose lines are pure-English (> {pct}% — reverted to English?)")
                },
            )
        }
        Some(language) => outcome(
            "plan-language",
            CheckState::Pass,
            format!("planLanguage={language}; no charset-specific check configured"),
        ),
        None => outcome(
            "plan-language",
            CheckState::Pass,
            "planLanguage not configured".to_string(),
        ),
    }
}

/// Drop the planning-authority metadata comment lines so its own paths are not
/// mistaken for machine anchors during parity.
fn without_meta_lines(text: &str) -> String {
    let mut out = String::new();
    let mut in_meta = false;
    for line in text.lines() {
        if line.contains("gal:planning-authority") {
            in_meta = true;
            continue;
        }
        if in_meta {
            if line.contains("-->") {
                in_meta = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Backtick-quoted tokens in a body.
fn backtick_tokens(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        rest = &rest[open + 1..];
        if let Some(close) = rest.find('`') {
            let tok = rest[..close].trim();
            if !tok.is_empty() {
                out.push(tok.to_string());
            }
            rest = &rest[close + 1..];
        } else {
            break;
        }
    }
    out
}

/// Cross-language-invariant machine anchors: uppercase-dash-number IDs (task /
/// test / OQ) and backtick-quoted path-like tokens. Prose is ignored.
fn machine_anchor_set(text: &str) -> std::collections::BTreeSet<String> {
    let body = without_meta_lines(text);
    let mut set = std::collections::BTreeSet::new();
    if let Ok(re) = regex::Regex::new(r"\b[A-Z]{1,4}-[0-9]{1,3}\b") {
        for m in re.find_iter(&body) {
            set.insert(m.as_str().to_string());
        }
    }
    for tok in backtick_tokens(&body) {
        let path_like = tok.contains('/')
            || (tok.contains('.') && tok.chars().any(|c| c.is_ascii_alphabetic()));
        if path_like {
            set.insert(tok);
        }
    }
    set
}

/// Terminal `equivalence-verdict` value written by `gal planning-stamp
/// --equivalence` once `/plan-to-prompt`'s equivalence gate passes.
const EQUIVALENCE_VERDICT_EQUIVALENT: &str = "EQUIVALENT";

/// A `prompt-hash` is a real stamped hash only when it is exactly 64 lowercase
/// hex characters — `PENDING_PROMPT_HASH` (`"none"`) and any malformed value
/// both fail this.
fn is_stamped_prompt_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// True only in the post-prompt terminal state: a real stamped `prompt-hash`
/// plus `equivalence-verdict: EQUIVALENT`. A missing EN draft is expected by
/// design once this holds — the draft's role ends at prompt generation.
/// Any other combination (pending placeholders, malformed hash paired with the
/// terminal verdict, or the terminal verdict without a valid hash) is a mixed
/// or pre-prompt state, so a missing draft there stays a hard failure.
fn is_post_prompt_terminal_state(meta: &LocalizedMeta) -> bool {
    meta.equivalence_verdict == EQUIVALENCE_VERDICT_EQUIVALENT
        && is_stamped_prompt_hash(&meta.prompt_hash)
}

/// Compare the machine-anchored subset of a localized plan against its EN draft.
/// Prose differences (the two are in different languages) never trip it.
fn parity_outcome(plan_text: &str, repo_root: &Path) -> CheckOutcome {
    let Some(meta) = parse_localized_meta(plan_text) else {
        return outcome(
            "machine-anchor-parity",
            CheckState::Pass,
            "no localized metadata block — parity not applicable".to_string(),
        );
    };
    let draft_path = repo_root.join(&meta.semantic_draft);
    let draft_body = match std::fs::read_to_string(&draft_path) {
        Ok(body) => body,
        Err(_) if is_post_prompt_terminal_state(&meta) => {
            return outcome(
                "machine-anchor-parity",
                CheckState::Pass,
                "EN draft deleted by design — prompt-hash/equivalence-verdict are post-prompt terminal"
                    .to_string(),
            );
        }
        Err(_) => {
            return outcome(
                "machine-anchor-parity",
                CheckState::Fail,
                format!("EN draft missing: {}", meta.semantic_draft),
            );
        }
    };
    let localized = machine_anchor_set(plan_text);
    let draft = machine_anchor_set(&draft_body);
    if localized == draft {
        outcome(
            "machine-anchor-parity",
            CheckState::Pass,
            format!(
                "{} machine-anchored items match localized <-> draft",
                localized.len()
            ),
        )
    } else {
        let only_localized: Vec<_> = localized.difference(&draft).cloned().collect();
        let only_draft: Vec<_> = draft.difference(&localized).cloned().collect();
        outcome("machine-anchor-parity", CheckState::Fail, format!("machine-anchor mismatch — localized-only: {only_localized:?}, draft-only: {only_draft:?}"))
    }
}

/// Validate a localized source plan's metadata block against the EN draft and
/// its own rendered-source hash. Runs only when a metadata block is present, so
/// English single-file source plans skip it entirely.
fn localized_metadata_outcome(plan_text: &str, repo_root: &Path) -> CheckOutcome {
    let Some(meta) = parse_localized_meta(plan_text) else {
        return outcome(
            "localized-metadata",
            CheckState::Pass,
            "no localized metadata block — single-file source plan".to_string(),
        );
    };
    // Once the prompt exists and equivalence has been stamped, the draft is
    // deleted by design (its role ends at prompt generation) and can never be
    // re-rendered against — so rendered-source-hash can drift on later
    // Approval-only edits with no way to re-stamp it. `prompt-hash` +
    // `equivalence-verdict` are the terminal authoritative proof at that
    // point; check this before any hash comparison that depends on the now-
    // gone draft (directly or via the self-referential rendered-source-hash).
    if is_post_prompt_terminal_state(&meta) {
        return outcome(
            "localized-metadata",
            CheckState::Pass,
            "EN draft deleted by design — prompt-hash/equivalence-verdict are post-prompt terminal"
                .to_string(),
        );
    }
    let actual = rendered_source_hash(plan_text);
    if actual != meta.rendered_source_hash {
        return outcome(
            "localized-metadata",
            CheckState::Fail,
            "rendered-source hash mismatch — reconcile localized source against the EN draft"
                .to_string(),
        );
    }
    let draft_path = repo_root.join(&meta.semantic_draft);
    let draft_body = match std::fs::read_to_string(&draft_path) {
        Ok(body) => body,
        Err(_) => {
            return outcome(
                "localized-metadata",
                CheckState::Fail,
                format!("EN draft missing: {}", meta.semantic_draft),
            );
        }
    };
    if rendered_source_hash(&draft_body) != meta.draft_hash {
        return outcome(
            "localized-metadata",
            CheckState::Fail,
            "EN draft hash mismatch — draft changed since last render".to_string(),
        );
    }
    outcome(
        "localized-metadata",
        CheckState::Pass,
        "localized metadata + draft/source hashes match".to_string(),
    )
}

// Starter set of high-risk simplified-Chinese-only characters whose traditional
// forms differ and which carry NO separate traditional meaning (so a hit is an
// unambiguous zh-CN drift, not a legitimate traditional word). Deliberately
// excludes collision-prone chars like 后 (皇后) and 里 (公里). Tunable in place.
const SIMPLIFIED_ONLY_CHARS: &[char] = &[
    '这', '个', '说', '时', '会', '来', '电', '实', '类', '码', '语', '单', '处', '层', '测', '现',
    '门', '对', '设', '应', '变', '书', '边', '样', '结', '让',
];

/// Collect any high-risk simplified-Chinese characters present in masked prose
/// (code spans / fences / headings / tables / technical tokens removed).
fn simplified_hits(text: &str) -> std::collections::BTreeSet<char> {
    let set: std::collections::HashSet<char> = SIMPLIFIED_ONLY_CHARS.iter().copied().collect();
    let mut hits = std::collections::BTreeSet::new();
    let mut in_fence = false;
    for raw in text.lines() {
        let trimmed = raw.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence
            || trimmed.starts_with('#')
            || trimmed.starts_with("<!--")
            || trimmed.starts_with("-->")
            || trimmed.starts_with('|')
        {
            continue;
        }
        for ch in line_narrative(raw).chars() {
            if set.contains(&ch) {
                hits.insert(ch);
            }
        }
    }
    hits
}

fn simplified_term_outcome(plan_text: &str, config_path: Option<&Path>) -> CheckOutcome {
    match configured_plan_language(config_path) {
        Some(language) if language.eq_ignore_ascii_case("zh-TW") => {
            let hits = simplified_hits(plan_text);
            if hits.is_empty() {
                outcome(
                    "simplified-term",
                    CheckState::Pass,
                    "planLanguage=zh-TW; no high-risk simplified-Chinese characters in prose"
                        .to_string(),
                )
            } else {
                let list: String = hits.iter().collect();
                outcome("simplified-term", CheckState::Fail, format!("planLanguage=zh-TW but prose contains simplified-Chinese character(s): {list}"))
            }
        }
        _ => outcome(
            "simplified-term",
            CheckState::Pass,
            "simplified-term check applies only to zh-TW".to_string(),
        ),
    }
}

fn run_checks(plan_text: &str, repo_root: &Path, config_path: Option<&Path>) -> Vec<CheckOutcome> {
    vec![
        open_questions_outcome(plan_text),
        required_sections_outcome(plan_text),
        approval_shape_outcome(plan_text),
        arch_review_outcome(plan_text),
        arch_review_consistency_outcome(plan_text),
        naming_gate_outcome(repo_root),
        plan_language_outcome(plan_text, config_path),
        simplified_term_outcome(plan_text, config_path),
        localized_metadata_outcome(plan_text, repo_root),
        parity_outcome(plan_text, repo_root),
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
            return ExitCode::Usage;
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
    let config_path = gal_foundation::paths::machine_config_path();
    let plan_text = std::fs::read_to_string(&parsed.target).unwrap_or_default();
    let receipt = Receipt {
        checks: run_checks(&plan_text, &repo_root, config_path.as_deref()),
    };
    if let Some(parent) = parsed.receipt.parent() {
        if let Err(err) = std::fs::create_dir_all(parent) {
            eprintln!(
                "gal planning-check: cannot create receipt dir {}: {err}",
                parent.display()
            );
            return ExitCode::Error;
        }
    }
    if let Err(err) = std::fs::write(&parsed.receipt, render_receipt(&receipt)) {
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

    #[test]
    fn default_receipt_path_is_plan_scoped_and_explicit_override_wins() {
        let target = Path::new(".dev/plans/planning-scope.md");
        assert_eq!(
            default_receipt_path(target),
            resolve_receipt_path(Some(target), "planning-check.receipt.md")
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
            tmp.path().join("docs").join("naming.md"),
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
        "# Plan\n\n## Goal\n目標\n\n## Requirements\n- 條件\n\n## Approach\n做法\n\n## Open Questions\nNone.\n\n## Approval\n- Human approval: [approved]\n- Architect review: [clear]\n- Design review: [not-requested]\n- Business review: [not-requested]\n\n## Review Results\n### Architecture Review\nAPPROVE\n\n<!-- ARCH_REVIEW: CLEAR -->\n".to_string()
    }

    #[test]
    fn clean_plan_passes() {
        let repo = temp_repo();
        let cfg = repo.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"zh-TW\"}\n").unwrap();
        let receipt = Receipt {
            checks: run_checks(&clean_plan(), repo.path(), Some(&cfg)),
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
            tmp.path().join("docs").join("naming.md"),
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
            checks: run_checks(&plan, repo.path(), None),
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
            checks: run_checks(&plan, repo.path(), None),
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
            checks: run_checks(&plan, repo.path(), None),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "arch-review-clear" && c.state == CheckState::Fail));
    }
    #[test]
    fn large_english_prose_fails() {
        let repo = temp_repo();
        let cfg = repo.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"zh-TW\"}\n").unwrap();
        let mut plan = String::from("# Plan\n\n## Goal\n");
        for _ in 0..12 {
            plan.push_str(
                "This section describes the reconcile flow before writing the receipt file.\n",
            );
        }
        let receipt = Receipt {
            checks: run_checks(&plan, repo.path(), Some(&cfg)),
        };
        assert!(
            receipt
                .checks
                .iter()
                .any(|c| c.name == "plan-language" && c.state == CheckState::Fail),
            "{:#?}",
            receipt.checks
        );
    }

    #[test]
    fn file_list_and_commands_no_false_positive() {
        let repo = temp_repo();
        let cfg = repo.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"zh-TW\"}\n").unwrap();
        let plan = "# Plan\n\n## Files\n\
- `crates/cli/src/commands/planning_check.rs` — 語言 gate 強化,遮蔽後檢查未遮蔽 prose\n\
- `plugins/gal-core/workflows/coding.md` — 三層權威與規劃 lifecycle 規則\n\n\
## Approach\n\
偵測到 rendered-source hash mismatch 時,必須先停在 read-only reconcile preflight,再把差異 merge 回 EN draft\n\
所有 machine-anchor heading 一律保留英文且不改名,其餘 narrative 必須符合 planLanguage\n";
        let receipt = Receipt {
            checks: run_checks(plan, repo.path(), Some(&cfg)),
        };
        assert!(
            receipt
                .checks
                .iter()
                .all(|c| !(c.name == "plan-language" && c.state == CheckState::Fail)),
            "{:#?}",
            receipt.checks
        );
    }

    fn localized_plan_with_matching_hashes(repo: &TempDir) -> String {
        let draft_rel = ".dev/plans/x.en.md";
        let draft_body = "# EN draft\n\n## Goal\nGoal text\n";
        std::fs::create_dir_all(repo.path().join(".dev/plans")).unwrap();
        std::fs::write(repo.path().join(draft_rel), draft_body).unwrap();
        let draft_hash = rendered_source_hash(draft_body);
        let plan = format!(
            "# Plan: 範例\n\n## Goal\n目標\n\n<!-- gal:planning-authority\nsemantic-draft: {draft_rel}\nplanLanguage: zh-TW\ndraft-hash: {draft_hash}\nrendered-source-hash: PLACEHOLDER\nprompt-hash: none\nequivalence-verdict: pending\n-->\n"
        );
        // rendered-source-hash excludes the metadata block, so stamping it in
        // does not change the body hash.
        let src_hash = rendered_source_hash(&plan);
        plan.replace("PLACEHOLDER", &src_hash)
    }

    #[test]
    fn matching_hash_passes() {
        let repo = temp_repo();
        let plan = localized_plan_with_matching_hashes(&repo);
        let out = localized_metadata_outcome(&plan, repo.path());
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn stale_rendered_hash_fails() {
        let repo = temp_repo();
        let plan = localized_plan_with_matching_hashes(&repo);
        // A prose edit after the last stamp makes the recorded hash stale.
        let edited = plan.replace("目標", "新目標內容");
        let out = localized_metadata_outcome(&edited, repo.path());
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn english_path_skips_metadata_check() {
        let repo = temp_repo();
        let plan = "# Plan\n\n## Goal\nGoal\n"; // no metadata block
        let out = localized_metadata_outcome(plan, repo.path());
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn stamped_fixture_passes_planning_check_hash() {
        use super::super::planning_authority::stamp_metadata_hash;
        let repo = temp_repo();
        std::fs::create_dir_all(repo.path().join(".dev/plans")).unwrap();
        std::fs::write(
            repo.path().join(".dev/plans/x.en.md"),
            "# EN draft\n\n## Goal\nGoal\n",
        )
        .unwrap();
        let localized_path = repo.path().join(".dev/plans/x.md");
        std::fs::write(&localized_path, "# Plan\n\n## Goal\n目標\n\n<!-- gal:planning-authority\nsemantic-draft: .dev/plans/x.en.md\nplanLanguage: zh-TW\ndraft-hash: PLACEHOLDER\nrendered-source-hash: PLACEHOLDER\nprompt-hash: none\nequivalence-verdict: pending\n-->\n").unwrap();
        stamp_metadata_hash(&localized_path).unwrap();
        let stamped = std::fs::read_to_string(&localized_path).unwrap();
        let out = localized_metadata_outcome(&stamped, repo.path());
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn prose_difference_does_not_trigger_parity() {
        let repo = temp_repo();
        let t1 = format!("T-{:02}", 1);
        let draft_rel = ".dev/plans/y.en.md";
        std::fs::create_dir_all(repo.path().join(".dev/plans")).unwrap();
        let draft = format!(
            "# EN draft\n\n## Tasks\n- [ ] {t1} — edit `crates/cli/x.rs` the English way\n"
        );
        std::fs::write(repo.path().join(draft_rel), &draft).unwrap();
        let localized = format!(
            "# Plan\n\n## Tasks\n- [ ] {t1} — 用中文改 `crates/cli/x.rs`\n\n<!-- gal:planning-authority\nsemantic-draft: {draft_rel}\nplanLanguage: zh-TW\ndraft-hash: aa\nrendered-source-hash: bb\nprompt-hash: none\nequivalence-verdict: pending\n-->\n"
        );
        let out = parity_outcome(&localized, repo.path());
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    /// Build a localized plan text (no EN draft written to disk) with the
    /// given `prompt-hash` / `equivalence-verdict` values, self-consistently
    /// stamped so `rendered-source-hash` matches (the draft-hash field itself
    /// is irrelevant once the draft is treated as deleted-by-design).
    fn localized_plan_no_draft(prompt_hash: &str, equivalence_verdict: &str) -> String {
        let plan = format!(
            "# Plan: 範例\n\n## Goal\n目標\n\n<!-- gal:planning-authority\nsemantic-draft: .dev/plans/gone.en.md\nplanLanguage: zh-TW\ndraft-hash: aa\nrendered-source-hash: PLACEHOLDER\nprompt-hash: {prompt_hash}\nequivalence-verdict: {equivalence_verdict}\n-->\n"
        );
        let src_hash = rendered_source_hash(&plan);
        plan.replace("PLACEHOLDER", &src_hash)
    }

    // Post-prompt terminal metadata (stamped 64-hex prompt-hash + EQUIVALENT)
    // passes both rows even though the EN draft is absent from disk.
    #[test]
    fn post_prompt_terminal_state_passes_without_draft() {
        let repo = temp_repo();
        let plan = localized_plan_no_draft(&"a".repeat(64), "EQUIVALENT");
        let localized = localized_metadata_outcome(&plan, repo.path());
        assert_eq!(localized.state, CheckState::Pass, "{localized:?}");
        let parity = parity_outcome(&plan, repo.path());
        assert_eq!(parity.state, CheckState::Pass, "{parity:?}");
    }

    // Pre-prompt placeholder state ("none" / "pending") with a missing draft
    // stays a hard failure on both rows — the strict pre-prompt behavior is
    // unchanged.
    #[test]
    fn pre_prompt_placeholder_state_fails_without_draft() {
        let repo = temp_repo();
        let plan = localized_plan_no_draft("none", "pending");
        let localized = localized_metadata_outcome(&plan, repo.path());
        assert_eq!(localized.state, CheckState::Fail, "{localized:?}");
        let parity = parity_outcome(&plan, repo.path());
        assert_eq!(parity.state, CheckState::Fail, "{parity:?}");
    }

    // Mixed state: terminal verdict but a non-stamped hash ("none") is not
    // post-prompt-terminal, so a missing draft still fails.
    #[test]
    fn mixed_state_equivalent_verdict_without_stamped_hash_fails_without_draft() {
        let repo = temp_repo();
        let plan = localized_plan_no_draft("none", "EQUIVALENT");
        let localized = localized_metadata_outcome(&plan, repo.path());
        assert_eq!(localized.state, CheckState::Fail, "{localized:?}");
        let parity = parity_outcome(&plan, repo.path());
        assert_eq!(parity.state, CheckState::Fail, "{parity:?}");
    }

    // Mixed state: stamped hash but a non-terminal verdict is not
    // post-prompt-terminal, so a missing draft still fails.
    #[test]
    fn mixed_state_stamped_hash_without_equivalent_verdict_fails_without_draft() {
        let repo = temp_repo();
        let plan = localized_plan_no_draft(&"a".repeat(64), "pending");
        let localized = localized_metadata_outcome(&plan, repo.path());
        assert_eq!(localized.state, CheckState::Fail, "{localized:?}");
        let parity = parity_outcome(&plan, repo.path());
        assert_eq!(parity.state, CheckState::Fail, "{parity:?}");
    }

    // A malformed hash (wrong length, uppercase, or non-hex) is never
    // post-prompt-terminal even paired with EQUIVALENT, so a missing draft
    // still fails.
    #[test]
    fn malformed_hash_variants_fail_without_draft() {
        let repo = temp_repo();
        for bad_hash in [
            "a".repeat(63),                 // too short
            "a".repeat(65),                 // too long
            "A".repeat(64),                 // uppercase, not lowercase-hex
            format!("{}g", "a".repeat(63)), // non-hex character
        ] {
            let plan = localized_plan_no_draft(&bad_hash, "EQUIVALENT");
            let localized = localized_metadata_outcome(&plan, repo.path());
            assert_eq!(
                localized.state,
                CheckState::Fail,
                "{bad_hash}: {localized:?}"
            );
            let parity = parity_outcome(&plan, repo.path());
            assert_eq!(parity.state, CheckState::Fail, "{bad_hash}: {parity:?}");
        }
    }

    #[test]
    fn parity_machine_anchor_mismatch_fails() {
        let repo = temp_repo();
        let t1 = format!("T-{:02}", 1);
        let t2 = format!("T-{:02}", 2);
        let draft_rel = ".dev/plans/y.en.md";
        std::fs::create_dir_all(repo.path().join(".dev/plans")).unwrap();
        let draft = format!("# EN draft\n\n## Tasks\n- [ ] {t1} — only one task\n");
        std::fs::write(repo.path().join(draft_rel), &draft).unwrap();
        // localized has an extra task id the draft lacks.
        let localized = format!(
            "# Plan\n\n## Tasks\n- [ ] {t1} — 任務一\n- [ ] {t2} — 多出來的任務\n\n<!-- gal:planning-authority\nsemantic-draft: {draft_rel}\nplanLanguage: zh-TW\ndraft-hash: aa\nrendered-source-hash: bb\nprompt-hash: none\nequivalence-verdict: pending\n-->\n"
        );
        let out = parity_outcome(&localized, repo.path());
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    fn valid_approval_block() -> String {
        "## Approval\n\n- Human approval: [approved]\n- Architect review: [clear]\n- Design review: [not-requested]\n- Business review: [not-requested]\n\n## Review Results\n".to_string()
    }

    // Valid four-line and `[not-required] — reason` blocks pass.
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

    // Unknown tokens, human `[clear]`, and old spaced spelling fail prescriptively.
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

    // Old shape, wrong order, and missing section fail without panic.
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

    // Bare date suffix and unbracketed verdict fail; em-dash reason passes.
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

    // Both tracked templates pass `approval_shape_outcome`.
    #[test]
    fn template_pin_plan_md_passes_approval_shape() {
        let text = read_template("plugins/gal-core/templates/plan.md");
        let out = approval_shape_outcome(&text);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn template_pin_plan_prompt_md_passes_approval_shape() {
        let text = read_template("plugins/gal-core/templates/plan-prompt.md");
        let out = approval_shape_outcome(&text);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    // A deliberate one-character corruption fails.
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

    // Architect `[clear]` plus standalone marker passes.
    #[test]
    fn arch_review_consistency_clear_with_marker_passes() {
        let plan = plan_with_marker(&valid_approval_block(), true);
        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    // Architect `[clear]` without the marker fails.
    #[test]
    fn arch_review_consistency_clear_without_marker_fails() {
        let plan = plan_with_marker(&valid_approval_block(), false);
        let out = arch_review_consistency_outcome(&plan);
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    // The marker present alongside a non-`[clear]` token fails.
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
    fn simplified_term_flagged() {
        let repo = temp_repo();
        let cfg = repo.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"zh-TW\"}\n").unwrap();
        // 这个...会...实现...现 are simplified-only; a zh-TW plan should use 這個/會/實現/現.
        let plan = "# Plan\n\n## Approach\n这个功能会实现相关现有逻辑\n";
        let out = simplified_term_outcome(plan, Some(&cfg));
        assert_eq!(out.state, CheckState::Fail, "{out:?}");
    }

    #[test]
    fn traditional_text_no_false_positive() {
        let repo = temp_repo();
        let cfg = repo.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"zh-TW\"}\n").unwrap();
        let plan = "# Plan\n\n## Approach\n這個功能會實現相關現有邏輯,並在 `crates/cli` 加入測試\n";
        let out = simplified_term_outcome(plan, Some(&cfg));
        assert_eq!(out.state, CheckState::Pass, "{out:?}");
    }

    #[test]
    fn zh_tw_without_cjk_fails() {
        let repo = temp_repo();
        let cfg = repo.path().join("config.json");
        std::fs::write(&cfg, "{\"planLanguage\":\"zh-TW\"}\n").unwrap();
        let plan = "# Plan\n\n## Goal\nGoal\n\n## Requirements\n- Requirement\n\n## Approach\nApproach\n\n## Open Questions\nNone.\n\n## Approval\n- Human approval: [approved]\n- Architect review: [clear]\n- Design review: [not-requested]\n- Business review: [not-requested]\n\n## Review Results\n### Architecture Review\nAPPROVE\n\n<!-- ARCH_REVIEW: CLEAR -->\n";
        let receipt = Receipt {
            checks: run_checks(plan, repo.path(), Some(&cfg)),
        };
        assert!(receipt
            .checks
            .iter()
            .any(|c| c.name == "plan-language" && c.state == CheckState::Fail));
    }
}
