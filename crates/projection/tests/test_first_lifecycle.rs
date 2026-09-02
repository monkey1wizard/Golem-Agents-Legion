use std::fs;
use std::path::PathBuf;

fn pipeline_contract() -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root");
    fs::read_to_string(root.join("plugins/gal-core/commands/gal-pipeline/SKILL.template.md"))
        .expect("pipeline template")
}

#[test]
fn lifecycle_contract_projects_every_required_stop_and_transition() {
    let contract = pipeline_contract();
    for required in [
        "### Test-First Lifecycle Contract",
        "Clean task entry",
        "Boundary and transition",
        "Runner and evaluator",
        "Required and not-applicable branches",
        "Dispute recovery",
        "Dirty-tree audit and commit gate",
        "prepared, ambiguous, unknown, rollback-unconfirmed, or missing journal",
        "canonical recovery outcomes",
        "Later prompt tampering",
        "later prompt-tamper stop",
        "committed journal",
        "all subsequent prompt/status mutations must go through the transition producer",
        "probe-defect",
        "implementation-defect",
        "contract-ambiguous",
        "canonical argv",
        "exact same-command green rerun",
        "post-commit cleanliness",
        "2e requires executor-log terminal state completed",
        "Retry Handoff — T-NN / IMPLEMENT",
    ] {
        assert!(
            contract.contains(required),
            "missing lifecycle clause: {required}"
        );
    }
}

#[test]
fn dispatch_necessity_scopes_marked_tasks_to_the_test_first_axis() {
    let contract = pipeline_contract();
    let rule = contract
        .split("**Dispatch-necessity rule.**")
        .nth(1)
        .expect("dispatch-necessity rule")
        .split("On the skip path")
        .next()
        .expect("dispatch-necessity rule body");

    assert!(rule.contains("unmarked tasks only"));
    assert!(rule.contains("every marked `test-first-v1` task"));
    assert!(rule.contains("the test-first axis governs"));
    assert!(rule.contains("TESTER phase is never skipped"));
    assert!(rule.contains("partition by marker presence"));
    assert!(contract.contains("### 2e — Implement (CODER model — test-first-v1 & legacy branches)"));
}

#[test]
fn markerless_legacy_binds_nothing_and_writes_directly() {
    let contract = pipeline_contract();
    assert!(contract.contains("Markerless legacy prompt semantics and later tamper stop"));
    assert!(contract.contains(
        "a markerless prompt carries no transition journal and performs no digest binding"
    ));
    assert!(contract.contains(
        "Readers retain support for historical `legacy-bootstrap` journal rows, but writable transitions reject creating new `legacy-bootstrap` rows"
    ));
    assert!(contract.contains("Markerless transition boundary"));
    assert!(contract.contains("prompt updates continue as direct writes without journal binding"));
}
