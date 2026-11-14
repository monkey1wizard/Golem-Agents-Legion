use dispatch::usage::UsageCategory;
use pipeline::usage::reconcile::{
    reconcile, ElapsedInterval, ObservationRecord, ReconcileInput, Reconciliation,
};
use pipeline::usage::{
    Publication, RecordKind, SnapshotEvidence, StoreError, UsageSnapshot, UsageStore,
    RECORD_SCHEMA_VERSION,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Barrier},
    thread,
};

const RUN: &str = "run-evidence";
const SNAPSHOT: &str = "snap-1";

fn observation(observation_id: &str, attempt_id: &str, output: u64) -> ObservationRecord {
    serde_json::from_value(serde_json::json!({
        "observation_id": observation_id,
        "observation": {
            "schema_version": 1,
            "identity": {
                "plan_id": "plan-a",
                "task_id": "alpha-task",
                "attempt_id": attempt_id,
                "executor": "codex",
                "session_id": format!("session-{attempt_id}"),
            },
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
            },
            "cost": {
                "value": { "state": "unavailable", "reason": "provider reported no cost" }
            }
        }
    }))
    .expect("inline observation is valid")
}

fn interval() -> ElapsedInterval {
    serde_json::from_value(serde_json::json!({
        "attempt_id": "attempt-1",
        "start_unix_ms": 1000,
        "end_unix_ms": 5000,
        "clock": { "kind": "controller" },
    }))
    .expect("inline interval is valid")
}

fn evidence_with(
    observations: Vec<ObservationRecord>,
    intervals: Vec<ElapsedInterval>,
) -> SnapshotEvidence {
    SnapshotEvidence {
        schema_version: RECORD_SCHEMA_VERSION,
        snapshot_id: SNAPSHOT.to_owned(),
        run_id: RUN.to_owned(),
        cutoff_unix_ms: 10_000_000,
        observations,
        intervals,
    }
}

/// Two observations and one interval, with their reconciliation and snapshot.
fn fixture() -> (Reconciliation, UsageSnapshot, SnapshotEvidence) {
    let evidence = evidence_with(
        vec![
            observation("obs-1", "attempt-1", 100),
            observation("obs-2", "attempt-2", 60),
        ],
        vec![interval()],
    );
    let reconciled = reconcile(ReconcileInput {
        run_id: RUN.to_owned(),
        cutoff_unix_ms: evidence.cutoff_unix_ms,
        observations: evidence.observations.clone(),
        intervals: evidence.intervals.clone(),
    })
    .expect("fixture reconciles");
    let snapshot = reconciled.snapshot(SNAPSHOT, None);
    (reconciled, snapshot, evidence)
}

fn open(common: &Path) -> UsageStore {
    UsageStore::open(common).expect("store opens under an existing common dir")
}

fn kind_dir(common: &Path, kind: RecordKind) -> PathBuf {
    common
        .join("gal")
        .join("usage")
        .join("v1")
        .join(RUN)
        .join(kind.dir_name())
}

fn evidence_record_path(common: &Path) -> PathBuf {
    kind_dir(common, RecordKind::Evidence).join(format!("{SNAPSHOT}.record.json"))
}

fn evidence_manifest_path(common: &Path) -> PathBuf {
    kind_dir(common, RecordKind::Evidence).join(format!("{SNAPSHOT}.manifest.json"))
}

fn remove_evidence(common: &Path) {
    fs::remove_file(evidence_record_path(common)).expect("evidence record exists");
    fs::remove_file(evidence_manifest_path(common)).expect("evidence manifest exists");
}

#[test]
fn rebuild_reproduces_the_original_reconciliation() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let (original, snapshot, evidence) = fixture();

    store.publish(&evidence).expect("evidence publishes");
    store.publish(&snapshot).expect("snapshot publishes");

    let rebuilt = store
        .rebuild_reconciliation(RUN, SNAPSHOT)
        .expect("rebuild succeeds from evidence");
    assert_eq!(rebuilt, original);
    assert_eq!(rebuilt.run.token(UsageCategory::Output).total, Some(160));
    assert_eq!(rebuilt.manifest, snapshot.observations);
}

