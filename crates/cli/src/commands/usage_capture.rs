//! Shared accounting capture for pipeline attempts and explicit collections.
//!
//! Every path that publishes a usage snapshot goes through this module, so the
//! evidence and the snapshot that lists it are written the same way everywhere.
//! Evidence is published before the snapshot that names it. A failure returns an
//! error to the caller, which reports it. An attempt outcome never depends on it.
//!
//! The module depends only on the `dispatch` and `pipeline` crates and `std`, so
//! integration tests can include it directly.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use dispatch::dispatch::DispatchQuota;
use dispatch::quota::{QuotaObservation, QuotaSnapshot};
use dispatch::usage::{Availability, UsageObservation};
use pipeline::usage::binding::{RunIdentity, RunManifest, RUN_MANIFEST_VERSION};
use pipeline::usage::quota::{
    publish_observation, publish_revision, return_reference, QuotaRecordError, QuotaRevision,
};
use pipeline::usage::reconcile::{
    reconcile, ClockDomain, ElapsedInterval, ObservationRecord, ReconcileError, ReconcileInput,
};
use pipeline::usage::store::{Publication, StoreError, UsageStore, MAX_PAGE_LIMIT};
use pipeline::usage::{
    digest_bytes, SnapshotEvidence, StoredRecord, UsageSnapshot, RECORD_SCHEMA_VERSION,
};
use thiserror::Error;

/// Failure of a capture or of a shared snapshot publication.
#[derive(Debug, Error)]
pub(crate) enum CaptureError {
    #[error("the executor log cannot be read: {0}")]
    Log(String),
    #[error("the usage snapshot cannot be encoded: {0}")]
    Encode(String),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Reconcile(#[from] ReconcileError),
    #[error("observation `{0}` is already recorded with different content")]
    Conflict(String),
    #[error("quota record cannot be stored: {0}")]
    Quota(#[from] QuotaRecordError),
}

/// Usage one attempt wrote to its executor log.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct AttemptUsage {
    pub(crate) attempt_id: String,
    pub(crate) observations: Vec<UsageObservation>,
    /// Controller-observed execution time of the attempt, when the log recorded it.
    pub(crate) elapsed_ms: Option<u64>,
}

/// What one capture changed in a run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CaptureReport {
    pub(crate) attempt_id: String,
    pub(crate) new_observations: usize,
    pub(crate) run_observations: usize,
    /// `None` when the attempt added nothing new, so no snapshot was published.
    pub(crate) snapshot_id: Option<String>,
    /// Orchestrator cutoff in force after this capture. It never moves backward.
    pub(crate) cutoff_unix_ms: u64,
}

/// Everything the published snapshots of one run already hold.
#[derive(Debug, Default)]
pub(crate) struct RunEvidence {
    pub(crate) observations: BTreeMap<String, ObservationRecord>,
    pub(crate) intervals: Vec<ElapsedInterval>,
    pub(crate) rate_set_digests: BTreeSet<String>,
    pub(crate) cutoff_unix_ms: Option<u64>,
}

/// Current Unix time in milliseconds. A clock before the epoch reads as zero.
pub(crate) fn unix_ms_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as u64)
}

/// Reads the usage block of an executor log. A log without the block returns `None`,
/// and so does a block that holds no observations. A malformed block is an error.
///
/// The null identity fields of each observation are filled before it is returned. A value
/// the provider supplied is never replaced.
pub(crate) fn read_attempt_usage(
    log: &Path,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
) -> Result<Option<AttemptUsage>, CaptureError> {
    let text = fs::read_to_string(log)
        .map_err(|error| CaptureError::Log(format!("{}: {error}", log.display())))?;
    if !text
        .lines()
        .any(|line| line.starts_with("usage_extension:"))
    {
        return Ok(None);
    }
    let field = |name: &str| text.lines().find_map(|line| line.strip_prefix(name));
    let encoded = field("usage_observations: ")
        .ok_or_else(|| CaptureError::Log("usage_observations is missing".to_owned()))?;
    let mut observations: Vec<UsageObservation> =
        serde_json::from_str(encoded).map_err(|error| {
            CaptureError::Log(format!("usage_observations is not valid JSON: {error}"))
        })?;
    if observations.is_empty() {
        return Ok(None);
    }
    let header = LogHeader::read(&text);
    for observation in &mut observations {
        fill_identity(observation, run_id, plan_id, attempt_id, &header);
    }
    Ok(Some(AttemptUsage {
        attempt_id: attempt_id.to_owned(),
        observations,
        elapsed_ms: field("usage_elapsed_ms: ").and_then(|value| value.parse().ok()),
    }))
}

