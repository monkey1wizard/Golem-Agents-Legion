//! Dedicated tests for the pure `TestFirstEvaluation` evaluator: marker and
//! generation classification, R4/R16 evidence validation, dispute grammar and
//! precedence, retry accounting, phase order, and the overall verdict.

use pipeline::task_spec::probe_evidence::{
    compute_env_digest, compute_probe_identity_bytes, compute_probe_set_digest,
    create_output_envelope, decode_argv_b64, encode_argv_b64, FailureClass, ObservedResult,
    ProbeReceipt, ProbeRecord, ProbeSelector, Termination, RUNNER_CONTRACT,
};
use pipeline::task_spec::{encode_base64url_unpadded, Applicability, TaskContract};
use pipeline::test_first_evidence::{
    compare_receipt_identities, evaluate, parse_expected_failure, validate_expected_failure,
    validate_phase_order, ContractMode, EvaluationInput, EvaluationVerdict, PhaseEvidence,
    LEGACY_PHASES, REQUIRED_PHASES,
};
use std::collections::BTreeMap;

fn tid(n: u32) -> String {
    format!("T-{n:02}")
}

fn did(task_n: u32, dispute_n: u32) -> String {
    format!("{}-D{dispute_n:02}", tid(task_n))
}

fn base_contract(task: &str) -> TaskContract {
    TaskContract {
        plan_slug: "plan".to_string(),
        task_id: task.to_string(),
        applicability: Applicability::Required,
        seam: "crates/pipeline/src/test_first_evidence.rs".to_string(),
        expected_failures: vec![format!(
            "EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64={} — locked failure",
            encode_base64url_unpadded(b"needle")
        )],
        production_paths: vec!["crates/pipeline/src/test_first_evidence.rs".to_string()],
        test_paths: vec!["crates/pipeline/tests/test_first_evidence.rs".to_string()],
        scaffold: "not-required".to_string(),
        not_applicable_rationale: String::new(),
        non_red_probe: String::new(),
        tp_rows: vec![format!("| TP-{:02} | ... |", 1)],
    }
}

fn generation_table(rows: &[(&str, u32, &str, &str)]) -> String {
    let mut table = String::from(
        "### Test-First Generations\n\n| Task | Generation | Contract Digest | Reason |\n| --- | --- | --- | --- |\n",
    );
    for (task, generation, digest, reason) in rows {
        table.push_str(&format!(
            "| {task} | {generation} | {digest} | {reason} |\n"
        ));
    }
    table
}

fn marked_prompt(generation_rows: &str) -> String {
    format!(
        "# Plan Prompt: Test\n\nPipeline Contract: test-first-v1\n\n## Status\n\n{generation_rows}\n## Tasks\n\n- [ ] {} — placeholder.\n",
        tid(27)
    )
}

fn legacy_prompt() -> String {
    format!(
        "# Plan Prompt: Test\n\n## Status\n\n## Tasks\n\n- [ ] {} — placeholder.\n",
        tid(1)
    )
}

fn phase(name: &str, completed: bool) -> PhaseEvidence {
    PhaseEvidence {
        phase: name.to_string(),
        completed,
    }
}

fn completed_phases(names: &[&str]) -> Vec<PhaseEvidence> {
    names.iter().map(|n| phase(n, true)).collect()
}

