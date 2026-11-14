use dispatch::quota::{
    gap_observation, missing, AccountIdentityCheck, AccountRef, Acquisition, BillingMode, BucketId,
    Freshness, ProviderId, QuotaIdentity, QuotaObservation, QuotaReference, QuotaRuntime,
    QuotaWindow, RuntimeVersion, Utilization, UtilizationDirection, UtilizationScale, WindowKind,
    COLLECTOR, QUOTA_SCHEMA_VERSION,
};
use dispatch::usage::{Availability, ObservationSource};
use pipeline::usage::binding::{BindingError, RunIdentity, RunManifest, RUN_MANIFEST_VERSION};
use pipeline::usage::quota::{
    publish_observation, publish_revision, return_reference, QuotaEntry, QuotaObservationRecord,
    QuotaRecordError, QuotaRevision,
};
use pipeline::usage::store::{record_digest, Publication, StoreError, UsageStore};
use pipeline::usage::{ObservationDigest, RecordKind, UsageSnapshot, RECORD_SCHEMA_VERSION};
use rust_decimal::Decimal;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
    thread,
};

const RUN: &str = "run-1";
const OBSERVED_AT: u64 = 1_800_000_100_000;
const COLLECTED_AT: u64 = 1_800_000_200_000;

fn open(common: &Path) -> UsageStore {
    UsageStore::open(common).expect("store opens under an existing common dir")
}

fn manifest(run_id: &str, attempt_id: &str) -> RunManifest {
    RunManifest::standalone(
        RunIdentity {
            version: RUN_MANIFEST_VERSION,
            run_id: run_id.to_owned(),
            canonical_worktree: "C:/work/golem".to_owned(),
            plan_scope: "feat-execution-usage-accounting".to_owned(),
            host_session: None,
            host_window: None,
        },
        attempt_id,
    )
}

fn identity(check: AccountIdentityCheck) -> QuotaIdentity {
    QuotaIdentity {
        runtime: QuotaRuntime::Codex,
        runtime_version: Some(RuntimeVersion::new("0.163.0").unwrap()),
        provider: ProviderId::new("openai").unwrap(),
        billing: BillingMode::Subscription,
        account: (check == AccountIdentityCheck::Verified)
            .then(|| AccountRef::new("acct-1").unwrap()),
        identity_check: check,
        host: None,
        auth_profile: None,
        model_scope: None,
    }
}

fn sample(
    check: AccountIdentityCheck,
    acquisition: Acquisition,
    observed_at: Option<u64>,
    collected_at: u64,
) -> QuotaObservation {
    QuotaObservation {
        schema_version: QUOTA_SCHEMA_VERSION,
        identity: identity(check),
        bucket: BucketId::new("codex-weekly").unwrap(),
        window: Availability::Reported {
            value: QuotaWindow {
                kind: WindowKind::Weekly,
                duration_minutes: Some(10_080),
                resets_at_unix_ms: Some(1_800_600_000_000),
            },
        },
        utilization: Availability::Reported {
            value: Utilization::new(
                Decimal::new(42, 2),
                UtilizationScale::Fraction,
                UtilizationDirection::Used,
            )
            .unwrap(),
        },
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
        query: None,
        acquisition,
        freshness: if observed_at.is_some() {
            Freshness::Current
        } else {
            Freshness::Unknown
        },
    }
}

fn publish(
    store: &UsageStore,
    attempt_id: &str,
    observation_id: &str,
    observation: QuotaObservation,
) -> QuotaEntry {
    let (_, entry) = publish_observation(
        store,
        RUN,
        attempt_id,
        &manifest(RUN, attempt_id),
        observation_id,
        observation,
    )
    .expect("observation publishes");
    entry
}

fn record_file(common: &Path, run_id: &str, kind: RecordKind, record_id: &str) -> PathBuf {
    common
        .join("gal")
        .join("usage")
        .join("v1")
        .join(run_id)
        .join(kind.dir_name())
        .join(format!("{record_id}.record.json"))
}

#[test]
fn published_observation_keeps_original_times_on_replay() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let original = sample(
        AccountIdentityCheck::Verified,
        Acquisition::Passive,
        Some(OBSERVED_AT),
        COLLECTED_AT,
    );

    let (first, entry) = publish_observation(
        &store,
        RUN,
        "attempt-a",
        &manifest(RUN, "attempt-a"),
        "obs-a",
        original.clone(),
    )
    .unwrap();
    assert_eq!(first, Publication::Published);

    let (replay, replayed_entry) = publish_observation(
        &store,
        RUN,
        "attempt-a",
        &manifest(RUN, "attempt-a"),
        "obs-a",
        original.clone(),
    )
    .unwrap();
    assert_eq!(replay, Publication::Replayed);
    assert_eq!(replayed_entry, entry);

    let stored: QuotaObservationRecord = store.read(RUN, "obs-a").unwrap();
    assert_eq!(stored.observation, original);
    assert_eq!(stored.observation.observed_at_unix_ms, Some(OBSERVED_AT));
    assert_eq!(stored.observation.collected_at_unix_ms, COLLECTED_AT);
}

