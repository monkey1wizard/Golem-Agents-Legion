//! `gal state-merge` — finalize-internal deterministic resolver for
//! `.dev/state.md` merge conflicts whose conflicted path set is exactly
//! that one file.
//!
//! Scope: the full resolver — index-stage reading, CRLF normalization,
//! section splitting, row-keyed and plain three-way merging, post-merge
//! invariants, and the verified commit/rollback transaction.

use gal_engine::ExitCode;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A fail-closed reason. Every variant maps to a nonzero exit and a stderr
/// message; none of them mutate the worktree or index before returning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StateMergeError {
    /// The input contains a bare `\r` not part of a `\r\n` pair, after CRLF
    /// normalization.
    BareCarriageReturn,
    /// A `## ` heading is not one of the recognized section names.
    UnknownSection { heading: String },
    /// The same recognized `## ` heading appears more than once.
    DuplicateSection { heading: String },
    /// A row-keyed section's table header does not exactly match the
    /// contract-defined column list for that section.
    UnexpectedTableHeader {
        section: &'static str,
        found: Vec<String>,
    },
    /// A row-keyed section has no parseable table (missing header/separator).
    MissingTable { section: &'static str },
    /// A data row's cell count does not match the header's cell count.
    CellCountMismatch {
        section: &'static str,
        row: Vec<String>,
    },
    /// Two data rows in the same section share the same merge key.
    DuplicateRowKey { section: &'static str, key: String },
    /// The same key was changed to different content on both sides (or
    /// modified on one side and deleted on the other) relative to base.
    DivergentRowEdit { section: &'static str, key: String },
    /// One side changed the relative order of base rows that still survive
    /// in the merge result (Active Plans / Session Continuity only).
    ReorderedBaseKeys { section: &'static str },
    /// A plain-text region (preamble, a whole non-row-keyed section, or a
    /// row-keyed section's scaffolding) differs from base on both sides,
    /// and the two sides disagree with each other.
    DivergentPlainRegion { section: &'static str },
    /// A Recent Close-outs `Date` cell is not a valid `YYYY-MM-DD`.
    InvalidCloseoutDate { date: String },
    /// The assembled merged file exceeds the normalized-LF byte budget.
    ByteBudgetExceeded { bytes: usize },
    /// `git` itself could not be spawned.
    GitSpawnFailed { message: String },
    /// No merge is currently in progress (`MERGE_HEAD` does not resolve).
    NoActiveMerge,
    /// The unmerged path set is not exactly `{.dev/state.md}`.
    UnmergedPathSetMismatch { paths: Vec<String> },
    /// One of the three index stages (1=base, 2=ours, 3=theirs) is missing
    /// for the conflicted path (e.g. add/add, delete/modify).
    MissingStage { stage: u8 },
    /// A stage's blob is not valid UTF-8.
    NonUtf8Stage { stage: u8 },
    /// A step of the verified commit sequence failed for a reason that is
    /// not itself a git-spawn failure (readback mismatch, unexpected
    /// `git` output shape, postcondition not observed).
    PostconditionFailed { reason: String },
    /// Base, ours, and theirs do not have the same set of recognized
    /// sections in the same order — an unrecognized/mismatched structural
    /// shape the resolver refuses to guess about.
    SectionShapeMismatch,
}

impl StateMergeError {
    pub(crate) fn message(&self) -> String {
        match self {
            StateMergeError::BareCarriageReturn => {
                "input contains a bare CR not part of a CRLF pair".to_string()
            }
            StateMergeError::UnknownSection { heading } => {
                format!("unrecognized section heading: `## {heading}`")
            }
            StateMergeError::DuplicateSection { heading } => {
                format!("duplicate section heading: `## {heading}`")
            }
            StateMergeError::UnexpectedTableHeader { section, found } => {
                format!("`## {section}`: unexpected table header {found:?}")
            }
            StateMergeError::MissingTable { section } => {
                format!("`## {section}`: no parseable table (missing header/separator)")
            }
            StateMergeError::CellCountMismatch { section, row } => {
                format!("`## {section}`: row cell count mismatch: {row:?}")
            }
            StateMergeError::DuplicateRowKey { section, key } => {
                format!("`## {section}`: duplicate row key `{key}`")
            }
            StateMergeError::DivergentRowEdit { section, key } => {
                format!("`## {section}`: divergent edit on row key `{key}`")
            }
            StateMergeError::ReorderedBaseKeys { section } => {
                format!("`## {section}`: a side reordered surviving base row keys")
            }
            StateMergeError::DivergentPlainRegion { section } => {
                format!("`{section}`: divergent plain-region edit")
            }
            StateMergeError::InvalidCloseoutDate { date } => {
                format!("Recent Close-outs: invalid Date cell `{date}` (expected YYYY-MM-DD)")
            }
            StateMergeError::ByteBudgetExceeded { bytes } => {
                format!(
                    "merged state.md is {bytes} bytes, exceeding the {MAX_STATE_MD_BYTES}-byte budget"
                )
            }
            StateMergeError::GitSpawnFailed { message } => {
                format!("failed to spawn git: {message}")
            }
            StateMergeError::NoActiveMerge => {
                "no merge is currently in progress (MERGE_HEAD not found)".to_string()
            }
            StateMergeError::UnmergedPathSetMismatch { paths } => {
                format!("unmerged path set is not exactly {{{STATE_MD_PATH}}}: {paths:?}")
            }
            StateMergeError::MissingStage { stage } => {
                format!("missing index stage {stage} for {STATE_MD_PATH}")
            }
            StateMergeError::NonUtf8Stage { stage } => {
                format!("index stage {stage} for {STATE_MD_PATH} is not valid UTF-8")
            }
            StateMergeError::PostconditionFailed { reason } => reason.clone(),
            StateMergeError::SectionShapeMismatch => {
                "base/ours/theirs do not share the same recognized section shape".to_string()
            }
        }
    }
}

/// The whole-file byte budget for the merged `state.md`, per the bounded
/// session-state contract.
pub(crate) const MAX_STATE_MD_BYTES: usize = 16_384;

/// Normalize CRLF to LF, then reject any remaining bare CR.
pub(crate) fn normalize_lf(input: &str) -> Result<String, StateMergeError> {
    let normalized = input.replace("\r\n", "\n");
    if normalized.contains('\r') {
        return Err(StateMergeError::BareCarriageReturn);
    }
    Ok(normalized)
}

/// One of the section names `state.md` recognizes: the row-keyed sections
/// plus the plain-three-way sections. `Preamble` is not a `## ` heading; it
/// is everything before the first one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum SectionKind {
    Preamble,
    ActivePlans,
    RecentCloseouts,
    ParkedPlans,
    GlobalDecisions,
    Blockers,
    SessionContinuity,
    SessionExecutionContext,
}

impl SectionKind {
    /// Parse a `## ` heading's exact text into a recognized section kind.
    fn from_heading(heading: &str) -> Option<SectionKind> {
        match heading {
            "Active Plans" => Some(SectionKind::ActivePlans),
            "Recent Close-outs" => Some(SectionKind::RecentCloseouts),
            "Parked Plans" => Some(SectionKind::ParkedPlans),
            "Global Decisions" => Some(SectionKind::GlobalDecisions),
            "Blockers" => Some(SectionKind::Blockers),
            "Session Continuity" => Some(SectionKind::SessionContinuity),
            "Session Execution Context" => Some(SectionKind::SessionExecutionContext),
            _ => None,
        }
    }

    /// The heading text, for error messages, of a section kind that came
    /// from a known section (never called on `Preamble`).
    fn heading_text(self) -> &'static str {
        match self {
            SectionKind::Preamble => "",
            SectionKind::ActivePlans => "Active Plans",
            SectionKind::RecentCloseouts => "Recent Close-outs",
            SectionKind::ParkedPlans => "Parked Plans",
            SectionKind::GlobalDecisions => "Global Decisions",
            SectionKind::Blockers => "Blockers",
            SectionKind::SessionContinuity => "Session Continuity",
            SectionKind::SessionExecutionContext => "Session Execution Context",
        }
    }

    /// Row-keyed sections merge data rows by key; every other section merges
    /// as a plain three-way text region.
    fn is_row_keyed(self) -> bool {
        matches!(
            self,
            SectionKind::ActivePlans
                | SectionKind::RecentCloseouts
                | SectionKind::ParkedPlans
                | SectionKind::SessionContinuity
        )
    }

    /// The exact expected header cells for a row-keyed section's table.
    /// Empty for non-row-keyed kinds (never consulted for those).
    fn expected_header(self) -> &'static [&'static str] {
        match self {
            SectionKind::ActivePlans => &["Plan", "File", "Plan Phase", "Last Activity"],
            SectionKind::RecentCloseouts => &["Date", "Plan", "Landing", "Result"],
            SectionKind::ParkedPlans => &["Plan", "File", "Status", "Reason / Next"],
            SectionKind::SessionContinuity => &[
                "Plan",
                "Source Plan",
                "Last Session",
                "Stopped At",
                "Next Step",
                "Context",
            ],
            _ => &[],
        }
    }

    /// The 0-based column index that supplies a data row's merge key.
    fn key_column(self) -> usize {
        match self {
            SectionKind::ActivePlans | SectionKind::ParkedPlans => 1, // File
            SectionKind::RecentCloseouts => 1,                        // Plan
            SectionKind::SessionContinuity => 1,                      // Source Plan
            _ => 0,
        }
    }
}

/// One data row of a row-keyed table: its merge key (the trimmed contents of
/// the section's key column) and every cell in header order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableRow {
    pub(crate) key: String,
    pub(crate) cells: Vec<String>,
}

