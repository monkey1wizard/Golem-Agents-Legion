//! Offline checks for the Codex setup examples delivered in `docs/setup.md`.
//!
//! These fixtures check the documented TOML and contract statements. They do
//! not implement Codex configuration resolution or prove Desktop permissions.

use toml::Value;

const KNOWN_ISSUE: &str = "### Known issue: the Codex Windows sandbox fails";
const FULL_ACCESS: &str = "### Required permission mode: Full access";
const CHILD_FLAG: &str = "--dangerously-bypass-approvals-and-sandbox";

fn doc(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs")
        .join(name);
    std::fs::read_to_string(path).expect("guide is available to integration tests")
}

fn section<'a>(source: &'a str, heading: &str) -> &'a str {
    let start = source.find(heading).expect("guide section heading exists");
    let body = &source[start + heading.len()..];
    body.split_once("\n### ")
        .map_or(body, |(section, _)| section)
}

fn first_fence<'a>(source: &'a str, language: &str) -> &'a str {
    let lines: Vec<_> = source.lines().collect();
    let start = lines
        .iter()
        .position(|line| line.trim() == format!("```{language}"))
        .expect("expected guide code fence");
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.trim() == "```")
        .map(|offset| start + 1 + offset)
        .expect("guide code fence closes");
    // Recover the exact source slice, without the fence indentation rules
    // affecting TOML parsing.
    let start_offset = source.find(lines[start]).unwrap() + lines[start].len();
    let end_offset = source[start_offset..].find(lines[end]).unwrap() + start_offset;
    source[start_offset..end_offset].trim_matches(['\r', '\n'])
}

fn inline_code_after<'a>(source: &'a str, marker: &str) -> &'a str {
    let tail = source
        .split_once(marker)
        .expect("guide contract marker exists")
        .1;
    let start = tail
        .find('`')
        .expect("guide contract value is code formatted")
        + 1;
    let value = &tail[start..];
    &value[..value.find('`').expect("guide contract value closes")]
}

fn parse(source: &str) -> Value {
    source.parse().expect("guide fixture should be valid TOML")
}

#[test]
fn full_access_example_disables_the_sandbox_with_one_top_level_key() {
    let guide = doc("setup.md");
    let example = parse(first_fence(section(&guide, FULL_ACCESS), "toml"));
    let table = example.as_table().expect("example is a TOML table");
    assert_eq!(table.len(), 1, "example sets exactly one key: {table:?}");
    assert_eq!(example["sandbox_mode"].as_str(), Some("danger-full-access"));
}

#[test]
fn full_access_example_merges_without_touching_unrelated_settings() {
    let guide = doc("setup.md");
    let update = parse(first_fence(section(&guide, FULL_ACCESS), "toml"));
    let mut existing = parse(
        r#"unrelated_setting = "keep"
approvals_reviewer = "auto_review"
sandbox_mode = "workspace-write"

[desktop]
unrelated_table_setting = 17"#,
    );
    existing
        .as_table_mut()
        .unwrap()
        .insert("sandbox_mode".to_owned(), update["sandbox_mode"].clone());
    assert_eq!(
        existing["sandbox_mode"].as_str(),
        Some("danger-full-access")
    );
    assert_eq!(existing["unrelated_setting"].as_str(), Some("keep"));
    assert_eq!(existing["approvals_reviewer"].as_str(), Some("auto_review"));
    assert_eq!(
        existing["desktop"]["unrelated_table_setting"].as_integer(),
        Some(17)
    );
}

#[test]
fn known_issue_names_symptom_log_and_reproduction() {
    let guide = doc("setup.md");
    let issue = section(&guide, KNOWN_ISSUE);
    assert!(issue.contains("helper_unknown_error: setup refresh had errors"));
    assert!(issue.contains(r"%USERPROFILE%\.codex\.sandbox\sandbox.<date>.log"));
    assert!(issue.contains("setup refresh completed with errors"));
    let probe = first_fence(issue, "powershell");
    assert!(probe.contains("codex exec --json -s workspace-write"));
    assert!(issue.contains("**Approve for me** (auto-review) is not a workaround."));
}

#[test]
fn documented_child_launch_matches_the_codex_adapter() {
    let setup = doc("setup.md");
    let issue = section(&setup, KNOWN_ISSUE);
    assert_eq!(
        inline_code_after(issue, "GAL launches every dispatched Codex child with "),
        CHILD_FLAG
    );

    let workflows = doc("workflows.md");
    assert_eq!(
        inline_code_after(&workflows, "GAL launches Codex with "),
        CHILD_FLAG
    );
    assert!(!workflows.contains("GAL launches Codex with `-s workspace-write`"));
}
