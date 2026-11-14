use dispatch::usage::UsageCategory;
use pipeline::usage::assessment::{
    assess, publish_judgment, Assessment, AssessmentError, AssessmentRecord, Assignment,
    BasisAssessment, BasisKey, Completeness, Outcome, Ratio,
};
use pipeline::usage::reconcile::{
    reconcile, Amount, ElapsedInterval, ObservationRecord, ReconcileInput, Reconciliation,
};
use pipeline::usage::{digest_bytes, UsageSnapshot, UsageStore, RECORD_SCHEMA_VERSION};
use rust_decimal::Decimal;
use serde::Deserialize;
use tempfile::TempDir;

const FIXTURES: &str = include_str!("fixtures/usage_accounting.json");

#[derive(Deserialize)]
struct FixtureFile {
    cases: Vec<FixtureCase>,
    assessment_cases: Vec<FixtureAssessment>,
}

#[derive(Deserialize)]
struct FixtureCase {
    name: String,
    run_id: String,
    cutoff_unix_ms: u64,
    observations: Vec<ObservationRecord>,
    intervals: Vec<ElapsedInterval>,
}

#[derive(Deserialize)]
struct FixtureAssessment {
    name: String,
    reconciliation_case: String,
    judgment: AssessmentRecord,
}

fn fixture() -> FixtureFile {
    serde_json::from_str(FIXTURES).expect("fixture file is valid JSON")
}

fn reconcile_case(name: &str) -> Reconciliation {
    let case = fixture()
        .cases
        .into_iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("fixture case `{name}` is missing"));
    reconcile(ReconcileInput {
        run_id: case.run_id,
        cutoff_unix_ms: case.cutoff_unix_ms,
        observations: case.observations,
        intervals: case.intervals,
    })
    .expect("fixture case reconciles")
}

/// Reconciles a fixture case and binds one fixture judgment to its snapshot digest.
fn judged(name: &str) -> (Reconciliation, UsageSnapshot, AssessmentRecord) {
    let case = fixture()
        .assessment_cases
        .into_iter()
        .find(|case| case.name == name)
        .unwrap_or_else(|| panic!("assessment case `{name}` is missing"));
    let reconciled = reconcile_case(&case.reconciliation_case);
    let snapshot = reconciled.snapshot(&case.judgment.snapshot_id, None);
    let mut judgment = case.judgment;
    judgment.snapshot_digest = snapshot_digest(&snapshot);
    (reconciled, snapshot, judgment)
}

fn snapshot_digest(snapshot: &UsageSnapshot) -> String {
    digest_bytes(&serde_json::to_vec(snapshot).expect("snapshot encodes"))
}

fn inline_judgment(
    reconciled: &Reconciliation,
    snapshot: &UsageSnapshot,
    assignments: Vec<Assignment>,
) -> AssessmentRecord {
    AssessmentRecord {
        schema_version: RECORD_SCHEMA_VERSION,
        assessment_id: format!("assess-{}", snapshot.snapshot_id),
        run_id: reconciled.run_id.clone(),
        snapshot_id: snapshot.snapshot_id.clone(),
        snapshot_digest: snapshot_digest(snapshot),
        useful_work_rule: "accepted when the run kept the change".to_owned(),
        author: "orchestrator".to_owned(),
        session_id: "review-session-3".to_owned(),
        model: "claude-haiku-5-5".to_owned(),
        recorded_at_unix_ms: 20_000_000,
        supersedes: None,
        assignments,
        residuals: Vec::new(),
    }
}

fn keep(contribution_id: &str, outcome: Outcome) -> Assignment {
    Assignment {
        contribution_id: contribution_id.to_owned(),
        outcome,
        reason: "kept by the review".to_owned(),
        evidence: vec!["review-log".to_owned()],
    }
}

fn output_basis(assessment: &Assessment) -> &BasisAssessment {
    assessment
        .bases
        .iter()
        .find(|basis| basis.basis == BasisKey::Tokens(UsageCategory::Output))
        .expect("output basis has a reconciled total")
}

#[test]
fn inclusive_residual_yields_sixty_accepted_and_one_forty_support() {
    let (reconciled, snapshot, judgment) = judged("inclusive_residual_is_proven");
    let assessment = assess(&judgment, &reconciled, &snapshot).expect("judgment is valid");
    let output = output_basis(&assessment);

    println!(
        "probe inclusive200/child60: total={:?} accepted={:?} support={:?} unused={:?} unresolved={:?}",
        output.total, output.accepted, output.support, output.unused, output.unresolved
    );
    assert_eq!(output.total, Amount::Tokens(200));
    assert_eq!(output.accepted, Amount::Tokens(60));
    assert_eq!(output.support, Amount::Tokens(140));
    assert_eq!(output.unused, Amount::Tokens(0));
    assert_eq!(output.unresolved, Amount::Tokens(0));
    assert_eq!(output.completeness, Completeness::Complete);
    assert!(assessment.unproven_residuals.is_empty());
}

