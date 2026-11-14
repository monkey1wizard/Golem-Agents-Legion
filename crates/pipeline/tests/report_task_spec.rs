use pipeline::report::CriterionBinding;
use pipeline::task_spec::{
    assemble_plan_task_spec_with_evidence, EvidenceBinding, TaskSpecInput,
    PIPELINE_CRITERION_RESULT_KEYS,
};

fn binding(criteria: Vec<CriterionBinding>) -> EvidenceBinding {
    EvidenceBinding::new_with_criteria(
        "attempt".into(),
        "a".repeat(64),
        "b".repeat(40),
        0,
        None,
        criteria,
    )
}

#[test]
fn criterion_specs_preserve_version_one_and_list_bound_combinations() {
    let template = r#"## Tasks

- [ ] {T1} — Exercise behavior.

## Test Plan

| ID | Type | Covers | Criteria |
| --- | --- | --- | --- |
| {P1} | unit | {T1} | [{"criterion_id":"shared","requirements":["R1"],"gate_id":"gate","pass_condition":"works","source_kind":"test","target":null,"security":null}] |
| {P2} | integration | {T1}..{T2} | [{"criterion_id":"shared","requirements":["R1"],"gate_id":"gate","pass_condition":"works","source_kind":"test","target":null,"security":null}] |
"#;
    let (t1, t2) = (plan_id("T", 1), plan_id("T", 2));
    let (p1, p2) = (plan_id("TP", 1), plan_id("TP", 2));
    let prompt = template
        .replace("{T1}", &t1)
        .replace("{T2}", &t2)
        .replace("{P1}", &p1)
        .replace("{P2}", &p2);
    let mut input = TaskSpecInput {
        task_scope: &t1,
        phase: "test",
        prompt_path: "plan.prompt.md",
        prompt_body: &prompt,
        generated: "date",
        git_branch: "branch",
        git_head: "head",
        convention_hints: None,
        receipt_path: None,
        agent_contract_body: None,
        fix_mode: false,
    };
    let empty = assemble_plan_task_spec_with_evidence(&input, Some(binding(vec![])))
        .unwrap()
        .markdown;
    let criterion = CriterionBinding {
        criterion_id: "shared".into(),
        spec_sha256: "1".repeat(64),
        membership_sha256: "2".repeat(64),
        artifact_set_sha256: "3".repeat(64),
        artifact_snapshot_sha256: "4".repeat(64),
    };
    let populated =
        assemble_plan_task_spec_with_evidence(&input, Some(binding(vec![criterion.clone()])))
            .unwrap()
            .markdown;
    assert!(empty.contains("schema_version=1"));
    assert!(!empty.contains("criterion_results"));
    assert!(populated.contains("schema_version=2"));
    assert!(populated.contains(&format!("tp_id={p1} criterion_id=shared gate_id=gate")));
    assert!(populated.contains(&format!("tp_id={p2} criterion_id=shared gate_id=gate")));
    for key in PIPELINE_CRITERION_RESULT_KEYS {
        assert!(populated.contains(key));
    }
    for hash in [
        &criterion.spec_sha256,
        &criterion.membership_sha256,
        &criterion.artifact_set_sha256,
        &criterion.artifact_snapshot_sha256,
    ] {
        assert!(populated.contains(hash));
    }
    input.phase = "audit";
    let audit_empty = assemble_plan_task_spec_with_evidence(&input, Some(binding(vec![])))
        .unwrap()
        .markdown;
    let audit_populated =
        assemble_plan_task_spec_with_evidence(&input, Some(binding(vec![criterion])))
            .unwrap()
            .markdown;
    assert_eq!(audit_empty, audit_populated);
    assert!(audit_populated.contains("schema_version=1"));
}

fn plan_id(prefix: &str, number: u32) -> String {
    format!("{prefix}-{number:02}")
}
