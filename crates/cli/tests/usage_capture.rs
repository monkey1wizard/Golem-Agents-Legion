//! Behavior of the shared accounting capture helper.
//!
//! The helper is compiled from its source file, so these tests run the production code
//! without a provider session, login, or installed executor.

#[path = "../src/commands/usage_capture.rs"]
#[allow(dead_code)]
mod usage_capture;

use dispatch::usage::{UsageCategory, UsageObservation};
use pipeline::usage::reconcile::{Allocation, TaskKey};
use pipeline::usage::{SnapshotEvidence, StoreError, UsageSnapshot, UsageStore};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use usage_capture::{AttemptUsage, CaptureError};

const NOW: u64 = 1_792_000_000_000;
const RUN: &str = "run-capture";
const PLAN: &str = "plan-capture";

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

fn attempt(attempt_id: &str, output: u64) -> AttemptUsage {
    AttemptUsage {
        attempt_id: attempt_id.to_owned(),
        observations: vec![output_observation(output)],
        elapsed_ms: Some(5_000),
    }
}

fn store() -> (TempDir, UsageStore) {
    let directory = TempDir::new().expect("temp dir");
    let store = UsageStore::open(directory.path()).expect("store opens");
    (directory, store)
}

fn snapshot_ids(store: &UsageStore) -> Vec<String> {
    usage_capture::record_ids::<UsageSnapshot>(store, RUN).expect("snapshots list")
}

/// The snapshot the factual reader would show, rebuilt from its stored evidence.
fn rebuilt_output_total(store: &UsageStore, snapshot_id: &str) -> Option<u64> {
    let reconciled = store
        .rebuild_reconciliation(RUN, snapshot_id)
        .expect("evidence rebuilds the snapshot");
    reconciled.run.token(UsageCategory::Output).total
}

#[test]
fn failed_attempt_and_resumed_success_total_160_after_recovery() {
    let (_directory, store) = store();
    let failed = usage_capture::capture_attempt(&store, RUN, &attempt("attempt-failed", 100), NOW)
        .expect("failed attempt captures");
    let resumed =
        usage_capture::capture_attempt(&store, RUN, &attempt("attempt-resumed", 60), NOW + 60_000)
            .expect("resumed attempt captures");

    assert_eq!(failed.new_observations, 1);
    assert_eq!(resumed.new_observations, 1);
    assert_eq!(resumed.run_observations, 2);
    let latest = resumed
        .snapshot_id
        .expect("resumed attempt publishes a snapshot");
    assert!(snapshot_ids(&store).contains(&latest));
    assert_eq!(rebuilt_output_total(&store, &latest), Some(160));

    let stored: UsageSnapshot = store.read(RUN, &latest).expect("snapshot reads");
    let rebuilt = store
        .rebuild_reconciliation(RUN, &latest)
        .expect("rebuild succeeds")
        .snapshot(&latest, None);
    assert_eq!(
        rebuilt, stored,
        "the rebuild reproduces the stored snapshot"
    );
}

#[test]
fn bound_cutoff_stays_explicit_through_recovery_and_replay() {
    let (_directory, store) = store();
    let first = usage_capture::capture_attempt(&store, RUN, &attempt("attempt-failed", 100), NOW)
        .expect("failed attempt captures");
    let recovered =
        usage_capture::capture_attempt(&store, RUN, &attempt("attempt-resumed", 60), NOW + 60_000)
            .expect("resumed attempt captures");
    assert_eq!(first.cutoff_unix_ms, NOW);
    assert_eq!(recovered.cutoff_unix_ms, NOW + 60_000);

    let latest = recovered.snapshot_id.expect("snapshot published");
    let evidence: SnapshotEvidence = store.read(RUN, &latest).expect("evidence reads");
    assert_eq!(evidence.cutoff_unix_ms, NOW + 60_000);

    let replay =
        usage_capture::capture_attempt(&store, RUN, &attempt("attempt-resumed", 60), NOW + 120_000)
            .expect("replayed attempt captures");
    assert_eq!(replay.new_observations, 0);
    assert_eq!(replay.snapshot_id, None, "a replay publishes nothing");
    assert_eq!(
        replay.cutoff_unix_ms,
        NOW + 60_000,
        "the cutoff does not move"
    );
    assert_eq!(snapshot_ids(&store).len(), 2);
}