/// A parsed row-keyed table: the non-data scaffolding before the first data
/// row (including the heading's leading blank lines, the header row, and the
/// separator row), the data rows themselves, and the scaffolding after the
/// last data row (e.g. a trailing explanatory comment). `prefix` and
/// `suffix` merge as plain three-way text; `rows` merge by key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct KeyedTable {
    pub(crate) prefix: String,
    pub(crate) rows: Vec<TableRow>,
    pub(crate) suffix: String,
}

/// Parse one markdown table row line (`| a | b |`) into trimmed cells, or
/// `None` if the line is not shaped like a table row at all.
fn parse_table_row_cells(line: &str) -> Option<Vec<String>> {
    let trimmed = line.trim();
    if trimmed.len() < 2 || !trimmed.starts_with('|') || !trimmed.ends_with('|') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    Some(inner.split('|').map(|c| c.trim().to_string()).collect())
}

/// A separator row's cells are each composed only of one or more `-`.
fn is_separator_row(cells: &[String]) -> bool {
    !cells.is_empty()
        && cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-'))
}

/// Parse a row-keyed section's body into scaffolding + data rows. Scans for
/// the first table-shaped line (the header), requires it to match the
/// section's exact expected header, requires the next line to be a valid
/// separator, then consumes consecutive table-shaped lines as data rows
/// (validating cell count and rejecting duplicate keys) until a
/// non-table-shaped line ends the data-row run.
pub(crate) fn parse_keyed_table(
    kind: SectionKind,
    body: &str,
) -> Result<KeyedTable, StateMergeError> {
    let section = kind.heading_text();
    let expected_header = kind.expected_header();
    let lines: Vec<&str> = body.split('\n').collect();

    let header_idx = lines
        .iter()
        .position(|line| parse_table_row_cells(line).is_some())
        .ok_or(StateMergeError::MissingTable { section })?;
    let header_cells = parse_table_row_cells(lines[header_idx]).expect("checked above");
    if header_cells != expected_header {
        return Err(StateMergeError::UnexpectedTableHeader {
            section,
            found: header_cells,
        });
    }

    let separator_idx = header_idx + 1;
    let separator_cells = lines
        .get(separator_idx)
        .and_then(|line| parse_table_row_cells(line))
        .filter(|cells| cells.len() == expected_header.len() && is_separator_row(cells))
        .ok_or(StateMergeError::MissingTable { section })?;
    let _ = separator_cells;

    let mut rows = Vec::new();
    let mut keys_seen: Vec<String> = Vec::new();
    let mut idx = separator_idx + 1;
    while let Some(cells) = lines.get(idx).and_then(|line| parse_table_row_cells(line)) {
        if cells.len() != expected_header.len() {
            return Err(StateMergeError::CellCountMismatch {
                section,
                row: cells,
            });
        }
        let key = cells[kind.key_column()].trim().to_string();
        if keys_seen.contains(&key) {
            return Err(StateMergeError::DuplicateRowKey { section, key });
        }
        keys_seen.push(key.clone());
        rows.push(TableRow { key, cells });
        idx += 1;
    }

    let prefix = lines[..=separator_idx].join("\n") + "\n";
    let suffix = if idx < lines.len() {
        lines[idx..].join("\n")
    } else {
        String::new()
    };

    Ok(KeyedTable {
        prefix,
        rows,
        suffix,
    })
}

/// Row-keyed three-way merge. `check_reorder` gates the surviving-base-key
/// reorder rule (Active Plans / Session Continuity only).
///
/// Per key present in base: unchanged-on-one-side takes the other side;
/// identical changes on both sides take that change; anything else
/// (divergent edit, or edit-vs-delete) fails closed. Per key absent from
/// base: present on one side is an addition; present on both with identical
/// content is a single addition; present on both with different content
/// fails closed as a divergent addition.
///
/// Output order: base order for surviving base keys; each addition is
/// anchored immediately after the nearest earlier surviving base key in its
/// own side's list (or at the very start if none); additions sharing an
/// anchor put ours before theirs.
pub(crate) fn merge_keyed_rows(
    section: &'static str,
    base: &[TableRow],
    ours: &[TableRow],
    theirs: &[TableRow],
    check_reorder: bool,
) -> Result<Vec<TableRow>, StateMergeError> {
    use std::collections::BTreeMap;

    let base_keys: Vec<String> = base.iter().map(|r| r.key.clone()).collect();
    let ours_keys: Vec<String> = ours.iter().map(|r| r.key.clone()).collect();
    let theirs_keys: Vec<String> = theirs.iter().map(|r| r.key.clone()).collect();

    let base_map: BTreeMap<&str, &TableRow> = base.iter().map(|r| (r.key.as_str(), r)).collect();
    let ours_map: BTreeMap<&str, &TableRow> = ours.iter().map(|r| (r.key.as_str(), r)).collect();
    let theirs_map: BTreeMap<&str, &TableRow> =
        theirs.iter().map(|r| (r.key.as_str(), r)).collect();

    let mut all_keys: Vec<String> = Vec::new();
    for k in base_keys
        .iter()
        .chain(ours_keys.iter())
        .chain(theirs_keys.iter())
    {
        if !all_keys.contains(k) {
            all_keys.push(k.clone());
        }
    }

    let mut outcome: BTreeMap<String, Option<TableRow>> = BTreeMap::new();
    for key in &all_keys {
        let b = base_map.get(key.as_str()).copied();
        let o = ours_map.get(key.as_str()).copied();
        let t = theirs_map.get(key.as_str()).copied();

        let resolved: Option<TableRow> = if let Some(base_row) = b {
            let o_cells = o.map(|r| &r.cells);
            let t_cells = t.map(|r| &r.cells);
            let base_cells = Some(&base_row.cells);
            if o_cells == base_cells {
                t.cloned()
            } else if t_cells == base_cells || o_cells == t_cells {
                o.cloned()
            } else {
                return Err(StateMergeError::DivergentRowEdit {
                    section,
                    key: key.clone(),
                });
            }
        } else {
            match (o, t) {
                (Some(orow), Some(trow)) => {
                    if orow.cells == trow.cells {
                        Some(orow.clone())
                    } else {
                        return Err(StateMergeError::DivergentRowEdit {
                            section,
                            key: key.clone(),
                        });
                    }
                }
                (Some(orow), None) => Some(orow.clone()),
                (None, Some(trow)) => Some(trow.clone()),
                (None, None) => None,
            }
        };
        outcome.insert(key.clone(), resolved);
    }

    let surviving_base: Vec<String> = base_keys
        .iter()
        .filter(|k| outcome.get(*k).is_some_and(|o| o.is_some()))
        .cloned()
        .collect();
    let surviving_set: std::collections::BTreeSet<String> =
        surviving_base.iter().cloned().collect();

    if check_reorder {
        let ours_surviving: Vec<&String> = ours_keys
            .iter()
            .filter(|k| surviving_set.contains(*k))
            .collect();
        let theirs_surviving: Vec<&String> = theirs_keys
            .iter()
            .filter(|k| surviving_set.contains(*k))
            .collect();
        let base_surviving: Vec<&String> = surviving_base.iter().collect();
        if ours_surviving != base_surviving || theirs_surviving != base_surviving {
            return Err(StateMergeError::ReorderedBaseKeys { section });
        }
    }

    fn anchor_for(
        idx: usize,
        side_keys: &[String],
        surviving: &std::collections::BTreeSet<String>,
    ) -> Option<String> {
        side_keys[..idx]
            .iter()
            .rev()
            .find(|k| surviving.contains(*k))
            .cloned()
    }

    let is_new_survivor = |k: &str| {
        !base_keys.contains(&k.to_string()) && outcome.get(k).is_some_and(|o| o.is_some())
    };

    let mut shared_added: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut ours_additions: Vec<(Option<String>, String)> = Vec::new();
    for (idx, k) in ours_keys.iter().enumerate() {
        if is_new_survivor(k) {
            let anchor = anchor_for(idx, &ours_keys, &surviving_set);
            ours_additions.push((anchor, k.clone()));
            if theirs_keys.contains(k) {
                shared_added.insert(k.clone());
            }
        }
    }
    let mut theirs_additions: Vec<(Option<String>, String)> = Vec::new();
    for (idx, k) in theirs_keys.iter().enumerate() {
        if is_new_survivor(k) && !shared_added.contains(k) {
            let anchor = anchor_for(idx, &theirs_keys, &surviving_set);
            theirs_additions.push((anchor, k.clone()));
        }
    }

    let mut final_order: Vec<String> = Vec::new();
    for (anchor, k) in &ours_additions {
        if anchor.is_none() {
            final_order.push(k.clone());
        }
    }
    for (anchor, k) in &theirs_additions {
        if anchor.is_none() {
            final_order.push(k.clone());
        }
    }
    for base_key in &surviving_base {
        final_order.push(base_key.clone());
        for (anchor, k) in &ours_additions {
            if anchor.as_ref() == Some(base_key) {
                final_order.push(k.clone());
            }
        }
        for (anchor, k) in &theirs_additions {
            if anchor.as_ref() == Some(base_key) {
                final_order.push(k.clone());
            }
        }
    }

    Ok(final_order
        .iter()
        .map(|k| {
            outcome
                .get(k)
                .cloned()
                .flatten()
                .expect("survivor has a resolved row")
        })
        .collect())
}

