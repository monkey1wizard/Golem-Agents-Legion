//! Behavior of quota capture and its collect rendering.
//!
//! The shared helper is compiled from its source file, so these tests run the production code
//! without a provider session, login, or installed executor.

#[path = "../src/commands/usage_capture.rs"]
#[allow(dead_code)]
mod usage_capture;

use dispatch::dispatch::DispatchQuota;
use dispatch::quota::{
    missing, AccountIdentityCheck, AccountRef, Acquisition, BillingMode, BucketId, Freshness,
    ProviderId, QueryMetadata, QuotaIdentity, QuotaObservation, QuotaReference, QuotaRuntime,
    QuotaSnapshot, QuotaWindow, RuntimeVersion, WindowKind, COLLECTOR, QUOTA_SCHEMA_VERSION,
};
use dispatch::usage::{Availability, ObservationSource, UsageCategory};
use pipeline::usage::binding::{RunIdentity, RunManifest, RUN_MANIFEST_VERSION};
use pipeline::usage::quota::{publish_observation, QuotaObservationRecord, QuotaRevision};
use pipeline::usage::store::record_digest;
use pipeline::usage::{UsageSnapshot, UsageStore};
use serde_json::json;
use tempfile::TempDir;
use usage_capture::AttemptUsage;

const RUN: &str = "run-quota";
const NOW: u64 = 1_792_000_000_000;
const OBSERVED_AT: u64 = 1_800_000_100_000;
const COLLECTED_AT: u64 = 1_800_000_200_000;

fn store() -> (TempDir, UsageStore) {
    let directory = TempDir::new().expect("temp dir");
    let store = UsageStore::open(directory.path()).expect("store opens");
    (directory, store)
}

fn identity_for(run_id: &str) -> RunIdentity {
    RunIdentity {
        version: RUN_MANIFEST_VERSION,
        run_id: run_id.to_owned(),
        canonical_worktree: "C:/work/golem".to_owned(),
        plan_scope: "feat-execution-usage-accounting".to_owned(),
        host_session: None,
        host_window: None,
    }
}

fn quota_identity(check: AccountIdentityCheck) -> QuotaIdentity {
    QuotaIdentity {
        runtime: QuotaRuntime::Codex,
        runtime_version: Some(RuntimeVersion::new("0.163.0").expect("version is valid")),
        provider: ProviderId::new("openai").expect("provider is valid"),
        billing: BillingMode::Subscription,
        account: (check == AccountIdentityCheck::Verified)
            .then(|| AccountRef::new("acct-shared").expect("account is valid")),
        identity_check: check,
        host: None,
        auth_profile: None,
        model_scope: None,
    }
}

/// One observation of the weekly window. Passive reads carry observed time and no control query.
fn observation(
    check: AccountIdentityCheck,
    acquisition: Acquisition,
    observed_at: Option<u64>,
    collected_at: u64,
    query: Option<QueryMetadata>,
) -> QuotaObservation {
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity: quota_identity(check),
        bucket: BucketId::new("codex-weekly").expect("bucket is valid"),
        window: Availability::Reported {
            value: QuotaWindow {
                kind: WindowKind::Weekly,
                duration_minutes: Some(10_080),
                resets_at_unix_ms: Some(1_800_600_000_000),
            },
        },
        utilization: missing("provider reports no utilization in this test"),
        used: missing("provider reports no absolute amount"),
        remaining: missing("provider reports no absolute amount"),
        limit: missing("provider reports no absolute amount"),
        observed_at_unix_ms: observed_at,
        collected_at_unix_ms: collected_at,
        source: ObservationSource {
            collector: Some(COLLECTOR.to_owned()),
            source_version: Some("0.163.0".to_owned()),
            cursor: None,
        },
        query,
        acquisition,
        freshness: if observed_at.is_some() {
            Freshness::Current
        } else {
            Freshness::Unknown
        },
    }
}

