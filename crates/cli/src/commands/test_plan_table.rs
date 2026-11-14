//! Shared `## Test Plan` markdown table parser.
//!
//! Extracted from `finalize_check.rs` so `refining_check.rs` can read from
//! one parser instead of duplicating the table-scan logic.

use super::finalize_check::split_md_row;
use pipeline::report::{CriterionMapping, CriterionPayload, CriterionTestPlan};
use std::collections::{BTreeMap, BTreeSet};

fn unfenced_lines(text: &str) -> Vec<(usize, &str)> {
    let mut fence = None;
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let trimmed = line.trim();
            if let Some(ch) = trimmed.chars().next().filter(|ch| matches!(ch, '`' | '~')) {
                let count = trimmed.chars().take_while(|current| *current == ch).count();
                if count >= 3 {
                    match fence {
                        None => fence = Some((ch, count)),
                        Some((open, length)) if open == ch && count >= length => fence = None,
                        _ => {}
                    }
                    return None;
                }
            }
            fence.is_none().then_some((index + 1, line))
        })
        .collect()
}

fn section_lines<'a>(lines: &[(usize, &'a str)], heading: &str) -> Vec<(usize, &'a str)> {
    let Some(start) = lines.iter().position(|(_, line)| line.trim() == heading) else {
        return Vec::new();
    };
    lines[start + 1..]
        .iter()
        .take_while(|(_, line)| !line.trim().starts_with("## "))
        .copied()
        .collect()
}

/// Scan the target plan's `## Test Plan` markdown table into data rows. Locates
/// `Type` and optional `Covers` columns by header cell name (never a fixed
/// index), and skips the markdown separator and fully-empty rows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TestPlanRow {
    pub(crate) id_cell: String,
    pub(crate) type_cell: String,
    pub(crate) description_cell: String,
    pub(crate) covers_cell: String,
    pub(crate) criteria_cell: Option<String>,
    pub(crate) line: usize,
}