/// One ordered region of a split `state.md`: either the preamble (no
/// heading) or a recognized `## ` section with its heading text and body
/// (everything up to, but not including, the next `## ` heading or EOF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) kind: SectionKind,
    /// The exact heading text (e.g. `Active Plans`), `None` for the preamble.
    pub(crate) heading: Option<String>,
    /// Body text, LF-normalized, not including the heading line itself.
    pub(crate) body: String,
}

/// Split LF-normalized `state.md` content into an ordered preamble plus one
/// section per recognized `## ` heading. Unknown or duplicate headings fail
/// closed before any section content is returned.
pub(crate) fn split_sections(input: &str) -> Result<Vec<Section>, StateMergeError> {
    let mut sections: Vec<Section> = Vec::new();
    let mut seen: Vec<SectionKind> = Vec::new();
    let mut current_kind = SectionKind::Preamble;
    let mut current_heading: Option<String> = None;
    let mut current_body = String::new();

    for line in input.split('\n') {
        if let Some(heading) = line.strip_prefix("## ") {
            let heading = heading.trim_end();
            let kind = SectionKind::from_heading(heading).ok_or_else(|| {
                StateMergeError::UnknownSection {
                    heading: heading.to_string(),
                }
            })?;
            if seen.contains(&kind) {
                return Err(StateMergeError::DuplicateSection {
                    heading: heading.to_string(),
                });
            }
            sections.push(Section {
                kind: current_kind,
                heading: current_heading.take(),
                body: current_body.clone(),
            });
            seen.push(kind);
            current_kind = kind;
            current_heading = Some(heading.to_string());
            current_body = String::new();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }
    sections.push(Section {
        kind: current_kind,
        heading: current_heading,
        body: current_body,
    });

    Ok(sections)
}

/// Plain three-way merge for a whole text region: one side unchanged from
/// base takes the other side; identical changes on both sides collapse to
/// one; anything else (both sides changed, and differently) fails closed.
pub(crate) fn merge_plain_region(
    section: &'static str,
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<String, StateMergeError> {
    if ours == base {
        Ok(theirs.to_string())
    } else if theirs == base || ours == theirs {
        Ok(ours.to_string())
    } else {
        Err(StateMergeError::DivergentPlainRegion { section })
    }
}

/// Strict `YYYY-MM-DD` format check: four digits, `-`, two digits, `-`, two
/// digits, month in `01..=12`, day in `01..=31`. This validates shape, not
/// full calendar correctness (e.g. `2026-02-30` passes) — sufficient for a
/// fail-closed guard against malformed or non-date content in the cell.
fn is_valid_iso_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let all_digits = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
    let (year, month, day) = (&date[0..4], &date[5..7], &date[8..10]);
    if !all_digits(year) || !all_digits(month) || !all_digits(day) {
        return false;
    }
    matches!(month.parse::<u32>(), Ok(1..=12)) && matches!(day.parse::<u32>(), Ok(1..=31))
}

/// Enforce Recent Close-outs's post-merge invariants: every `Date` cell must
/// be a valid `YYYY-MM-DD`; rows sort newest-first with a stable sort (ties
/// preserve input order, which the row-keyed merge already orders
/// ours-before-theirs at a shared anchor); result truncates to the newest
/// two rows. Fails closed on the first invalid date, before any sort.
pub(crate) fn enforce_recent_closeouts_invariants(
    rows: Vec<TableRow>,
    date_column: usize,
) -> Result<Vec<TableRow>, StateMergeError> {
    for row in &rows {
        let date = row
            .cells
            .get(date_column)
            .map(String::as_str)
            .unwrap_or_default();
        if !is_valid_iso_date(date) {
            return Err(StateMergeError::InvalidCloseoutDate {
                date: date.to_string(),
            });
        }
    }
    let mut sorted = rows;
    sorted.sort_by(|a, b| b.cells[date_column].cmp(&a.cells[date_column]));
    sorted.truncate(2);
    Ok(sorted)
}

/// Enforce the whole-file normalized-LF byte budget before any write.
pub(crate) fn enforce_byte_budget(content: &str) -> Result<(), StateMergeError> {
    if content.len() > MAX_STATE_MD_BYTES {
        return Err(StateMergeError::ByteBudgetExceeded {
            bytes: content.len(),
        });
    }
    Ok(())
}

/// Repo-relative path of the one file this resolver ever touches.
pub(crate) const STATE_MD_PATH: &str = ".dev/state.md";

/// The three index stages read for a `.dev/state.md` merge conflict, each
/// LF-normalized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageContents {
    pub(crate) base: String,
    pub(crate) ours: String,
    pub(crate) theirs: String,
}

/// Run `git` in `workdir`, returning the raw output (no trimming — callers
/// that need blob bytes exactly must not lose a trailing newline).
fn run_git(workdir: &Path, args: &[&str]) -> Result<std::process::Output, StateMergeError> {
    Command::new("git")
        .current_dir(workdir)
        .args(args)
        .output()
        .map_err(|e| StateMergeError::GitSpawnFailed {
            message: e.to_string(),
        })
}

/// Independently verify a merge is in progress and that the unmerged path
/// set is exactly `{.dev/state.md}`, before any stage is read. This is the
/// resolver's own re-check of the invocation boundary — it does not trust
/// the caller (finalize or a direct invocation) to have gated correctly.
pub(crate) fn require_active_conflict(workdir: &Path) -> Result<(), StateMergeError> {
    let merge_head = run_git(workdir, &["rev-parse", "--verify", "-q", "MERGE_HEAD"])?;
    if !merge_head.status.success() {
        return Err(StateMergeError::NoActiveMerge);
    }

    let diff = run_git(workdir, &["diff", "--name-only", "--diff-filter=U", "-z"])?;
    if !diff.status.success() {
        return Err(StateMergeError::UnmergedPathSetMismatch { paths: Vec::new() });
    }
    let stdout = String::from_utf8_lossy(&diff.stdout);
    let paths: Vec<String> = stdout
        .split('\0')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect();
    if paths != [STATE_MD_PATH.to_string()] {
        return Err(StateMergeError::UnmergedPathSetMismatch { paths });
    }
    Ok(())
}

/// Read and LF-normalize one index stage (1=base, 2=ours, 3=theirs) of
/// `.dev/state.md`. A nonzero `git show` exit means the stage is missing
/// (add/add has no stage 1; delete/modify has no stage for the deleted
/// side) — fail closed rather than guessing content.
fn read_stage(workdir: &Path, stage: u8) -> Result<String, StateMergeError> {
    let spec = format!(":{stage}:{STATE_MD_PATH}");
    let output = run_git(workdir, &["show", &spec])?;
    if !output.status.success() {
        return Err(StateMergeError::MissingStage { stage });
    }
    let raw =
        String::from_utf8(output.stdout).map_err(|_| StateMergeError::NonUtf8Stage { stage })?;
    normalize_lf(&raw)
}

/// Verify the invocation boundary, then read and LF-normalize all three
/// stages of `.dev/state.md`.
pub(crate) fn read_all_stages(workdir: &Path) -> Result<StageContents, StateMergeError> {
    require_active_conflict(workdir)?;
    Ok(StageContents {
        base: read_stage(workdir, 1)?,
        ours: read_stage(workdir, 2)?,
        theirs: read_stage(workdir, 3)?,
    })
}

/// One unmerged index entry for `.dev/state.md`: the fields
/// `git ls-files -u` reports (`<mode> <object_id> <stage>`), kept as
/// strings since they are only ever fed back verbatim into
/// `git update-index --index-info`, never interpreted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StageEntry {
    pub(crate) mode: String,
    pub(crate) object_id: String,
    pub(crate) stage: String,
}

/// Test-only fault-injection seam: forces one phase of the verified commit
/// sequence to fail without reproducing a real disk/OS failure. Every field
/// is `false` (no injected fault) in production use.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct FaultSeam {
    pub(crate) fail_candidate_verify: bool,
    pub(crate) fail_backup_move: bool,
    pub(crate) fail_install: bool,
    pub(crate) fail_install_verify: bool,
    pub(crate) fail_git_add: bool,
    pub(crate) fail_postcondition: bool,
    pub(crate) fail_restore: bool,
}

/// How a verified commit attempt failed, carrying the honest classification
/// the caller needs to choose the exact stderr marker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CommitFailure {
    /// Failed before any mutation began; `state.md` and the index are
    /// byte-identical to before the call.
    PreMutation(String),
    /// Failed after mutation began, but the rollback was verified exact
    /// (worktree bytes and index stage entries both match the pre-call
    /// snapshot).
    RolledBack(String),
    /// Failed after mutation began, and the rollback could not be proven
    /// exact. The repository may be in a different state than before the
    /// call; never treat this as inert.
    RollbackUnconfirmed(String),
}

impl CommitFailure {
    /// The stable stderr marker for this outcome.
    pub(crate) fn marker(&self) -> &'static str {
        match self {
            CommitFailure::PreMutation(_) | CommitFailure::RolledBack(_) => {
                "STATE_MERGE: unresolved"
            }
            CommitFailure::RollbackUnconfirmed(_) => "STATE_MERGE: rollback-unconfirmed",
        }
    }

    /// The human-readable detail behind the classification.
    pub(crate) fn detail(&self) -> &str {
        match self {
            CommitFailure::PreMutation(m)
            | CommitFailure::RolledBack(m)
            | CommitFailure::RollbackUnconfirmed(m) => m,
        }
    }
}

