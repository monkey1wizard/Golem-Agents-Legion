//! Repo-hygiene `gal finalize-check` rows: bounded generated-file existence,
//! contract-roster parity, bounded state, and doc-link resolution. All four
//! checks reuse the `CheckOutcome` / `CheckState` receipt vocabulary defined in
//! `finalize_check` — no second check vocabulary.
//!
//! Repo-shape scope guard: `project-source-doc-existence` and `state-bound` are
//! universal (every GAL-initialized repo owns these generated files).
//! `contract-roster-parity` and `doc-link-resolution` are GAL-source-repo scoped
//! — emitted only when `plugins/gal-core/` exists at the repo root. A downstream
//! repo without that directory never sees those two rows at all: they are
//! **absent, not `NotRun`** (`CheckState` has no not-applicable state, and
//! `NotRun` never passes), so GAL's own contract hygiene never blocks a
//! downstream repo's finalize.

use super::finalize_check::{split_md_row, CheckOutcome, CheckState};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Repo-shape scope guard predicate: this repo owns GAL's own source contracts
/// (`plugins/gal-core/` exists at the repo root).
pub(crate) fn is_gal_source_repo(repo_root: &Path) -> bool {
    repo_root.join("plugins").join("gal-core").is_dir()
}

/// Extract the body lines of a `## <heading>` section: everything after the
/// heading line up to (not including) the next `## ` heading or end of file.
fn section_body<'a>(lines: &'a [&'a str], heading: &str) -> Option<&'a [&'a str]> {
    let idx = lines.iter().position(|l| l.trim() == heading)?;
    let start = idx + 1;
    let end = lines[start..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|off| start + off)
        .unwrap_or(lines.len());
    Some(&lines[start..end])
}

/// True when every cell in a split row is a markdown separator cell (`-`/`:`).
fn is_separator_row(cells: &[String]) -> bool {
    cells
        .iter()
        .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
}

/// check: `project-source-doc-existence` (universal).
///
/// Locates `.dev/project.md`'s `## Source Documents` table and finds the `Path`
/// column **by header name**, so both `Document | Path | Last Verified` and
/// `Path | Type | Notes` are accepted. An empty template table (header + zero
/// data rows) passes vacuously. Missing/malformed section, missing/duplicate
/// `Path` header, duplicate path, escape, or a missing target all fail.
pub(crate) fn check_project_source_doc_existence(repo_root: &Path) -> CheckOutcome {
    let name = "project-source-doc-existence".to_string();
    let project_md_path = repo_root.join(".dev").join("project.md");
    let Ok(text) = std::fs::read_to_string(&project_md_path) else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("cannot read {}", project_md_path.display()),
        };
    };

    let lines: Vec<&str> = text.lines().collect();
    let Some(body) = section_body(&lines, "## Source Documents") else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "missing ## Source Documents section".to_string(),
        };
    };
    let mut rows = body.iter().map(|l| l.trim()).filter(|l| l.starts_with('|'));

    let Some(header) = rows.next() else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: "## Source Documents has no table header".to_string(),
        };
    };
    let header_cells = split_md_row(header);
    let path_positions: Vec<usize> = header_cells
        .iter()
        .enumerate()
        .filter(|(_, c)| c.eq_ignore_ascii_case("path"))
        .map(|(i, _)| i)
        .collect();
    if path_positions.len() != 1 {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!(
                "expected exactly one `Path` header column, found {}",
                path_positions.len()
            ),
        };
    }
    let path_col = path_positions[0];
    let repo_resolved = match repo_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return CheckOutcome {
                name,
                command: None,
                state: CheckState::Fail,
                summary: format!("cannot canonicalize repo root: {e}"),
            }
        }
    };

    let mut seen = std::collections::BTreeSet::new();
    let mut problems = Vec::new();
    let mut data_rows = 0usize;
    for row in rows {
        let cells = split_md_row(row);
        if is_separator_row(&cells) || cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        data_rows += 1;
        let raw = cells.get(path_col).map(String::as_str).unwrap_or("");
        let path_str = raw.trim().trim_matches('`').trim();
        if path_str.is_empty() {
            problems.push(format!("row {data_rows}: empty Path cell"));
            continue;
        }
        if !seen.insert(path_str.to_string()) {
            problems.push(format!("duplicate path: {path_str}"));
            continue;
        }
        match repo_root.join(path_str).canonicalize() {
            Ok(resolved) if resolved.starts_with(&repo_resolved) => {}
            Ok(_) => problems.push(format!("escapes repo root: {path_str}")),
            Err(_) => problems.push(format!("missing target: {path_str}")),
        }
    }

    if problems.is_empty() {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: if data_rows == 0 {
                "empty Source Documents table".to_string()
            } else {
                format!("{data_rows} source doc path(s) verified")
            },
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: problems.join(" | "),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// check: contract-roster-parity (GAL-source-repo scoped)
// ─────────────────────────────────────────────────────────────────────────────

/// Extract every backtick-quoted span (`` `...` ``) from `text`.
fn backtick_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut in_span = false;
    let mut start = 0usize;
    for (i, ch) in text.char_indices() {
        if ch == '`' {
            if in_span {
                spans.push(text[start..i].to_string());
                in_span = false;
            } else {
                start = i + 1;
                in_span = true;
            }
        }
    }
    spans
}