#[test]
fn conflicting_observation_content_is_refused_and_publishes_nothing() {
    let (_directory, store) = store();
    usage_capture::capture_attempt(&store, RUN, &attempt("attempt-a", 100), NOW)
        .expect("first capture publishes");
    let before = snapshot_ids(&store);

    let error = usage_capture::capture_attempt(&store, RUN, &attempt("attempt-a", 60), NOW + 1)
        .expect_err("the same observation id with other content is a conflict");
    assert!(matches!(error, CaptureError::Conflict(ref id) if id == "attempt-a-0000"));
    assert_eq!(snapshot_ids(&store), before);
}

#[test]
fn capture_log_reads_the_usage_block_and_skips_logs_without_it() {
    let (directory, store) = store();
    drop(store);
    let common = directory.path();
    let encoded = serde_json::to_string(&vec![output_observation(100)]).expect("encodes");
    let log = directory.path().join("attempt.log");
    fs::write(
        &log,
        format!(
            "usage_extension:  v1\n\
             usage_source:     fixture\n\
             usage_collection: complete\n\
             usage_evidence:   none\n\
             usage_elapsed_ms: 5000\n\
             usage_observations: {encoded}\n"
        ),
    )
    .expect("log writes");

    let report = usage_capture::capture_log(common, RUN, PLAN, "attempt-log", &log)
        .expect("usage block captures")
        .expect("usage block is present");
    assert_eq!(report.new_observations, 1);
    assert!(report.snapshot_id.is_some());

    let plain = directory.path().join("plain.log");
    fs::write(&plain, "usage_collection: not-attempted\n").expect("log writes");
    let skipped = usage_capture::capture_log(common, RUN, PLAN, "plain", &plain).expect("no error");
    assert_eq!(
        skipped, None,
        "a log without a usage block captures nothing"
    );
}

/// An observation whose identity holds exactly the given fields, and whose models are null.
fn observation_with_identity(identity: serde_json::Value) -> UsageObservation {
    let mut observation = output_observation(100);
    observation.identity = serde_json::from_value(identity).expect("identity is valid");
    observation
}

/// An executor log with the given header lines, then one usage block of the observations.
fn write_log(path: &Path, header: &str, observations: &[UsageObservation]) -> PathBuf {
    let encoded = serde_json::to_string(observations).expect("observations encode");
    fs::write(
        path,
        format!(
            "GAL-DISPATCH-LOG v1\n{header}usage_extension:  v1\n\
             usage_observations: {encoded}\n---STDOUT---\n"
        ),
    )
    .expect("log writes");
    path.to_path_buf()
}

#[test]
fn capture_log_fills_null_identity_from_the_header_and_allocates_the_task() {
    let directory = TempDir::new().expect("temp dir");
    let log = write_log(
        &directory.path().join("attempt.log"),
        "task_id:         task-a\n\
         phase:           implement\n\
         session_id:      header-session\n\
         actual_model:    model-x\n",
        &[observation_with_identity(json!({}))],
    );

    usage_capture::capture_log(directory.path(), RUN, PLAN, "attempt-log", &log)
        .expect("usage block captures")
        .expect("usage block is present");

    let store = UsageStore::open(directory.path()).expect("store reopens");
    let latest = usage_capture::record_ids::<UsageSnapshot>(&store, RUN)
        .expect("snapshots list")
        .into_iter()
        .last()
        .expect("snapshot published");
    let evidence: SnapshotEvidence = store.read(RUN, &latest).expect("evidence reads");
    let identity = &evidence.observations[0].observation.identity;
    assert_eq!(identity.run_id.as_deref(), Some(RUN));
    assert_eq!(identity.plan_id.as_deref(), Some(PLAN));
    assert_eq!(identity.attempt_id.as_deref(), Some("attempt-log"));
    assert_eq!(identity.task_id.as_deref(), Some("task-a"));
    assert_eq!(identity.phase.as_deref(), Some("implement"));
    assert_eq!(identity.session_id.as_deref(), Some("header-session"));
    let models = &evidence.observations[0].observation.models;
    assert_eq!(models.configured.as_deref(), Some("model-x"));
    assert_eq!(
        models.observed, None,
        "the header never sets the observed model"
    );

    let reconciled = store
        .rebuild_reconciliation(RUN, &latest)
        .expect("evidence rebuilds the snapshot");
    assert!(!reconciled.contributions.is_empty());
    let task = Allocation::Task(TaskKey {
        plan_id: Some(PLAN.to_owned()),
        task_id: "task-a".to_owned(),
    });
    assert!(
        reconciled
            .contributions
            .iter()
            .all(|row| row.allocation == task),
        "every row is allocated to the plan and task, with no no_task row"
    );
}

