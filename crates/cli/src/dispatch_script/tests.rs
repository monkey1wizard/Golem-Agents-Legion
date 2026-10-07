use super::*;

fn offload_block(
    target: &OffloadTarget,
    phase: &str,
    task: &str,
    prompt: &str,
    workdir: &str,
    fix: bool,
) -> DispatchBlock {
    super::build::offload_block(target, phase, task, prompt, workdir, fix).unwrap()
}

/// Parse a rendered/fixture block into a sorted `Vec<(key, value)>` of its field
/// lines (ignores the framing markers + blank lines). Field-set comparison (D-001).
fn field_set(block: &str) -> Vec<(String, String)> {
    let mut fields: Vec<(String, String)> = block
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && *l != "--- GAL DISPATCH ---" && *l != "--- END DISPATCH ---")
        .filter_map(|l| {
            l.split_once(": ")
                .map(|(k, v)| (k.to_string(), v.to_string()))
        })
        .collect();
    fields.sort();
    fields
}

fn assert_parity(rendered: &str, fixture: &str) {
    assert_eq!(
        field_set(rendered),
        field_set(fixture),
        "field-set parity mismatch\n--- rendered ---\n{rendered}\n--- fixture ---\n{fixture}"
    );
}

const DIRECT_FIXTURE: &str = include_str!("../../tests/fixtures/dispatch-script/direct-golem.txt");
const INTENT_INIT: &str = include_str!("../../tests/fixtures/dispatch-script/intent-init.txt");
const INTENT_RESEARCH: &str =
    include_str!("../../tests/fixtures/dispatch-script/intent-research.txt");
const INTENT_DEEP_RESEARCH: &str =
    include_str!("../../tests/fixtures/dispatch-script/intent-deep-research.txt");
const INTENT_PIPELINE: &str =
    include_str!("../../tests/fixtures/dispatch-script/intent-pipeline.txt");
const BOUND_IMPLEMENT: &str =
    include_str!("../../tests/fixtures/dispatch-script/bound-implement.txt");
const BOUND_TEST: &str = include_str!("../../tests/fixtures/dispatch-script/bound-test.txt");
const BOUND_AUDIT: &str = include_str!("../../tests/fixtures/dispatch-script/bound-audit.txt");
const OFFLOAD: &str = include_str!("../../tests/fixtures/dispatch-script/offload.txt");

#[test]
fn resolve_golem_accepts_prefixed_and_bare() {
    assert_eq!(
        resolve_golem("architect").as_deref(),
        Some("golem-architect")
    );
    assert_eq!(
        resolve_golem("golem-architect").as_deref(),
        Some("golem-architect")
    );
    assert_eq!(resolve_golem("releaser").as_deref(), Some("golem-releaser"));
    assert_eq!(resolve_golem("steward").as_deref(), Some("golem-steward"));
    assert_eq!(
        resolve_golem("golem-steward").as_deref(),
        Some("golem-steward")
    );
}

#[test]
fn resolve_golem_rejects_unknown() {
    assert_eq!(resolve_golem("wizard"), None);
    assert_eq!(resolve_golem("golem-wizard"), None);
    assert_eq!(resolve_golem(""), None);
}

#[test]
fn parse_discuss_context_in_context_vs_isolated() {
    // discuss <role> → in-context
    let (golem, in_ctx) = parse_discuss_context("discuss", &["architect".to_string()]).unwrap();
    assert_eq!(golem, "golem-architect");
    assert!(in_ctx, "discuss prefix must set in_context=true");

    // discuss golem-analyst (full name) → in-context
    let (golem2, in_ctx2) =
        parse_discuss_context("discuss", &["golem-analyst".to_string()]).unwrap();
    assert_eq!(golem2, "golem-analyst");
    assert!(in_ctx2);

    // bare role → isolated
    let (golem3, in_ctx3) = parse_discuss_context("architect", &[]).unwrap();
    assert_eq!(golem3, "golem-architect");
    assert!(!in_ctx3, "bare role must set in_context=false");

    // discuss with no tokens → None (no resolvable role)
    assert!(parse_discuss_context("discuss", &[]).is_none());

    // unknown role → None
    assert!(parse_discuss_context("wizard", &[]).is_none());
    assert!(parse_discuss_context("discuss", &["wizard".to_string()]).is_none());
}

#[test]
fn discuss_block_carries_consult_mode_in_context() {
    let rendered = build_dispatch_block(
        "discuss",
        &["architect".to_string()],
        &RoutingTable::default(),
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
        rendered.contains("CONSULT_MODE: in-context"),
        "/gal discuss architect must emit CONSULT_MODE: in-context; got: {rendered}"
    );
    assert!(
        rendered.contains("ROLE: golem-architect"),
        "role present: {rendered}"
    );
}