#[test]
fn concurrent_identical_publishers_produce_one_observation() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let barrier = Arc::new(Barrier::new(8));

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let store = store.clone();
            let barrier = Arc::clone(&barrier);
            let observation = sample(
                AccountIdentityCheck::Verified,
                Acquisition::Queried,
                Some(OBSERVED_AT),
                COLLECTED_AT,
            );
            let manifest = manifest(RUN, "attempt-a");
            thread::spawn(move || {
                barrier.wait();
                publish_observation(&store, RUN, "attempt-a", &manifest, "obs-a", observation)
                    .map(|(publication, _)| publication)
            })
        })
        .collect();
    let outcomes: Vec<Publication> = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .expect("thread completes")
                .expect("publish succeeds")
        })
        .collect();

    let published = outcomes
        .iter()
        .filter(|outcome| **outcome == Publication::Published)
        .count();
    let replayed = outcomes
        .iter()
        .filter(|outcome| **outcome == Publication::Replayed)
        .count();
    assert_eq!((published, replayed), (1, 7));

    let page = store
        .list_page::<QuotaObservationRecord>(RUN, None, 10)
        .unwrap();
    assert_eq!(page.entries.len(), 1);
}

#[test]
fn conflicting_observation_is_rejected_and_original_is_kept() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let original = sample(
        AccountIdentityCheck::Verified,
        Acquisition::Passive,
        Some(OBSERVED_AT),
        COLLECTED_AT,
    );
    publish(&store, "attempt-a", "obs-a", original.clone());

    let changed = sample(
        AccountIdentityCheck::Verified,
        Acquisition::Passive,
        Some(OBSERVED_AT),
        COLLECTED_AT + 60_000,
    );
    let error = publish_observation(
        &store,
        RUN,
        "attempt-a",
        &manifest(RUN, "attempt-a"),
        "obs-a",
        changed,
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QuotaRecordError::Store(StoreError::Conflict(ref id)) if id == "obs-a"
    ));

    let stored: QuotaObservationRecord = store.read(RUN, "obs-a").unwrap();
    assert_eq!(stored.observation, original);
}

fn reference_for(
    store: &UsageStore,
    attempt_id: &str,
    observation: QuotaObservation,
) -> Availability<QuotaReference> {
    let entry = publish(store, attempt_id, "obs-ref", observation);
    let (_, revision) = publish_revision(store, RUN, None, vec![entry]).unwrap();
    return_reference(store, &revision, attempt_id)
        .unwrap()
        .reference
}

#[test]
fn verified_passive_reading_returns_reported_reference() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let observation = sample(
        AccountIdentityCheck::Verified,
        Acquisition::Passive,
        Some(OBSERVED_AT),
        COLLECTED_AT,
    );

    match reference_for(&store, "attempt-a", observation) {
        Availability::Reported { value } => {
            assert_eq!(value.snapshot_id.as_str(), "quota-r1");
        }
        other => panic!("expected a reported reference, found {other:?}"),
    }
}

#[test]
fn unverified_account_keeps_partial_reference_with_reason() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let observation = sample(
        AccountIdentityCheck::Unverified,
        Acquisition::Queried,
        Some(OBSERVED_AT),
        COLLECTED_AT,
    );

    match reference_for(&store, "attempt-a", observation) {
        Availability::Partial { reason, .. } => assert!(reason.contains("unverified")),
        other => panic!("expected a partial reference, found {other:?}"),
    }
}

#[test]
fn mismatched_account_and_failed_query_return_unavailable() {
    let mismatch_common = tempfile::tempdir().expect("tempdir");
    let mismatch_store = open(mismatch_common.path());
    let mismatch = sample(
        AccountIdentityCheck::Mismatch,
        Acquisition::Queried,
        Some(OBSERVED_AT),
        COLLECTED_AT,
    );
    match reference_for(&mismatch_store, "attempt-a", mismatch) {
        Availability::Unavailable { reason } => assert!(reason.contains("does not match")),
        other => panic!("expected an unavailable reference, found {other:?}"),
    }

    let gap_common = tempfile::tempdir().expect("tempdir");
    let gap_store = open(gap_common.path());
    let gap = gap_observation(
        QuotaIdentity::unverified(
            QuotaRuntime::Codex,
            ProviderId::new("openai").unwrap(),
            BillingMode::Subscription,
        ),
        Acquisition::Failed {
            reason: "probe deadline expired".to_owned(),
        },
        COLLECTED_AT,
        "0.163.0",
    );
    match reference_for(&gap_store, "attempt-b", gap) {
        Availability::Unavailable { reason } => assert_eq!(reason, "probe deadline expired"),
        other => panic!("expected an unavailable reference, found {other:?}"),
    }
}