#[test]
fn shared_child_residual_stays_unresolved_when_the_child_is_counted() {
    let (reconciled, snapshot, judgment) = judged("shared_child_residual_is_unproven");
    let assessment = assess(&judgment, &reconciled, &snapshot).expect("judgment is valid");
    let output = output_basis(&assessment);

    assert_eq!(output.total, Amount::Tokens(260));
    assert_eq!(output.accepted, Amount::Tokens(60));
    assert_eq!(output.support, Amount::Tokens(0));
    assert_eq!(output.unresolved, Amount::Tokens(200));
    assert_eq!(assessment.unproven_residuals.len(), 1);
    assert_eq!(
        assessment.unproven_residuals[0].parent_contribution_id,
        "obs-parent/output"
    );
}

#[test]
fn reassessment_keeps_both_versions_and_the_snapshot_digest() {
    let (reconciled, snapshot, first) = judged("inclusive_residual_is_proven");
    let directory = TempDir::new().expect("temporary directory");
    let store = UsageStore::open(directory.path()).expect("store opens");
    store.publish(&snapshot).expect("snapshot publishes");
    publish_judgment(&store, &first).expect("first judgment publishes");

    let mut second = first.clone();
    second.assessment_id = "assess-inclusive-2".to_owned();
    second.supersedes = Some(first.assessment_id.clone());
    second.recorded_at_unix_ms += 1;
    second.residuals[0].outcome = Outcome::Unused;
    second.residuals[0].reason = "the parent's own turns were not used".to_owned();
    publish_judgment(&store, &second).expect("superseding judgment publishes");

    let stored_first: AssessmentRecord = store
        .read("run-inclusive", "assess-inclusive-1")
        .expect("first judgment reads back");
    let stored_second: AssessmentRecord = store
        .read("run-inclusive", "assess-inclusive-2")
        .expect("second judgment reads back");
    assert_eq!(stored_first, first);
    assert_eq!(stored_second, second);

    let stored_snapshot: UsageSnapshot = store
        .read("run-inclusive", "snap-inclusive")
        .expect("snapshot reads back");
    assert_eq!(snapshot_digest(&stored_snapshot), first.snapshot_digest);

    let original = assess(&stored_first, &reconciled, &stored_snapshot).expect("original");
    let revised = assess(&stored_second, &reconciled, &stored_snapshot).expect("revised");
    assert_eq!(output_basis(&original).support, Amount::Tokens(140));
    assert_eq!(output_basis(&revised).support, Amount::Tokens(0));
    assert_eq!(output_basis(&revised).unused, Amount::Tokens(140));
    assert_eq!(output_basis(&revised).accepted, Amount::Tokens(60));
}

#[test]
fn supersession_across_snapshots_is_rejected() {
    let (reconciled, snapshot, first) = judged("inclusive_residual_is_proven");
    let directory = TempDir::new().expect("temporary directory");
    let store = UsageStore::open(directory.path()).expect("store opens");
    store.publish(&snapshot).expect("snapshot publishes");
    publish_judgment(&store, &first).expect("first judgment publishes");

    let other = reconciled.snapshot("snap-other", None);
    store.publish(&other).expect("other snapshot publishes");
    let mut stale = first.clone();
    stale.assessment_id = "assess-other-1".to_owned();
    stale.snapshot_id = "snap-other".to_owned();
    stale.snapshot_digest = snapshot_digest(&other);
    stale.supersedes = Some("assess-inclusive-1".to_owned());

    let error = publish_judgment(&store, &stale).expect_err("cross-snapshot supersession fails");
    assert!(matches!(
        error,
        AssessmentError::SupersessionMismatch { .. }
    ));
}

#[test]
fn snapshot_digest_mismatch_is_rejected() {
    let (reconciled, snapshot, mut judgment) = judged("inclusive_residual_is_proven");
    judgment.snapshot_digest = "0".repeat(64);

    assert_eq!(
        assess(&judgment, &reconciled, &snapshot),
        Err(AssessmentError::SnapshotDigestMismatch)
    );
}

#[test]
fn mismatched_snapshot_is_rejected() {
    let (reconciled, snapshot, judgment) = judged("inclusive_residual_is_proven");

    let mut missing_observation = snapshot.clone();
    missing_observation.observations.pop();
    assert!(matches!(
        assess(&judgment, &reconciled, &missing_observation),
        Err(AssessmentError::SnapshotMismatch(_))
    ));

    let mut other_version = snapshot.clone();
    other_version.reconciliation_version += 1;
    assert!(matches!(
        assess(&judgment, &reconciled, &other_version),
        Err(AssessmentError::SnapshotMismatch(_))
    ));
}