#[test]
fn bare_role_has_no_consult_mode_field() {
    let rendered = build_dispatch_block(
        "architect",
        &[],
        &RoutingTable::default(),
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
        !rendered.contains("CONSULT_MODE"),
        "bare /gal architect must not emit CONSULT_MODE; got: {rendered}"
    );
}

/// All four discuss-capable roles must emit MODE: direct + CONSULT_MODE: in-context and
/// must not produce an error block (covers all ten roles — the other six are in
/// discuss_unsupported_roles_fail_closed below).
#[test]
fn discuss_all_capable_roles_emit_direct_and_consult_mode() {
    for role in &["analyst", "architect", "designer", "releaser"] {
        let rendered = build_dispatch_block(
            "discuss",
            &[role.to_string()],
            &RoutingTable::default(),
            &StateContext::idle(),
            "<REPO>",
        )
        .render();
        assert!(
            !rendered.contains("COMMAND: error"),
            "/gal discuss {role} must not error; got: {rendered}"
        );
        assert!(
            rendered.contains("MODE: direct"),
            "/gal discuss {role} must emit MODE: direct; got: {rendered}"
        );
        assert!(
            rendered.contains("CONSULT_MODE: in-context"),
            "/gal discuss {role} must emit CONSULT_MODE: in-context; got: {rendered}"
        );
        assert!(
            rendered.contains(&format!("ROLE: golem-{role}")),
            "/gal discuss {role} must emit ROLE: golem-{role}; got: {rendered}"
        );
    }
}

/// All six unsupported discuss roles must fail closed: COMMAND: error + role-specific
/// ACTION only — no ROLE, no MODE, no CONSULT_MODE.
#[test]
fn discuss_unsupported_roles_fail_closed() {
    // (role, expected recovery substring)
    let cases: &[(&str, &str)] = &[
        ("debugger", "/gal debugger"),
        ("steward", "/gal steward"),
        ("implementer", "/gal pipeline"),
        ("tester", "/gal pipeline"),
        ("auditor", "/gal finalize"),
        ("researcher", "/gal research"),
    ];
    for (role, recovery_hint) in cases {
        let rendered = build_dispatch_block(
            "discuss",
            &[role.to_string()],
            &RoutingTable::default(),
            &StateContext::idle(),
            "<REPO>",
        )
        .render();
        assert!(
            rendered.contains("COMMAND: error"),
            "/gal discuss {role} must produce error block; got: {rendered}"
        );
        assert!(
            rendered.contains(recovery_hint),
            "/gal discuss {role} error must mention '{recovery_hint}'; got: {rendered}"
        );
        assert!(
            !rendered.contains("ROLE:"),
            "/gal discuss {role} error block must not contain ROLE; got: {rendered}"
        );
        assert!(
            !rendered.contains("\nMODE:"),
            "/gal discuss {role} error block must not contain MODE; got: {rendered}"
        );
        assert!(
            !rendered.contains("CONSULT_MODE:"),
            "/gal discuss {role} error block must not contain CONSULT_MODE; got: {rendered}"
        );
    }
}

#[test]
fn mode_rules_match_shell() {
    // directly callable roles → direct regardless of incidental flags
    assert_eq!(golem_mode("golem-debugger", false, false), "direct");
    assert_eq!(golem_mode("golem-debugger", true, false), "direct");
    assert_eq!(golem_mode("golem-architect", true, false), "direct");
    assert_eq!(golem_mode("golem-analyst", false, false), "direct");
    assert_eq!(golem_mode("golem-designer", false, false), "direct");
    assert_eq!(golem_mode("golem-releaser", false, false), "direct");
    assert_eq!(golem_mode("golem-steward", true, false), "direct");
    // pipeline golem + pipeline context → bound
    assert_eq!(golem_mode("golem-implementer", true, false), "bound");
    assert_eq!(golem_mode("golem-tester", true, false), "bound");
    assert_eq!(golem_mode("golem-auditor", true, false), "bound");
    // orchestrated-only golem WITHOUT orchestration context → orchestrated-only
    assert_eq!(
        golem_mode("golem-implementer", false, false),
        "orchestrated-only"
    );
    assert_eq!(
        golem_mode("golem-tester", false, false),
        "orchestrated-only"
    );
    assert_eq!(
        golem_mode("golem-auditor", false, false),
        "orchestrated-only"
    );
    assert_eq!(
        golem_mode("golem-researcher", false, false),
        "orchestrated-only"
    );
    // finalize-branch-audit context authorizes auditor
    assert_eq!(golem_mode("golem-auditor", false, true), "bound");
    // ...but finalize-branch-audit must NOT let other execution roles bypass the gate
    // (regression: `/gal implementer --finalize-branch-audit` was reaching "bound").
    assert_eq!(
        golem_mode("golem-implementer", false, true),
        "orchestrated-only"
    );
    assert_eq!(golem_mode("golem-tester", false, true), "orchestrated-only");
    assert_eq!(
        golem_mode("golem-researcher", false, true),
        "orchestrated-only"
    );
}