#[derive(Clone)]
struct ProbeFixture {
    record: ProbeRecord,
    output_ref: String,
    envelope: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
fn make_probe(
    id: &str,
    selector: ProbeSelector,
    observed: ObservedResult,
    stdout: &[u8],
    stderr: &[u8],
    expected_failure_ref: Option<&str>,
    failure_class: Option<FailureClass>,
    termination: Termination,
) -> ProbeFixture {
    let argv_b64 = encode_argv_b64(&["probe".to_string()]).unwrap();
    let env_digest = compute_env_digest(&[("KEY", "VALUE")]).unwrap();
    let (envelope, output_digest, output_ref) = create_output_envelope(stdout, stderr);
    let record = ProbeRecord {
        id: id.to_string(),
        selector,
        argv: vec!["probe".to_string()],
        argv_b64,
        env_digest,
        expected_failure_ref: expected_failure_ref.map(str::to_string),
        observed,
        failure_class,
        termination,
        timeout_ms: 2_000,
        output_ref: output_ref.clone(),
        output_digest,
    };
    ProbeFixture {
        record,
        output_ref,
        envelope,
    }
}

/// `verdict` is derived the same way the real runner derives it: `pass` only
/// when every record observed `pass` (the probe runner's own `publish_receipt`).
/// Callers do not get to assert an inconsistent verdict against their own fixtures.
fn make_receipt(
    plan: &str,
    task: &str,
    generation: u32,
    contract_digest: &str,
    phase: &str,
    expectation: &str,
    fixtures: &[&ProbeFixture],
) -> ProbeReceipt {
    let verdict = if fixtures
        .iter()
        .all(|f| f.record.observed == ObservedResult::Pass)
    {
        "pass"
    } else {
        "fail"
    };
    make_receipt_with_forged_verdict(
        plan,
        task,
        generation,
        contract_digest,
        phase,
        expectation,
        verdict,
        fixtures,
    )
}

/// Builds a receipt with an explicitly forced `verdict`, bypassing the honest
/// derivation `make_receipt` performs — for proving the evaluator catches a
/// receipt whose self-reported verdict does not match its own records
/// (tampering/corruption), independent of the phase/expectation verdict check.
#[allow(clippy::too_many_arguments)]
fn make_receipt_with_forged_verdict(
    plan: &str,
    task: &str,
    generation: u32,
    contract_digest: &str,
    phase: &str,
    expectation: &str,
    verdict: &str,
    fixtures: &[&ProbeFixture],
) -> ProbeReceipt {
    let identities: Vec<Vec<u8>> = fixtures
        .iter()
        .map(|f| {
            let (_, frame) = decode_argv_b64(&f.record.argv_b64).unwrap();
            compute_probe_identity_bytes(
                &f.record.id,
                f.record.selector,
                &frame,
                &f.record.env_digest,
                f.record.expected_failure_ref.as_deref(),
                f.record.timeout_ms,
            )
        })
        .collect();
    ProbeReceipt {
        plan: plan.to_string(),
        task: task.to_string(),
        generation,
        contract_digest: contract_digest.to_string(),
        phase: phase.to_string(),
        expectation: expectation.to_string(),
        runner_contract: RUNNER_CONTRACT.to_string(),
        probe_set_digest: compute_probe_set_digest(&identities),
        verdict: verdict.to_string(),
        records: fixtures.iter().map(|f| f.record.clone()).collect(),
    }
}

fn envelopes(fixtures: &[&ProbeFixture]) -> BTreeMap<String, Vec<u8>> {
    fixtures
        .iter()
        .map(|f| (f.output_ref.clone(), f.envelope.clone()))
        .collect()
}

// ── Legacy mode ─────────────────────────────────────────────────────────────

#[test]
fn legacy_prompt_uses_legacy_phases_and_passes_without_a_contract() {
    let prompt = legacy_prompt();
    let phases = completed_phases(LEGACY_PHASES);
    let task = tid(1);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: None,
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.mode, ContractMode::Legacy);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.is_pass());
    assert_eq!(result.generation, None);
}

#[test]
fn no_completed_phases_yields_not_run_with_no_errors() {
    let prompt = legacy_prompt();
    let phases: Vec<PhaseEvidence> = LEGACY_PHASES.iter().map(|n| phase(n, false)).collect();
    let task = tid(1);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: None,
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    // Gaps still produce an error (required phases never completed), so a
    // clean NotRun requires the phase list to be empty, not merely all-false.
    let input_empty = EvaluationInput {
        phases: &[],
        ..input
    };
    let result_empty = evaluate(&input_empty);
    assert!(!result.errors.is_empty());
    assert_eq!(result_empty.verdict, EvaluationVerdict::NotRun);
    assert!(result_empty.errors.is_empty(), "{:?}", result_empty.errors);
}

// ── Test-first mode: generation ledger ──────────────────────────────────────

#[test]
fn test_first_task_without_contract_fails() {
    let table = generation_table(&[]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &tid(27),
        prompt_body: &prompt,
        contract: None,
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.mode, ContractMode::TestFirst);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
    assert!(!result.errors.is_empty());
}