/// Snapshot `.dev/state.md`'s unmerged index entries via
/// `git ls-files -u -- <path>`, parsing each `<mode> <object> <stage>\t<path>`
/// line.
fn snapshot_unmerged_entries(
    workdir: &Path,
    path: &str,
) -> Result<Vec<StageEntry>, StateMergeError> {
    let out = run_git(workdir, &["ls-files", "-u", "--", path])?;
    if !out.status.success() {
        return Err(StateMergeError::PostconditionFailed {
            reason: format!(
                "git ls-files -u failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ),
        });
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut entries = Vec::new();
    for line in stdout.lines() {
        let (meta, _path) =
            line.split_once('\t')
                .ok_or_else(|| StateMergeError::PostconditionFailed {
                    reason: format!("unexpected `git ls-files -u` line: {line}"),
                })?;
        let parts: Vec<&str> = meta.split(' ').collect();
        if parts.len() != 3 {
            return Err(StateMergeError::PostconditionFailed {
                reason: format!("unexpected `git ls-files -u` line: {line}"),
            });
        }
        entries.push(StageEntry {
            mode: parts[0].to_string(),
            object_id: parts[1].to_string(),
            stage: parts[2].to_string(),
        });
    }
    Ok(entries)
}

/// Feed `entries` back into the index via `git update-index --index-info`,
/// first clearing any stage-0 entry the failed attempt may have left behind
/// (`--ignore-unmatch` makes this a no-op when there is nothing to clear).
fn reconstruct_index_entries(
    workdir: &Path,
    entries: &[StageEntry],
) -> Result<(), StateMergeError> {
    let _ = run_git(
        workdir,
        &["rm", "--cached", "--ignore-unmatch", "--", STATE_MD_PATH],
    )?;

    let mut input = String::new();
    for e in entries {
        input.push_str(&format!(
            "{} {} {}\t{}\n",
            e.mode, e.object_id, e.stage, STATE_MD_PATH
        ));
    }

    use std::io::Write;
    let mut child = Command::new("git")
        .current_dir(workdir)
        .args(["update-index", "--index-info"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| StateMergeError::GitSpawnFailed {
            message: e.to_string(),
        })?;
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(input.as_bytes())
        .map_err(|e| StateMergeError::GitSpawnFailed {
            message: e.to_string(),
        })?;
    let out = child
        .wait_with_output()
        .map_err(|e| StateMergeError::GitSpawnFailed {
            message: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(StateMergeError::PostconditionFailed {
            reason: format!(
                "git update-index --index-info failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ),
        });
    }
    Ok(())
}

/// `git hash-object --stdin`, for comparing staged blob ids against
/// in-memory content without writing a throwaway loose object ourselves.
fn hash_object(workdir: &Path, content: &[u8]) -> Result<String, StateMergeError> {
    use std::io::Write;
    let mut child = Command::new("git")
        .current_dir(workdir)
        .args(["hash-object", "--stdin"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| StateMergeError::GitSpawnFailed {
            message: e.to_string(),
        })?;
    child
        .stdin
        .take()
        .expect("stdin was piped")
        .write_all(content)
        .map_err(|e| StateMergeError::GitSpawnFailed {
            message: e.to_string(),
        })?;
    let out = child
        .wait_with_output()
        .map_err(|e| StateMergeError::GitSpawnFailed {
            message: e.to_string(),
        })?;
    if !out.status.success() {
        return Err(StateMergeError::PostconditionFailed {
            reason: format!(
                "git hash-object failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ),
        });
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// A unique path beside `target`, for the candidate/backup temp files —
/// same directory (same filesystem) so the later rename is a same-volume
/// move, not a copy.
fn unique_sibling_path(target: &Path, label: &str) -> PathBuf {
    let dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("state.md");
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    dir.join(format!(
        ".{file_name}.state-merge-{label}-{}-{nanos}",
        std::process::id()
    ))
}

/// Verify the exact success postcondition: no unmerged entries remain for
/// `path`, exactly one stage-0 entry whose blob equals `expected`, and the
/// worktree bytes at `path` equal `expected`.
fn verify_postcondition(
    workdir: &Path,
    path: &str,
    expected: &[u8],
) -> Result<(), StateMergeError> {
    let unmerged = snapshot_unmerged_entries(workdir, path)?;
    if !unmerged.is_empty() {
        return Err(StateMergeError::PostconditionFailed {
            reason: "unmerged index entries remain after git add".to_string(),
        });
    }

    let ls = run_git(workdir, &["ls-files", "-s", "--", path])?;
    if !ls.status.success() {
        return Err(StateMergeError::PostconditionFailed {
            reason: format!(
                "git ls-files -s failed: {}",
                String::from_utf8_lossy(&ls.stderr)
            ),
        });
    }
    let stdout = String::from_utf8_lossy(&ls.stdout);
    let lines: Vec<&str> = stdout.lines().collect();
    if lines.len() != 1 {
        return Err(StateMergeError::PostconditionFailed {
            reason: format!("expected exactly one stage-0 entry, got {}", lines.len()),
        });
    }
    let (meta, _) =
        lines[0]
            .split_once('\t')
            .ok_or_else(|| StateMergeError::PostconditionFailed {
                reason: "unexpected `git ls-files -s` line".to_string(),
            })?;
    let parts: Vec<&str> = meta.split(' ').collect();
    if parts.len() != 3 || parts[2] != "0" {
        return Err(StateMergeError::PostconditionFailed {
            reason: "expected a single stage-0 entry".to_string(),
        });
    }
    let staged_oid = parts[1];
    let expected_oid = hash_object(workdir, expected)?;
    if staged_oid != expected_oid {
        return Err(StateMergeError::PostconditionFailed {
            reason: "staged blob does not match candidate content".to_string(),
        });
    }

    let worktree_bytes =
        std::fs::read(workdir.join(path)).map_err(|e| StateMergeError::PostconditionFailed {
            reason: format!("read back worktree bytes: {e}"),
        })?;
    if worktree_bytes != expected {
        return Err(StateMergeError::PostconditionFailed {
            reason: "worktree bytes do not match candidate content".to_string(),
        });
    }
    Ok(())
}

/// Restore the worktree and index to `original_bytes`/`snapshot_entries`
/// after a failure past the point of mutation. Returns `Err` only when the
/// restore itself could not be verified exact — the caller must then emit
/// `rollback-unconfirmed`, never claim inertness.
fn restore_after_failure(
    workdir: &Path,
    state_path: &Path,
    backup_path: &Path,
    candidate_path: &Path,
    original_bytes: &[u8],
    snapshot_entries: &[StageEntry],
    seam: FaultSeam,
) -> Result<(), String> {
    if seam.fail_restore {
        return Err("injected restore failure".to_string());
    }

    if backup_path.exists() {
        let _ = std::fs::remove_file(state_path);
        std::fs::rename(backup_path, state_path)
            .map_err(|e| format!("restore worktree bytes: {e}"))?;
    }
    let _ = std::fs::remove_file(candidate_path);

    let current_bytes =
        std::fs::read(state_path).map_err(|e| format!("verify restored worktree bytes: {e}"))?;
    if current_bytes != original_bytes {
        return Err("restored worktree bytes do not match the pre-call snapshot".to_string());
    }

    let current_entries =
        snapshot_unmerged_entries(workdir, STATE_MD_PATH).map_err(|e| e.message())?;
    if current_entries != snapshot_entries {
        reconstruct_index_entries(workdir, snapshot_entries).map_err(|e| e.message())?;
        let reverified =
            snapshot_unmerged_entries(workdir, STATE_MD_PATH).map_err(|e| e.message())?;
        if reverified != snapshot_entries {
            return Err("restored index entries do not match the pre-call snapshot".to_string());
        }
    }

    Ok(())
}

/// The verified backup/install/commit transaction: snapshot worktree bytes
/// and unmerged index entries; write
/// and read back a unique candidate; backup-move the conflicted file;
/// install and read back the candidate; `git add`; verify the exact success
/// postcondition. Any failure after the backup-move restores the worktree
/// and index from the snapshot and reports whether that restore was itself
/// verified.
pub(crate) fn commit_merged_state(
    workdir: &Path,
    merged_content: &str,
    seam: FaultSeam,
) -> Result<(), CommitFailure> {
    let state_path = workdir.join(STATE_MD_PATH);

    let original_bytes = std::fs::read(&state_path)
        .map_err(|e| CommitFailure::PreMutation(format!("read worktree bytes: {e}")))?;
    let snapshot_entries = snapshot_unmerged_entries(workdir, STATE_MD_PATH)
        .map_err(|e| CommitFailure::PreMutation(e.message()))?;

    let candidate_path = unique_sibling_path(&state_path, "candidate");
    let candidate_result: Result<(), String> = (|| {
        std::fs::write(&candidate_path, merged_content)
            .map_err(|e| format!("write candidate: {e}"))?;
        if seam.fail_candidate_verify {
            return Err("injected candidate verify failure".to_string());
        }
        let readback =
            std::fs::read(&candidate_path).map_err(|e| format!("read back candidate: {e}"))?;
        if readback != merged_content.as_bytes() {
            return Err("candidate readback mismatch".to_string());
        }
        Ok(())
    })();
    if let Err(msg) = candidate_result {
        let _ = std::fs::remove_file(&candidate_path);
        return Err(CommitFailure::PreMutation(msg));
    }

    let backup_path = unique_sibling_path(&state_path, "backup");
    let attempt: Result<(), String> = (|| {
        if seam.fail_backup_move {
            return Err("injected backup-move failure".to_string());
        }
        std::fs::rename(&state_path, &backup_path).map_err(|e| format!("backup move: {e}"))?;

        if seam.fail_install {
            return Err("injected install failure".to_string());
        }
        std::fs::rename(&candidate_path, &state_path).map_err(|e| format!("install: {e}"))?;

        if seam.fail_install_verify {
            return Err("injected install verify failure".to_string());
        }
        let installed =
            std::fs::read(&state_path).map_err(|e| format!("read back installed: {e}"))?;
        if installed != merged_content.as_bytes() {
            return Err("installed readback mismatch".to_string());
        }

        if seam.fail_git_add {
            return Err("injected git add failure".to_string());
        }
        let add_out = run_git(workdir, &["add", "--", STATE_MD_PATH]).map_err(|e| e.message())?;
        if !add_out.status.success() {
            return Err(format!(
                "git add failed: {}",
                String::from_utf8_lossy(&add_out.stderr)
            ));
        }

        if seam.fail_postcondition {
            return Err("injected postcondition failure".to_string());
        }
        verify_postcondition(workdir, STATE_MD_PATH, merged_content.as_bytes())
            .map_err(|e| e.message())?;

        Ok(())
    })();

    match attempt {
        Ok(()) => {
            if backup_path.exists() {
                if let Err(e) = std::fs::remove_file(&backup_path) {
                    eprintln!(
                        "gal state-merge: warning: failed to clean up backup residue {}: {e}",
                        backup_path.display()
                    );
                }
            }
            Ok(())
        }
        Err(msg) => match restore_after_failure(
            workdir,
            &state_path,
            &backup_path,
            &candidate_path,
            &original_bytes,
            &snapshot_entries,
            seam,
        ) {
            Ok(()) => Err(CommitFailure::RolledBack(msg)),
            Err(rollback_msg) => Err(CommitFailure::RollbackUnconfirmed(format!(
                "{msg}; rollback also failed: {rollback_msg}"
            ))),
        },
    }
}

/// Render one merged data row back to its markdown table line.
fn render_row(row: &TableRow) -> String {
    format!("| {} |\n", row.cells.join(" | "))
}

/// Merge one row-keyed section's already-parsed table across base/ours/
/// theirs into its final body text (prefix + rendered rows + suffix).
fn merge_keyed_section_body(
    kind: SectionKind,
    base_body: &str,
    ours_body: &str,
    theirs_body: &str,
) -> Result<String, StateMergeError> {
    let section = kind.heading_text();
    let base_table = parse_keyed_table(kind, base_body)?;
    let ours_table = parse_keyed_table(kind, ours_body)?;
    let theirs_table = parse_keyed_table(kind, theirs_body)?;

    let check_reorder = matches!(
        kind,
        SectionKind::ActivePlans | SectionKind::SessionContinuity
    );
    let mut rows = merge_keyed_rows(
        section,
        &base_table.rows,
        &ours_table.rows,
        &theirs_table.rows,
        check_reorder,
    )?;
    if kind == SectionKind::RecentCloseouts {
        // Date is column 0 in the Recent Close-outs header.
        rows = enforce_recent_closeouts_invariants(rows, 0)?;
    }

    let prefix = merge_plain_region(
        section,
        &base_table.prefix,
        &ours_table.prefix,
        &theirs_table.prefix,
    )?;
    let suffix = merge_plain_region(
        section,
        &base_table.suffix,
        &ours_table.suffix,
        &theirs_table.suffix,
    )?;

    let mut body = prefix;
    for row in &rows {
        body.push_str(&render_row(row));
    }
    body.push_str(&suffix);
    Ok(body)
}

/// Merge the whole LF-normalized `state.md` across base/ours/theirs:
/// section-split each side, require an identical recognized section shape
/// on all three, merge each section by its kind (row-keyed vs plain),
/// reassemble in base's section order, then enforce the whole-file byte
/// budget before any write.
pub(crate) fn merge_full_state(
    base: &str,
    ours: &str,
    theirs: &str,
) -> Result<String, StateMergeError> {
    let base_secs = split_sections(base)?;
    let ours_secs = split_sections(ours)?;
    let theirs_secs = split_sections(theirs)?;

    let base_kinds: Vec<SectionKind> = base_secs.iter().map(|s| s.kind).collect();
    let ours_kinds: Vec<SectionKind> = ours_secs.iter().map(|s| s.kind).collect();
    let theirs_kinds: Vec<SectionKind> = theirs_secs.iter().map(|s| s.kind).collect();
    if ours_kinds != base_kinds || theirs_kinds != base_kinds {
        return Err(StateMergeError::SectionShapeMismatch);
    }

    let base_map: std::collections::BTreeMap<SectionKind, &Section> =
        base_secs.iter().map(|s| (s.kind, s)).collect();
    let ours_map: std::collections::BTreeMap<SectionKind, &Section> =
        ours_secs.iter().map(|s| (s.kind, s)).collect();
    let theirs_map: std::collections::BTreeMap<SectionKind, &Section> =
        theirs_secs.iter().map(|s| (s.kind, s)).collect();

    let mut output = String::new();
    for kind in &base_kinds {
        let b = base_map[kind];
        let o = ours_map[kind];
        let t = theirs_map[kind];

        let merged_body = if *kind == SectionKind::Preamble {
            merge_plain_region("preamble", &b.body, &o.body, &t.body)?
        } else if kind.is_row_keyed() {
            merge_keyed_section_body(*kind, &b.body, &o.body, &t.body)?
        } else {
            merge_plain_region(kind.heading_text(), &b.body, &o.body, &t.body)?
        };

        if *kind != SectionKind::Preamble {
            output.push_str("## ");
            output.push_str(kind.heading_text());
            output.push('\n');
        }
        output.push_str(&merged_body);
    }

    enforce_byte_budget(&output)?;
    Ok(output)
}

/// Read the three stages, merge them, and commit — the full resolver flow.
/// Every failure before `commit_merged_state` is pre-mutation by
/// construction (reading stages and merging are pure/read-only).
fn resolve_and_commit(workdir: &Path) -> Result<(), CommitFailure> {
    let stages = read_all_stages(workdir).map_err(|e| CommitFailure::PreMutation(e.message()))?;
    let merged = merge_full_state(&stages.base, &stages.ours, &stages.theirs)
        .map_err(|e| CommitFailure::PreMutation(e.message()))?;
    commit_merged_state(workdir, &merged, FaultSeam::default())
}

/// `gal state-merge` — zero arguments expected. Extra arguments are a usage
/// error. Resolves the current `.dev/state.md` merge conflict end to end:
/// exit 0 only on an observed staged success; any failure prints the exact
/// `STATE_MERGE: unresolved` or `STATE_MERGE: rollback-unconfirmed` marker.
pub(crate) fn cmd_state_merge(args: &[String]) -> ExitCode {
    // args[0] is the "state-merge" subcommand token itself; no further
    // arguments are accepted.
    if args.len() > 1 {
        eprintln!("usage: gal state-merge");
        return ExitCode::Usage;
    }

    let workdir = match std::env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("gal state-merge: cannot determine working directory: {e}");
            return ExitCode::Error;
        }
    };

    match resolve_and_commit(&workdir) {
        Ok(()) => {
            println!("gal state-merge: resolved and staged {STATE_MD_PATH}");
            ExitCode::Success
        }
        Err(failure) => {
            eprintln!("{}", failure.marker());
            eprintln!("gal state-merge: {}", failure.detail());
            ExitCode::Error
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── merge_plain_region ───────────────────────────────────────────────

    #[test]
    fn merge_plain_region_takes_one_sided_change() {
        assert_eq!(
            merge_plain_region("Blockers", "base", "base", "theirs changed"),
            Ok("theirs changed".to_string())
        );
        assert_eq!(
            merge_plain_region("Blockers", "base", "ours changed", "base"),
            Ok("ours changed".to_string())
        );
    }

    #[test]
    fn merge_plain_region_collapses_identical_change() {
        assert_eq!(
            merge_plain_region("Blockers", "base", "same", "same"),
            Ok("same".to_string())
        );
    }

    #[test]
    fn merge_plain_region_rejects_divergent_change() {
        assert_eq!(
            merge_plain_region("Blockers", "base", "ours", "theirs"),
            Err(StateMergeError::DivergentPlainRegion {
                section: "Blockers"
            })
        );
    }

    #[test]
    fn merge_plain_region_no_change_returns_base() {
        assert_eq!(
            merge_plain_region("Blockers", "base", "base", "base"),
            Ok("base".to_string())
        );
    }

    // ── enforce_recent_closeouts_invariants ─────────────────────────────

    #[test]
    fn closeouts_invariants_sort_newest_first_and_trim_to_two() {
        let rows = vec![
            row("A", &["2026-07-01", "A", "x", "y"]),
            row("B", &["2026-07-29", "B", "x", "y"]),
            row("C", &["2026-07-30", "C", "x", "y"]),
        ];
        let result = enforce_recent_closeouts_invariants(rows, 0).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].cells[0], "2026-07-30");
        assert_eq!(result[1].cells[0], "2026-07-29");
    }

    #[test]
    fn closeouts_invariants_stable_tie_break_preserves_input_order() {
        let rows = vec![
            row("ours", &["2026-07-30", "ours", "x", "y"]),
            row("theirs", &["2026-07-30", "theirs", "x", "y"]),
        ];
        let result = enforce_recent_closeouts_invariants(rows, 0).unwrap();
        assert_eq!(result[0].key, "ours");
        assert_eq!(result[1].key, "theirs");
    }

    #[test]
    fn closeouts_invariants_rejects_invalid_date() {
        let rows = vec![row("A", &["not-a-date", "A", "x", "y"])];
        let err = enforce_recent_closeouts_invariants(rows, 0).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::InvalidCloseoutDate {
                date: "not-a-date".to_string()
            }
        );
    }

    #[test]
    fn is_valid_iso_date_accepts_and_rejects() {
        assert!(is_valid_iso_date("2026-07-30"));
        assert!(!is_valid_iso_date("2026-13-01")); // month out of range
        assert!(!is_valid_iso_date("2026-07-00")); // day out of range
        assert!(!is_valid_iso_date("26-07-30")); // wrong length
        assert!(!is_valid_iso_date("2026/07/30")); // wrong separators
        assert!(!is_valid_iso_date("")); // empty
    }

    // ── enforce_byte_budget ──────────────────────────────────────────────

    #[test]
    fn byte_budget_accepts_content_at_or_under_limit() {
        let content = "a".repeat(MAX_STATE_MD_BYTES);
        assert_eq!(enforce_byte_budget(&content), Ok(()));
    }

    #[test]
    fn byte_budget_rejects_content_over_limit() {
        let content = "a".repeat(MAX_STATE_MD_BYTES + 1);
        assert_eq!(
            enforce_byte_budget(&content),
            Err(StateMergeError::ByteBudgetExceeded {
                bytes: MAX_STATE_MD_BYTES + 1
            })
        );
    }

    // ── require_active_conflict / read_all_stages (integration) ─────────

    fn git(dir: &Path, args: &[&str]) -> std::process::Output {
        Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .expect("git spawn")
    }

    fn git_ok(dir: &Path, args: &[&str]) {
        let out = git(dir, args);
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    fn init_repo() -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        git_ok(tmp.path(), &["init", "--initial-branch=main"]);
        git_ok(tmp.path(), &["config", "user.email", "test@example.com"]);
        git_ok(tmp.path(), &["config", "user.name", "Test"]);
        std::fs::create_dir_all(tmp.path().join(".dev")).unwrap();
        tmp
    }

    fn write_state(repo: &Path, content: &str) {
        std::fs::create_dir_all(repo.join(".dev")).unwrap();
        std::fs::write(repo.join(STATE_MD_PATH), content).unwrap();
    }

    #[test]
    fn require_active_conflict_rejects_no_merge_in_progress() {
        let repo = init_repo();
        write_state(repo.path(), "base\n");
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "base"]);

        let err = require_active_conflict(repo.path()).unwrap_err();
        assert_eq!(err, StateMergeError::NoActiveMerge);
    }

    #[test]
    fn conflict_on_state_md_only_reads_lf_normalized_stages() {
        let repo = init_repo();
        write_state(repo.path(), "base content\n");
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "base"]);

        git_ok(repo.path(), &["checkout", "-b", "feature"]);
        write_state(repo.path(), "theirs content\n");
        git_ok(repo.path(), &["commit", "-am", "theirs"]);

        git_ok(repo.path(), &["checkout", "main"]);
        write_state(repo.path(), "ours content\n");
        git_ok(repo.path(), &["commit", "-am", "ours"]);

        let merge_out = git(repo.path(), &["merge", "feature"]);
        assert!(!merge_out.status.success(), "merge should conflict");

        require_active_conflict(repo.path()).unwrap();
        let stages = read_all_stages(repo.path()).unwrap();
        assert_eq!(stages.base, "base content\n");
        assert_eq!(stages.ours, "ours content\n");
        assert_eq!(stages.theirs, "theirs content\n");
    }

    #[test]
    fn require_active_conflict_rejects_extra_conflicted_path() {
        let repo = init_repo();
        write_state(repo.path(), "base\n");
        std::fs::write(repo.path().join("other.md"), "base\n").unwrap();
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "base"]);

        git_ok(repo.path(), &["checkout", "-b", "feature"]);
        write_state(repo.path(), "theirs\n");
        std::fs::write(repo.path().join("other.md"), "theirs\n").unwrap();
        git_ok(repo.path(), &["commit", "-am", "theirs"]);

        git_ok(repo.path(), &["checkout", "main"]);
        write_state(repo.path(), "ours\n");
        std::fs::write(repo.path().join("other.md"), "ours\n").unwrap();
        git_ok(repo.path(), &["commit", "-am", "ours"]);

        let merge_out = git(repo.path(), &["merge", "feature"]);
        assert!(!merge_out.status.success());

        let err = require_active_conflict(repo.path()).unwrap_err();
        match err {
            StateMergeError::UnmergedPathSetMismatch { paths } => {
                assert_eq!(paths.len(), 2);
            }
            other => panic!("expected UnmergedPathSetMismatch, got {other:?}"),
        }
    }

    #[test]
    fn read_stage_rejects_add_add_missing_base() {
        let repo = init_repo();
        std::fs::write(repo.path().join("root.md"), "root\n").unwrap();
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "root, no state.md yet"]);

        git_ok(repo.path(), &["checkout", "-b", "feature"]);
        write_state(repo.path(), "theirs added\n");
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "theirs adds state.md"]);

        git_ok(repo.path(), &["checkout", "main"]);
        write_state(repo.path(), "ours added\n");
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "ours adds state.md"]);

        let merge_out = git(repo.path(), &["merge", "feature"]);
        assert!(!merge_out.status.success());

        require_active_conflict(repo.path()).unwrap();
        let err = read_all_stages(repo.path()).unwrap_err();
        assert_eq!(err, StateMergeError::MissingStage { stage: 1 });
    }

    // ── commit_merged_state (integration, fault-injected) ────────────────

    /// Set up a repo with a genuine 3-way conflict on `.dev/state.md` and
    /// return its path, positioned exactly as the resolver would find it
    /// (MERGE_HEAD set, one unmerged path, three stages present).
    fn conflicted_repo() -> tempfile::TempDir {
        let tmp = init_repo();
        write_state(tmp.path(), "base content\n");
        git_ok(tmp.path(), &["add", "."]);
        git_ok(tmp.path(), &["commit", "-m", "base"]);

        git_ok(tmp.path(), &["checkout", "-b", "feature"]);
        write_state(tmp.path(), "theirs content\n");
        git_ok(tmp.path(), &["commit", "-am", "theirs"]);

        git_ok(tmp.path(), &["checkout", "main"]);
        write_state(tmp.path(), "ours content\n");
        git_ok(tmp.path(), &["commit", "-am", "ours"]);

        let merge_out = git(tmp.path(), &["merge", "feature"]);
        assert!(!merge_out.status.success(), "merge should conflict");
        tmp
    }

    #[test]
    fn commit_merged_state_happy_path_succeeds_and_cleans_up() {
        let repo = conflicted_repo();
        let original_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();
        assert_eq!(original_entries.len(), 3);

        let result = commit_merged_state(repo.path(), "merged content\n", FaultSeam::default());
        assert_eq!(result, Ok(()));

        let bytes = std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap();
        assert_eq!(bytes, b"merged content\n");
        let entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();
        assert!(entries.is_empty());

        // Backup/candidate residue cleaned up.
        let leftovers: Vec<_> = std::fs::read_dir(repo.path().join(".dev"))
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains("state-merge"))
            .collect();
        assert!(leftovers.is_empty(), "residue left behind: {leftovers:?}");
    }

    #[test]
    fn commit_merged_state_candidate_verify_failure_is_byte_inert() {
        let repo = conflicted_repo();
        let before = std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap();
        let before_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();

        let seam = FaultSeam {
            fail_candidate_verify: true,
            ..Default::default()
        };
        let result = commit_merged_state(repo.path(), "merged content\n", seam);
        assert!(matches!(result, Err(CommitFailure::PreMutation(_))));
        assert_eq!(result.unwrap_err().marker(), "STATE_MERGE: unresolved");

        let after = std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap();
        assert_eq!(before, after);
        let after_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();
        assert_eq!(before_entries, after_entries);
    }

    #[test]
    fn commit_merged_state_backup_move_failure_stays_inert() {
        let repo = conflicted_repo();
        let before = std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap();
        let before_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();

        let seam = FaultSeam {
            fail_backup_move: true,
            ..Default::default()
        };
        let result = commit_merged_state(repo.path(), "merged content\n", seam);
        assert!(matches!(result, Err(CommitFailure::RolledBack(_))));
        assert_eq!(result.unwrap_err().marker(), "STATE_MERGE: unresolved");

        assert_eq!(
            std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap(),
            before
        );
        assert_eq!(
            snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap(),
            before_entries
        );
    }

    #[test]
    fn commit_merged_state_install_failure_rolls_back() {
        let repo = conflicted_repo();
        let before = std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap();
        let before_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();

        let seam = FaultSeam {
            fail_install: true,
            ..Default::default()
        };
        let result = commit_merged_state(repo.path(), "merged content\n", seam);
        assert!(matches!(result, Err(CommitFailure::RolledBack(_))));

        assert_eq!(
            std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap(),
            before
        );
        assert_eq!(
            snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap(),
            before_entries
        );
    }

    #[test]
    fn commit_merged_state_git_add_failure_rolls_back_worktree_and_index() {
        let repo = conflicted_repo();
        let before = std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap();
        let before_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();

        let seam = FaultSeam {
            fail_git_add: true,
            ..Default::default()
        };
        let result = commit_merged_state(repo.path(), "merged content\n", seam);
        assert!(matches!(result, Err(CommitFailure::RolledBack(_))));

        assert_eq!(
            std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap(),
            before
        );
        assert_eq!(
            snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap(),
            before_entries
        );
    }

    #[test]
    fn commit_merged_state_postcondition_failure_reconstructs_conflicted_index() {
        let repo = conflicted_repo();
        let before = std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap();
        let before_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();
        assert_eq!(before_entries.len(), 3);

        // git add already ran for real here, so the index moved to stage 0
        // before the injected postcondition failure — this is the case that
        // exercises reconstruct_index_entries, not just a worktree restore.
        let seam = FaultSeam {
            fail_postcondition: true,
            ..Default::default()
        };
        let result = commit_merged_state(repo.path(), "merged content\n", seam);
        assert!(matches!(result, Err(CommitFailure::RolledBack(_))));

        assert_eq!(
            std::fs::read(repo.path().join(STATE_MD_PATH)).unwrap(),
            before
        );
        let after_entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();
        assert_eq!(after_entries, before_entries);
    }

    #[test]
    fn commit_merged_state_restore_failure_reports_rollback_unconfirmed() {
        let repo = conflicted_repo();

        let seam = FaultSeam {
            fail_postcondition: true,
            fail_restore: true,
            ..Default::default()
        };
        let result = commit_merged_state(repo.path(), "merged content\n", seam);
        match result {
            Err(CommitFailure::RollbackUnconfirmed(_)) => {}
            other => panic!("expected RollbackUnconfirmed, got {other:?}"),
        }
        assert_eq!(
            result.unwrap_err().marker(),
            "STATE_MERGE: rollback-unconfirmed"
        );
    }

    #[test]
    fn zero_args_in_non_merge_repo_fails_closed() {
        let _g = super::super::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let repo = init_repo();
        write_state(repo.path(), "no conflict here\n");
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "base"]);

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();
        let result = cmd_state_merge(&["state-merge".to_string()]);
        std::env::set_current_dir(original_cwd).unwrap();

        assert_eq!(result, ExitCode::Error);
    }

    #[test]
    fn extra_args_return_usage() {
        assert_eq!(
            cmd_state_merge(&["state-merge".to_string(), "extra".to_string()]),
            ExitCode::Usage
        );
    }

    const TEMPLATE_SHAPE: &str = "# GAL State\n\
\n\
## Active Plans\n\
\n\
| Plan | File | Plan Phase | Last Activity |\n\
| --- | --- | --- | --- |\n\
\n\
## Recent Close-outs\n\
\n\
| Date | Plan | Landing | Result |\n\
| --- | --- | --- | --- |\n\
\n\
## Global Decisions\n\
\n\
| Date | Decision | Rationale | Scope |\n\
| --- | --- | --- | --- |\n\
\n\
## Blockers\n\
\n\
None.\n\
\n\
## Session Continuity\n\
\n\
| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |\n\
| --- | --- | --- | --- | --- | --- |\n\
\n\
## Session Execution Context\n\
\n\
Dispatched node:\n\
Execution mode:\n\
Notes:\n";

    fn active_plans_row(name: &str, file: &str) -> String {
        format!("| {name} | {file} | draft | 2026-07-31 |\n")
    }

    /// Insert one Active Plans row into the template fixture right after
    /// its separator line.
    fn with_active_plans_row(row: &str) -> String {
        TEMPLATE_SHAPE.replacen(
            "| --- | --- | --- | --- |\n\n## Recent Close-outs",
            &format!("| --- | --- | --- | --- |\n{row}\n## Recent Close-outs"),
            1,
        )
    }

    #[test]
    fn merge_full_state_disjoint_active_plans_additions_both_survive() {
        let base = TEMPLATE_SHAPE;
        let ours = with_active_plans_row(&active_plans_row("Alpha", "a.md"));
        let theirs = with_active_plans_row(&active_plans_row("Beta", "b.md"));

        let merged = merge_full_state(base, &ours, &theirs).unwrap();
        assert!(merged.contains("Alpha"));
        assert!(merged.contains("Beta"));
        // Reassembly stays parseable and round-trips through split again.
        let sections = split_sections(&merged).unwrap();
        assert_eq!(sections.len(), 7);
    }

    #[test]
    fn merge_full_state_rejects_section_shape_mismatch() {
        let base = TEMPLATE_SHAPE;
        let ours = TEMPLATE_SHAPE.replace("## Blockers", "## Renamed Blockers Heading");
        // `Renamed Blockers Heading` isn't recognized, so this actually fails
        // at split_sections (UnknownSection) before shape comparison — still
        // exercises the fail-closed path for a structural edit.
        let err = merge_full_state(base, &ours, base).unwrap_err();
        assert!(matches!(err, StateMergeError::UnknownSection { .. }));
    }

    #[test]
    fn cmd_state_merge_resolves_a_real_conflict_end_to_end() {
        let _g = super::super::ENV_GUARD
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let repo = init_repo();
        write_state(repo.path(), TEMPLATE_SHAPE);
        git_ok(repo.path(), &["add", "."]);
        git_ok(repo.path(), &["commit", "-m", "base"]);

        git_ok(repo.path(), &["checkout", "-b", "feature"]);
        write_state(
            repo.path(),
            &with_active_plans_row(&active_plans_row("Beta", "b.md")),
        );
        git_ok(repo.path(), &["commit", "-am", "theirs"]);

        git_ok(repo.path(), &["checkout", "main"]);
        write_state(
            repo.path(),
            &with_active_plans_row(&active_plans_row("Alpha", "a.md")),
        );
        git_ok(repo.path(), &["commit", "-am", "ours"]);

        let merge_out = git(repo.path(), &["merge", "feature"]);
        assert!(!merge_out.status.success(), "merge should conflict");

        let original_cwd = std::env::current_dir().unwrap();
        std::env::set_current_dir(repo.path()).unwrap();
        let result = cmd_state_merge(&["state-merge".to_string()]);
        std::env::set_current_dir(original_cwd).unwrap();

        assert_eq!(result, ExitCode::Success);
        let bytes = std::fs::read_to_string(repo.path().join(STATE_MD_PATH)).unwrap();
        assert!(bytes.contains("Alpha"));
        assert!(bytes.contains("Beta"));
        let entries = snapshot_unmerged_entries(repo.path(), STATE_MD_PATH).unwrap();
        assert!(entries.is_empty());
    }

    // ── normalize_lf ─────────────────────────────────────────────────────

    #[test]
    fn normalize_lf_converts_crlf() {
        assert_eq!(normalize_lf("a\r\nb\r\n"), Ok("a\nb\n".to_string()));
    }

    #[test]
    fn normalize_lf_rejects_bare_cr() {
        assert_eq!(
            normalize_lf("a\rb\n"),
            Err(StateMergeError::BareCarriageReturn)
        );
    }

    #[test]
    fn normalize_lf_passes_through_plain_lf() {
        assert_eq!(normalize_lf("a\nb\n"), Ok("a\nb\n".to_string()));
    }

    // ── split_sections ───────────────────────────────────────────────────

    /// Pinned template shape: `plugins/gal-core/templates/state.md`'s
    /// recognized sections plus `## Parked Plans` (the template's known
    /// gap). If the template gains/loses a section, this test's fixture
    /// must be updated deliberately, not silently pass.
    const TEMPLATE_SHAPE_PLUS_PARKED: &str = "# GAL State\n\
\n\
<!-- preamble comment -->\n\
\n\
## Active Plans\n\
\n\
| Plan | File | Plan Phase | Last Activity |\n\
| --- | --- | --- | --- |\n\
\n\
## Recent Close-outs\n\
\n\
| Date | Plan | Landing | Result |\n\
| --- | --- | --- | --- |\n\
\n\
## Parked Plans\n\
\n\
| Plan | File | Status | Reason / Next |\n\
| --- | --- | --- | --- |\n\
\n\
## Global Decisions\n\
\n\
| Date | Decision | Rationale | Scope |\n\
| --- | --- | --- | --- |\n\
\n\
## Blockers\n\
\n\
None.\n\
\n\
## Session Continuity\n\
\n\
| Plan | Source Plan | Last Session | Stopped At | Next Step | Context |\n\
| --- | --- | --- | --- | --- | --- |\n\
\n\
## Session Execution Context\n\
\n\
Dispatched node:\n\
Execution mode:\n\
Notes:\n";

    #[test]
    fn split_sections_accepts_template_shape_plus_parked_plans() {
        let sections = split_sections(TEMPLATE_SHAPE_PLUS_PARKED).unwrap();
        let kinds: Vec<SectionKind> = sections.iter().map(|s| s.kind).collect();
        assert_eq!(
            kinds,
            vec![
                SectionKind::Preamble,
                SectionKind::ActivePlans,
                SectionKind::RecentCloseouts,
                SectionKind::ParkedPlans,
                SectionKind::GlobalDecisions,
                SectionKind::Blockers,
                SectionKind::SessionContinuity,
                SectionKind::SessionExecutionContext,
            ]
        );
        assert_eq!(sections[0].heading, None);
        assert_eq!(sections[1].heading.as_deref(), Some("Active Plans"));
    }

    #[test]
    fn split_sections_rejects_unknown_heading() {
        let input = "## Active Plans\n\nbody\n\n## Not A Real Section\n\nbody\n";
        assert_eq!(
            split_sections(input),
            Err(StateMergeError::UnknownSection {
                heading: "Not A Real Section".to_string()
            })
        );
    }

    #[test]
    fn split_sections_rejects_duplicate_heading() {
        let input = "## Blockers\n\nNone.\n\n## Blockers\n\nNone.\n";
        assert_eq!(
            split_sections(input),
            Err(StateMergeError::DuplicateSection {
                heading: "Blockers".to_string()
            })
        );
    }

    #[test]
    fn split_sections_preamble_only_when_no_headings() {
        let sections = split_sections("just text\nmore text\n").unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0].kind, SectionKind::Preamble);
        assert_eq!(sections[0].heading, None);
    }

    // ── parse_keyed_table ────────────────────────────────────────────────

    const ACTIVE_PLANS_BODY: &str = "\n\
| Plan | File | Plan Phase | Last Activity |\n\
| --- | --- | --- | --- |\n\
| Alpha | `.dev/plans/alpha.md` | refining | 2026-07-30 |\n\
| Beta | `.dev/plans/beta.md` | draft | 2026-07-29 |\n\
\n\
<!-- trailing scaffolding comment -->\n";

    #[test]
    fn parse_keyed_table_extracts_rows_and_scaffolding() {
        let table = parse_keyed_table(SectionKind::ActivePlans, ACTIVE_PLANS_BODY).unwrap();
        assert_eq!(table.rows.len(), 2);
        assert_eq!(table.rows[0].key, "`.dev/plans/alpha.md`");
        assert_eq!(table.rows[1].key, "`.dev/plans/beta.md`");
        assert!(table
            .prefix
            .contains("| Plan | File | Plan Phase | Last Activity |"));
        assert!(table.prefix.contains("| --- | --- | --- | --- |"));
        assert!(!table.prefix.contains("Alpha"));
        assert!(table.suffix.contains("trailing scaffolding comment"));
    }

    #[test]
    fn parse_keyed_table_rejects_unexpected_header() {
        let body = "\n| Plan | Wrong Column |\n| --- | --- |\n";
        let err = parse_keyed_table(SectionKind::ActivePlans, body).unwrap_err();
        assert!(matches!(
            err,
            StateMergeError::UnexpectedTableHeader {
                section: "Active Plans",
                ..
            }
        ));
    }

    #[test]
    fn parse_keyed_table_rejects_missing_separator() {
        let body = "\n| Plan | File | Plan Phase | Last Activity |\nnot a separator\n";
        let err = parse_keyed_table(SectionKind::ActivePlans, body).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::MissingTable {
                section: "Active Plans"
            }
        );
    }

    #[test]
    fn parse_keyed_table_rejects_missing_table() {
        let body = "\nNo table here at all.\n";
        let err = parse_keyed_table(SectionKind::ActivePlans, body).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::MissingTable {
                section: "Active Plans"
            }
        );
    }

    #[test]
    fn parse_keyed_table_rejects_cell_count_mismatch() {
        let body = "\n\
| Plan | File | Plan Phase | Last Activity |\n\
| --- | --- | --- | --- |\n\
| Alpha | `.dev/plans/alpha.md` | refining |\n";
        let err = parse_keyed_table(SectionKind::ActivePlans, body).unwrap_err();
        assert!(matches!(
            err,
            StateMergeError::CellCountMismatch {
                section: "Active Plans",
                ..
            }
        ));
    }

    #[test]
    fn parse_keyed_table_rejects_duplicate_key() {
        let body = "\n\
| Plan | File | Plan Phase | Last Activity |\n\
| --- | --- | --- | --- |\n\
| Alpha | same.md | refining | 2026-07-30 |\n\
| Alpha Two | same.md | draft | 2026-07-29 |\n";
        let err = parse_keyed_table(SectionKind::ActivePlans, body).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::DuplicateRowKey {
                section: "Active Plans",
                key: "same.md".to_string()
            }
        );
    }

    #[test]
    fn parse_keyed_table_accepts_empty_table() {
        let body = "\n| Plan | File | Plan Phase | Last Activity |\n| --- | --- | --- | --- |\n";
        let table = parse_keyed_table(SectionKind::ActivePlans, body).unwrap();
        assert!(table.rows.is_empty());
        assert_eq!(table.suffix, "");
    }

    // ── merge_keyed_rows ─────────────────────────────────────────────────

    fn row(key: &str, cells: &[&str]) -> TableRow {
        TableRow {
            key: key.to_string(),
            cells: cells.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn merge_keyed_rows_disjoint_additions_keep_base_rows_in_place() {
        let base = vec![row("a", &["A", "a", "1"])];
        let ours = vec![row("a", &["A", "a", "1"]), row("b", &["B", "b", "1"])];
        let theirs = vec![row("a", &["A", "a", "1"]), row("c", &["C", "c", "1"])];
        let merged = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap();
        let keys: Vec<&str> = merged.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b", "c"]);
    }

    #[test]
    fn merge_keyed_rows_rejects_surviving_base_key_reorder() {
        let base = vec![row("a", &["1"]), row("b", &["1"])];
        let ours = vec![row("b", &["1"]), row("a", &["1"])];
        let theirs = base.clone();
        let err = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::ReorderedBaseKeys {
                section: "Active Plans"
            }
        );
    }

    #[test]
    fn merge_keyed_rows_allows_reorder_when_check_disabled() {
        let base = vec![row("a", &["1"]), row("b", &["1"])];
        let ours = vec![row("b", &["1"]), row("a", &["1"])];
        let theirs = base.clone();
        let merged = merge_keyed_rows("Parked Plans", &base, &ours, &theirs, false).unwrap();
        // Reorder isn't checked, but output order still follows base order.
        let keys: Vec<&str> = merged.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
    }

    #[test]
    fn merge_keyed_rows_rejects_divergent_edit() {
        let base = vec![row("a", &["1"])];
        let ours = vec![row("a", &["2"])];
        let theirs = vec![row("a", &["3"])];
        let err = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::DivergentRowEdit {
                section: "Active Plans",
                key: "a".to_string()
            }
        );
    }

    #[test]
    fn merge_keyed_rows_rejects_edit_vs_delete() {
        let base = vec![row("a", &["1"])];
        let ours = vec![row("a", &["2"])];
        let theirs: Vec<TableRow> = vec![];
        let err = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::DivergentRowEdit {
                section: "Active Plans",
                key: "a".to_string()
            }
        );
    }

    #[test]
    fn merge_keyed_rows_one_sided_edit_applies() {
        let base = vec![row("a", &["1"])];
        let ours = vec![row("a", &["2"])];
        let theirs = vec![row("a", &["1"])];
        let merged = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap();
        assert_eq!(merged[0].cells, vec!["2".to_string()]);
    }

    #[test]
    fn merge_keyed_rows_unanimous_delete_removes_row() {
        let base = vec![row("a", &["1"]), row("b", &["1"])];
        let ours = vec![row("b", &["1"])];
        let theirs = vec![row("b", &["1"])];
        let merged = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap();
        let keys: Vec<&str> = merged.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["b"]);
    }

    #[test]
    fn merge_keyed_rows_shared_addition_counted_once() {
        let base: Vec<TableRow> = vec![];
        let ours = vec![row("a", &["1"])];
        let theirs = vec![row("a", &["1"])];
        let merged = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap();
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn merge_keyed_rows_divergent_addition_rejected() {
        let base: Vec<TableRow> = vec![];
        let ours = vec![row("a", &["1"])];
        let theirs = vec![row("a", &["2"])];
        let err = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap_err();
        assert_eq!(
            err,
            StateMergeError::DivergentRowEdit {
                section: "Active Plans",
                key: "a".to_string()
            }
        );
    }

    #[test]
    fn merge_keyed_rows_same_anchor_ours_before_theirs() {
        let base = vec![row("a", &["1"])];
        let ours = vec![row("a", &["1"]), row("x", &["1"])];
        let theirs = vec![row("a", &["1"]), row("y", &["1"])];
        let merged = merge_keyed_rows("Active Plans", &base, &ours, &theirs, true).unwrap();
        let keys: Vec<&str> = merged.iter().map(|r| r.key.as_str()).collect();
        assert_eq!(keys, vec!["a", "x", "y"]);
    }

    #[test]
    fn parse_keyed_table_recent_closeouts_keys_on_full_plan_cell() {
        let body = "\n\
| Date | Plan | Landing | Result |\n\
| --- | --- | --- | --- |\n\
| 2026-07-29 | Foo (foo-slug) | abc123 | ABSORBED |\n";
        let table = parse_keyed_table(SectionKind::RecentCloseouts, body).unwrap();
        assert_eq!(table.rows[0].key, "Foo (foo-slug)");
    }
}