fn passive(check: AccountIdentityCheck, collected_at: u64) -> QuotaObservation {
    observation(
        check,
        Acquisition::Passive,
        Some(OBSERVED_AT),
        collected_at,
        None,
    )
}

fn queried(check: AccountIdentityCheck) -> QuotaObservation {
    observation(
        check,
        Acquisition::Queried,
        Some(COLLECTED_AT),
        COLLECTED_AT,
        Some(QueryMetadata {
            method: "account/rateLimits/read".to_owned(),
            duration_ms: 42,
        }),
    )
}

/// A launched return: a passive read from its output, then its own control query.
fn collected(check: AccountIdentityCheck) -> DispatchQuota {
    DispatchQuota::Collected {
        observations: vec![passive(check, COLLECTED_AT), queried(check)],
    }
}

fn reference(snapshot: &QuotaSnapshot) -> QuotaReference {
    match &snapshot.reference {
        Availability::Reported { value } => value.clone(),
        other => panic!("expected a reported reference, found {other:?}"),
    }
}

fn unavailable_reason(snapshot: &QuotaSnapshot) -> String {
    match &snapshot.reference {
        Availability::Unavailable { reason } => reason.clone(),
        other => panic!("expected an unavailable snapshot, found {other:?}"),
    }
}

fn revision_ids(store: &UsageStore) -> Vec<String> {
    usage_capture::record_ids::<QuotaRevision>(store, RUN).expect("revisions list")
}

fn output_observation(output: u64) -> dispatch::usage::UsageObservation {
    serde_json::from_value(json!({
        "tokens": {
            "output": {
                "count": {
                    "value": { "state": "reported", "value": output },
                    "semantics": "delta",
                    "descendants": "own",
                    "window": { "start_unix_ms": 1000, "end_unix_ms": 2000 }
                },
                "relation": { "kind": "exclusive" }
            }
        }
    }))
    .expect("output observation is valid")
}

fn usage_for(attempt_id: &str, output: u64) -> AttemptUsage {
    AttemptUsage {
        attempt_id: attempt_id.to_owned(),
        observations: vec![output_observation(output)],
        elapsed_ms: Some(5_000),
    }
}

/// The output total the factual reader rebuilds from the run's single usage snapshot.
fn rebuilt_output_total(store: &UsageStore) -> Option<u64> {
    let snapshots = usage_capture::record_ids::<UsageSnapshot>(store, RUN).expect("snapshots list");
    let [snapshot_id] = snapshots.as_slice() else {
        panic!("expected one usage snapshot, found {snapshots:?}");
    };
    store
        .rebuild_reconciliation(RUN, snapshot_id)
        .expect("evidence rebuilds the snapshot")
        .run
        .token(UsageCategory::Output)
        .total
}

#[test]
fn automatic_return_keeps_the_exact_reference_of_each_attempt() {
    let (_dir, store) = store();
    let identity = identity_for(RUN);

    let first = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-a"),
        &collected(AccountIdentityCheck::Verified),
    );
    let second = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-b"),
        &collected(AccountIdentityCheck::Verified),
    );

    let first_reference = reference(&first);
    assert_eq!(first_reference.snapshot_id.as_str(), "quota-r1");
    assert_eq!(reference(&second).snapshot_id.as_str(), "quota-r2");

    // The reference names the return's own control query, the last observation of the return.
    let stored: QuotaObservationRecord = store
        .read(RUN, "attempt-a-quota-0001")
        .expect("control query is stored");
    assert_eq!(stored.observation.acquisition, Acquisition::Queried);
    assert_eq!(
        first_reference.observation_digest.as_str(),
        record_digest(&stored).expect("digest computes")
    );
    assert_eq!(
        revision_ids(&store),
        ["quota-r1".to_owned(), "quota-r2".to_owned()]
    );
}