/// Identity fields that the header of an executor log holds. A `none` value, an empty
/// value, or a missing field reads as `None`.
struct LogHeader {
    task_id: Option<String>,
    phase: Option<String>,
    session_id: Option<String>,
    actual_model: Option<String>,
}

impl LogHeader {
    /// Reads the header, which ends at the stdout marker.
    fn read(text: &str) -> Self {
        let header = |name: &str| {
            text.lines()
                .take_while(|line| *line != "---STDOUT---")
                .find_map(|line| line.strip_prefix(name)?.strip_prefix(':'))
                .map(str::trim)
                .filter(|value| !value.is_empty() && *value != "none")
                .map(str::to_owned)
        };
        Self {
            task_id: header("task_id"),
            phase: header("phase"),
            session_id: header("session_id"),
            actual_model: header("actual_model"),
        }
    }
}

/// Fills the null identity fields of one observation from the run, the plan, the attempt,
/// and the log header. The header's `actual_model` fills the configured model only, and
/// never the observed model.
fn fill_identity(
    observation: &mut UsageObservation,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
    header: &LogHeader,
) {
    let identity = &mut observation.identity;
    fill_missing(&mut identity.run_id, Some(run_id));
    fill_missing(&mut identity.plan_id, Some(plan_id));
    fill_missing(&mut identity.attempt_id, Some(attempt_id));
    fill_missing(&mut identity.task_id, header.task_id.as_deref());
    fill_missing(&mut identity.phase, header.phase.as_deref());
    fill_missing(&mut identity.session_id, header.session_id.as_deref());
    fill_missing(
        &mut observation.models.configured,
        header.actual_model.as_deref(),
    );
}

/// Sets a field that is still null. A field that already holds a value keeps it.
fn fill_missing(field: &mut Option<String>, value: Option<&str>) {
    if field.is_none() {
        *field = value.map(str::to_owned);
    }
}

/// Lists the record identifiers of one kind in a run, page by page.
pub(crate) fn record_ids<R: StoredRecord>(
    store: &UsageStore,
    run_id: &str,
) -> Result<Vec<String>, StoreError> {
    let mut ids = Vec::new();
    let mut after: Option<String> = None;
    loop {
        let page = store.list_page::<R>(run_id, after.as_deref(), MAX_PAGE_LIMIT)?;
        ids.extend(page.entries.into_iter().map(|entry| entry.record_id));
        match page.next_after {
            Some(next) => after = Some(next),
            None => return Ok(ids),
        }
    }
}

/// Reads the evidence and snapshot metadata of every snapshot in a run. An observation
/// that two snapshots hold with different content is a conflict, never a choice.
pub(crate) fn read_run_evidence(
    store: &UsageStore,
    run_id: &str,
) -> Result<RunEvidence, CaptureError> {
    let mut evidence = RunEvidence::default();
    for snapshot_id in record_ids::<UsageSnapshot>(store, run_id)? {
        let snapshot: UsageSnapshot = store.read(run_id, &snapshot_id)?;
        evidence.rate_set_digests.extend(snapshot.rate_set_digest);
        let record: SnapshotEvidence = match store.read(run_id, &snapshot_id) {
            Err(StoreError::NotFound(_)) => {
                return Err(StoreError::EvidenceMissing(snapshot_id).into());
            }
            other => other?,
        };
        evidence.cutoff_unix_ms = Some(
            evidence
                .cutoff_unix_ms
                .map_or(record.cutoff_unix_ms, |known| {
                    known.max(record.cutoff_unix_ms)
                }),
        );
        for observation in record.observations {
            match evidence
                .observations
                .entry(observation.observation_id.clone())
            {
                Entry::Vacant(slot) => {
                    slot.insert(observation);
                }
                Entry::Occupied(known) => {
                    if known.get() != &observation {
                        return Err(CaptureError::Conflict(observation.observation_id));
                    }
                }
            }
        }
        for interval in record.intervals {
            if !evidence.intervals.contains(&interval) {
                evidence.intervals.push(interval);
            }
        }
    }
    Ok(evidence)
}