#[test]
fn orchestrated_only_gate_produces_error_block() {
    for role in &["implementer", "tester", "auditor", "researcher"] {
        let rendered = build_dispatch_block(
            role,
            &[],
            &RoutingTable::default(),
            &StateContext::idle(),
            "<REPO>",
        )
        .render();
        assert!(
            rendered.contains("COMMAND: error"),
            "bare /{role} without orchestration context must produce error block; got: {rendered}"
        );
        assert!(
            rendered.contains("orchestrated-only"),
            "error message must mention orchestrated-only for /{role}; got: {rendered}"
        );
    }
}

#[test]
fn research_subcommand_path_does_not_hit_orchestrated_only_gate() {
    // research/deep-research are intent_base subcommands, never golem_mode → no reject.
    for intent in &["research", "deep-research"] {
        let rendered = build_dispatch_block(
            intent,
            &[],
            &RoutingTable::default(),
            &StateContext::idle(),
            "<REPO>",
        )
        .render();
        assert!(
            !rendered.contains("COMMAND: error"),
            "{intent} subcommand must not hit orchestrated-only gate; got: {rendered}"
        );
    }
    // Bare golem-researcher without orchestration context → still rejects.
    let rendered = build_dispatch_block(
        "researcher",
        &[],
        &RoutingTable::default(),
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
        rendered.contains("COMMAND: error"),
        "bare researcher without orchestration context must reject; got: {rendered}"
    );
}

#[test]
fn finalize_branch_audit_emits_loud_non_dispatch_error_block() {
    let state_md = "## Active Plans\n\n| Plan | Workflow State | File |\n| --- | --- | --- |\n| foo | IMPLEMENT | `.dev/plans/foo.prompt.md` |\n\n## Blockers\n";
    let state = StateContext {
        workflow_state: Some("IMPLEMENT".to_string()),
        active_plan_path: active_plan_path_from_state(state_md),
    };
    let tokens = vec!["--finalize-branch-audit".to_string()];
    let rendered = build_dispatch_block(
        "auditor",
        &tokens,
        &RoutingTable::default(),
        &state,
        "<REPO>",
    )
    .render();
    assert!(
        rendered.contains("COMMAND: error"),
        "finalize branch audit emits loud non-dispatch error block; got: {rendered}"
    );
    assert!(
        rendered.contains("DEGRADED_SAME_RUNTIME"),
        "error ACTION must mention DEGRADED_SAME_RUNTIME; got: {rendered}"
    );
    assert!(
        !rendered.contains("PIPELINE_PHASE"),
        "finalize branch audit must not emit PIPELINE_PHASE; got: {rendered}"
    );
    assert!(
        !rendered.contains("ACTIVE_EXECUTION_PROMPT"),
        "finalize branch audit must not leak ACTIVE_EXECUTION_PROMPT; got: {rendered}"
    );
    assert!(
        !rendered.contains("OFFLOAD"),
        "finalize branch audit must not emit OFFLOAD; got: {rendered}"
    );
}

#[test]
fn finalize_branch_audit_with_explicit_prompt_token_emits_loud_non_dispatch_error_block() {
    let state_md = "## Active Plans\n\n| Plan | Workflow State | File |\n| --- | --- | --- |\n| foo | IMPLEMENT | `.dev/plans/foo.prompt.md` |\n\n## Blockers\n";
    let state = StateContext {
        workflow_state: Some("IMPLEMENT".to_string()),
        active_plan_path: active_plan_path_from_state(state_md),
    };
    let tokens = vec![
        "--finalize-branch-audit".to_string(),
        "#file:.dev/plans/bar.prompt.md".to_string(),
    ];
    let rendered = build_dispatch_block(
        "golem-auditor",
        &tokens,
        &RoutingTable::default(),
        &state,
        "<REPO>",
    )
    .render();
    assert!(
        rendered.contains("COMMAND: error"),
        "finalize branch audit emits loud non-dispatch error block; got: {rendered}"
    );
    assert!(
        rendered.contains("DEGRADED_SAME_RUNTIME"),
        "error ACTION must mention DEGRADED_SAME_RUNTIME; got: {rendered}"
    );
    assert!(
        !rendered.contains("PIPELINE_PHASE"),
        "finalize branch audit must not emit PIPELINE_PHASE; got: {rendered}"
    );
    assert!(
        !rendered.contains("ACTIVE_EXECUTION_PROMPT"),
        "finalize branch audit must not leak ACTIVE_EXECUTION_PROMPT; got: {rendered}"
    );
    assert!(
        !rendered.contains("OFFLOAD"),
        "finalize branch audit must not emit OFFLOAD; got: {rendered}"
    );
}