/// Extract every `[label](target)` markdown-link target from `text`. Skips
/// unterminated brackets/parens rather than panicking.
fn markdown_link_targets(text: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let mut i = 0usize;
    while let Some(open_bracket_rel) = text[i..].find('[') {
        let open_bracket = i + open_bracket_rel;
        let Some(close_bracket_rel) = text[open_bracket..].find(']') else {
            break;
        };
        let close_bracket = open_bracket + close_bracket_rel;
        let after_bracket = close_bracket + 1;
        if text[after_bracket..].starts_with('(') {
            let paren_open = after_bracket + 1;
            if let Some(close_paren_rel) = text[paren_open..].find(')') {
                let close_paren = paren_open + close_paren_rel;
                targets.push(text[paren_open..close_paren].to_string());
                i = close_paren + 1;
                continue;
            }
        }
        i = open_bracket + 1;
    }
    targets
}

/// Direct child directory names of `dir` (non-recursive).
fn dir_names(dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read entry in {}: {e}", dir.display()))?;
        if entry.path().is_dir() {
            if let Some(n) = entry.file_name().to_str() {
                names.insert(n.to_string());
            }
        }
    }
    Ok(names)
}

/// Direct child file names of `dir` (non-recursive).
fn file_names(dir: &Path) -> Result<BTreeSet<String>, String> {
    let entries =
        std::fs::read_dir(dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
    let mut names = BTreeSet::new();
    for entry in entries {
        let entry = entry.map_err(|e| format!("cannot read entry in {}: {e}", dir.display()))?;
        if entry.path().is_file() {
            if let Some(n) = entry.file_name().to_str() {
                names.insert(n.to_string());
            }
        }
    }
    Ok(names)
}

/// Build a duplicate-checked roster set from raw extracted items, recording a
/// `duplicate roster entry` problem for every repeat under `family`.
fn roster_set(raw: Vec<String>, family: &str, problems: &mut Vec<String>) -> BTreeSet<String> {
    let mut roster = BTreeSet::new();
    for item in raw {
        if !roster.insert(item.clone()) {
            problems.push(format!("{family}: duplicate roster entry: {item}"));
        }
    }
    roster
}

/// Bidirectional diff between a roster set and a disk set, appending
/// `unrostered disk entry` / `phantom roster entry` problems under `family`.
fn diff_roster_and_disk(
    roster: &BTreeSet<String>,
    disk: &BTreeSet<String>,
    family: &str,
    problems: &mut Vec<String>,
) {
    for missing in disk.difference(roster) {
        problems.push(format!("{family}: unrostered disk entry: {missing}"));
    }
    for phantom in roster.difference(disk) {
        problems.push(format!("{family}: phantom roster entry: {phantom}"));
    }
}

fn commands_roster_problems(repo_root: &Path) -> Result<Vec<String>, String> {
    let commands_dir = repo_root.join("plugins").join("gal-core").join("commands");
    let path = commands_dir.join("commands.md");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("commands: cannot read {}: {e}", path.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let body = section_body(&lines, "## Source Files")
        .ok_or_else(|| "commands: missing ## Source Files section".to_string())?;
    let body_text = body.join("\n");
    let raw: Vec<String> = backtick_spans(&body_text)
        .into_iter()
        .filter_map(|s| {
            s.strip_prefix("commands/")
                .and_then(|rest| rest.strip_suffix("/SKILL.template.md"))
                .map(|dir| dir.to_string())
        })
        .collect();

    let mut problems = Vec::new();
    let roster = roster_set(raw, "commands", &mut problems);
    let disk = dir_names(&commands_dir).map_err(|e| format!("commands: {e}"))?;
    diff_roster_and_disk(&roster, &disk, "commands", &mut problems);
    Ok(problems)
}

fn agents_roster_problems(repo_root: &Path) -> Result<Vec<String>, String> {
    let agents_dir = repo_root.join("plugins").join("gal-core").join("agents");
    let path = agents_dir.join("agents.md");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("agents: cannot read {}: {e}", path.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let body = section_body(&lines, "## Agents And Responsibilities")
        .ok_or_else(|| "agents: missing ## Agents And Responsibilities section".to_string())?;
    let raw: Vec<String> = markdown_link_targets(&body.join("\n"))
        .into_iter()
        .filter(|t| t.ends_with(".agent.md"))
        .collect();

    let mut problems = Vec::new();
    let roster = roster_set(raw, "agents", &mut problems);
    let disk: BTreeSet<String> = file_names(&agents_dir)
        .map_err(|e| format!("agents: {e}"))?
        .into_iter()
        .filter(|n| n.ends_with(".agent.md"))
        .collect();
    diff_roster_and_disk(&roster, &disk, "agents", &mut problems);
    Ok(problems)
}

fn templates_roster_problems(repo_root: &Path) -> Result<Vec<String>, String> {
    let templates_dir = repo_root.join("plugins").join("gal-core").join("templates");
    let path = templates_dir.join("templates.md");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("templates: cannot read {}: {e}", path.display()))?;
    let lines: Vec<&str> = text.lines().collect();
    let file_templates = section_body(&lines, "## File Templates")
        .ok_or_else(|| "templates: missing ## File Templates section".to_string())?;
    let other_files = section_body(&lines, "## Other Files")
        .ok_or_else(|| "templates: missing ## Other Files section".to_string())?;
    let mut raw = markdown_link_targets(&file_templates.join("\n"));
    raw.extend(markdown_link_targets(&other_files.join("\n")));

    let mut problems = Vec::new();
    let roster = roster_set(raw, "templates", &mut problems);
    let disk = file_names(&templates_dir).map_err(|e| format!("templates: {e}"))?;
    diff_roster_and_disk(&roster, &disk, "templates", &mut problems);
    Ok(problems)
}

/// check: `contract-roster-parity` (GAL-source-repo scoped).
///
/// Bidirectional roster<->disk parity across three families: commands
/// (`commands.md` `## Source Files` backtick paths <-> `commands/*/` dirs),
/// agents (`agents.md` `## Agents And Responsibilities` markdown-link targets
/// <-> `agents/*.agent.md` files), and templates (`templates.md`
/// `## File Templates` + `## Other Files` markdown-link targets, unioned,
/// <-> every direct file in `templates/`, including `templates.md` itself).
/// Missing sections, malformed/duplicate rows, phantom entries, and
/// unrostered disk files all fail.
pub(crate) fn check_contract_roster_parity(repo_root: &Path) -> CheckOutcome {
    let name = "contract-roster-parity".to_string();
    let mut problems = Vec::new();

    for result in [
        commands_roster_problems(repo_root),
        agents_roster_problems(repo_root),
        templates_roster_problems(repo_root),
    ] {
        match result {
            Ok(mut p) => problems.append(&mut p),
            Err(e) => problems.push(e),
        }
    }

    if problems.is_empty() {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: "commands/agents/templates rosters match disk".to_string(),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: problems.join(" | "),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// check: state-bound (universal, mode-aware)
// ─────────────────────────────────────────────────────────────────────────────

/// Which `gal finalize-check` invocation mode is running — determines whether
/// an absent `## Recent Close-outs` section is tolerated (legacy migration
/// path) or fails, which checks run at all, and the receipt's default path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum HygieneMode {
    #[default]
    Full,
    HygieneOnly,
}

impl HygieneMode {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            HygieneMode::Full => "full",
            HygieneMode::HygieneOnly => "hygiene-only",
        }
    }
}

/// Whole-file cap after CRLF->LF normalization, per
/// `conventions/token-budget.md` § Bounded Session State.
const STATE_MD_MAX_BYTES: usize = 16_384;
/// Maximum `## Recent Close-outs` data rows.
const STATE_MD_MAX_ROWS: usize = 2;

fn normalize_lf(text: &str) -> String {
    text.replace("\r\n", "\n")
}

/// Every `<!--...-->` comment span in `text` (may span multiple lines).
fn html_comment_spans(text: &str) -> Vec<String> {
    let mut spans = Vec::new();
    let mut i = 0usize;
    while let Some(start_rel) = text[i..].find("<!--") {
        let start = i + start_rel;
        let Some(end_rel) = text[start..].find("-->") else {
            break;
        };
        let end = start + end_rel + 3;
        spans.push(text[start..end].to_string());
        i = end;
    }
    spans
}

/// Exact legacy-comment predicate (`conventions/token-budget.md` § Bounded
/// Session State): the comment opens `<!-- YYYY-MM-DD:` **and** its body
/// contains both `finalized on main` and `ABSORBED`. A comment merely
/// discussing those words without the date-prefixed opening does not match.
fn is_legacy_predicate_comment(comment: &str) -> bool {
    let Some(after_open) = comment.trim().strip_prefix("<!-- ") else {
        return false;
    };
    let date_ok = after_open.len() >= 11 && {
        let b = after_open.as_bytes();
        b[4] == b'-'
            && b[7] == b'-'
            && b[10] == b':'
            && after_open[0..4].bytes().all(|c| c.is_ascii_digit())
            && after_open[5..7].bytes().all(|c| c.is_ascii_digit())
            && after_open[8..10].bytes().all(|c| c.is_ascii_digit())
    };
    date_ok && comment.contains("finalized on main") && comment.contains("ABSORBED")
}

/// Count non-header, non-separator, non-blank data rows in a `## <heading>`
/// markdown table body.
fn count_data_rows(body: &[&str]) -> usize {
    let mut rows = body.iter().map(|l| l.trim()).filter(|l| l.starts_with('|'));
    if rows.next().is_none() {
        return 0; // no header row → no table
    }
    rows.filter(|row| {
        let cells = split_md_row(row);
        !is_separator_row(&cells) && !cells.iter().all(|c| c.is_empty())
    })
    .count()
}

/// check: `state-bound` (universal, mode-aware).
///
/// When `## Recent Close-outs` is **absent**: `full` mode tolerates the
/// legacy shape (so an existing repo can reach the Sequence-5 migration);
/// `hygiene-only` fails. When the section is **present**: both modes enforce
/// the 16,384 B LF-normalized cap, the 2-row cap, and zero exact-predicate
/// legacy comments anywhere in the file. Unreadable `.dev/state.md` fails in
/// either mode.
pub(crate) fn check_state_bound(repo_root: &Path, mode: HygieneMode) -> CheckOutcome {
    let name = "state-bound".to_string();
    let state_path = repo_root.join(".dev").join("state.md");
    let Ok(raw) = std::fs::read_to_string(&state_path) else {
        return CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: format!("cannot read {}", state_path.display()),
        };
    };
    let normalized = normalize_lf(&raw);
    let lines: Vec<&str> = normalized.lines().collect();

    let Some(body) = section_body(&lines, "## Recent Close-outs") else {
        return match mode {
            HygieneMode::Full => CheckOutcome {
                name,
                command: None,
                state: CheckState::Pass,
                summary: "## Recent Close-outs absent — legacy shape tolerated in full mode"
                    .to_string(),
            },
            HygieneMode::HygieneOnly => CheckOutcome {
                name,
                command: None,
                state: CheckState::Fail,
                summary: "## Recent Close-outs absent — required in hygiene-only mode".to_string(),
            },
        };
    };

    let mut problems = Vec::new();

    let data_rows = count_data_rows(body);
    if data_rows > STATE_MD_MAX_ROWS {
        problems.push(format!(
            "{data_rows} Recent Close-outs row(s) exceed the {STATE_MD_MAX_ROWS}-row cap"
        ));
    }

    let legacy_count = html_comment_spans(&normalized)
        .iter()
        .filter(|c| is_legacy_predicate_comment(c))
        .count();
    if legacy_count > 0 {
        problems.push(format!(
            "{legacy_count} exact-predicate legacy comment(s) present"
        ));
    }

    let size = normalized.len();
    if size > STATE_MD_MAX_BYTES {
        problems.push(format!(
            "{size} B exceeds the {STATE_MD_MAX_BYTES} B LF-normalized cap"
        ));
    }

    if problems.is_empty() {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Pass,
            summary: format!("{size} B, {data_rows} Recent Close-outs row(s), 0 legacy comments"),
        }
    } else {
        CheckOutcome {
            name,
            command: None,
            state: CheckState::Fail,
            summary: problems.join(" | "),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// check: doc-link-resolution (GAL-source-repo scoped)
// ─────────────────────────────────────────────────────────────────────────────

/// Tracked Markdown paths (repo-relative, forward-slash) in scope:
/// `README.md`, `docs/**/*.md`, `plugins/gal-core/**/*.md`. Fails closed on
/// any `git ls-files` invocation error.
///
/// The two `**` patterns use the `:(glob)` pathspec magic deliberately: git's
/// *default* pathspec matching treats a bare `**` as requiring at least one
/// full directory segment, so `docs/**/*.md` would silently exclude direct
/// children of `docs/` (e.g. `docs/manual.md`, `docs/architecture.md`) —
/// verified against this repo, where the un-prefixed pattern dropped 7 direct
/// children. `:(glob)` restores the expected "zero or more directories"
/// glob semantic.
fn tracked_markdown_paths(repo_root: &Path) -> Result<Vec<String>, String> {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .arg("ls-files")
        .arg("--")
        .arg("README.md")
        .arg(":(glob)docs/**/*.md")
        .arg(":(glob)plugins/gal-core/**/*.md")
        .output()
        .map_err(|e| format!("git ls-files failed to spawn: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-files exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    Ok(text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|l| l.to_string())
        .collect())
}

/// Replace every backtick-delimited inline-code span in `line` with spaces of
/// the same length, so link-like text inside code samples is never scanned.
fn mask_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for ch in line.chars() {
        if ch == '`' {
            in_code = !in_code;
            out.push(' ');
        } else if in_code {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

/// One `(line_no, raw_target)` link occurrence found outside code.
struct LinkOccurrence {
    line_no: usize,
    target: String,
}

/// Scan `text` for markdown links (inline `[label](target)`, image
/// `![alt](target)`, and reference-style `[label][ref]` / `[label][]`
/// resolved against `[ref]: target` definitions found anywhere in the same
/// file) outside fenced code blocks and inline code spans.
fn scan_markdown_links(text: &str) -> Vec<LinkOccurrence> {
    let lines: Vec<&str> = text.lines().collect();

    // Pass 1: collect reference-style link definitions, skipping fences.
    let mut ref_defs: BTreeMap<String, String> = BTreeMap::new();
    let mut in_fence = false;
    for raw_line in &lines {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('[') {
            if let Some(close) = rest.find(']') {
                if let Some(target_part) = rest[close + 1..].strip_prefix(':') {
                    let label = rest[..close].trim().to_lowercase();
                    if let Some(target) = target_part.split_whitespace().next() {
                        if !label.is_empty() {
                            ref_defs.insert(label, target.to_string());
                        }
                    }
                }
            }
        }
    }

    // Pass 2: extract link occurrences, again skipping fences and masking
    // inline-code spans so backtick-quoted samples are never scanned.
    let mut occurrences = Vec::new();
    in_fence = false;
    for (idx, raw_line) in lines.iter().enumerate() {
        let trimmed = raw_line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        let line_no = idx + 1;
        let masked = mask_inline_code(raw_line);
        let mut i = 0usize;
        while let Some(open_rel) = masked[i..].find('[') {
            let open = i + open_rel;
            let Some(close_rel) = masked[open..].find(']') else {
                break;
            };
            let close = open + close_rel;
            let after = close + 1;
            if masked[after..].starts_with('(') {
                let popen = after + 1;
                if let Some(pclose_rel) = masked[popen..].find(')') {
                    let pclose = popen + pclose_rel;
                    occurrences.push(LinkOccurrence {
                        line_no,
                        target: masked[popen..pclose].trim().to_string(),
                    });
                    i = pclose + 1;
                    continue;
                }
            } else if masked[after..].starts_with('[') {
                let ref_open = after + 1;
                if let Some(ref_close_rel) = masked[ref_open..].find(']') {
                    let ref_close = ref_open + ref_close_rel;
                    let mut ref_label = masked[ref_open..ref_close].trim().to_lowercase();
                    if ref_label.is_empty() {
                        ref_label = masked[open + 1..close].trim().to_lowercase();
                    }
                    if let Some(target) = ref_defs.get(&ref_label) {
                        occurrences.push(LinkOccurrence {
                            line_no,
                            target: target.clone(),
                        });
                    }
                    i = ref_close + 1;
                    continue;
                }
            }
            i = open + 1;
        }
    }
    occurrences
}

/// True when `target` is out of scope for local file-existence resolution:
/// an external URL/scheme, a same-document anchor, or empty.
fn is_out_of_scope_link_target(target: &str) -> bool {
    target.is_empty()
        || target.starts_with('#')
        || target.contains("://")
        || target.starts_with("mailto:")
        || target.starts_with("//")
}

/// check: `doc-link-resolution` (GAL-source-repo scoped).
///
/// Enumerates tracked Markdown (`README.md`, `docs/**/*.md`,
/// `plugins/gal-core/**/*.md`) via `git ls-files`, reads working-tree
/// contents, and validates every in-scope relative link target resolves to
/// an existing file that canonicalizes to inside the repo root (rejects
/// lexical `..` escapes and symlink escapes alike). Fails closed on
/// inventory or read failure; reports repo-relative `file:line` for every
/// broken link. External URLs, same-document anchors, and files outside the
/// tracked/scoped set (ignored overlays, untracked scratch) are out of
/// scope.
pub(crate) fn check_doc_link_resolution(repo_root: &Path) -> CheckOutcome {
    let name = "doc-link-resolution".to_string();
    let repo_resolved = match repo_root.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return CheckOutcome {
                name,
                command: None,
                state: CheckState::Fail,
                summary: format!("cannot canonicalize repo root: {e}"),
            }
        }
    };

    let tracked = match tracked_markdown_paths(repo_root) {
        Ok(paths) => paths,
        Err(e) => {
            return CheckOutcome {
                name,
                command: Some("git ls-files".to_string()),
                state: CheckState::Fail,
                summary: e,
            }
        }
    };

    let mut problems = Vec::new();
    let mut checked_links = 0usize;
    for rel_path in &tracked {
        let abs_path = repo_root.join(rel_path);
        let Ok(text) = std::fs::read_to_string(&abs_path) else {
            problems.push(format!("{rel_path}: cannot read tracked file"));
            continue;
        };
        let file_dir = abs_path.parent().map(Path::to_path_buf).unwrap_or_default();

        for link in scan_markdown_links(&text) {
            let target_no_fragment = link
                .target
                .split(['#', '?'])
                .next()
                .unwrap_or(&link.target)
                .trim();
            if is_out_of_scope_link_target(&link.target) || target_no_fragment.is_empty() {
                continue;
            }
            checked_links += 1;
            let candidate: PathBuf = file_dir.join(target_no_fragment);
            match candidate.canonicalize() {
                Ok(resolved) if resolved.starts_with(&repo_resolved) => {}
                Ok(_) => problems.push(format!(
                    "{rel_path}:{}: link escapes repo root: {target_no_fragment}",
                    link.line_no
                )),
                Err(_) => problems.push(format!(
                    "{rel_path}:{}: missing link target: {target_no_fragment}",
                    link.line_no
                )),
            }
        }
    }

    if problems.is_empty() {
        CheckOutcome {
            name,
            command: Some("git ls-files".to_string()),
            state: CheckState::Pass,
            summary: format!(
                "{} tracked doc(s), {checked_links} in-scope link(s) resolved",
                tracked.len()
            ),
        }
    } else {
        CheckOutcome {
            name,
            command: Some("git ls-files".to_string()),
            state: CheckState::Fail,
            summary: problems.join(" | "),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write(dir: &Path, rel: &str, content: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, content).unwrap();
    }

    fn project_md_with(dir: &Path, body: &str) {
        write(dir, ".dev/project.md", body);
    }

    #[test]
    fn is_gal_source_repo_detects_plugins_dir() {
        let tmp = TempDir::new().unwrap();
        assert!(!is_gal_source_repo(tmp.path()));
        std::fs::create_dir_all(tmp.path().join("plugins").join("gal-core")).unwrap();
        assert!(is_gal_source_repo(tmp.path()));
    }

    #[test]
    fn source_doc_existence_document_path_header_order_pass() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "README.md", "hi");
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Document | Path | Last Verified |\n| --- | --- | --- |\n| readme | `README.md` | 2026-07-16 |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn source_doc_existence_path_type_notes_header_order_pass() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "docs/naming.md", "hi");
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Path | Type | Notes |\n| --- | --- | --- |\n| `docs/naming.md` | docs | naming authority |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn source_doc_existence_empty_template_table_pass() {
        let tmp = TempDir::new().unwrap();
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Document | Path | Last Verified |\n| --- | --- | --- |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn source_doc_existence_missing_path_header_fails() {
        let tmp = TempDir::new().unwrap();
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Document | Notes |\n| --- | --- |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
    }

    #[test]
    fn source_doc_existence_duplicate_path_header_fails() {
        let tmp = TempDir::new().unwrap();
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Path | Path |\n| --- | --- |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
    }

    #[test]
    fn source_doc_existence_duplicate_row_fails() {
        let tmp = TempDir::new().unwrap();
        write(tmp.path(), "README.md", "hi");
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Path | Notes |\n| --- | --- |\n| `README.md` | a |\n| `README.md` | b |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("duplicate"));
    }

    #[test]
    fn source_doc_existence_missing_target_fails() {
        let tmp = TempDir::new().unwrap();
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Path | Notes |\n| --- | --- |\n| `docs/does-not-exist.md` | a |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("missing target"));
    }

    #[test]
    fn source_doc_existence_escape_fails() {
        let tmp = TempDir::new().unwrap();
        // A sibling directory outside the repo root that the row tries to escape into.
        let outside = tmp
            .path()
            .parent()
            .unwrap()
            .join("finalize-hygiene-escape-target");
        std::fs::write(&outside, "outside").unwrap();
        project_md_with(
            tmp.path(),
            "## Source Documents\n\n| Path | Notes |\n| --- | --- |\n| `../finalize-hygiene-escape-target` | a |\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("escapes"));
        let _ = std::fs::remove_file(&outside);
    }

    #[test]
    fn source_doc_existence_missing_section_fails() {
        let tmp = TempDir::new().unwrap();
        project_md_with(
            tmp.path(),
            "# Project\n\nNo source-documents section here.\n",
        );
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
    }

    #[test]
    fn source_doc_existence_unreadable_project_md_fails() {
        let tmp = TempDir::new().unwrap();
        let out = check_project_source_doc_existence(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
    }

    // --- contract-roster-parity ---

    fn gal_core_dir(tmp: &TempDir) -> std::path::PathBuf {
        tmp.path().join("plugins").join("gal-core")
    }

    /// A minimal, internally-consistent GAL-source-repo roster fixture: 2
    /// command dirs, 2 agent files, 3 template files — each exactly matched by
    /// its roster doc.
    fn write_valid_roster_fixture(tmp: &TempDir) {
        let core = gal_core_dir(tmp);
        write(&core, "commands/alpha/SKILL.template.md", "alpha skill");
        write(&core, "commands/beta/SKILL.template.md", "beta skill");
        write(
            &core,
            "commands/commands.md",
            "# Commands\n\n## Source Files\n\n| File | Purpose |\n| --- | --- |\n| `commands/alpha/SKILL.template.md` | Alpha |\n| `commands/beta/SKILL.template.md` | Beta |\n",
        );
        write(&core, "agents/golem-alpha.agent.md", "alpha agent");
        write(&core, "agents/golem-beta.agent.md", "beta agent");
        write(
            &core,
            "agents/agents.md",
            "# Agents\n\n## Agents And Responsibilities\n\n| Agent | Purpose |\n| --- | --- |\n| [golem-alpha](golem-alpha.agent.md) | Alpha |\n| [golem-beta](golem-beta.agent.md) | Beta |\n",
        );
        write(&core, "templates/one.md", "one");
        write(&core, "templates/two.md", "two");
        write(
            &core,
            "templates/templates.md",
            "# Templates\n\n## File Templates\n\n| Template | Purpose |\n| --- | --- |\n| [one.md](one.md) | One |\n\n## Other Files\n\n| File | Purpose |\n| --- | --- |\n| [two.md](two.md) | Two |\n| [templates.md](templates.md) | This file |\n",
        );
    }

    #[test]
    fn roster_parity_valid_fixture_passes() {
        let tmp = TempDir::new().unwrap();
        write_valid_roster_fixture(&tmp);
        let out = check_contract_roster_parity(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn roster_parity_missing_section_fails() {
        let tmp = TempDir::new().unwrap();
        write_valid_roster_fixture(&tmp);
        write(
            &gal_core_dir(&tmp).join("commands"),
            "commands.md",
            "# Commands\n\nNo Source Files section.\n",
        );
        let out = check_contract_roster_parity(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("commands: missing"));
    }

    #[test]
    fn roster_parity_duplicate_entry_fails() {
        let tmp = TempDir::new().unwrap();
        write_valid_roster_fixture(&tmp);
        write(
            &gal_core_dir(&tmp).join("agents"),
            "agents.md",
            "# Agents\n\n## Agents And Responsibilities\n\n| Agent | Purpose |\n| --- | --- |\n| [golem-alpha](golem-alpha.agent.md) | Alpha |\n| [golem-alpha](golem-alpha.agent.md) | Alpha again |\n| [golem-beta](golem-beta.agent.md) | Beta |\n",
        );
        let out = check_contract_roster_parity(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("agents: duplicate roster entry"));
    }

    #[test]
    fn roster_parity_phantom_entry_fails() {
        let tmp = TempDir::new().unwrap();
        write_valid_roster_fixture(&tmp);
        // Roster cites a template file that does not exist on disk.
        write(
            &gal_core_dir(&tmp).join("templates"),
            "templates.md",
            "# Templates\n\n## File Templates\n\n| Template | Purpose |\n| --- | --- |\n| [one.md](one.md) | One |\n| [ghost.md](ghost.md) | Ghost |\n\n## Other Files\n\n| File | Purpose |\n| --- | --- |\n| [two.md](two.md) | Two |\n| [templates.md](templates.md) | This file |\n",
        );
        let out = check_contract_roster_parity(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out
            .summary
            .contains("templates: phantom roster entry: ghost.md"));
    }

    #[test]
    fn roster_parity_unrostered_disk_entry_fails() {
        let tmp = TempDir::new().unwrap();
        write_valid_roster_fixture(&tmp);
        // A disk command directory the roster never mentions.
        write(
            &gal_core_dir(&tmp).join("commands").join("gamma"),
            "SKILL.template.md",
            "gamma skill",
        );
        let out = check_contract_roster_parity(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out
            .summary
            .contains("commands: unrostered disk entry: gamma"));
    }

    // --- state-bound ---

    fn state_md_with(tmp: &TempDir, body: &str) {
        write(tmp.path(), ".dev/state.md", body);
    }

    fn recent_closeouts_body(rows: usize) -> String {
        let mut out = String::from(
            "## Recent Close-outs\n\n| Date | Plan | Landing | Result |\n| --- | --- | --- | --- |\n",
        );
        for i in 0..rows {
            out.push_str(&format!(
                "| 2026-07-1{i} | Plan {i} | abc{i:03} | ABSORBED |\n"
            ));
        }
        out
    }

    #[test]
    fn state_bound_absent_section_full_pass() {
        let tmp = TempDir::new().unwrap();
        state_md_with(
            &tmp,
            "# GAL State\n\n## Active Plans\n\nNo Recent Close-outs section yet.\n",
        );
        let out = check_state_bound(tmp.path(), HygieneMode::Full);
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn state_bound_absent_section_hygiene_only_fails() {
        let tmp = TempDir::new().unwrap();
        state_md_with(
            &tmp,
            "# GAL State\n\n## Active Plans\n\nNo Recent Close-outs section yet.\n",
        );
        let out = check_state_bound(tmp.path(), HygieneMode::HygieneOnly);
        assert_eq!(out.state, CheckState::Fail);
    }

    #[test]
    fn state_bound_present_zero_one_two_rows_pass_both_modes() {
        for rows in 0..=2 {
            let tmp = TempDir::new().unwrap();
            state_md_with(&tmp, &recent_closeouts_body(rows));
            for mode in [HygieneMode::Full, HygieneMode::HygieneOnly] {
                let out = check_state_bound(tmp.path(), mode);
                assert_eq!(
                    out.state,
                    CheckState::Pass,
                    "rows={rows} mode={mode:?}: {}",
                    out.summary
                );
            }
        }
    }

    #[test]
    fn state_bound_three_rows_fails_both_modes() {
        let tmp = TempDir::new().unwrap();
        state_md_with(&tmp, &recent_closeouts_body(3));
        for mode in [HygieneMode::Full, HygieneMode::HygieneOnly] {
            let out = check_state_bound(tmp.path(), mode);
            assert_eq!(out.state, CheckState::Fail, "mode={mode:?}");
            assert!(out.summary.contains("exceed"));
        }
    }

    #[test]
    fn state_bound_exact_cap_passes_one_byte_over_fails() {
        let tmp = TempDir::new().unwrap();
        let base = recent_closeouts_body(1);
        let padded = base.clone() + &" ".repeat(STATE_MD_MAX_BYTES - base.len());
        assert_eq!(padded.len(), STATE_MD_MAX_BYTES);
        state_md_with(&tmp, &padded);
        let out = check_state_bound(tmp.path(), HygieneMode::Full);
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);

        let tmp2 = TempDir::new().unwrap();
        let over = padded + " ";
        assert_eq!(over.len(), STATE_MD_MAX_BYTES + 1);
        state_md_with(&tmp2, &over);
        let out2 = check_state_bound(tmp2.path(), HygieneMode::Full);
        assert_eq!(out2.state, CheckState::Fail);
        assert!(out2.summary.contains("exceeds"));
    }

    #[test]
    fn state_bound_exact_predicate_legacy_comment_fails() {
        let tmp = TempDir::new().unwrap();
        let mut body = recent_closeouts_body(1);
        body.push_str(
            "\n<!-- 2026-07-01: some-plan finalized on main (ABSORBED, plan files deleted). -->\n",
        );
        state_md_with(&tmp, &body);
        let out = check_state_bound(tmp.path(), HygieneMode::Full);
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("legacy comment"));
    }

    #[test]
    fn state_bound_marker_word_discussion_is_not_a_false_positive() {
        let tmp = TempDir::new().unwrap();
        let mut body = recent_closeouts_body(1);
        body.push_str(
            "\n<!-- A legacy close-out comment mentions finalized on main and ABSORBED, but this one is not date-prefixed. -->\n",
        );
        state_md_with(&tmp, &body);
        let out = check_state_bound(tmp.path(), HygieneMode::Full);
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn state_bound_crlf_normalized_before_size_check() {
        let tmp = TempDir::new().unwrap();
        let lf_body = recent_closeouts_body(1);
        let crlf_body = lf_body.replace('\n', "\r\n");
        state_md_with(&tmp, &crlf_body);
        let out = check_state_bound(tmp.path(), HygieneMode::Full);
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
        assert!(out.summary.contains(&format!("{} B", lf_body.len())));
    }

    #[test]
    fn state_bound_unreadable_file_fails_both_modes() {
        let tmp = TempDir::new().unwrap();
        for mode in [HygieneMode::Full, HygieneMode::HygieneOnly] {
            let out = check_state_bound(tmp.path(), mode);
            assert_eq!(out.state, CheckState::Fail, "mode={mode:?}");
        }
    }

    // --- doc-link-resolution ---

    fn make_git_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["init", "-q", root])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "config", "user.email", "t@t"])
            .output()
            .unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "config", "user.name", "t"])
            .output()
            .unwrap();
        tmp
    }

    fn git_stage_all(tmp: &TempDir) {
        let root = tmp.path().to_str().unwrap();
        std::process::Command::new("git")
            .args(["-C", root, "add", "-A"])
            .output()
            .unwrap();
    }

    #[test]
    fn doc_link_direct_child_of_scoped_dir_is_tracked() {
        // Regression: git's default (non-`:(glob)`) pathspec matching treats a
        // bare `**` as requiring >=1 full directory segment, so `docs/**/*.md`
        // alone would silently skip direct children of `docs/`. A broken link
        // in a direct-child file must still be caught.
        let tmp = make_git_repo();
        write(tmp.path(), "docs/direct-child.md", "[broken](missing.md)\n");
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("docs/direct-child.md:1"));
    }

    #[test]
    fn doc_link_valid_inline_link_passes() {
        let tmp = make_git_repo();
        write(tmp.path(), "README.md", "See [docs](docs/guide.md).\n");
        write(tmp.path(), "docs/guide.md", "# Guide\n");
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn doc_link_missing_target_fails() {
        let tmp = make_git_repo();
        write(tmp.path(), "README.md", "See [nope](docs/missing.md).\n");
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("missing link target"));
        assert!(out.summary.contains("README.md:1"));
    }

    #[test]
    fn doc_link_repo_escape_fails() {
        let tmp = make_git_repo();
        // A real file that exists, but only by escaping the repo root lexically.
        let outside = tmp
            .path()
            .parent()
            .unwrap()
            .join("doc-link-escape-target.md");
        std::fs::write(&outside, "outside").unwrap();
        write(
            tmp.path(),
            "docs/a.md",
            "See [escape](../../doc-link-escape-target.md).\n",
        );
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
        assert!(out.summary.contains("escapes repo root"));
        let _ = std::fs::remove_file(&outside);
    }

    #[test]
    fn doc_link_fragment_suffix_resolves_file_part_only() {
        let tmp = make_git_repo();
        write(
            tmp.path(),
            "README.md",
            "See [section](docs/guide.md#some-heading).\n",
        );
        write(tmp.path(), "docs/guide.md", "# Guide\n\n## Some Heading\n");
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn doc_link_same_document_anchor_out_of_scope() {
        let tmp = make_git_repo();
        write(tmp.path(), "README.md", "See [section](#some-heading).\n");
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn doc_link_fenced_code_block_excluded() {
        let tmp = make_git_repo();
        write(
            tmp.path(),
            "README.md",
            "```text\n[fake](does-not-exist.md)\n```\n",
        );
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn doc_link_inline_code_span_excluded() {
        let tmp = make_git_repo();
        write(
            tmp.path(),
            "README.md",
            "Example: `[fake](does-not-exist.md)` is just code.\n",
        );
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn doc_link_reference_style_link_resolves() {
        let tmp = make_git_repo();
        write(
            tmp.path(),
            "README.md",
            "See [the guide][guide-ref].\n\n[guide-ref]: docs/guide.md\n",
        );
        write(tmp.path(), "docs/guide.md", "# Guide\n");
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn doc_link_untracked_file_out_of_scope() {
        let tmp = make_git_repo();
        write(tmp.path(), "README.md", "Fine.\n");
        git_stage_all(&tmp);
        // Untracked scratch file with a broken link — never staged, so never scanned.
        write(
            tmp.path(),
            "docs/scratch.md",
            "[broken](does-not-exist.md)\n",
        );
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }

    #[test]
    fn doc_link_inventory_failure_fails_closed() {
        let tmp = TempDir::new().unwrap(); // not a git repo at all
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Fail);
    }

    #[test]
    fn doc_link_external_url_out_of_scope() {
        let tmp = make_git_repo();
        write(
            tmp.path(),
            "README.md",
            "See [external](https://example.com/nope) and [mail](mailto:a@b.com).\n",
        );
        git_stage_all(&tmp);
        let out = check_doc_link_resolution(tmp.path());
        assert_eq!(out.state, CheckState::Pass, "{}", out.summary);
    }
}