/// Publishes the evidence and then the snapshot that lists its digests.
///
/// The snapshot identifier is a digest of the evidence and the rate set, so identical
/// content has one identifier and publishing it again is a replay.
pub(crate) fn publish_snapshot(
    store: &UsageStore,
    run_id: &str,
    cutoff_unix_ms: u64,
    mut observations: Vec<ObservationRecord>,
    intervals: Vec<ElapsedInterval>,
    rate_set_digest: Option<String>,
) -> Result<(String, Publication), CaptureError> {
    observations.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let reconciled = reconcile(ReconcileInput {
        run_id: run_id.to_owned(),
        cutoff_unix_ms,
        observations: observations.clone(),
        intervals: intervals.clone(),
    })?;
    let mut evidence = SnapshotEvidence {
        schema_version: RECORD_SCHEMA_VERSION,
        snapshot_id: String::new(),
        run_id: run_id.to_owned(),
        cutoff_unix_ms,
        observations,
        intervals,
    };
    let identity = serde_json::to_vec(&(&evidence, &rate_set_digest))
        .map_err(|error| CaptureError::Encode(error.to_string()))?;
    let snapshot_id = format!("snap-{}", digest_bytes(&identity));
    evidence.snapshot_id = snapshot_id.clone();
    let snapshot = reconciled.snapshot(&snapshot_id, rate_set_digest);
    store.publish(&evidence)?;
    let publication = store.publish(&snapshot)?;
    Ok((snapshot_id, publication))
}

/// Merges one attempt's observations and elapsed interval into its run.
///
/// The new snapshot lists every observation of the run, so a later report needs only
/// the latest snapshot. The cutoff never moves backward, so a recovered run keeps the
/// cutoff it was bound to. An attempt that adds nothing publishes nothing.
pub(crate) fn capture_attempt(
    store: &UsageStore,
    run_id: &str,
    usage: &AttemptUsage,
    now_unix_ms: u64,
) -> Result<CaptureReport, CaptureError> {
    let mut evidence = read_run_evidence(store, run_id)?;
    let mut new_observations = 0;
    for (index, observation) in usage.observations.iter().enumerate() {
        let record = ObservationRecord {
            observation_id: format!("{}-{index:04}", usage.attempt_id),
            observation: observation.clone(),
        };
        match evidence.observations.entry(record.observation_id.clone()) {
            Entry::Vacant(slot) => {
                slot.insert(record);
                new_observations += 1;
            }
            Entry::Occupied(known) => {
                if known.get() != &record {
                    return Err(CaptureError::Conflict(record.observation_id));
                }
            }
        }
    }

    // An attempt has one elapsed interval, written by its first capture. A later capture of
    // the same attempt must not stamp a new interval at its own clock time.
    let mut interval_added = false;
    let interval_known = evidence
        .intervals
        .iter()
        .any(|known| known.attempt_id == usage.attempt_id);
    if let (Some(elapsed_ms), false) = (usage.elapsed_ms, interval_known) {
        evidence.intervals.push(ElapsedInterval {
            attempt_id: usage.attempt_id.clone(),
            start_unix_ms: now_unix_ms.saturating_sub(elapsed_ms),
            end_unix_ms: now_unix_ms,
            clock: ClockDomain::Controller,
        });
        interval_added = true;
    }

    let cutoff_unix_ms = evidence
        .cutoff_unix_ms
        .map_or(now_unix_ms, |known| known.max(now_unix_ms));
    let run_observations = evidence.observations.len();
    if new_observations == 0 && !interval_added {
        return Ok(CaptureReport {
            attempt_id: usage.attempt_id.clone(),
            new_observations,
            run_observations,
            snapshot_id: None,
            cutoff_unix_ms: evidence.cutoff_unix_ms.unwrap_or(now_unix_ms),
        });
    }

    let rate_set_digest = match evidence.rate_set_digests.len() {
        1 => evidence.rate_set_digests.iter().next().cloned(),
        _ => None,
    };
    let (snapshot_id, _) = publish_snapshot(
        store,
        run_id,
        cutoff_unix_ms,
        evidence.observations.into_values().collect(),
        evidence.intervals,
        rate_set_digest,
    )?;
    Ok(CaptureReport {
        attempt_id: usage.attempt_id.clone(),
        new_observations,
        run_observations,
        snapshot_id: Some(snapshot_id),
        cutoff_unix_ms,
    })
}