#[test]
fn missing_generation_ledger_row_fails() {
    let table = generation_table(&[]);
    let prompt = marked_prompt(&table);
    let contract = base_contract(&tid(27));
    let phases = completed_phases(REQUIRED_PHASES);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &tid(27),
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
    assert!(result.generation.is_none());
}

#[test]
fn duplicate_generation_row_fails() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init"), (&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
}

#[test]
fn generation_gap_fails() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[
        (&task, 1, &digest, "init"),
        (&task, 3, &digest, "phase-rerun"),
    ]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
}

#[test]
fn invalid_generation_reason_fails() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "because-i-said-so")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
}

#[test]
fn generation_contract_digest_mismatch_fails() {
    let contract = base_contract(&tid(27));
    let task = tid(27);
    let wrong_digest = "0".repeat(64);
    let table = generation_table(&[(&task, 1, &wrong_digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
}

#[test]
fn valid_single_generation_row_resolves_generation_and_digest() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[],
        output_envelopes: &BTreeMap::new(),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    // No receipts published yet, so evidence validation still fails — but
    // generation resolution itself must succeed independently of that.
    assert_eq!(result.generation, Some(1));
    assert_eq!(result.contract_digest.as_deref(), Some(digest.as_str()));
}

// ── Phase order ──────────────────────────────────────────────────────────────

#[test]
fn phase_order_accepts_exact_required_sequence() {
    let mut errors = Vec::new();
    let phases = completed_phases(REQUIRED_PHASES);
    let completed = validate_phase_order(&phases, REQUIRED_PHASES, &mut errors);
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(completed, REQUIRED_PHASES);
}

#[test]
fn phase_order_rejects_out_of_order_completion() {
    let mut errors = Vec::new();
    let phases = vec![phase("test", true), phase("scaffold", true)];
    validate_phase_order(&phases, REQUIRED_PHASES, &mut errors);
    assert!(!errors.is_empty());
}

#[test]
fn phase_order_rejects_gaps() {
    let mut errors = Vec::new();
    let phases = vec![phase("scaffold", true), phase("test", true)];
    validate_phase_order(&phases, REQUIRED_PHASES, &mut errors);
    assert!(errors.iter().any(|e| e.contains("gaps")));
}

#[test]
fn phase_order_rejects_unknown_phase_name() {
    let mut errors = Vec::new();
    let phases = vec![phase("bogus-phase", true)];
    validate_phase_order(&phases, REQUIRED_PHASES, &mut errors);
    assert!(errors.iter().any(|e| e.contains("unknown phase")));
}

// ── Disputes ─────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
fn dispute_line(
    id: &str,
    generation: u32,
    digest: &str,
    basis: &str,
    classification: &str,
    status: &str,
    retry_before: u32,
    retry_after: u32,
) -> String {
    let refs = encode_base64url_unpadded(b"[]");
    format!(
        "dispute_id={id};generation={generation};contract_digest={digest};lock_refs_b64={refs};evidence_refs_b64={refs};basis={basis};classification={classification};status={status};retry_before={retry_before};retry_after={retry_after}"
    )
}

#[test]
fn well_formed_open_implementation_defect_stays_open_with_valid_retry_transition() {
    let digest = "a".repeat(64);
    let line = dispute_line(
        did(27, 1).as_str(),
        1,
        &digest,
        "behavior-wrong",
        "implementation-defect",
        "open",
        0,
        1,
    );
    let mut errors = Vec::new();
    let open = pipeline::test_first_evidence::validate_dispute_history(
        &[line],
        Some(1),
        Some(&digest),
        0,
        &mut errors,
    );
    assert!(errors.is_empty(), "{errors:?}");
    assert_eq!(open, vec![did(27, 1)]);
}

#[test]
fn dispute_with_wrong_retry_transition_fails() {
    let digest = "a".repeat(64);
    let line = dispute_line(
        did(27, 1).as_str(),
        1,
        &digest,
        "behavior-wrong",
        "implementation-defect",
        "open",
        0,
        5, // should be retry_before + 1 = 1
    );
    let mut errors = Vec::new();
    pipeline::test_first_evidence::validate_dispute_history(
        &[line],
        Some(1),
        Some(&digest),
        0,
        &mut errors,
    );
    assert!(errors
        .iter()
        .any(|e| e.contains("invalid retry transition")));
}

#[test]
fn dispute_stale_generation_or_digest_fails() {
    let digest = "a".repeat(64);
    let line = dispute_line(
        did(27, 1).as_str(),
        1,
        &digest,
        "probe-assertion",
        "probe-defect",
        "resolved",
        0,
        0,
    );
    let mut errors = Vec::new();
    // Current generation is 2, not 1: this dispute is stale.
    pipeline::test_first_evidence::validate_dispute_history(
        &[line],
        Some(2),
        Some(&digest),
        0,
        &mut errors,
    );
    assert!(errors.iter().any(|e| e.contains("stale dispute")));
}

#[test]
fn dispute_non_monotonic_ids_fail() {
    let digest = "a".repeat(64);
    let first = dispute_line(
        did(27, 2).as_str(),
        1,
        &digest,
        "probe-fixture",
        "probe-defect",
        "resolved",
        0,
        0,
    );
    let second = dispute_line(
        did(27, 1).as_str(),
        1,
        &digest,
        "probe-fixture",
        "probe-defect",
        "resolved",
        0,
        0,
    );
    let mut errors = Vec::new();
    pipeline::test_first_evidence::validate_dispute_history(
        &[first, second],
        Some(1),
        Some(&digest),
        0,
        &mut errors,
    );
    assert!(errors.iter().any(|e| e.contains("strictly increasing")));
}

#[test]
fn dispute_invalid_basis_classification_mapping_fails() {
    let digest = "a".repeat(64);
    // contract-ambiguous requires a "contract-*" basis, not "behavior-wrong".
    let line = dispute_line(
        did(27, 1).as_str(),
        1,
        &digest,
        "behavior-wrong",
        "contract-ambiguous",
        "open",
        0,
        0,
    );
    let mut errors = Vec::new();
    pipeline::test_first_evidence::validate_dispute_history(
        &[line],
        Some(1),
        Some(&digest),
        0,
        &mut errors,
    );
    assert!(!errors.is_empty());
}

#[test]
fn dispute_malformed_payload_fails_closed() {
    let mut errors = Vec::new();
    pipeline::test_first_evidence::validate_dispute_history(
        &["not;a;valid;dispute;payload".to_string()],
        Some(1),
        Some("a".repeat(64).as_str()),
        0,
        &mut errors,
    );
    assert!(!errors.is_empty());
}

#[test]
fn latest_row_for_a_dispute_id_controls_status() {
    let digest = "a".repeat(64);
    // The append-only ledger legitimately writes the SAME dispute id twice:
    // once to open it, later to resolve it. "Monotonic dispute IDs" (R7)
    // governs the order NEW ids are first assigned, not every line — a status
    // update must be allowed to reappear for an already-seen id without being
    // treated as an out-of-order new dispute.
    let opened = dispute_line(
        did(27, 1).as_str(),
        1,
        &digest,
        "behavior-wrong",
        "implementation-defect",
        "open",
        0,
        1,
    );
    let resolved = dispute_line(
        did(27, 1).as_str(),
        1,
        &digest,
        "behavior-wrong",
        "implementation-defect",
        "resolved",
        1,
        1,
    );
    let mut errors = Vec::new();
    let open = pipeline::test_first_evidence::validate_dispute_history(
        &[opened, resolved],
        Some(1),
        Some(&digest),
        1,
        &mut errors,
    );
    assert!(errors.is_empty(), "{errors:?}");
    // The latest row (resolved) controls status, so the dispute is no longer open.
    assert!(open.is_empty(), "{open:?}");
}

// ── Receipts and evidence ────────────────────────────────────────────────────

#[test]
fn receipt_with_notrun_observation_is_never_valid_evidence() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);

    let fixture = make_probe(
        "P-01",
        ProbeSelector::NonRed,
        ObservedResult::NotRun,
        b"",
        b"",
        None,
        None,
        Termination::SpawnError,
    );
    // Forge verdict=pass (matching the test/pass phase's own expectation) so the
    // per-record NotRun check is what's actually exercised here, independent of
    // the verdict-consistency check.
    let receipt = make_receipt_with_forged_verdict(
        "plan",
        &task,
        1,
        &digest,
        "test",
        "pass",
        "pass",
        &[&fixture],
    );
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[receipt],
        output_envelopes: &envelopes(&[&fixture]),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
    assert!(result.errors.iter().any(|e| e.contains("NotRun")));
}