#[test]
fn provider_supplied_identity_survives_and_none_header_stays_null() {
    let directory = TempDir::new().expect("temp dir");
    let log = write_log(
        &directory.path().join("attempt.log"),
        "task_id:         task-a\n\
         session_id:      none\n",
        &[observation_with_identity(json!({
            "session_id": "provider-session",
            "task_id": "provider-task"
        }))],
    );

    usage_capture::capture_log(directory.path(), RUN, PLAN, "attempt-log", &log)
        .expect("usage block captures")
        .expect("usage block is present");

    let store = UsageStore::open(directory.path()).expect("store reopens");
    let latest = usage_capture::record_ids::<UsageSnapshot>(&store, RUN)
        .expect("snapshots list")
        .into_iter()
        .last()
        .expect("snapshot published");
    let evidence: SnapshotEvidence = store.read(RUN, &latest).expect("evidence reads");
    let identity = &evidence.observations[0].observation.identity;
    assert_eq!(identity.session_id.as_deref(), Some("provider-session"));
    assert_eq!(identity.task_id.as_deref(), Some("provider-task"));
    assert_eq!(identity.phase, None, "a missing header field stays null");
    assert_eq!(
        evidence.observations[0].observation.models.configured, None,
        "a missing actual_model stays null"
    );
}

#[test]
fn header_value_of_none_leaves_field_null() {
    let directory = TempDir::new().expect("temp dir");
    let log = write_log(
        &directory.path().join("attempt.log"),
        "task_id:         none\n\
         phase:           none\n\
         session_id:      none\n\
         actual_model:    none\n",
        &[observation_with_identity(json!({}))],
    );

    usage_capture::capture_log(directory.path(), RUN, PLAN, "attempt-log", &log)
        .expect("usage block captures")
        .expect("usage block is present");

    let store = UsageStore::open(directory.path()).expect("store reopens");
    let latest = usage_capture::record_ids::<UsageSnapshot>(&store, RUN)
        .expect("snapshots list")
        .into_iter()
        .last()
        .expect("snapshot published");
    let evidence: SnapshotEvidence = store.read(RUN, &latest).expect("evidence reads");
    let identity = &evidence.observations[0].observation.identity;
    assert_eq!(identity.run_id.as_deref(), Some(RUN));
    assert_eq!(identity.plan_id.as_deref(), Some(PLAN));
    assert_eq!(identity.attempt_id.as_deref(), Some("attempt-log"));
    assert_eq!(identity.task_id, None, "header task_id: none stays null");
    assert_eq!(identity.phase, None, "header phase: none stays null");
    assert_eq!(
        identity.session_id, None,
        "header session_id: none stays null"
    );
    assert_eq!(
        evidence.observations[0].observation.models.configured, None,
        "header actual_model: none stays null"
    );
    assert_eq!(
        evidence.observations[0].observation.models.observed, None,
        "observed model stays null"
    );
}

#[test]
fn capturing_the_same_log_twice_adds_nothing() {
    let directory = TempDir::new().expect("temp dir");
    let log = write_log(
        &directory.path().join("attempt.log"),
        "task_id:         task-a\nsession_id:      header-session\n",
        &[observation_with_identity(json!({}))],
    );

    let first = usage_capture::capture_log(directory.path(), RUN, PLAN, "attempt-log", &log)
        .expect("first capture")
        .expect("usage block is present");
    let replay = usage_capture::capture_log(directory.path(), RUN, PLAN, "attempt-log", &log)
        .expect("replayed capture")
        .expect("usage block is present");

    assert_eq!(first.new_observations, 1);
    assert_eq!(replay.new_observations, 0);
    assert_eq!(replay.snapshot_id, None, "a replay publishes nothing");
}

#[test]
fn malformed_usage_block_is_reported_and_publishes_nothing() {
    let (directory, store) = store();
    drop(store);
    let common = directory.path();
    let log = directory.path().join("broken.log");
    fs::write(
        &log,
        "usage_extension:  v1\nusage_observations: serialization failed: boom\n",
    )
    .expect("log writes");

    let error = usage_capture::capture_log(common, RUN, PLAN, "broken", &log)
        .expect_err("a malformed block is an explicit error");
    assert!(matches!(error, CaptureError::Log(_)), "{error}");

    let store = UsageStore::open(common).expect("store reopens");
    let listed = usage_capture::record_ids::<UsageSnapshot>(&store, RUN);
    assert!(listed.expect("list works").is_empty());
    let missing = store.read::<SnapshotEvidence>(RUN, "snap-none");
    assert!(matches!(missing, Err(StoreError::NotFound(_))));
}
