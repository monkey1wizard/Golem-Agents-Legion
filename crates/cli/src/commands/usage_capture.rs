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

#[derive(Default)]
pub(crate) struct AttemptAttribution {
    pub configured_model: Option<String>,
    pub effort: Option<String>,
    pub observed_model: Option<String>,
    pub observed_effort: Option<String>,
    pub role: Option<String>,
    pub phase: String,
    pub task_id: Option<String>,
    pub session_id: Option<String>,
    pub route_source: Option<String>,
    pub one_shot_override: Option<bool>,
    pub source_plan: Option<std::path::PathBuf>,
}

fn ensure_run(
    common_dir: &Path,
    workdir: &Path,
    run_id: &str,
    plan_id: &str,
    source_plan: Option<&Path>,
) -> Result<
    (
        pipeline::report::EvidenceStore,
        pipeline::report::DesignRosterVersion,
    ),
    String,
> {
    let evidence_store =
        pipeline::report::EvidenceStore::open(common_dir).map_err(|error| error.to_string())?;
    let lock_dir = common_dir.join("gal/evidence/v1");
    fs::create_dir_all(&lock_dir).map_err(|error| error.to_string())?;
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_dir.join(".run-lock"))
        .map_err(|error| error.to_string())?;
    fs2::FileExt::lock_exclusive(&lock).map_err(|error| error.to_string())?;
    let roster;
    match evidence_store.read(run_id, "run") {
        Ok(record) => {
            return Ok((
                evidence_store,
                pipeline::report::DesignRosterVersion {
                    design_roster_version: record.content["design_roster_version"]
                        .as_str()
                        .map(str::to_owned),
                    reason: record.content["design_roster_reason"]
                        .as_str()
                        .map(str::to_owned),
                },
            ))
        }
        Err(StoreError::NotFound(_)) => {
            roster = pipeline::report::read_design_roster_version(workdir)
                .map_err(|error| error.to_string())?;
            let source = source_plan.map(|path| match fs::read(path) {
                Ok(bytes) => serde_json::json!({"path": path, "sha256": digest_bytes(&bytes)}),
                Err(error) => {
                    serde_json::json!({"path": path, "sha256": null, "reason": error.to_string()})
                }
            });
            let content = serde_json::json!({
                "plan_id": plan_id, "source_plan": source,
                "design_roster_version": roster.design_roster_version,
                "design_roster_reason": roster.reason
            });
            let record = pipeline::report::EvidenceRecord {
                schema_version: 1,
                event_id: "run".into(),
                operation_id: run_id.into(),
                run_id: run_id.into(),
                plan_generation: Some(plan_id.into()),
                task_id: None,
                attempt_id: None,
                parent_attempt_id: None,
                phase: "run".into(),
                source_kind: "run".into(),
                source_version: "1".into(),
                recorded_at_unix_ms: unix_ms_now(),
                content_hash: digest_bytes(
                    &serde_json::to_vec(&content).map_err(|error| error.to_string())?,
                ),
                content,
            };
            evidence_store
                .publish(&record)
                .map_err(|error| error.to_string())?;
        }
        Err(error) => return Err(error.to_string()),
    }
    Ok((evidence_store, roster))
}