#[test]
fn stale_or_cross_plan_receipt_is_rejected() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);

    let fixture = make_probe(
        "P-01",
        ProbeSelector::NonRed,
        ObservedResult::Pass,
        b"out",
        b"",
        None,
        None,
        Termination::Exit(0),
    );
    // Wrong plan slug — this receipt belongs to a different plan entirely.
    let receipt = make_receipt("other-plan", &task, 1, &digest, "test", "pass", &[&fixture]);
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[receipt],
        output_envelopes: &envelopes(&[&fixture]),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
    assert!(result
        .errors
        .iter()
        .any(|e| e.contains("stale or cross-plan")));
}

#[test]
fn red_receipt_containing_a_passing_probe_is_rejected() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);

    // observed=Pass inside a (test,red) receipt: infrastructure success can
    // never be red evidence.
    let fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Pass,
        b"needle",
        b"",
        Some("EF-01"),
        None,
        Termination::Exit(0),
    );
    // Forge verdict=fail (matching the test/red phase's own expectation) so the
    // per-record check is what's actually exercised, independent of the
    // verdict-consistency check.
    let receipt = make_receipt_with_forged_verdict(
        "plan",
        &task,
        1,
        &digest,
        "test",
        "red",
        "fail",
        &[&fixture],
    );
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[receipt],
        output_envelopes: &envelopes(&[&fixture]),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
    assert!(result
        .errors
        .iter()
        .any(|e| e.contains("non-failing probe")));
}