/// Representative direct-role golden: field-set parity fixes the direct block protocol
/// shape (replaces the retired `consult-golem.txt` / `utility-golem.txt` class-shape
/// fixtures — same protocol shape, one canonical fixture).
#[test]
fn direct_block_matches_fixture() {
    let golem = "golem-architect";
    let mode = golem_mode(golem, false, false);
    assert_eq!(mode, "direct");
    assert_parity(&golem_block(golem, mode, &[]).render(), DIRECT_FIXTURE);
}

/// Table-driven role-field parity: all six directly callable roles emit `MODE: direct`,
/// the correct `ROLE`, and a bare call never carries `CONSULT_MODE`.
#[test]
fn direct_roles_have_mode_direct_and_no_consult_mode() {
    for role in &[
        "architect",
        "analyst",
        "designer",
        "releaser",
        "debugger",
        "steward",
    ] {
        let rendered = build_dispatch_block(
            role,
            &[],
            &RoutingTable::default(),
            &StateContext::idle(),
            "<REPO>",
        )
        .render();
        assert!(
            rendered.contains("MODE: direct"),
            "bare /gal {role} must emit MODE: direct; got: {rendered}"
        );
        assert!(
            rendered.contains(&format!("ROLE: golem-{role}")),
            "bare /gal {role} must emit ROLE: golem-{role}; got: {rendered}"
        );
        assert!(
            !rendered.contains("CONSULT_MODE"),
            "bare /gal {role} must not emit CONSULT_MODE; got: {rendered}"
        );
        assert!(
            !rendered.contains("MODE: consult") && !rendered.contains("MODE: utility"),
            "bare /gal {role} must not emit a retired mode value; got: {rendered}"
        );
    }
}

// ── intent blocks ───────────────────────────────────────────────────

#[test]
fn intent_blocks_match_fixtures() {
    for (intent, fixture) in [
        ("init", INTENT_INIT),
        ("research", INTENT_RESEARCH),
        ("deep-research", INTENT_DEEP_RESEARCH),
        ("pipeline", INTENT_PIPELINE),
    ] {
        let block = intent_block(intent).unwrap();
        assert_parity(&block.render(), fixture);
    }
}

/// Locked seam: the output of `intent_base`/`intent_block`. The research and
/// deep-research intent blocks must currently instruct the *current session* to
/// research itself (a single skill-activation ACTION) — this must become one action
/// per worker (`WORKER_0`/`WORKER_1`/`WORKER_2`) plus an ORCHESTRATOR compare/
/// adjudicate/check/document step, resolved from the routing table at render time.
/// Expected red: neither block yet carries per-worker actions.
#[test]
fn research_intent_emits_three_worker_actions() {
    let rendered = intent_block("research").unwrap().render();
    assert!(
        rendered.contains("WORKER_0")
            && rendered.contains("WORKER_1")
            && rendered.contains("WORKER_2"),
        "research intent emits three worker actions"
    );
}

/// Same requirement for deep-research (same locked seam, other fixture).
#[test]
fn deep_research_intent_emits_three_worker_actions() {
    let rendered = intent_block("deep-research").unwrap().render();
    assert!(
        rendered.contains("WORKER_0")
            && rendered.contains("WORKER_1")
            && rendered.contains("WORKER_2"),
        "deep-research intent emits three worker actions"
    );
}

/// Neither block may instruct the current session to research itself: the base
/// `ACTION` produced by `intent_base` for research/deep-research must not read as a
/// self-directed skill activation once this seam is implemented. Expected red: it still does.
#[test]
fn research_intents_do_not_self_activate_in_current_session() {
    for intent in &["research", "deep-research"] {
        let (action, _) = intent_base(intent).unwrap();
        assert!(
            !action.contains("Activate the /gal"),
            "{intent} ACTION must not instruct the current session to research itself; got: {action}"
        );
    }
}

#[test]
fn intent_base_rejects_non_intents() {
    assert!(intent_base("golem-architect").is_none());
    assert!(intent_base("frobnicate").is_none());
    assert!(intent_block("frobnicate").is_none());
}

// ── state context + pipeline-phase metadata ────────────────────────

#[test]
fn bullet_list_active_plans_yields_no_active_plan() {
    // The current repo form — `Get-ActivePlanPath` requires a header table, so a
    // bullet list resolves to idle (this is why the bound fixtures show IDLE).
    let state = "## Active Plans\n\n- .dev/plans/foo.prompt.md\n- .dev/plans/bar.prompt.md\n\n## Session Continuity\n";
    assert_eq!(active_plan_path_from_state(state), None);
}

#[test]
fn header_table_active_plans_resolves_plan_cell() {
    let state = "## Active Plans\n\n| Plan | Workflow State | File |\n| --- | --- | --- |\n| foo | IMPLEMENT | `.dev/plans/foo.prompt.md` |\n\n## Blockers\n";
    assert_eq!(
        active_plan_path_from_state(state).as_deref(),
        Some(".dev/plans/foo.prompt.md")
    );
}