#[test]
fn shared_account_quota_is_referenced_per_attempt_and_never_changes_cost() {
    let (_dir, store) = store();
    usage_capture::capture_attempt(&store, RUN, &usage_for("attempt-a", 100), NOW)
        .expect("usage captures");
    let usage_before = rebuilt_output_total(&store);
    let snapshots_before =
        usage_capture::record_ids::<UsageSnapshot>(&store, RUN).expect("snapshots list");

    let identity = identity_for(RUN);
    let first = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-a"),
        &collected(AccountIdentityCheck::Verified),
    );
    let second = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-b"),
        &collected(AccountIdentityCheck::Verified),
    );

    // Both attempts read the same account, yet each keeps its own reference and no sum exists.
    assert_ne!(reference(&first), reference(&second));
    assert_eq!(usage_before, rebuilt_output_total(&store));
    assert_eq!(
        usage_capture::record_ids::<UsageSnapshot>(&store, RUN).expect("snapshots list"),
        snapshots_before
    );
    assert_eq!(usage_before, Some(100));
}

#[test]
fn retry_and_resume_keep_earlier_receipts_and_replays_add_nothing() {
    let (_dir, store) = store();
    let identity = identity_for(RUN);
    let check = AccountIdentityCheck::Verified;

    let first =
        usage_capture::capture_quota(&store, &identity, Some("attempt-a"), &collected(check));
    let first_revision: QuotaRevision = store.read(RUN, "quota-r1").expect("first revision");

    usage_capture::capture_quota(&store, &identity, Some("attempt-retry"), &collected(check));
    usage_capture::capture_quota(&store, &identity, Some("attempt-resume"), &collected(check));

    let stored_first: QuotaRevision = store.read(RUN, "quota-r1").expect("first revision");
    assert_eq!(
        stored_first, first_revision,
        "the earlier revision stays unchanged"
    );
    assert_eq!(revision_ids(&store).len(), 3);

    // A replayed capture of an attempt that already has a reference keeps that reference.
    let replay =
        usage_capture::capture_quota(&store, &identity, Some("attempt-a"), &collected(check));
    assert_eq!(reference(&replay), reference(&first));
    assert_eq!(revision_ids(&store).len(), 3, "a replay writes no revision");
}

#[test]
fn unknown_scope_and_unlaunched_attempts_report_a_reason_without_a_reference() {
    let (_dir, store) = store();
    let identity = identity_for(RUN);

    let host = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-host"),
        &DispatchQuota::UnknownScope {
            reason: "host child transcript carries no verified account identity".to_owned(),
        },
    );
    assert_eq!(
        unavailable_reason(&host),
        "host child transcript carries no verified account identity"
    );

    let unlaunched = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-unlaunched"),
        &DispatchQuota::NotAttempted,
    );
    assert!(unavailable_reason(&unlaunched).contains("not launched"));
    assert!(
        revision_ids(&store).is_empty(),
        "no quota record is written"
    );
}

#[test]
fn pipeline_return_stores_its_quota_for_the_attempt_and_prints_the_reference() {
    let (_dir, store) = store();
    let identity = usage_capture::pipeline_run_identity(
        RUN,
        "C:/work/golem",
        "feat-execution-usage-accounting",
    );
    assert_eq!(identity, identity_for(RUN));

    let snapshot = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-pipeline"),
        &collected(AccountIdentityCheck::Verified),
    );

    let revision: QuotaRevision = store.read(RUN, "quota-r1").expect("revision is stored");
    assert!(
        revision
            .entries
            .iter()
            .any(|entry| entry.attempt_id == "attempt-pipeline"),
        "the revision lists the pipeline attempt"
    );
    assert!(usage_capture::quota_text(&snapshot, None).starts_with("reported, revision quota-r1"));
}