#[test]
fn green_receipt_containing_a_failing_probe_is_rejected() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);

    let fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"nope",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    // Forge verdict=pass (matching the green-rerun/green phase's own
    // expectation) so the per-record check is what's actually exercised,
    // independent of the verdict-consistency check.
    let receipt = make_receipt_with_forged_verdict(
        "plan",
        &task,
        1,
        &digest,
        "green-rerun",
        "green",
        "pass",
        &[&fixture],
    );
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[receipt],
        output_envelopes: &envelopes(&[&fixture]),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
    assert!(result.errors.iter().any(|e| e.contains("failing probe")));
}

#[test]
fn matching_red_and_green_identities_pass_and_probe_set_digest_resolves() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);

    let red_fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"needle",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    let green_fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Pass,
        b"anything",
        b"",
        Some("EF-01"),
        None,
        Termination::Exit(0),
    );
    let red = make_receipt("plan", &task, 1, &digest, "test", "red", &[&red_fixture]);
    let green = make_receipt(
        "plan",
        &task,
        1,
        &digest,
        "green-rerun",
        "green",
        &[&green_fixture],
    );
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[red, green],
        output_envelopes: &envelopes(&[&red_fixture, &green_fixture]),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.is_pass());
    assert!(result.probe_set_digest.is_some());
}

#[test]
fn mismatched_red_and_green_identity_counts_fail() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);

    let red_fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"needle",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    let red_fixture_2 = make_probe(
        "P-02",
        ProbeSelector::NonRed,
        ObservedResult::Pass,
        b"ok",
        b"",
        None,
        None,
        Termination::Exit(0),
    );
    let green_fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Pass,
        b"anything",
        b"",
        Some("EF-01"),
        None,
        Termination::Exit(0),
    );
    // Red has 2 records, green only 1 — the record count itself must match.
    let red = make_receipt(
        "plan",
        &task,
        1,
        &digest,
        "test",
        "red",
        &[&red_fixture, &red_fixture_2],
    );
    let green = make_receipt(
        "plan",
        &task,
        1,
        &digest,
        "green-rerun",
        "green",
        &[&green_fixture],
    );
    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[red, green],
        output_envelopes: &envelopes(&[&red_fixture, &red_fixture_2, &green_fixture]),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert_eq!(result.verdict, EvaluationVerdict::Fail);
}