#[test]
fn active_plan_path_from_state_skips_planning_draft_cells() {
    // A table row whose only cell is a planning-language EN draft or equivalence
    // receipt must never resolve as the active plan.
    let state = "## Active Plans\n\n| Plan | Workflow State | File |\n| --- | --- | --- |\n| foo | IMPLEMENT | `.dev/plans/foo.en.md` |\n\n## Blockers\n";
    assert_eq!(active_plan_path_from_state(state), None);
}

#[test]
fn active_plans_section_boundary_is_respected() {
    // A table in a later section must not be read as active plans.
    let state = "## Active Plans\n\n- bullet\n\n## Other\n\n| Plan | Workflow State |\n| --- | --- |\n| x.md | IMPLEMENT |\n";
    assert_eq!(active_plan_path_from_state(state), None);
}

fn assert_bound_parity(golem: &str, phase: &str, task: &str, fixture: &str) {
    let block = bound_golem_block(
        golem,
        phase,
        task,
        false,
        true,
        &StateContext::idle(),
        "<REPO>",
    );
    assert_parity(&block.render(), fixture);
}

#[test]
fn bound_test_audit_match_fixtures() {
    assert_bound_parity("golem-implementer", "implement", "T-NN", BOUND_IMPLEMENT);
    // delta context mode: no WORKFLOW_STATE / files list.
    assert_bound_parity("golem-tester", "test", "T-NN", BOUND_TEST);
    assert_bound_parity("golem-auditor", "audit", "T-NN", BOUND_AUDIT);
}

#[test]
fn fix_mode_adds_field() {
    let b = bound_golem_block(
        "golem-implementer",
        "implement",
        "T-NN",
        true,
        true,
        &StateContext::idle(),
        "<REPO>",
    );
    assert!(b.render().contains("FIX_MODE: true"));
}

// ── headless OFFLOAD ────────────────────────────────────────────────

use dispatch::routing::{RouteEntry, RoutingTable};

fn routing_with(role: &str, executor: &str, model: &str) -> RoutingTable {
    let mut t = RoutingTable::default();
    t.entries.insert(
        role.to_string(),
        RouteEntry {
            executor: executor.to_string(),
            model: model.to_string(),
            ssh_target: None,
            remote_workdir: None,
            effort: None,
            timeout_secs: None,
        },
    );
    t
}

#[test]
fn resolve_offload_target_uses_phase_role_and_routing() {
    let routing = routing_with("CODER", "claude", "claude-haiku-4-5");
    let t = resolve_offload_target("implement", &routing).unwrap();
    assert_eq!(t.role, "CODER");
    assert_eq!(t.executor, "claude");
    assert_eq!(t.model, "claude-haiku-4-5");

    // test phase → TESTER role, which is not routed → None (regular dispatch)
    assert!(resolve_offload_target("test", &routing).is_none());
    // invalid phase → None
    assert!(resolve_offload_target("review", &routing).is_none());
    // empty routing → None
    assert!(resolve_offload_target("implement", &RoutingTable::default()).is_none());
}

#[test]
fn offload_block_matches_fixture() {
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "claude".to_string(),
        model: "claude-haiku-4-5".to_string(),
        effort: Some("medium".to_string()),
    };
    let dispatch_target = "<REPO>/.dev/plans/x.prompt.md";
    let block = offload_block(
        &target,
        "implement",
        "T-NN",
        dispatch_target,
        "<REPO>",
        false,
    );
    assert_parity(&block.render(), OFFLOAD);
}

#[test]
fn offload_action_contract_rejects_invalid_inputs_and_quotes_paths() {
    let target = OffloadTarget {
        role: "CODER".into(),
        executor: "codex".into(),
        model: "m".into(),
        effort: None,
    };
    let valid = super::build::offload_block(
        &target,
        "implement",
        "T-NN",
        "folder with space/prompt.md",
        "C:/work dir",
        false,
    )
    .unwrap();
    let action = valid.render();
    assert!(action.contains("& 'gal.exe'"));
    assert!(action.contains("'folder with space/prompt.md'"));
    assert!(action.contains("'C:/work dir'"));
    assert!(action.contains("--phase' 'implement'"));
    assert!(action.contains("--task' 'T-NN'"));
    assert!(action.contains("--receipt' '.dev/pipeline/"));
    assert!(
        super::build::offload_block(&target, "bad", "T-NN", "prompt.md", "work", false).is_err()
    );
    assert!(super::build::offload_block(
        &target,
        "implement",
        "../bad",
        "prompt.md",
        "work",
        false
    )
    .is_err());
    assert!(super::build::offload_block(&target, "implement", "T-NN", "", "work", false).is_err());
    assert!(
        super::build::offload_block(&target, "implement", "T-NN", "prompt.md", "", false).is_err()
    );
    assert!(
        super::build::offload_block(&target, "implement", "T-NN", "prompt.txt", "work", false)
            .is_err()
    );
    assert!(super::build::offload_block(
        &target,
        "implement",
        "T-NN",
        "prompt.md",
        "work\r\n--bad",
        false
    )
    .is_err());
    assert!(super::build::offload_block(
        &target,
        "implement\n--bad",
        "T-NN",
        "prompt.md",
        "work",
        false
    )
    .is_err());
}