#[test]
fn pipeline_return_without_launch_prints_the_reason_instead_of_a_reference() {
    let (_dir, store) = store();
    let identity = identity_for(RUN);

    let snapshot = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-pipeline"),
        &DispatchQuota::NotAttempted,
    );

    let text = usage_capture::quota_text(&snapshot, None);
    assert!(text.starts_with("unavailable. "), "found {text}");
    assert!(text.contains("not launched"), "found {text}");
    assert!(revision_ids(&store).is_empty());
}

#[test]
fn storage_conflict_reports_a_reason_and_leaves_no_reference() {
    let (_dir, store) = store();
    let identity = identity_for(RUN);

    // A different observation already holds the identifier this attempt would write.
    let manifest = RunManifest::standalone(identity.clone(), "attempt-a");
    publish_observation(
        &store,
        RUN,
        "attempt-a",
        &manifest,
        "attempt-a-quota-0000",
        passive(AccountIdentityCheck::Verified, COLLECTED_AT + 1),
    )
    .expect("the earlier observation publishes");

    let snapshot = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-a"),
        &collected(AccountIdentityCheck::Verified),
    );

    let reason = unavailable_reason(&snapshot);
    assert!(reason.starts_with("quota is not stored: "), "{reason}");
    assert!(
        revision_ids(&store).is_empty(),
        "no dangling reference is written"
    );
}

#[test]
fn account_states_keep_their_reasons_and_partial_scope() {
    let (_dir, store) = store();
    let identity = identity_for(RUN);

    let mismatch = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-mismatch"),
        &collected(AccountIdentityCheck::Mismatch),
    );
    assert_eq!(
        unavailable_reason(&mismatch),
        "account identity does not match the attempt, so quota is not attributed to it"
    );

    let failed = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-failed"),
        &DispatchQuota::Collected {
            observations: vec![observation(
                AccountIdentityCheck::Verified,
                Acquisition::Failed {
                    reason: "refresh failed with exit code 1".to_owned(),
                },
                None,
                COLLECTED_AT,
                None,
            )],
        },
    );
    assert_eq!(
        unavailable_reason(&failed),
        "refresh failed with exit code 1"
    );

    let unverified = usage_capture::capture_quota(
        &store,
        &identity,
        Some("attempt-unverified"),
        &collected(AccountIdentityCheck::Unverified),
    );
    match &unverified.reference {
        Availability::Partial { value, reason } => {
            // The attempt is the third one this run captured, so its revision is the third.
            assert_eq!(value.snapshot_id.as_str(), "quota-r3");
            assert!(reason.contains("unverified"), "{reason}");
        }
        other => panic!("expected a partial reference, found {other:?}"),
    }
}

#[test]
fn orchestrator_quota_reports_that_no_attempt_holds_it() {
    let (_dir, store) = store();
    let snapshot = usage_capture::capture_quota(
        &store,
        &identity_for(RUN),
        None,
        &collected(AccountIdentityCheck::Verified),
    );

    assert!(unavailable_reason(&snapshot).starts_with("orchestrator usage has no attempt"));
    assert!(revision_ids(&store).is_empty());
}

#[test]
fn quota_text_names_the_reference_its_version_and_its_control_trace() {
    let (_dir, store) = store();
    let identity = identity_for(RUN);
    let check = AccountIdentityCheck::Verified;
    let snapshot =
        usage_capture::capture_quota(&store, &identity, Some("attempt-a"), &collected(check));
    let text = usage_capture::quota_text(&snapshot, Some(&queried(check)));

    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "{text}");
    assert!(
        lines[0].starts_with("reported, revision quota-r1, observation "),
        "{text}"
    );
    assert!(lines[1].contains("runtime version 0.163.0"), "{text}");
    assert!(lines[1].contains("identity Verified"), "{text}");
    assert!(lines[1].contains("acquisition queried"), "{text}");
    assert!(lines[1].contains("freshness Current"), "{text}");
    assert!(
        lines[1].contains("control query account/rateLimits/read in 42 ms"),
        "{text}"
    );
}