#[test]
fn refresh_appends_revision_and_keeps_earlier_bytes_and_references() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let entry_a = publish(
        &store,
        "attempt-a",
        "obs-a",
        sample(
            AccountIdentityCheck::Verified,
            Acquisition::Passive,
            Some(OBSERVED_AT),
            COLLECTED_AT,
        ),
    );
    let (_, rev1) = publish_revision(&store, RUN, None, vec![entry_a]).unwrap();
    let rev1_path = record_file(common.path(), RUN, RecordKind::QuotaRevision, "quota-r1");
    let rev1_bytes = fs::read(&rev1_path).unwrap();
    let reference_a = return_reference(&store, &rev1, "attempt-a").unwrap();

    let entry_b = publish(
        &store,
        "attempt-b",
        "obs-b",
        sample(
            AccountIdentityCheck::Verified,
            Acquisition::Queried,
            Some(OBSERVED_AT + 3_600_000),
            COLLECTED_AT + 3_600_000,
        ),
    );
    let (_, rev2) = publish_revision(&store, RUN, Some(&rev1), vec![entry_b]).unwrap();

    assert_eq!(rev2.revision, 2);
    assert_eq!(rev2.previous_revision_id.as_deref(), Some("quota-r1"));
    assert_eq!(rev2.entries.len(), 2);
    assert_eq!(rev2.entries[0], rev1.entries[0]);
    assert_eq!(fs::read(&rev1_path).unwrap(), rev1_bytes);

    let stored_rev1: QuotaRevision = store.read(RUN, "quota-r1").unwrap();
    assert_eq!(stored_rev1, rev1);
    assert_eq!(
        return_reference(&store, &stored_rev1, "attempt-a").unwrap(),
        reference_a
    );
}

#[test]
fn stale_previous_revision_and_duplicate_attempts_are_rejected() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let entry_a = publish(
        &store,
        "attempt-a",
        "obs-a",
        sample(
            AccountIdentityCheck::Verified,
            Acquisition::Passive,
            Some(OBSERVED_AT),
            COLLECTED_AT,
        ),
    );
    let entry_a_again = publish(
        &store,
        "attempt-a",
        "obs-a-2",
        sample(
            AccountIdentityCheck::Verified,
            Acquisition::Queried,
            Some(OBSERVED_AT + 1_000),
            COLLECTED_AT + 1_000,
        ),
    );
    let (_, rev1) = publish_revision(&store, RUN, None, vec![entry_a.clone()]).unwrap();

    let mut stale = rev1.clone();
    stale.entries.clear();
    let error = publish_revision(&store, RUN, Some(&stale), vec![]).unwrap_err();
    assert!(matches!(
        error,
        QuotaRecordError::StaleRevision(ref id) if id == "quota-r1"
    ));

    let error = publish_revision(&store, RUN, None, vec![entry_a, entry_a_again]).unwrap_err();
    assert!(matches!(
        error,
        QuotaRecordError::DuplicateAttempt(ref id) if id == "attempt-a"
    ));
}

#[test]
fn tampered_entry_digest_is_rejected() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let mut entry = publish(
        &store,
        "attempt-a",
        "obs-a",
        sample(
            AccountIdentityCheck::Verified,
            Acquisition::Passive,
            Some(OBSERVED_AT),
            COLLECTED_AT,
        ),
    );
    entry.observation_digest = "0".repeat(64);

    let error = publish_revision(&store, RUN, None, vec![entry]).unwrap_err();
    assert!(matches!(
        error,
        QuotaRecordError::EntryMismatch(ref id) if id == "obs-a"
    ));
}

#[test]
fn attempt_from_another_run_is_rejected() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());

    let error = publish_observation(
        &store,
        "run-2",
        "attempt-a",
        &manifest(RUN, "attempt-a"),
        "obs-a",
        sample(
            AccountIdentityCheck::Verified,
            Acquisition::Passive,
            Some(OBSERVED_AT),
            COLLECTED_AT,
        ),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        QuotaRecordError::Binding(BindingError::AttemptNotBound { ref run_id, .. })
            if run_id == "run-2"
    ));
    assert!(store
        .list_page::<QuotaObservationRecord>("run-2", None, 10)
        .unwrap()
        .entries
        .is_empty());
}

#[test]
fn quota_records_leave_usage_snapshots_and_their_hashes_unchanged() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let usage = UsageSnapshot {
        schema_version: RECORD_SCHEMA_VERSION,
        snapshot_id: "snap-1".to_owned(),
        run_id: RUN.to_owned(),
        reconciliation_version: 1,
        rate_set_digest: None,
        observations: vec![ObservationDigest {
            observation_id: "usage-1".to_owned(),
            digest: "b".repeat(64),
        }],
    };
    store.publish(&usage).unwrap();
    let usage_digest = record_digest(&usage).unwrap();
    let usage_page = store.list_page::<UsageSnapshot>(RUN, None, 10).unwrap();

    let entry = publish(
        &store,
        "attempt-a",
        "obs-a",
        sample(
            AccountIdentityCheck::Verified,
            Acquisition::Passive,
            Some(OBSERVED_AT),
            COLLECTED_AT,
        ),
    );
    publish_revision(&store, RUN, None, vec![entry]).unwrap();

    assert_eq!(
        store.list_page::<UsageSnapshot>(RUN, None, 10).unwrap(),
        usage_page
    );
    let reread: UsageSnapshot = store.read(RUN, "snap-1").unwrap();
    assert_eq!(record_digest(&reread).unwrap(), usage_digest);
}