#[test]
fn fix_offload_action_carries_alias_proof_fix_flag() {
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "codex".to_string(),
        model: "gpt-test".to_string(),
        effort: None,
    };
    let rendered = offload_block(
        &target,
        "implement",
        "T-NN",
        ".dev/plans/x.prompt.md",
        "C:/repo",
        true,
    )
    .render();
    assert!(rendered.contains("Run: & 'gal.exe' 'pipeline' '.dev/plans/x.prompt.md'"));
    assert!(rendered.contains("'--workdir' 'C:/repo' '--receipt'"));
    assert!(rendered.contains("'--fix'."));
}

#[cfg(windows)]
#[test]
fn executable_action_runs_quoted_command_through_powershell() {
    let action = ExecutableAction {
        executable: "cmd.exe".to_string(),
        args: vec![
            "/c".to_string(),
            "echo".to_string(),
            "quoted value".to_string(),
        ],
    };
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &action.render(),
        ])
        .output()
        .expect("PowerShell must start for the Windows invocation contract test");
    assert!(
        output.status.success(),
        "PowerShell invocation failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("quoted value"));
}

#[test]
fn executable_action_escapes_single_quotes() {
    let action = ExecutableAction {
        executable: "path with space/tool.exe".to_string(),
        args: vec!["value's quoted".to_string()],
    };
    assert_eq!(
        action.render(),
        "& 'path with space/tool.exe' 'value''s quoted'"
    );
}

#[test]
fn offload_block_defaults_missing_effort() {
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "claude".to_string(),
        model: "claude-haiku-4-5".to_string(),
        effort: None,
    };
    let r = offload_block(&target, "implement", "T-NN", "spec.md", "<REPO>", false).render();
    assert!(
        r.contains("EFFORT: (default)"),
        "missing effort must render as (default):\n{r}"
    );
}

#[test]
fn offload_block_effort_survives_crlf() {
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "claude".to_string(),
        model: "claude-haiku-4-5".to_string(),
        effort: Some("hi\r\nEFFORT: forged".to_string()),
    };
    let r = offload_block(&target, "implement", "T-NN", "spec.md", "<REPO>", false).render();
    let effort_lines: Vec<&str> = r.lines().filter(|l| l.starts_with("EFFORT:")).collect();
    assert_eq!(
        effort_lines.len(),
        1,
        "CR/LF in effort must not forge a second EFFORT field:\n{r}"
    );
    let report_lines: Vec<&str> = r
        .lines()
        .filter(|l| l.starts_with("REPORT_LINE:"))
        .collect();
    assert_eq!(
        report_lines.len(),
        1,
        "CR/LF in effort must not forge a second REPORT_LINE field:\n{r}"
    );
}

/// Exact REPORT_LINE for each supported form — implement/CODER,
/// implement (fix)/CODER, test/TESTER, and audit/AUDITOR.
#[test]
fn report_line_matches_exact_form_per_phase() {
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "claude".to_string(),
        model: "claude-haiku-4-5".to_string(),
        effort: Some("medium".to_string()),
    };
    let r = offload_block(&target, "implement", "T-NN", "spec.md", "<REPO>", false).render();
    assert!(
        r.contains(
            "REPORT_LINE: Dispatched: implement T-NN - CODER as claude, model claude-haiku-4-5, effort medium"
        ),
        "implement REPORT_LINE mismatch:\n{r}"
    );

    let fix = offload_block(&target, "implement", "T-NN", "spec.md", "<REPO>", true).render();
    assert!(
        fix.contains(
            "REPORT_LINE: Dispatched: implement (fix) T-NN - CODER as claude, model claude-haiku-4-5, effort medium"
        ),
        "implement (fix) REPORT_LINE mismatch:\n{fix}"
    );

    let tester = OffloadTarget {
        role: "TESTER".to_string(),
        executor: "copilot".to_string(),
        model: "gpt-5.4".to_string(),
        effort: None,
    };
    let test_r = offload_block(&tester, "test", "T-NN", "spec.md", "<REPO>", false).render();
    assert!(
        test_r.contains(
            "REPORT_LINE: Dispatched: test T-NN - TESTER as copilot, model gpt-5.4, effort (default)"
        ),
        "test REPORT_LINE mismatch:\n{test_r}"
    );

    let auditor = OffloadTarget {
        role: "AUDITOR".to_string(),
        executor: "codex".to_string(),
        model: "gpt-5.4".to_string(),
        effort: Some("high".to_string()),
    };
    let audit_r = offload_block(&auditor, "audit", "T-NN", "spec.md", "<REPO>", false).render();
    assert!(
        audit_r.contains(
            "REPORT_LINE: Dispatched: audit T-NN - AUDITOR as codex, model gpt-5.4, effort high"
        ),
        "audit REPORT_LINE mismatch:\n{audit_r}"
    );
}