#[test]
fn factual_fields_are_rejected_at_input() {
    let (_, _, judgment) = judged("inclusive_residual_is_proven");
    let mut value = serde_json::to_value(&judgment).expect("judgment encodes");
    value["total"] = serde_json::json!(200);

    assert!(serde_json::from_value::<AssessmentRecord>(value).is_err());
}

#[test]
fn duplicate_unknown_uncounted_and_overlapping_rows_are_rejected() {
    let (reconciled, snapshot, base) = judged("inclusive_residual_is_proven");
    let reject = |judgment: AssessmentRecord| {
        assess(&judgment, &reconciled, &snapshot).expect_err("judgment is invalid")
    };

    let mut duplicate = base.clone();
    duplicate.assignments.push(base.assignments[0].clone());
    assert_eq!(
        reject(duplicate),
        AssessmentError::DuplicateAssignment("obs-child/output".to_owned())
    );

    let mut unknown = base.clone();
    unknown.assignments[0].contribution_id = "obs-missing/output".to_owned();
    assert_eq!(
        reject(unknown),
        AssessmentError::UnknownRow("obs-missing/output".to_owned())
    );

    let mut uncounted = base.clone();
    uncounted.residuals.clear();
    assert_eq!(
        reject(uncounted),
        AssessmentError::NotCounted("obs-child/output".to_owned())
    );

    let mut overlapping = base.clone();
    overlapping
        .assignments
        .push(keep("obs-parent/output", Outcome::Unused));
    assert_eq!(
        reject(overlapping),
        AssessmentError::Overlap("obs-parent/output".to_owned())
    );
}

#[test]
fn ratios_stay_on_one_basis_and_label_known_subtotals() {
    let reconciled = reconcile_case("retry_attempts_all_count");
    let snapshot = reconciled.snapshot("snap-retry", None);
    let judgment = inline_judgment(
        &reconciled,
        &snapshot,
        vec![
            keep("obs-attempt-1/output", Outcome::Accepted),
            keep("obs-attempt-2/output", Outcome::Accepted),
            keep("obs-attempt-2/cost", Outcome::Accepted),
        ],
    );
    let assessment = assess(&judgment, &reconciled, &snapshot).expect("judgment is valid");

    let output = output_basis(&assessment);
    assert_eq!(
        output.accepted_ratio,
        Some(Ratio {
            value: Decimal::ONE,
            completeness: Completeness::Complete,
        })
    );
    let cost = assessment
        .bases
        .iter()
        .find(|basis| matches!(basis.basis, BasisKey::Money { .. }))
        .expect("cost basis has a known total");
    assert_eq!(
        cost.accepted_ratio,
        Some(Ratio {
            value: Decimal::ONE,
            completeness: Completeness::KnownSubtotal,
        })
    );

    let unknown = reconcile_case("all_costs_unknown");
    let unknown_snapshot = unknown.snapshot("snap-unknown-cost", None);
    let empty = inline_judgment(&unknown, &unknown_snapshot, Vec::new());
    let unknown_assessment =
        assess(&empty, &unknown, &unknown_snapshot).expect("judgment is valid");
    assert!(unknown_assessment
        .bases
        .iter()
        .all(|basis| matches!(basis.basis, BasisKey::Tokens(_))));
    let unknown_output = output_basis(&unknown_assessment);
    assert_eq!(unknown_output.unresolved, Amount::Tokens(160));
    assert_eq!(
        unknown_output
            .accepted_ratio
            .as_ref()
            .map(|ratio| ratio.value),
        Some(Decimal::ZERO)
    );
}

#[test]
fn zero_denominator_makes_ratios_unavailable() {
    let observation: ObservationRecord = serde_json::from_str(
        r#"{
          "observation_id": "obs-zero",
          "observation": {
            "schema_version": 1,
            "identity": { "plan_id": "plan-a", "task_id": "zero-task", "attempt_id": "attempt-zero", "session_id": "session-zero" },
            "tokens": {
              "output": {
                "count": {
                  "value": { "state": "reported", "value": 0 },
                  "semantics": "delta",
                  "descendants": "own",
                  "window": { "start_unix_ms": 1000, "end_unix_ms": 2000 }
                },
                "relation": { "kind": "exclusive" }
              }
            }
          }
        }"#,
    )
    .expect("valid observation json");
    let reconciled = reconcile(ReconcileInput {
        run_id: "run-zero".to_owned(),
        cutoff_unix_ms: 10_000,
        observations: vec![observation],
        intervals: Vec::new(),
    })
    .expect("reconciles zero tokens");
    let snapshot = reconciled.snapshot("snap-zero", None);
    let judgment = inline_judgment(&reconciled, &snapshot, Vec::new());
    let assessment = assess(&judgment, &reconciled, &snapshot).expect("judgment is valid");
    let output = output_basis(&assessment);
    assert_eq!(output.total, Amount::Tokens(0));
    assert_eq!(output.accepted_ratio, None);
}
