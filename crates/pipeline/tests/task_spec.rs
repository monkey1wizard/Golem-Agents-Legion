use pipeline::task_spec::{
    agent_contract_rel, assemble_task_spec, extract_plan_slug, is_writeback_in_scope,
    render_phase_writeback, validate_repo_path, TaskSpecInput,
};

fn task_id(n: u32) -> String {
    format!("T-{n:02}")
}

fn tp_id(n: u32) -> String {
    format!("TP-{n:02}")
}

fn prompt() -> String {
    format!(
        "# Plan\n\n## Files to Create or Modify\n\n- `crates/pipeline/src/task_spec.rs`\n\n## Tasks\n\n- [ ] {} — Update task-spec rendering.\n\n## Test Plan\n\n| ID | Type | Description | Tasks |\n| --- | --- | --- | --- |\n| {} | unit | Render the task spec | {} |\n",
        task_id(1),
        tp_id(1),
        task_id(1)
    )
}

#[test]
fn affected_files_and_task_goal_are_rendered() {
    let body = prompt();
    let spec = assemble_task_spec(&TaskSpecInput {
        task_scope: &task_id(1),
        phase: "implement",
        prompt_path: ".dev/plans/example.prompt.md",
        prompt_body: &body,
        generated: "2026-09-14T00:00:00Z",
        git_branch: "main",
        git_head: "abc123",
        convention_hints: None,
        receipt_path: None,
        agent_contract_body: None,
        fix_mode: false,
    })
    .unwrap();
    assert!(spec.markdown.contains("Update task-spec rendering"));
    assert!(spec.markdown.contains("crates/pipeline/src/task_spec.rs"));
    assert!(!spec.markdown.contains("Test-first:"));
    assert!(!spec.markdown.contains("Pipeline Contract: test-first-v1"));
}

#[test]
fn explicit_agent_contract_is_inlined() {
    let body = prompt();
    let contract = "agent contract body";
    let spec = assemble_task_spec(&TaskSpecInput {
        task_scope: &task_id(1),
        phase: "audit",
        prompt_path: "example.prompt.md",
        prompt_body: &body,
        generated: "now",
        git_branch: "main",
        git_head: "head",
        convention_hints: None,
        receipt_path: None,
        agent_contract_body: Some(contract),
        fix_mode: false,
    })
    .unwrap();
    assert_eq!(spec.markdown.matches(contract).count(), 1);
}

#[test]
fn plan_slug_parsing_strips_prompt_suffix() {
    assert_eq!(extract_plan_slug(".dev/plans/fix-foo.prompt.md"), "fix-foo");
    assert_eq!(
        extract_plan_slug("C:/repo/.dev/plans/feat-bar.md"),
        "feat-bar"
    );
}

#[test]
fn repository_path_validation_rejects_non_repo_paths() {
    assert_eq!(
        validate_repo_path("crates/pipeline/src/task_spec.rs").unwrap(),
        "crates/pipeline/src/task_spec.rs"
    );
    assert!(validate_repo_path("/absolute/path.rs").is_err());
    assert!(validate_repo_path("crates/../pipeline/src/task_spec.rs").is_err());
}

#[test]
fn agent_contract_paths_follow_phase_roles() {
    assert!(agent_contract_rel("test").ends_with("golem-tester.agent.md"));
    assert!(agent_contract_rel("audit").ends_with("golem-auditor.agent.md"));
    assert!(agent_contract_rel("investigate").ends_with("golem-researcher.agent.md"));
    assert!(agent_contract_rel("implement").ends_with("golem-implementer.agent.md"));
}

#[test]
fn investigate_stays_out_of_writeback_scope() {
    assert!(!is_writeback_in_scope("investigate"));
}

#[test]
fn metadata_first_test_receipt_is_rejected_without_prompt_mutation() {
    let task = task_id(1);
    let prompt = format!(
        "# Plan\n\n## Test Results\n\nunchanged sentinel\n\n## Status\n\nCurrent Task: {task}\n"
    );
    let original = prompt.clone();
    let receipt = format!(
        "Pipeline Contract: test-first-v1\n### [{task}] 2026-09-23\nVerdict: PASS\nEvidence: focused test\n"
    );

    let error =
        render_phase_writeback(prompt.as_bytes(), receipt.as_bytes(), "test", &task).unwrap_err();
    assert!(error
        .to_string()
        .contains("must start with exactly one task heading"));
    assert_eq!(prompt, original);
}

#[test]
fn heading_first_test_receipt_writes_to_test_results() {
    let task = task_id(1);
    let prompt = format!("# Plan\n\n## Test Results\n\n## Status\n\nCurrent Task: {task}\n");
    let receipt = format!("### [{task}] 2026-09-23\nVerdict: PASS\nEvidence: focused test\n");

    let rendered =
        render_phase_writeback(prompt.as_bytes(), receipt.as_bytes(), "test", &task).unwrap();
    let rendered = String::from_utf8(rendered).unwrap();
    let test_results = rendered.find("## Test Results").unwrap();
    let receipt_position = rendered.find(&format!("### [{task}] 2026-09-23")).unwrap();
    let status = rendered.find("## Status").unwrap();
    assert!(test_results < receipt_position && receipt_position < status);
    assert!(rendered.contains("Verdict: PASS\nEvidence: focused test"));
}