#[test]
fn compare_receipt_identities_directly_accepts_matching_and_rejects_differing() {
    let a = make_probe(
        "P-01",
        ProbeSelector::NonRed,
        ObservedResult::Fail,
        b"x",
        b"",
        None,
        None,
        Termination::Exit(1),
    );
    let b = make_probe(
        "P-01",
        ProbeSelector::NonRed,
        ObservedResult::Pass,
        b"y",
        b"",
        None,
        None,
        Termination::Exit(0),
    );
    let task = tid(1);
    let red = make_receipt("plan", &task, 1, &"a".repeat(64), "test", "red", &[&a]);
    let green = make_receipt(
        "plan",
        &task,
        1,
        &"a".repeat(64),
        "green-rerun",
        "green",
        &[&b],
    );
    assert!(compare_receipt_identities(&red, &green).is_ok());

    let different_id = make_probe(
        "P-02",
        ProbeSelector::NonRed,
        ObservedResult::Pass,
        b"y",
        b"",
        None,
        None,
        Termination::Exit(0),
    );
    let mismatched_green = make_receipt(
        "plan",
        &task,
        1,
        &"a".repeat(64),
        "green-rerun",
        "green",
        &[&different_id],
    );
    assert!(compare_receipt_identities(&red, &mismatched_green).is_err());
}

// ── Cross-actor environment divergence ───────────────────────────────────────

/// `test-first-v1` has the red receipt authored by a dispatched TESTER and the
/// green rerun authored by the in-process ORCHESTRATOR, so the two are produced
/// by different process trees and never share an ambient environment. Cross-receipt
/// comparison must therefore ignore `env_digest`, or every dispatched
/// `Test-first: required` task fails by construction.
fn with_env_digest(fixture: &ProbeFixture, pairs: &[(&str, &str)]) -> ProbeFixture {
    let mut out = fixture.clone();
    out.record.env_digest = compute_env_digest(pairs).unwrap();
    out
}

#[test]
fn red_and_green_differing_only_in_env_digest_pass() {
    let contract = base_contract(&tid(27));
    let digest = contract.compute_digest();
    let task = tid(27);
    let table = generation_table(&[(&task, 1, &digest, "init")]);
    let prompt = marked_prompt(&table);
    let phases = completed_phases(REQUIRED_PHASES);

    let red_fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"needle",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    let green_fixture = with_env_digest(
        &make_probe(
            "P-01",
            ProbeSelector::Acceptance,
            ObservedResult::Pass,
            b"anything",
            b"",
            Some("EF-01"),
            None,
            Termination::Exit(0),
        ),
        &[("KEY", "VALUE"), ("DISPATCHED_SUBPROCESS", "1")],
    );
    assert_ne!(
        red_fixture.record.env_digest, green_fixture.record.env_digest,
        "fixture must actually reproduce the cross-process env divergence"
    );

    let red = make_receipt("plan", &task, 1, &digest, "test", "red", &[&red_fixture]);
    let green = make_receipt(
        "plan",
        &task,
        1,
        &digest,
        "green-rerun",
        "green",
        &[&green_fixture],
    );
    assert_ne!(
        red.probe_set_digest, green.probe_set_digest,
        "the receipts' own digests must genuinely differ, as they do in production"
    );

    let input = EvaluationInput {
        plan_slug: "plan",
        task_id: &task,
        prompt_body: &prompt,
        contract: Some(&contract),
        receipts: &[red, green],
        output_envelopes: &envelopes(&[&red_fixture, &green_fixture]),
        disputes: &[],
        retry_count: 0,
        phases: &phases,
    };
    let result = evaluate(&input);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.is_pass());
    assert!(result.probe_set_digest.is_some());
}