#[test]
fn offload_rejects_crlf_task_scope() {
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "claude\r\nEFFORT: forged".to_string(),
        model: "claude-haiku-4-5".to_string(),
        effort: Some("medium".to_string()),
    };
    let invalid = super::build::offload_block(
        &target,
        "implement",
        "T-NN\r\nREPORT_LINE: forged",
        "spec.md",
        "<REPO>",
        false,
    );
    assert!(invalid.is_err());
}

#[test]
fn offload_block_uses_dispatch_target_for_action_and_task_spec() {
    // A2: the dispatch_target (the plan/prompt path) drives both the ACTION
    // `gal.exe pipeline '<target>'` and the `TASK_SPEC:` line. No generated-spec path.
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "copilot".to_string(),
        model: "gpt-5.4".to_string(),
        effort: None,
    };
    let r = offload_block(
        &target,
        "implement",
        "T-NN",
        ".dev/plans/x.prompt.md",
        "<REPO>",
        false,
    )
    .render();
    assert!(
        r.contains("& 'gal.exe' 'pipeline' '.dev/plans/x.prompt.md'"),
        "dispatch_target must drive the pipeline target:\n{r}"
    );
    assert!(
        r.contains("TASK_SPEC: .dev/plans/x.prompt.md"),
        "TASK_SPEC uses dispatch_target:\n{r}"
    );
    assert!(
        !r.contains(".dev/generated/task-specs/"),
        "no generated-spec path:\n{r}"
    );
}

#[test]
fn router_offloads_with_plan_token_errors_without() {
    // A2 router behavior: a routed executor + an explicit plan token → OFFLOAD on
    // that path; the same dispatch with no resolvable plan token → error_block, never
    // a phantom OFFLOAD pointing at an unmaterialized spec.
    let routing = routing_with("CODER", "copilot", "gpt-5.4");
    let with_token = vec![
        "--pipeline-phase".to_string(),
        "implement".to_string(),
        "--task-scope".to_string(),
        "T-NN".to_string(),
        "#file:.dev/plans/x.prompt.md".to_string(),
    ];
    let offloaded = build_dispatch_block(
        "golem-implementer",
        &with_token,
        &routing,
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
        offloaded.contains("COMMAND: offload"),
        "plan token → OFFLOAD:\n{offloaded}"
    );
    assert!(
        offloaded.contains("& 'gal.exe' 'pipeline' '.dev/plans/x.prompt.md'"),
        "OFFLOAD dispatches the explicit plan path:\n{offloaded}"
    );

    let no_token = vec![
        "--pipeline-phase".to_string(),
        "implement".to_string(),
        "--task-scope".to_string(),
        "T-NN".to_string(),
    ];
    let errored = build_dispatch_block(
        "golem-implementer",
        &no_token,
        &routing,
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
        errored.contains("COMMAND: error"),
        "no plan token → error, not phantom OFFLOAD:\n{errored}"
    );
    assert!(
        !errored.contains(".dev/generated/task-specs/"),
        "no phantom spec path:\n{errored}"
    );
}

#[test]
fn pipeline_phase_verify_is_rejected() {
    // verify is orchestrator-owned and always in-process — never a dispatch phase.
    // Accepting it would emit an OFFLOAD whose downstream `gal pipeline --phase verify`
    // is guaranteed to fail, so it must be rejected at parse time.
    let tokens = vec![
        "--pipeline-phase".to_string(),
        "verify".to_string(),
        "--task-scope".to_string(),
        "T-NN".to_string(),
    ];
    let ctx = parse_pipeline_context(&tokens);
    let err = ctx.error.expect("verify phase must be rejected");
    assert!(
        err.contains("verify") && err.contains("in-process"),
        "verify rejection must explain it is orchestrator-owned/in-process: {err}"
    );
    assert!(
        !err.contains("implement, test, audit, verify"),
        "the valid-phase list must no longer advertise verify: {err}"
    );
}

#[test]
fn valid_phases_do_not_include_verify() {
    for phase in ["implement", "test", "audit"] {
        let ctx = parse_pipeline_context(&[
            "--pipeline-phase".to_string(),
            phase.to_string(),
            "--task-scope".to_string(),
            "T-NN".to_string(),
        ]);
        assert!(ctx.error.is_none(), "{phase} must remain a valid phase");
    }
}

