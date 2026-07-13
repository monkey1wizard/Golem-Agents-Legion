use super::*;

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

const CONSULT_FIXTURE: &str =
    include_str!("../../tests/fixtures/dispatch-script/consult-golem.txt");
const UTILITY_FIXTURE: &str =
    include_str!("../../tests/fixtures/dispatch-script/utility-golem.txt");
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

#[test]
fn mode_rules_match_shell() {
    // utility golems → utility regardless of pipeline context
    assert_eq!(golem_mode("golem-debugger", false, false), "utility");
    assert_eq!(golem_mode("golem-debugger", true, false), "utility");
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
    // consult/utility roles always callable
    assert_eq!(golem_mode("golem-architect", true, false), "consult");
    assert_eq!(golem_mode("golem-analyst", false, false), "consult");
    assert_eq!(golem_mode("golem-designer", false, false), "consult");
    // steward is a first-class consult golem (not utility, not pipeline)
    assert_eq!(golem_mode("golem-steward", true, false), "consult");
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
fn finalize_branch_audit_flag_authorizes_auditor() {
    let tokens = vec!["--finalize-branch-audit".to_string()];
    let rendered = build_dispatch_block(
        "auditor",
        &tokens,
        &RoutingTable::default(),
        &StateContext::idle(),
        "<REPO>",
    )
    .render();
    assert!(
            !rendered.contains("COMMAND: error"),
            "--finalize-branch-audit must bypass the orchestrated-only gate for auditor; got: {rendered}"
        );
}

#[test]
fn consult_block_matches_fixture() {
    let golem = "golem-architect";
    let mode = golem_mode(golem, false, false);
    assert_eq!(mode, "consult");
    assert_parity(&golem_block(golem, mode, &[]).render(), CONSULT_FIXTURE);
}

#[test]
fn utility_block_matches_fixture() {
    let golem = "golem-debugger";
    let mode = golem_mode(golem, false, false);
    assert_eq!(mode, "utility");
    assert_parity(&golem_block(golem, mode, &[]).render(), UTILITY_FIXTURE);
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
fn bound_implement_matches_fixture() {
    // full context mode: WORKFLOW_STATE + PIPELINE_CONTEXT_FILES present.
    assert_bound_parity("golem-implementer", "implement", "T-NNN", BOUND_IMPLEMENT);
}

#[test]
fn bound_test_audit_match_fixtures() {
    // delta context mode: no WORKFLOW_STATE / files list.
    assert_bound_parity("golem-tester", "test", "T-NNN", BOUND_TEST);
    assert_bound_parity("golem-auditor", "audit", "T-NNN", BOUND_AUDIT);
}

#[test]
fn context_mode_is_full_for_implement_delta_otherwise() {
    let mut full = DispatchBlock::new();
    push_pipeline_metadata(
        &mut full,
        "implement",
        true,
        &StateContext::idle(),
        "<REPO>",
    );
    assert!(full.render().contains("PIPELINE_CONTEXT_MODE: full"));
    assert!(full.render().contains("WORKFLOW_STATE: IDLE"));
    let mut delta = DispatchBlock::new();
    push_pipeline_metadata(&mut delta, "test", true, &StateContext::idle(), "<REPO>");
    assert!(delta.render().contains("PIPELINE_CONTEXT_MODE: delta"));
    assert!(!delta.render().contains("WORKFLOW_STATE"));
    assert!(!delta.render().contains("PIPELINE_CONTEXT_FILES"));
}

#[test]
fn fix_mode_adds_field() {
    let b = bound_golem_block(
        "golem-implementer",
        "implement",
        "T-NNN",
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
    };
    let dispatch_target = "<REPO>/.dev/plans/x.prompt.md";
    let block = offload_block(&target, "implement", "T-NNN", dispatch_target, "<REPO>");
    assert_parity(&block.render(), OFFLOAD);
}

#[test]
fn offload_block_uses_dispatch_target_for_action_and_task_spec() {
    // A2: the dispatch_target (the plan/prompt path) drives both the ACTION
    // `gal pipeline '<target>'` and the `TASK_SPEC:` line. No generated-spec path.
    let target = OffloadTarget {
        role: "CODER".to_string(),
        executor: "copilot".to_string(),
        model: "gpt-5.4".to_string(),
    };
    let r = offload_block(
        &target,
        "implement",
        "T-NNN",
        ".dev/plans/x.prompt.md",
        "<REPO>",
    )
    .render();
    assert!(
        r.contains("gal pipeline '.dev/plans/x.prompt.md'"),
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
        "T-NNN".to_string(),
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
        offloaded.contains("gal pipeline '.dev/plans/x.prompt.md'"),
        "OFFLOAD dispatches the explicit plan path:\n{offloaded}"
    );

    let no_token = vec![
        "--pipeline-phase".to_string(),
        "implement".to_string(),
        "--task-scope".to_string(),
        "T-NNN".to_string(),
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
        "T-NNN".to_string(),
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
            "T-NNN".to_string(),
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
        "T-NNN".to_string(),
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
        "T-NNN".to_string(),
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
    };
    let r = offload_block(&target, "implement", "T-NNN", "spec.md", "<REPO>").render();
    assert!(r.contains("gal pipeline 'spec.md'"));
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
        "T-NNN".to_string(),
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