pub(crate) fn scan_test_plan_rows(plan_text: &str) -> Vec<TestPlanRow> {
    let lines = unfenced_lines(plan_text);
    let table = section_lines(&lines, "## Test Plan");
    let mut rows = table
        .iter()
        .filter_map(|(line, text)| text.trim().starts_with('|').then_some((*line, text.trim())));

    let Some((_, header)) = rows.next() else {
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
    let id_col = header_cells
        .iter()
        .position(|c| c.eq_ignore_ascii_case("id"));
    let description_col = header_cells
        .iter()
        .position(|c| c.eq_ignore_ascii_case("description"));
    let criteria_col = header_cells
        .iter()
        .position(|c| c.eq_ignore_ascii_case("criteria"));
    let mut result = Vec::new();
    for (line, row) in rows {
        let cells = split_md_row(row);
        if cells
            .iter()
            .all(|c| !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':'))
        {
            continue;
        }
        if cells.iter().all(|c| c.is_empty()) {
            continue;
        }
        result.push(TestPlanRow {
            id_cell: id_col
                .and_then(|col| cells.get(col))
                .cloned()
                .unwrap_or_default(),
            type_cell: cells.get(type_col).cloned().unwrap_or_default(),
            description_cell: description_col
                .and_then(|col| cells.get(col))
                .cloned()
                .unwrap_or_default(),
            covers_cell: covers_col
                .and_then(|col| cells.get(col))
                .cloned()
                .unwrap_or_default(),
            criteria_cell: criteria_col.map(|col| cells.get(col).cloned().unwrap_or_default()),
            line,
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

pub(crate) fn parse_criteria(
    row: &TestPlanRow,
) -> Result<Option<Vec<pipeline::report::CriterionDeclaration>>, String> {
    let Some(cell) = &row.criteria_cell else {
        return Ok(None);
    };
    let error = |message: &str| format!("line {}: {message}", row.line);
    let raw: serde_json::Value = serde_json::from_str(cell).map_err(|e| error(&e.to_string()))?;
    let items = raw
        .as_array()
        .ok_or_else(|| error("Criteria must be an array"))?;
    if items.is_empty() {
        return Err(error("Criteria must not be empty"));
    }
    for item in items {
        let object = item
            .as_object()
            .ok_or_else(|| error("criterion must be an object"))?;
        for key in [
            "criterion_id",
            "requirements",
            "gate_id",
            "pass_condition",
            "source_kind",
            "target",
            "security",
        ] {
            if !object.contains_key(key) {
                return Err(error(&format!("criterion missing {key}")));
            }
        }
    }
    let declarations: Vec<pipeline::report::CriterionDeclaration> =
        serde_json::from_str(cell).map_err(|e| error(&e.to_string()))?;
    let mut ids = std::collections::BTreeSet::new();
    for declaration in &declarations {
        let id = declaration.criterion_id.as_bytes();
        if id.is_empty()
            || id.len() > 64
            || !id[0].is_ascii_lowercase()
            || !id
                .iter()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'-')
            || !ids.insert(&declaration.criterion_id)
        {
            return Err(error("invalid or duplicate criterion_id"));
        }
        if declaration.requirements.is_empty()
            || declaration.gate_id.trim().is_empty()
            || declaration.pass_condition.trim().is_empty()
        {
            return Err(error(
                "criterion requirements, gate_id and pass_condition must be non-empty",
            ));
        }
        use pipeline::report::CriterionSource;
        let valid_target = match declaration.source_kind {
            CriterionSource::Test | CriterionSource::OrchestratorCheck => {
                declaration.target.is_none()
            }
            CriterionSource::GoalVerification => declaration.target.as_deref() == Some("goal"),
            CriterionSource::OwnerAcceptance => {
                declaration.target.as_deref().is_some_and(|target| {
                    target.len() == 5
                        && target.starts_with("OA-")
                        && target.as_bytes()[3..].iter().all(u8::is_ascii_digit)
                })
            }
        };
        if !valid_target
            || declaration.security.as_ref().is_some_and(|security| {
                security.rule_or_threat.trim().is_empty()
                    || security.pass_condition.trim().is_empty()
            })
        {
            return Err(error("invalid target or security criterion"));
        }
    }
    Ok(Some(declarations))
}

pub(crate) fn strict_covered_task_ids(cell: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut seen = BTreeSet::new();
    for part in cell.split(',').map(str::trim) {
        let valid = if let Some((start, end)) = part.split_once("..") {
            match (exact_task_id(start.trim()), exact_task_id(end.trim())) {
                (Some(start), Some(end)) => start <= end,
                _ => false,
            }
        } else {
            exact_task_id(part).is_some()
        };
        if !valid {
            return Err(format!("invalid Covers token: {part}"));
        }
        for task in covered_task_ids(part) {
            if !seen.insert(task.clone()) {
                return Err(format!("duplicate Covers mapping: {task}"));
            }
            result.push(task);
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CriterionContract {
    pub schema_version: u32,
    pub criteria: BTreeMap<String, CriterionPayload>,
}

impl CriterionContract {
    pub(crate) fn value(&self) -> serde_json::Value {
        serde_json::json!({"schema_version": self.schema_version, "criteria": self.criteria})
    }

    pub(crate) fn digest(&self) -> Result<String, String> {
        serde_json::to_vec(&self.value())
            .map(|bytes| pipeline::usage::digest_bytes(&bytes))
            .map_err(|error| error.to_string())
    }

    pub(crate) fn task_ids(&self) -> BTreeSet<String> {
        self.criteria
            .values()
            .flat_map(|criterion| {
                criterion
                    .mappings
                    .iter()
                    .map(|mapping| mapping.task_id.clone())
            })
            .collect()
    }

    pub(crate) fn files(&self, text: &str) -> BTreeMap<String, BTreeSet<String>> {
        self.task_ids()
            .into_iter()
            .map(|task| {
                let block =
                    pipeline::task_spec::extract_plan_task_goal(text, &task).unwrap_or_default();
                let mut paths = BTreeSet::new();
                let mut in_files = false;
                for line in block.lines().skip(1) {
                    let trimmed = line.trim();
                    let content = if let Some(rest) = trimmed.strip_prefix("- **Files**:") {
                        in_files = true;
                        rest
                    } else if trimmed.starts_with("- **") {
                        in_files = false;
                        continue;
                    } else if in_files {
                        trimmed
                    } else {
                        continue;
                    };
                    paths.extend(
                        content
                            .split('`')
                            .enumerate()
                            .filter(|(index, _)| index % 2 == 1)
                            .map(|(_, path)| path.to_owned()),
                    );
                }
                (task, paths)
            })
            .collect()
    }
}

pub(crate) fn has_criteria_column(text: &str) -> bool {
    section_lines(&unfenced_lines(text), "## Test Plan")
        .iter()
        .find(|(_, line)| line.trim().starts_with('|'))
        .is_some_and(|(_, line)| {
            split_md_row(line)
                .iter()
                .any(|cell| cell.eq_ignore_ascii_case("Criteria"))
        })
}

pub(crate) fn parse_criterion_contract(text: &str) -> Result<Option<CriterionContract>, String> {
    if !has_criteria_column(text) {
        return Ok(None);
    }
    let lines = unfenced_lines(text);
    let table = section_lines(&lines, "## Test Plan");
    let (header_line, header) = table
        .iter()
        .find(|(_, line)| line.trim().starts_with('|'))
        .ok_or("## Test Plan: missing Criteria header")?;
    let headers = split_md_row(header);
    for name in ["ID", "Type", "Description", "Covers", "Criteria"] {
        if headers
            .iter()
            .filter(|cell| cell.eq_ignore_ascii_case(name))
            .count()
            != 1
        {
            return Err(format!("line {header_line}: expected one {name} column"));
        }
    }
    for (line, row) in table
        .iter()
        .filter(|(line, row)| line > header_line && row.trim().starts_with('|'))
    {
        if split_md_row(row).len() != headers.len() {
            return Err(format!(
                "line {line}: wrong cell count, encode raw pipes as JSON Unicode escapes"
            ));
        }
    }
    let requirements = section_lines(&lines, "## Requirements")
        .iter()
        .filter_map(|(_, line)| {
            let rest = line.trim().strip_prefix("- [")?;
            let rest = rest.get(1..)?.strip_prefix("] ")?;
            let id = rest.split_whitespace().next()?;
            (id.starts_with('R') && id.len() > 1 && id[1..].bytes().all(|ch| ch.is_ascii_digit()))
                .then(|| id.to_owned())
        })
        .collect::<BTreeSet<_>>();
    let mut tasks = BTreeSet::new();
    for (line, text) in section_lines(&lines, "## Tasks") {
        if let Some(task) = pipeline::task_spec::parse_plan_task_bullet(text) {
            if !tasks.insert(task.id.to_string()) {
                return Err(format!("line {line}: duplicate task {}", task.id));
            }
        }
    }
    let owners = section_lines(&lines, "## Owner Acceptance")
        .iter()
        .filter_map(|(_, line)| {
            let cells = split_md_row(line);
            cells
                .first()
                .filter(|id| exact_numbered_id(id, "OA-"))
                .cloned()
        })
        .collect::<BTreeSet<_>>();
    let mut contract = CriterionContract {
        schema_version: 1,
        criteria: BTreeMap::new(),
    };
    let mut tests = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for row in scan_test_plan_rows(text) {
        let error = |message: String| format!("line {} ({}): {message}", row.line, row.id_cell);
        if !exact_numbered_id(&row.id_cell, "TP-") || !tests.insert(row.id_cell.clone()) {
            return Err(error("invalid or duplicate TP ID".into()));
        }
        if row.type_cell.trim().is_empty() {
            return Err(error("Type is empty".into()));
        }
        let covers = strict_covered_task_ids(&row.covers_cell)
            .map_err(error)?
            .into_iter()
            .collect::<BTreeSet<_>>();
        for task in &covers {
            if !tasks.contains(task) {
                return Err(error(format!("Covers references nonexistent task {task}")));
            }
        }
        covered.extend(covers.iter().cloned());
        let declarations = parse_criteria(&row)?.ok_or_else(|| error("missing Criteria".into()))?;
        for mut declaration in declarations {
            declaration.gate_id = normalize_text(&declaration.gate_id);
            declaration.pass_condition = normalize_text(&declaration.pass_condition);
            if let Some(security) = &mut declaration.security {
                security.rule_or_threat = normalize_text(&security.rule_or_threat);
                security.pass_condition = normalize_text(&security.pass_condition);
            }
            for requirement in &declaration.requirements {
                if !requirements.contains(requirement) {
                    return Err(error(format!("nonexistent requirement {requirement}")));
                }
            }
            if declaration.source_kind == pipeline::report::CriterionSource::OwnerAcceptance
                && !declaration
                    .target
                    .as_ref()
                    .is_some_and(|target| owners.contains(target))
            {
                return Err(error("nonexistent owner acceptance target".into()));
            }
            if declaration.source_kind == pipeline::report::CriterionSource::OrchestratorCheck
                && !matches!(row.type_cell.as_str(), "grep" | "manual" | "documentation")
            {
                return Err(error(
                    "orchestrator-check requires grep, manual, or documentation Type".into(),
                ));
            }
            let id = declaration.criterion_id.clone();
            let payload = contract
                .criteria
                .entry(id.clone())
                .or_insert_with(|| CriterionPayload {
                    schema_version: 1,
                    declaration: declaration.clone(),
                    test_plans: BTreeSet::new(),
                    mappings: BTreeSet::new(),
                });
            if payload.declaration != declaration {
                return Err(error(format!("conflicting declaration for {id}")));
            }
            payload.test_plans.insert(CriterionTestPlan {
                id: row.id_cell.clone(),
                test_type: row.type_cell.clone(),
                covers: covers.clone(),
            });
            for task in &covers {
                if !payload.mappings.insert(CriterionMapping {
                    tp_id: row.id_cell.clone(),
                    task_id: task.clone(),
                    gate_id: declaration.gate_id.clone(),
                }) {
                    return Err(error(format!("duplicate mapping for {id} and {task}")));
                }
            }
        }
    }
    if contract.criteria.is_empty() || tasks.is_empty() || tasks != covered {
        return Err(format!(
            "line {header_line}: Criteria must cover every task, uncovered: {:?}",
            tasks.difference(&covered).collect::<Vec<_>>()
        ));
    }
    Ok(Some(contract))
}

fn exact_numbered_id(text: &str, prefix: &str) -> bool {
    text.strip_prefix(prefix)
        .is_some_and(|digits| digits.len() == 2 && digits.bytes().all(|ch| ch.is_ascii_digit()))
}

fn normalize_text(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub(crate) fn criterion_contract_check(text: &str) -> Option<super::finalize_check::CheckOutcome> {
    use super::finalize_check::{CheckOutcome, CheckState};
    match parse_criterion_contract(text) {
        Ok(None) => None,
        result => Some(CheckOutcome {
            name: "criterion-contract".into(),
            command: None,
            state: if result.is_ok() {
                CheckState::Pass
            } else {
                CheckState::Fail
            },
            summary: match result {
                Ok(Some(contract)) => format!(
                    "{} criteria, membership {}",
                    contract.criteria.len(),
                    contract.digest().unwrap_or_default()
                ),
                Err(error) => error,
                Ok(None) => unreachable!(),
            },
        }),
    }
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

    #[test]
    fn criteria_cells_preserve_json_and_skip_fenced_rows() {
        let json = r#"[{"pass_condition":"a\u007cb\\c\"d"}]"#;
        let plan = format!(
            "## Test Plan\n\n```text\n## Example\n| ID | Type | Description | Covers | Criteria |\n| example | unit | ignored | {} | [] |\n```\n\n| ID | Type | Description | Covers | Criteria |\n| --- | --- | --- | --- | --- |\n| example | unit | probe | {} | {json} |\n",
            tid(1), tid(1)
        );
        let rows = scan_test_plan_rows(&plan);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].criteria_cell.as_deref(), Some(json));
        assert_eq!(rows[0].id_cell, "example");
        assert_eq!(rows[0].description_cell, "probe");
        assert_eq!(rows[0].covers_cell, tid(1));
    }

    #[test]
    fn criteria_require_closed_complete_declarations() {
        let plan = |cell: &str| {
            format!("## Test Plan\n| ID | Type | Description | Covers | Criteria |\n| --- | --- | --- | --- | --- |\n| probe | unit | probe | {} | {cell} |\n", tid(1))
        };
        let valid = r#"[{"criterion_id":"probe","requirements":["R1"],"gate_id":"gate","pass_condition":"a\u007cb","source_kind":"test","target":null,"security":null}]"#;
        let rows = scan_test_plan_rows(&plan(valid));
        assert_eq!(
            parse_criteria(&rows[0]).unwrap().unwrap()[0].pass_condition,
            "a|b"
        );
        for invalid in [
            "[]",
            r#"[{"criterion_id":"probe"}]"#,
            &valid.replace("\"target\":null", "\"target\":null,\"target\":null"),
            &valid.replace("\"security\":null", "\"security\":null,\"extra\":true"),
        ] {
            let rows = scan_test_plan_rows(&plan(invalid));
            assert!(parse_criteria(&rows[0]).unwrap_err().contains("line 4"));
        }
    }
}