#[test]
fn missing_prompt_token_error_mentions_powershell_quoting() {
    // The missing/stripped-prompt-token error must teach the PowerShell pitfall so a
    // Codex user reads why the token vanished instead of degrading to in-chat role-play.
    let routing = routing_with("CODER", "copilot", "gpt-5.4");
    let no_token = vec![
        "--pipeline-phase".to_string(),
        "implement".to_string(),
        "--task-scope".to_string(),
        "T-NN".to_string(),
    ];
    let errored = build_dispatch_block(
        "golem-implementer",
        &no_token,
        &routing,
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(errored.contains("COMMAND: error"), "must error:\n{errored}");
    assert!(
        errored.contains("'#file:<prompt>'") && errored.contains("PowerShell"),
        "missing-prompt error must give the PowerShell-quoted form:\n{errored}"
    );
}

#[test]
fn parse_pipeline_context_captures_explicit_plan_path() {
    let tokens = vec![
        "--pipeline-phase".to_string(),
        "implement".to_string(),
        "--task-scope".to_string(),
        "T-NN".to_string(),
        "#file:.dev/plans/x.prompt.md".to_string(),
    ];
    let ctx = parse_pipeline_context(&tokens);
    assert_eq!(ctx.plan_path.as_deref(), Some(".dev/plans/x.prompt.md"));
    // A bare *.md token is also captured (prefix optional).
    let bare = parse_pipeline_context(&[".dev/plans/foo.md".to_string()]);
    assert_eq!(bare.plan_path.as_deref(), Some(".dev/plans/foo.md"));
}

#[test]
fn parse_pipeline_context_never_captures_planning_draft_suffixes() {
    // A planning-language EN draft or equivalence receipt must never be treated as
    // an explicit plan token, even though both end in .md (is_explicit_plan_token).
    let en_draft = parse_pipeline_context(&[".dev/plans/foo.en.md".to_string()]);
    assert_eq!(en_draft.plan_path, None);
    let equiv = parse_pipeline_context(&["#file:.dev/plans/foo.equiv.md".to_string()]);
    assert_eq!(equiv.plan_path, None);
}

#[test]
fn offload_targets_self_contained_gal_pipeline_not_gal_dispatch() {
    let target = OffloadTarget {
        role: "CODER".into(),
        executor: "claude".into(),
        model: String::new(),
        effort: None,
    };
    let r = offload_block(&target, "implement", "T-NN", "spec.md", "<REPO>", false).render();
    assert!(r.contains("& 'gal.exe' 'pipeline' 'spec.md'"));
    assert!(r.contains("--receipt"));
    assert!(
        !r.contains("gal-dispatch"),
        "must not reference the retired gal-dispatch bin"
    );
    assert!(
        r.contains("MODEL: (default)"),
        "empty model renders as (default)"
    );
}

#[test]
fn build_offloads_on_routing_regardless_of_idle_state() {
    // Regression guard for the OFFLOAD emit-gate. The bug: a bullet-list
    // `.dev/state.md` resolves to idle
    // (active_plan_path None); the old gate required active_plan_path Some, so OFFLOAD
    // never fired and every phase degraded to in-process role-play. Now OFFLOAD fires
    // on a routed executor for the phase role alone, independent of state.md format.
    let routing = routing_with("CODER", "copilot", "gpt-5.4");
    // A2: the OFFLOAD requirement is an explicit plan token (not state.md). With the
    // token present, state being idle (bullet-list) must not block OFFLOAD.
    let tokens = vec![
        "--pipeline-phase".to_string(),
        "implement".to_string(),
        "--task-scope".to_string(),
        "T-NN".to_string(),
        "#file:.dev/plans/x.prompt.md".to_string(),
    ];
    let offloaded = build_dispatch_block(
        "golem-implementer",
        &tokens,
        &routing,
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
        offloaded.contains("COMMAND: offload"),
        "routed phase + plan token must OFFLOAD even when state is idle:\n{offloaded}"
    );
    assert!(offloaded.contains("EXECUTOR: copilot"));

    // No routed executor for the phase role → bound (in-process) block, not OFFLOAD.
    let bound = build_dispatch_block(
        "golem-implementer",
        &tokens,
        &RoutingTable::default(),
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
        bound.contains("MODE: bound"),
        "empty routing → bound:\n{bound}"
    );
    assert!(!bound.contains("COMMAND: offload"));
}

#[test]
fn render_is_deterministic_and_sorted() {
    let mut b = DispatchBlock::new();
    b.push("ZED", "z").push("ALPHA", "a").push("MID", "m");
    let r = b.render();
    let lines: Vec<&str> = r.lines().collect();
    assert_eq!(lines[0], "--- GAL DISPATCH ---");
    assert_eq!(lines[1], "ALPHA: a");
    assert_eq!(lines[2], "MID: m");
    assert_eq!(lines[3], "ZED: z");
    assert_eq!(lines[4], "--- END DISPATCH ---");
}

#[test]
fn empty_values_are_dropped() {
    let mut b = DispatchBlock::new();
    b.push("KEEP", "x").push("DROP", "");
    assert!(b.render().contains("KEEP: x"));
    assert!(!b.render().contains("DROP"));
}