pub(crate) fn expect_attempt(
    common_dir: &Path,
    workdir: &Path,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
    phase: &str,
    source_plan: Option<&Path>,
) -> Result<(), String> {
    let (store, roster) = ensure_run(common_dir, workdir, run_id, plan_id, source_plan)?;
    let content = serde_json::json!({"status": "expected", "reason": "attempt result pending",
        "design_roster_version": roster.design_roster_version, "design_roster_reason": roster.reason});
    let record = pipeline::report::EvidenceRecord {
        schema_version: 1,
        event_id: format!("expected-{attempt_id}"),
        operation_id: attempt_id.into(),
        run_id: run_id.into(),
        plan_generation: Some(plan_id.into()),
        task_id: None,
        attempt_id: Some(attempt_id.into()),
        parent_attempt_id: None,
        phase: phase.into(),
        source_kind: "attempt-expected".into(),
        source_version: "1".into(),
        recorded_at_unix_ms: unix_ms_now(),
        content_hash: digest_bytes(
            &serde_json::to_vec(&content).map_err(|error| error.to_string())?,
        ),
        content,
    };
    store.publish(&record).map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn capture_native(
    common_dir: &Path,
    workdir: &Path,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
    attribution: &AttemptAttribution,
    usage: &dispatch::dispatch::DispatchUsage,
) -> Result<(), String> {
    use dispatch::dispatch::UsageCollection;
    use dispatch::executors::UsageCapability;
    let (native_state, native_reasons) = usage.native.completeness();
    let mut reasons = native_reasons.to_vec();
    if usage.collection == UsageCollection::NotAttempted {
        reasons.push("attempt not started or usage collection not attempted".into());
    }
    reasons.extend(usage.reasons.iter().cloned());
    if let UsageCollection::Partial { reason } = &usage.collection {
        reasons.push(reason.clone());
    }
    let source = match &usage.native.capability {
        UsageCapability::Mapped { source_version } => *source_version,
        UsageCapability::Unsupported { reason } => {
            reasons.push(reason.clone());
            "unsupported"
        }
    };
    if usage.native.observations.is_empty() {
        reasons.push("no native usage observations".into());
    }
    for observation in &usage.native.observations {
        for metric in [&observation.tokens.input, &observation.tokens.output] {
            match &metric.count.value {
                Availability::Unavailable { reason }
                | Availability::Partial { reason, .. }
                | Availability::Malformed { reason } => reasons.push(reason.clone()),
                _ => {}
            }
        }
    }
    let state = if native_state == "malformed" || usage.completeness == "malformed" {
        if reasons.is_empty() {
            reasons.push("usage parsing malformed".into());
        }
        "malformed"
    } else if !reasons.is_empty() || native_state != "complete" || usage.completeness != "complete"
    {
        if reasons.is_empty() {
            reasons.push("usage collection incomplete".into());
        }
        "partial"
    } else {
        "complete"
    };
    let mut observations = usage.native.observations.clone();
    for observation in &mut observations {
        let header = LogHeader {
            task_id: attribution.task_id.clone(),
            phase: Some(attribution.phase.clone()),
            session_id: attribution.session_id.clone(),
            actual_model: attribution.configured_model.clone(),
        };
        fill_identity(observation, run_id, plan_id, attempt_id, &header);
    }
    let store = UsageStore::open(common_dir).map_err(|error| error.to_string())?;
    let report = if usage.collection == UsageCollection::NotAttempted || observations.is_empty() {
        None
    } else {
        Some(
            capture_attempt(
                &store,
                run_id,
                &AttemptUsage {
                    attempt_id: attempt_id.into(),
                    observations: observations.clone(),
                    elapsed_ms: usage
                        .elapsed
                        .map(|elapsed| elapsed.as_millis().min(u64::MAX as u128) as u64),
                },
                unix_ms_now(),
            )
            .map_err(|error| error.to_string())?,
        )
    };
    let (evidence_store, roster) = ensure_run(
        common_dir,
        workdir,
        run_id,
        plan_id,
        attribution.source_plan.as_deref(),
    )?;
    let observed_models: BTreeSet<_> = observations
        .iter()
        .filter_map(|o| o.models.observed.as_deref())
        .chain(attribution.observed_model.as_deref())
        .collect();
    let content = serde_json::json!({
        "plan_id": plan_id, "configured_model": attribution.configured_model,
        "configured_model_unknown_reason": attribution.configured_model.is_none().then_some("configured model not recorded"),
        "observed_models": observed_models,
        "observed_model_unknown_reason": observed_models.is_empty().then_some("provider did not report a model"),
        "effort": attribution.effort,
        "effort_unknown_reason": attribution.effort.is_none().then_some("configured effort not recorded"),
        "observed_effort": attribution.observed_effort,
        "observed_effort_unknown_reason": attribution.observed_effort.is_none().then_some("provider did not report effort"),
        "role": {"value": attribution.role, "reason": attribution.role.is_none().then_some("generic invocation has no role")},
        "route_source": attribution.route_source,
        "route_source_unknown_reason": attribution.route_source.is_none().then_some("route source not recorded"),
        "one_shot_override": attribution.one_shot_override,
        "one_shot_override_unknown_reason": attribution.one_shot_override.is_none().then_some("route override not recorded"),
        "completeness": state, "reasons": reasons, "usage_evidence_id": usage.evidence_id,
        "snapshot_id": report.and_then(|report| report.snapshot_id), "stdout_source": stream_diagnostics(usage.stdout_source.as_ref()),
        "stderr_source": stream_diagnostics(usage.stderr_source.as_ref()),
        "design_roster_version": roster.design_roster_version, "design_roster_reason": roster.reason
    });
    let content_hash =
        digest_bytes(&serde_json::to_vec(&content).map_err(|error| error.to_string())?);
    let record = pipeline::report::EvidenceRecord {
        schema_version: 1,
        event_id: format!("usage-{attempt_id}-{}", &content_hash[..16]),
        operation_id: attempt_id.into(),
        run_id: run_id.into(),
        plan_generation: Some(plan_id.into()),
        task_id: attribution.task_id.clone(),
        attempt_id: Some(attempt_id.into()),
        parent_attempt_id: None,
        phase: attribution.phase.clone(),
        source_kind: "attempt-usage".into(),
        source_version: source.into(),
        recorded_at_unix_ms: unix_ms_now(),
        content_hash,
        content,
    };
    let record = match evidence_store.read(run_id, &record.event_id) {
        Ok(previous) if previous.content == record.content => previous,
        Ok(_) => return Err("integrity-failure: usage sidecar content differs".into()),
        Err(StoreError::NotFound(_)) => record,
        Err(error) => return Err(error.to_string()),
    };
    evidence_store
        .publish(&record)
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn stream_diagnostics(source: Option<&dispatch::dispatch::StreamDiagnostics>) -> serde_json::Value {
    source.map_or(serde_json::Value::Null, |source| {
        serde_json::json!({
            "dropped_range": source.dropped_range, "completeness": source.completeness,
            "reasons": source.reasons, "first_output_ms": source.first_output_ms,
            "last_output_ms": source.last_output_ms, "read_error": source.read_error
        })
    })
}

pub(crate) fn read_host_attempt(
    log: &Path,
) -> Option<dispatch::host_attempt_state::HostAttemptState> {
    let text = fs::read_to_string(log).ok()?;
    let attempt_id = text
        .lines()
        .take_while(|line| *line != "---STDOUT---")
        .find_map(|line| line.strip_prefix("attempt_id:"))
        .map(str::trim)?;
    let path = dispatch::host_attempt_state::HostAttemptState::manifest_path_for(
        log.parent()?.parent()?,
        attempt_id,
    )
    .ok()?;
    let state = dispatch::host_attempt_state::HostAttemptState::load(&path, attempt_id).ok()?;
    (state.log_path == log).then_some(state)
}

pub(crate) fn capture_logged_attempt(
    common_dir: &Path,
    workdir: &Path,
    run_id: &str,
    plan_id: &str,
    attempt_id: &str,
    log: Option<&Path>,
    source_plan: Option<&Path>,
) -> Result<(), String> {
    use dispatch::dispatch::{DispatchUsage, UsageCollection};
    let text = log.map(fs::read_to_string).transpose();
    let read_error = text.as_ref().err().map(ToString::to_string);
    let text = text.ok().flatten().unwrap_or_default();
    let header = LogHeader::read(&text);
    let field = |key: &str| {
        text.lines()
            .take_while(|line| *line != "---STDOUT---")
            .find_map(|line| line.strip_prefix(key)?.strip_prefix(':'))
            .map(str::trim)
            .filter(|value| !value.is_empty() && *value != "none")
    };
    let phase = header.phase.as_deref().unwrap_or("dispatch");
    let role = dispatch::stage::Phase::from_str(phase)
        .ok()
        .map(|phase| phase.role().to_owned());
    let table = dispatch::routing::load_routing_default();
    let route = role.as_deref().and_then(|role| table.get(role));
    let one_shot = field("route_digest").is_some();
    let mut attribution = AttemptAttribution {
        configured_model: header.actual_model.clone(),
        effort: (!one_shot)
            .then(|| {
                route
                    .filter(|route| Some(&route.model) == header.actual_model.as_ref())
                    .and_then(|route| route.effort.clone())
            })
            .flatten(),
        role,
        phase: phase.into(),
        task_id: (!matches!(phase, "investigate" | "finalize-review"))
            .then_some(header.task_id)
            .flatten(),
        session_id: header.session_id,
        route_source: Some(
            if one_shot {
                "one-shot"
            } else {
                "executor-routing"
            }
            .into(),
        ),
        one_shot_override: Some(one_shot),
        source_plan: source_plan.map(Path::to_path_buf),
        ..Default::default()
    };
    if let Some(state) = log.and_then(read_host_attempt) {
        attribution.configured_model = Some(state.model);
        attribution.effort = state.effort;
        attribution.observed_model = header.actual_model;
        attribution.observed_effort = field("actual_effort").map(str::to_owned);
        attribution.one_shot_override = Some(state.model_source == "explicit-one-shot");
        attribution.route_source = Some(state.model_source);
    }
    let mut usage = DispatchUsage::not_attempted();
    if let Some(executor) = field("executor") {
        if let Some(adapter) = dispatch::executors::get_adapter(executor) {
            let stdout = text
                .split_once("---STDOUT---")
                .map(|(_, tail)| {
                    tail.split_once("---STDERR---")
                        .map_or(tail, |(stdout, _)| stdout)
                })
                .unwrap_or("");
            usage.native = adapter.extract_native_usage(stdout);
        }
    }
    if field("usage_extension").is_some() {
        usage
            .reasons
            .push("native parse sidecar not recorded in executor log".into());
        usage.collection = match field("usage_collection") {
            Some("complete") => UsageCollection::Complete,
            Some("not-attempted") => UsageCollection::NotAttempted,
            other => UsageCollection::Partial {
                reason: other.unwrap_or("usage collection missing").into(),
            },
        };
        if let Some(encoded) = field("usage_observations") {
            match serde_json::from_str(encoded) {
                Ok(observations) => usage.native.observations = observations,
                Err(error) => {
                    usage.completeness = "malformed".into();
                    usage
                        .reasons
                        .push(format!("usage observations malformed: {error}"));
                }
            }
        }
        usage.elapsed = field("usage_elapsed_ms")
            .and_then(|value| value.parse().ok())
            .map(std::time::Duration::from_millis);
        usage.evidence_id = field("usage_evidence").map(str::to_owned);
    } else {
        usage
            .reasons
            .push(read_error.unwrap_or_else(|| "usage block absent or attempt not started".into()));
    }
    capture_native(
        common_dir,
        workdir,
        run_id,
        plan_id,
        attempt_id,
        &attribution,
        &usage,
    )
}

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
