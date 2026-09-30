//! Offline checks for the Codex setup examples delivered in `docs/setup.md`.
//!
//! These fixtures check the documented TOML and contract statements. They do
//! not implement Codex configuration resolution or prove Desktop permissions.

use toml::Value;

fn guide() -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/setup.md");
    std::fs::read_to_string(path).expect("setup guide is available to integration tests")
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

fn mode_b_example(source: &str) -> &str {
    first_fence(section(source, "### Mode B: Custom `config.toml`"), "toml")
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

fn is_documented_profile_file(value: &Value) -> bool {
    value.get("profiles").is_none()
        && ["approval_policy", "approvals_reviewer", "sandbox_mode"]
            .iter()
            .all(|key| value.get(key).is_some())
}

#[test]
fn parses_guide_example_and_merges_existing_table() {
    let guide = guide();
    let example = mode_b_example(&guide);
    let update = parse(example);
    assert_eq!(update["approval_policy"].as_str(), Some("on-request"));
    assert_eq!(update["approvals_reviewer"].as_str(), Some("auto_review"));
    assert_eq!(update["sandbox_mode"].as_str(), Some("workspace-write"));
    assert_eq!(
        update["sandbox_workspace_write"]["network_access"].as_bool(),
        Some(true)
    );
    assert_eq!(
        update["sandbox_workspace_write"]["writable_roots"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    let mut existing = parse(
        r#"unrelated_setting = "keep"
approval_policy = "on-failure"

[sandbox_workspace_write]
network_access = false
unrelated_table_setting = 17"#,
    );
    let root = existing.as_table_mut().unwrap();
    for key in ["approval_policy", "approvals_reviewer", "sandbox_mode"] {
        root.insert(key.to_owned(), update[key].clone());
    }
    for (key, value) in update["sandbox_workspace_write"].as_table().unwrap() {
        existing["sandbox_workspace_write"]
            .as_table_mut()
            .unwrap()
            .insert(key.clone(), value.clone());
    }
    assert_eq!(existing["unrelated_setting"].as_str(), Some("keep"));
    assert_eq!(
        existing["sandbox_workspace_write"]["unrelated_table_setting"].as_integer(),
        Some(17)
    );
    assert_eq!(
        existing["sandbox_workspace_write"]["network_access"].as_bool(),
        Some(true)
    );
}

#[test]
fn rejects_duplicate_table_from_guide_example() {
    let guide = guide();
    let example = mode_b_example(&guide);
    let duplicate = format!("{example}\n\n[sandbox_workspace_write]\nnetwork_access = false\n");
    assert!(duplicate.parse::<Value>().is_err());
}

#[test]
fn rollback_restores_changed_keys_and_preserves_unrelated_fields() {
    let mut current = parse(
        r#"unrelated_setting = "keep"
approval_policy = "on-request"
new_unrelated_setting = 42

[sandbox_workspace_write]
network_access = true
writable_roots = ["C:/work"]
unrelated_table_setting = "keep too""#,
    );
    let backup = parse(
        r#"approval_policy = "on-failure"

[sandbox_workspace_write]
network_access = false
writable_roots = []"#,
    );
    for key in ["approval_policy", "approvals_reviewer", "sandbox_mode"] {
        match backup.get(key) {
            Some(value) => current[key] = value.clone(),
            None => {
                current.as_table_mut().unwrap().remove(key);
            }
        }
    }
    for key in ["network_access", "writable_roots"] {
        match backup["sandbox_workspace_write"].get(key) {
            Some(value) => current["sandbox_workspace_write"][key] = value.clone(),
            None => {
                current["sandbox_workspace_write"]
                    .as_table_mut()
                    .unwrap()
                    .remove(key);
            }
        }
    }
    assert_eq!(current["approval_policy"].as_str(), Some("on-failure"));
    assert_eq!(current["unrelated_setting"].as_str(), Some("keep"));
    assert_eq!(current["new_unrelated_setting"].as_integer(), Some(42));
    assert_eq!(
        current["sandbox_workspace_write"]["network_access"].as_bool(),
        Some(false)
    );
    assert_eq!(
        current["sandbox_workspace_write"]["unrelated_table_setting"].as_str(),
        Some("keep too")
    );
}

#[test]
fn cli_profile_contract_uses_current_route() {
    let guide = guide();
    let profile_directory = section(&guide, "### Mode B: Custom `config.toml`");
    let profile_filename = inline_code_after(profile_directory, "CLI profile can use ");
    let profile_command = inline_code_after(profile_directory, "with ");
    assert_eq!(profile_filename, "gal.config.toml");
    assert_eq!(profile_command, "codex --profile gal");
    assert!(profile_directory
        .contains("Do not rely on the CLI profile being selected automatically by Desktop."));

    assert!(profile_directory.contains("Account for command-line overrides"));
    assert!(profile_directory.contains("organization restrictions"));

    // The documented profile is a separate config file with top-level keys.
    let profile_file: Value = r#"approval_policy = "on-request"
approvals_reviewer = "auto_review"
sandbox_mode = "workspace-write""#
        .parse()
        .expect("documented profile fixture is valid TOML");
    assert_eq!(profile_file["approval_policy"].as_str(), Some("on-request"));
    assert_eq!(
        profile_file["sandbox_mode"].as_str(),
        Some("workspace-write")
    );

    // An inline profile table is not the separate profile-file shape.
    let inline_profile: Value = "[profiles.gal]\nsandbox_mode = \"workspace-write\""
        .parse()
        .expect("legacy inline profile is syntactically valid TOML");
    assert!(inline_profile.get("profiles").is_some());
    assert!(is_documented_profile_file(&profile_file));
    assert!(!is_documented_profile_file(&inline_profile));
}

#[test]
fn outer_configuration_is_distinct_from_child_policy() {
    let setup = guide();
    let outer = parse(mode_b_example(&setup));
    assert_eq!(outer["sandbox_mode"].as_str(), Some("workspace-write"));
    let approve_mode = section(&setup, "### Mode A: Approve for me");
    assert!(approve_mode.contains("a matching `allow` rule runs that command outside the sandbox"));
    assert!(setup.contains(
        "Dispatched Codex child processes remain strictly constrained by `workspace-write`."
    ));

    let workflows = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/workflows.md"),
    )
    .expect("workflow guide is available to integration tests");
    let child_args = inline_code_after(&workflows, "GAL launches Codex with ");
    assert_eq!(child_args, "-s workspace-write");
    assert_eq!(outer["sandbox_mode"].as_str(), Some("workspace-write"));
    assert_eq!(child_args, "-s workspace-write");
}