/// Captures the usage block of one attempt log into the run's usage store.
/// Returns `None` when the log carries no usage to capture. The plan identifier fills
/// the observations that name no plan.
pub(crate) fn capture_log(
    common_dir: &Path,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
    log: &Path,
) -> Result<Option<CaptureReport>, CaptureError> {
    let Some(usage) = read_attempt_usage(log, run_id, plan_id, attempt_id)? else {
        return Ok(None);
    };
    let store = UsageStore::open(common_dir)?;
    capture_attempt(&store, run_id, &usage, unix_ms_now()).map(Some)
}

/// Run identity of a pipeline attempt, in the form that `gal usage collect` binds. It holds the
/// preflight run, the canonical worktree, and the plan scope, and no host session.
pub(crate) fn pipeline_run_identity(
    run_id: &str,
    canonical_worktree: &str,
    plan_scope: &str,
) -> RunIdentity {
    RunIdentity {
        version: RUN_MANIFEST_VERSION,
        run_id: run_id.to_owned(),
        canonical_worktree: canonical_worktree.to_owned(),
        plan_scope: plan_scope.to_owned(),
        host_session: None,
        host_window: None,
    }
}

const NO_ATTEMPT_REASON: &str =
    "orchestrator usage has no attempt to attach quota to, so no quota reference is recorded";
const NOT_ATTEMPTED_REASON: &str = "the attempt was not launched, so no account was read";
const NO_OBSERVATION_REASON: &str = "the quota read returned no observation";

/// Stores the quota that one return or source read holds, and returns the exact snapshot reference.
///
/// The reference names the attempt's last observation, which is the return's own control query when
/// one ran. Earlier passive observations stay stored as receipts. A storage failure never becomes an
/// error here. It becomes an unavailable snapshot that carries the reason, so no reference dangles.
/// An attempt that already holds a reference keeps it, and a repeated capture writes nothing new.
pub(crate) fn capture_quota(
    store: &UsageStore,
    identity: &RunIdentity,
    attempt_id: Option<&str>,
    quota: &DispatchQuota,
) -> QuotaSnapshot {
    let Some(attempt_id) = attempt_id else {
        return QuotaSnapshot::unavailable(NO_ATTEMPT_REASON);
    };
    match quota {
        DispatchQuota::NotAttempted => QuotaSnapshot::unavailable(NOT_ATTEMPTED_REASON),
        DispatchQuota::UnknownScope { reason } => QuotaSnapshot::unavailable(reason),
        DispatchQuota::Collected { observations } if observations.is_empty() => {
            QuotaSnapshot::unavailable(NO_OBSERVATION_REASON)
        }
        DispatchQuota::Collected { observations } => {
            let manifest = RunManifest::standalone(identity.clone(), attempt_id);
            store_quota(store, &manifest, observations).unwrap_or_else(|error| {
                QuotaSnapshot::unavailable(&format!("quota is not stored: {error}"))
            })
        }
    }
}