/// The projection must not over-widen: every field it still covers must keep
/// rejecting a red/green pair, even when both share one `env_digest`.
#[test]
fn shared_env_digest_does_not_excuse_any_other_identity_difference() {
    let red_fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"needle",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    let base_green = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Pass,
        b"anything",
        b"",
        Some("EF-01"),
        None,
        Termination::Exit(0),
    );
    let task = tid(27);
    let cd = "a".repeat(64);
    let red = make_receipt("plan", &task, 1, &cd, "test", "red", &[&red_fixture]);
    assert!(
        compare_receipt_identities(
            &red,
            &make_receipt(
                "plan",
                &task,
                1,
                &cd,
                "green-rerun",
                "green",
                &[&base_green]
            )
        )
        .is_ok(),
        "control: an otherwise-identical pair must pass"
    );

    let mut different_id = base_green.clone();
    different_id.record.id = "P-02".to_string();

    let mut different_argv = base_green.clone();
    different_argv.record.argv = vec!["other".to_string()];
    different_argv.record.argv_b64 = encode_argv_b64(&["other".to_string()]).unwrap();

    let mut different_timeout = base_green.clone();
    different_timeout.record.timeout_ms = 9_000;

    let mut different_ef = base_green.clone();
    different_ef.record.expected_failure_ref = Some("EF-02".to_string());

    for (label, varied) in [
        ("id", &different_id),
        ("argv", &different_argv),
        ("timeout_ms", &different_timeout),
        ("expected_failure_ref", &different_ef),
    ] {
        assert_eq!(
            red_fixture.record.env_digest, varied.record.env_digest,
            "{label}: env must be held constant so only the named field varies"
        );
        let green = make_receipt("plan", &task, 1, &cd, "green-rerun", "green", &[varied]);
        assert!(
            compare_receipt_identities(&red, &green).is_err(),
            "{label} difference must still be rejected"
        );
    }
}

/// A receipt whose header digest was written under the unchanged env-inclusive
/// formula must still parse and self-verify — archived evidence stays readable.
#[test]
fn archived_env_inclusive_receipt_still_parses() {
    let fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"needle",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    let receipt = make_receipt(
        "plan",
        &tid(27),
        1,
        &"a".repeat(64),
        "test",
        "red",
        &[&fixture],
    );
    let bytes = receipt.to_receipt_bytes();
    let parsed = ProbeReceipt::parse_receipt_bytes(&bytes).expect("archived receipt must parse");
    assert_eq!(parsed.probe_set_digest, receipt.probe_set_digest);
}

// ── Expected-failure parsing and matching ────────────────────────────────────

#[test]
fn parse_expected_failure_round_trips_a_well_formed_line() {
    let line = format!(
        "EF-01;class=error-code;term=exit:3;stream=combined;matcher_b64={} — prose",
        encode_base64url_unpadded(b"boom")
    );
    let parsed = parse_expected_failure(&line).unwrap();
    assert_eq!(parsed.reference, "EF-01");
    assert_eq!(parsed.class, FailureClass::ErrorCode);
    assert_eq!(parsed.matcher, "boom");
    assert_eq!(parsed.prose, "prose");
}

#[test]
fn parse_expected_failure_rejects_malformed_header() {
    assert!(parse_expected_failure("not a valid line").is_err());
    assert!(parse_expected_failure(
        "EF-01;class=bogus;term=nonzero;stream=stdout;matcher_b64= — x"
    )
    .is_err());
}

#[test]
fn validate_expected_failure_matches_locked_class_termination_and_stream() {
    let fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"contains needle here",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    let expected = parse_expected_failure(&format!(
        "EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64={} — locked",
        encode_base64url_unpadded(b"needle")
    ))
    .unwrap();
    assert!(validate_expected_failure(&fixture.record, &expected, &fixture.envelope).is_ok());
}

#[test]
fn validate_expected_failure_rejects_a_mismatched_matcher() {
    let fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"nothing relevant here",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(1),
    );
    let expected = parse_expected_failure(&format!(
        "EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64={} — locked",
        encode_base64url_unpadded(b"needle")
    ))
    .unwrap();
    assert!(validate_expected_failure(&fixture.record, &expected, &fixture.envelope).is_err());
}

#[test]
fn validate_expected_failure_rejects_a_mismatched_termination() {
    let fixture = make_probe(
        "P-01",
        ProbeSelector::Acceptance,
        ObservedResult::Fail,
        b"needle",
        b"",
        Some("EF-01"),
        Some(FailureClass::Assertion),
        Termination::Exit(0), // does not satisfy "nonzero"
    );
    let expected = parse_expected_failure(&format!(
        "EF-01;class=assertion;term=nonzero;stream=stdout;matcher_b64={} — locked",
        encode_base64url_unpadded(b"needle")
    ))
    .unwrap();
    assert!(validate_expected_failure(&fixture.record, &expected, &fixture.envelope).is_err());
}