#[test]
fn tampered_evidence_fails_the_rebuild_and_names_the_snapshot() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let (_, snapshot, evidence) = fixture();
    store.publish(&evidence).expect("evidence publishes");
    store.publish(&snapshot).expect("snapshot publishes");

    let original = fs::read(evidence_record_path(common.path())).expect("record is readable");
    let tampered = String::from_utf8(original)
        .expect("record is UTF-8 JSON")
        .replace("\"value\":60", "\"value\":61");
    fs::write(evidence_record_path(common.path()), tampered).expect("tamper record");

    let error = store
        .rebuild_reconciliation(RUN, SNAPSHOT)
        .expect_err("tampered evidence must not rebuild");
    assert!(matches!(&error, StoreError::DigestMismatch(id) if id == SNAPSHOT));
    assert!(error.to_string().contains(SNAPSHOT));
}

#[test]
fn deleted_evidence_fails_the_rebuild_and_names_the_snapshot() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let (_, snapshot, evidence) = fixture();
    store.publish(&evidence).expect("evidence publishes");
    store.publish(&snapshot).expect("snapshot publishes");

    remove_evidence(common.path());

    let error = store
        .rebuild_reconciliation(RUN, SNAPSHOT)
        .expect_err("missing evidence must not rebuild");
    assert!(matches!(&error, StoreError::EvidenceMissing(id) if id == SNAPSHOT));
    assert!(error.to_string().contains(SNAPSHOT));
}

#[test]
fn evidence_that_differs_from_a_listed_digest_names_the_observation() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let (_, snapshot, evidence) = fixture();
    store.publish(&evidence).expect("evidence publishes");
    store.publish(&snapshot).expect("snapshot publishes");

    remove_evidence(common.path());
    let altered = evidence_with(
        vec![
            observation("obs-1", "attempt-1", 100),
            observation("obs-2", "attempt-2", 61),
        ],
        vec![interval()],
    );
    store.publish(&altered).expect("altered evidence publishes");

    let error = store
        .rebuild_reconciliation(RUN, SNAPSHOT)
        .expect_err("altered observation must not rebuild");
    assert!(matches!(
        &error,
        StoreError::EvidenceMismatch { snapshot_id, observation_id }
            if snapshot_id == SNAPSHOT && observation_id == "obs-2"
    ));
    assert!(error.to_string().contains("obs-2"));
}

#[test]
fn evidence_missing_a_listed_observation_fails_the_rebuild() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let (_, snapshot, evidence) = fixture();
    store.publish(&evidence).expect("evidence publishes");
    store.publish(&snapshot).expect("snapshot publishes");

    remove_evidence(common.path());
    let partial = evidence_with(
        vec![observation("obs-1", "attempt-1", 100)],
        vec![interval()],
    );
    store.publish(&partial).expect("partial evidence publishes");

    let error = store
        .rebuild_reconciliation(RUN, SNAPSHOT)
        .expect_err("a partial total must not rebuild");
    assert!(matches!(
        &error,
        StoreError::EvidenceMismatch { observation_id, .. } if observation_id == "obs-2"
    ));
}

#[test]
fn identical_evidence_replays_and_conflicting_evidence_is_rejected() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let (_, _, evidence) = fixture();

    assert_eq!(
        store.publish(&evidence).expect("first publish"),
        Publication::Published
    );
    assert_eq!(
        store.publish(&evidence).expect("replay"),
        Publication::Replayed
    );

    let conflicting = evidence_with(evidence.observations.clone(), Vec::new());
    assert!(matches!(
        store.publish(&conflicting),
        Err(StoreError::Conflict(id)) if id == SNAPSHOT
    ));
}

#[test]
fn concurrent_identical_publishes_leave_one_copy() {
    let common = tempfile::tempdir().expect("tempdir");
    let store = open(common.path());
    let (_, _, evidence) = fixture();

    let contenders = 4;
    let barrier = Arc::new(Barrier::new(contenders));
    let handles: Vec<_> = (0..contenders)
        .map(|_| {
            let store = store.clone();
            let evidence = evidence.clone();
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                store.publish(&evidence)
            })
        })
        .collect();
    let outcomes: Vec<Publication> = handles
        .into_iter()
        .map(|handle| {
            handle
                .join()
                .expect("publisher thread")
                .expect("publish succeeds")
        })
        .collect();

    let published = outcomes
        .iter()
        .filter(|o| **o == Publication::Published)
        .count();
    assert_eq!(published, 1);
    let page = store
        .list_page::<SnapshotEvidence>(RUN, None, 10)
        .expect("evidence pages");
    assert_eq!(page.entries.len(), 1);
}