fn store_quota(
    store: &UsageStore,
    manifest: &RunManifest,
    observations: &[QuotaObservation],
) -> Result<QuotaSnapshot, CaptureError> {
    let run_id = manifest.identity.run_id.as_str();
    let attempt_id = manifest.attempt_id.as_str();
    let revisions = read_quota_revisions(store, run_id)?;
    // The revision that first listed the attempt names its reference for good.
    // Later revisions only extend the chain, so the reference never moves.
    if let Some(revision) = revisions.iter().find(|revision| {
        revision
            .entries
            .iter()
            .any(|entry| entry.attempt_id == attempt_id)
    }) {
        return Ok(return_reference(store, revision, attempt_id)?);
    }
    let mut reference = None;
    for (index, observation) in observations.iter().enumerate() {
        let observation_id = format!("{attempt_id}-quota-{index:04}");
        let (_, entry) = publish_observation(
            store,
            run_id,
            attempt_id,
            manifest,
            &observation_id,
            observation.clone(),
        )?;
        reference = Some(entry);
    }
    let Some(reference) = reference else {
        return Ok(QuotaSnapshot::unavailable(NO_OBSERVATION_REASON));
    };
    let (_, revision) = publish_revision(store, run_id, revisions.last(), vec![reference])?;
    Ok(return_reference(store, &revision, attempt_id)?)
}

/// Every stored revision of a run, oldest first.
fn read_quota_revisions(
    store: &UsageStore,
    run_id: &str,
) -> Result<Vec<QuotaRevision>, CaptureError> {
    let mut revisions = Vec::new();
    for record_id in record_ids::<QuotaRevision>(store, run_id)? {
        revisions.push(store.read(run_id, &record_id)?);
    }
    revisions.sort_by_key(|revision: &QuotaRevision| revision.revision);
    Ok(revisions)
}

/// Text of one quota snapshot, with the observation it reads from when one is known.
pub(crate) fn quota_text(
    snapshot: &QuotaSnapshot,
    observation: Option<&QuotaObservation>,
) -> String {
    let state = match &snapshot.reference {
        Availability::Reported { value } => format!(
            "reported, revision {}, observation {}",
            value.snapshot_id.as_str(),
            value.observation_digest.as_str()
        ),
        Availability::Partial { value, reason } => format!(
            "partial, revision {}, observation {}. {reason}",
            value.snapshot_id.as_str(),
            value.observation_digest.as_str()
        ),
        Availability::Unsupported { reason } => format!("unsupported. {reason}"),
        Availability::Unavailable { reason } => format!("unavailable. {reason}"),
        Availability::Malformed { reason } => format!("malformed. {reason}"),
    };
    match observation {
        Some(observation) => format!("{state}\n  {}", observation_text(observation)),
        None => state,
    }
}

fn observation_text(observation: &QuotaObservation) -> String {
    let version = observation
        .identity
        .runtime_version
        .as_ref()
        .map_or("not recorded", |version| version.as_str());
    let observed = observation
        .observed_at_unix_ms
        .map_or("not recorded".to_owned(), |ms| format!("{ms} ms"));
    let query = observation
        .query
        .as_ref()
        .map_or("no control query".to_owned(), |query| {
            format!("control query {} in {} ms", query.method, query.duration_ms)
        });
    format!(
        "runtime version {version}, identity {:?}, observed {observed}, collected {} ms, acquisition {}, freshness {:?}, {query}",
        observation.identity.identity_check,
        observation.collected_at_unix_ms,
        acquisition_text(&observation.acquisition),
        observation.freshness,
    )
}

fn acquisition_text(acquisition: &dispatch::quota::Acquisition) -> String {
    use dispatch::quota::Acquisition;
    match acquisition {
        Acquisition::Passive => "passive".to_owned(),
        Acquisition::Queried => "queried".to_owned(),
        Acquisition::Failed { reason } => format!("failed. {reason}"),
        Acquisition::TimedOut { reason } => format!("timed out. {reason}"),
        Acquisition::Unsupported { reason } => format!("unsupported. {reason}"),
    }
}
