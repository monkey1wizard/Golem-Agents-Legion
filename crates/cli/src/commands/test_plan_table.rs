//! Shared `## Test Plan` markdown table parser.
//!
//! Extracted from `finalize_check.rs` so both of its consumers —
//! `scan_test_plan_kind` and `task_probe_types` — read from one parser instead
//! of duplicating the table-scan logic.

use super::finalize_check::split_md_row;

/// Scan the target plan's `## Test Plan` markdown table into data rows. Locates
/// `Type` and optional `Covers` columns by header cell name (never a fixed
/// index), and skips the markdown separator and fully-empty rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TestPlanRow {
    pub(crate) type_cell: String,
    pub(crate) covers_cell: String,
}

pub(crate) fn scan_test_plan_rows(plan_text: &str) -> Vec<TestPlanRow> {
    let lines: Vec<&str> = plan_text.lines().collect();
    let Some(idx) = lines.iter().position(|l| l.trim() == "## Test Plan") else {
        return Vec::new();
    };
    let start = idx + 1;
    let end = lines[start..]
        .iter()
        .position(|l| l.starts_with("## "))
        .map(|off| start + off)
        .unwrap_or(lines.len());
    let mut rows = lines[start..end]
        .iter()
        .map(|l| l.trim())
        .filter(|l| l.starts_with('|'));

    // Header row → find the `Type` column by name.
    let Some(header) = rows.next() else {
        return Vec::new();
    };
    let header_cells = split_md_row(header);
    let Some(type_col) = header_cells
        .iter()
        .position(|c| c.eq_ignore_ascii_case("type"))
    else {
        return Vec::new();
    };
    let covers_col = header_cells
        .iter()
        .position(|c| c.eq_ignore_ascii_case("covers"));
    let mut result = Vec::new();
    for row in rows {
        let cells = split_md_row(row);
        // Skip the markdown separator row (every cell only `-`/`:`).
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        // Skip a fully-empty row.
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        result.push(TestPlanRow {
            type_cell: cells.get(type_col).cloned().unwrap_or_default(),
            covers_cell: covers_col
                .and_then(|col| cells.get(col))
                .cloned()
                .unwrap_or_default(),
        });
    }
    result
}

pub(crate) fn covered_task_ids(cell: &str) -> Vec<String> {
    let mut task_ids = Vec::new();
    for part in cell.split(',').map(str::trim) {
        let Some(range) = part.split_once("..") else {
            if let Some(task_id) = exact_task_id(part) {
                task_ids.push(task_id);
            }
            continue;
        };
        let Some(start) = exact_task_id(range.0.trim()) else {
            continue;
        };
        let Some(end) = exact_task_id(range.1.trim()) else {
            continue;
        };
        let start_number = start[2..].parse::<u32>().ok();
        let end_number = end[2..].parse::<u32>().ok();
        let (Some(start_number), Some(end_number)) = (start_number, end_number) else {
            continue;
        };
        if start_number <= end_number {
            task_ids.extend((start_number..=end_number).map(|number| format!("T-{number:02}")));
        }
    }
    task_ids
}

pub(crate) fn exact_task_id(text: &str) -> Option<String> {
    (text.len() == 4
        && text.starts_with("T-")
        && text.as_bytes()[2].is_ascii_digit()
        && text.as_bytes()[3].is_ascii_digit())
    .then(|| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compose plan-task IDs at runtime — never hardcode `T-NN` literals in
    // source (naming-gate provenance rule: plan-task IDs belong only in .dev).
    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    #[test]
    fn covers_cells_expand_only_task_ids() {
        assert_eq!(
            covered_task_ids(&format!("{}..{}", tid(1), tid(5))).len(),
            5
        );
        assert_eq!(
            covered_task_ids(&format!("{}, {}", tid(2), tid(3))).len(),
            2
        );
        assert_eq!(
            covered_task_ids(&format!("{}, R1-R8, requirement text", tid(4))).len(),
            1
        );
        assert!(covered_task_ids("all, R1-R8, free-form prose").is_empty());
    }
}
