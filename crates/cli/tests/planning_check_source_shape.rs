use std::process::Command;
use tempfile::TempDir;

mod common;
use common::test_gal_exe;

struct Outcome {
    exit_ok: bool,
    shape_row: String,
}

fn run_planning_check(plan: &str) -> Outcome {
    let temp = TempDir::new().unwrap();
    let plan_path = temp.path().join("plan.md");
    let receipt_path = temp.path().join("receipt.md");
    std::fs::write(&plan_path, plan).unwrap();
    let output = Command::new(test_gal_exe())
        .current_dir(temp.path())
        .args([
            "planning-check",
            plan_path.to_str().unwrap(),
            "--receipt",
            receipt_path.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let receipt = std::fs::read_to_string(&receipt_path).unwrap_or_else(|error| {
        panic!(
            "no receipt written: {error}\n{}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    let shape_row = receipt
        .lines()
        .find(|line| line.starts_with("| source-plan-shape |"))
        .unwrap_or_else(|| panic!("missing source-plan-shape row:\n{receipt}"))
        .to_string();
    Outcome {
        exit_ok: output.status.success(),
        shape_row,
    }
}

fn assert_rejected(plan: &str, reason: &str, line: &str) {
    let outcome = run_planning_check(plan);
    assert!(!outcome.exit_ok, "plan was accepted:\n{plan}");
    assert!(
        outcome
            .shape_row
            .starts_with("| source-plan-shape | fail |"),
        "{}",
        outcome.shape_row
    );
    assert!(outcome.shape_row.contains(reason), "{}", outcome.shape_row);
    assert!(outcome.shape_row.contains(line), "{}", outcome.shape_row);
}

fn assert_shape_passes(plan: &str) {
    let outcome = run_planning_check(plan);
    assert!(
        outcome
            .shape_row
            .starts_with("| source-plan-shape | pass |"),
        "{}",
        outcome.shape_row
    );
}

fn first_template_comment() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../plugins/gal-core/templates/plan.md");
    let template = std::fs::read_to_string(path).unwrap();
    let start = template.find("<!--").expect("template has a comment");
    let end = start + template[start..].find("-->").expect("comment is closed") + 3;
    template[start..end].to_string()
}

#[test]
fn displaced_approval_fails_with_its_line() {
    assert_rejected(
        "## Goal\n\n## Approval\n",
        "first heading must be Approval",
        "line 1",
    );
}

#[test]
fn swapped_standard_headings_fail_with_their_line() {
    assert_rejected(
        "## Approval\n## Requirements\n## Goal\n",
        "out of template order",
        "line 3",
    );
}

#[test]
fn duplicate_standard_heading_fails_with_its_line() {
    assert_rejected("## Approval\n## Goal\n## Goal\n", "duplicate", "line 3");
}

#[test]
fn leaked_template_guidance_fails_with_its_line() {
    let plan = format!("## Approval\n{}\n## Goal\n", first_template_comment());
    assert_rejected(&plan, "author guidance", "line 2");
}

#[test]
fn indented_fence_markers_do_not_hide_duplicate_headings() {
    for marker in ["    ```", "\t```", "    ~~~"] {
        let plan = format!("## Approval\n## Goal\n\n{marker}\n## Goal\n```\n");
        assert_rejected(&plan, "duplicate", "line 5");
    }
}

#[test]
fn headings_inside_a_real_fence_stay_inert() {
    assert_shape_passes("## Approval\n## Goal\n\n```text\n## Goal\n```\n");
}

#[test]
fn custom_headings_after_approval_and_omitted_optional_sections_pass() {
    assert_shape_passes("## Approval\n## Goal\n## Local Notes\n");
}

#[test]
fn crlf_plan_with_ordinary_comments_and_literal_examples_passes() {
    let plan = [
        "## Approval",
        "- Human approval: [approved]",
        "<!-- Zone: UTC -->",
        "<!-- PLAN CONTRACT: kept as an ordinary comment -->",
        "## Goal",
        "Write `<!-- PLAN CONTRACT -->` as an inline example.",
        "```text",
        "<!-- literal example inside a fence -->",
        "```",
        "",
    ]
    .join("\r\n");
    assert_shape_passes(&plan);
}
