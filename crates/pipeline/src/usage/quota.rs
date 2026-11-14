//! Immutable quota observations and the per-attempt references that point to them.
//!
//! Quota describes shared account capacity. Its records live in their own
//! directories beside usage records, so no usage aggregate reads them, and no
//! type here can add a value to a task cost.
//!
//! Each observation is one record bound to one attempt through the digest of
//! that attempt's run manifest. A run keeps a chain of revisions. A revision
//! lists every entry published so far and may only extend the revision before
//! it. A later refresh therefore adds a revision, and the earlier revision and
//! its return references stay byte-identical.

use super::binding::{check_attempt_binding, BindingError, RunManifest};
use super::store::{record_digest, Publication, StoreError, UsageStore};
use super::{RecordKind, StoredRecord, RECORD_SCHEMA_VERSION};
use dispatch::quota::{
    AccountIdentityCheck, Acquisition, ObservationDigest, QuotaError, QuotaObservation,
    QuotaReference, QuotaSnapshot, SnapshotId, QUOTA_SCHEMA_VERSION,
};
use dispatch::usage::Availability;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const MISMATCH_REASON: &str =
    "account identity does not match the attempt, so quota is not attributed to it";
const UNVERIFIED_REASON: &str =
    "account identity is unverified, so the quota scope is not proven for this attempt";

#[derive(Debug, Error)]
pub enum QuotaRecordError {
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Binding(#[from] BindingError),
    #[error(transparent)]
    Quota(#[from] QuotaError),
    #[error("quota entry for observation `{0}` does not match its stored record")]
    EntryMismatch(String),
    #[error("attempt `{0}` already has a return reference in this revision chain")]
    DuplicateAttempt(String),
    #[error("revision `{0}` is no longer the stored revision it was read as")]
    StaleRevision(String),
    #[error("no quota entry names attempt `{0}`")]
    NoEntry(String),
}

/// Immutable factual quota observation for one attempt of one run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaObservationRecord {
    pub schema_version: u32,
    pub observation_id: String,
    pub run_id: String,
    pub attempt_id: String,
    /// Digest of the run manifest that binds `attempt_id` to `run_id`.
    pub run_manifest_digest: String,
    pub observation: QuotaObservation,
}

impl StoredRecord for QuotaObservationRecord {
    const KIND: RecordKind = RecordKind::QuotaObservation;

    fn schema_version(&self) -> u32 {
        self.schema_version
    }

    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn record_id(&self) -> &str {
        &self.observation_id
    }
}

/// One attempt's link to a stored observation. The digest covers the observation record bytes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaEntry {
    pub attempt_id: String,
    pub observation_id: String,
    pub observation_digest: String,
}

/// Factual revision of a run's quota chain. It lists every entry published so far.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QuotaRevision {
    pub schema_version: u32,
    pub revision_id: String,
    pub run_id: String,
    /// One-based position in the run's chain.
    pub revision: u32,
    pub previous_revision_id: Option<String>,
    pub entries: Vec<QuotaEntry>,
}

impl StoredRecord for QuotaRevision {
    const KIND: RecordKind = RecordKind::QuotaRevision;

    fn schema_version(&self) -> u32 {
        self.schema_version
    }

    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn record_id(&self) -> &str {
        &self.revision_id
    }
}

/// Publishes one attempt's quota observation after checking that the attempt belongs to the run.
///
/// An identical replay returns `Replayed` and keeps the stored record. Different content under
/// the same `observation_id` is a conflict, so an original observation time never changes.
pub fn publish_observation(
    store: &UsageStore,
    run_id: &str,
    attempt_id: &str,
    manifest: &RunManifest,
    observation_id: &str,
    observation: QuotaObservation,
) -> Result<(Publication, QuotaEntry), QuotaRecordError> {
    observation.validate()?;
    check_attempt_binding(manifest, run_id, attempt_id)?;
    let record = QuotaObservationRecord {
        schema_version: RECORD_SCHEMA_VERSION,
        observation_id: observation_id.to_owned(),
        run_id: run_id.to_owned(),
        attempt_id: attempt_id.to_owned(),
        run_manifest_digest: manifest.digest()?,
        observation,
    };
    let publication = store.publish(&record)?;
    let entry = QuotaEntry {
        attempt_id: attempt_id.to_owned(),
        observation_id: observation_id.to_owned(),
        observation_digest: record_digest(&record)?,
    };
    Ok((publication, entry))
}

/// Appends `new_entries` to a run's quota chain as its next revision.
///
/// `previous` must still be the stored revision it was read as. Each new entry must match its
/// stored observation, and an attempt can appear only once. Earlier revisions are never rewritten.
pub fn publish_revision(
    store: &UsageStore,
    run_id: &str,
    previous: Option<&QuotaRevision>,
    new_entries: Vec<QuotaEntry>,
) -> Result<(Publication, QuotaRevision), QuotaRecordError> {
    let (number, mut entries, previous_revision_id) = match previous {
        Some(prior) => {
            let stored: QuotaRevision = store.read(run_id, &prior.revision_id)?;
            if stored != *prior {
                return Err(QuotaRecordError::StaleRevision(prior.revision_id.clone()));
            }
            (
                prior.revision + 1,
                prior.entries.clone(),
                Some(prior.revision_id.clone()),
            )
        }
        None => (1, Vec::new(), None),
    };
    for entry in new_entries {
        if entries
            .iter()
            .any(|known| known.attempt_id == entry.attempt_id)
        {
            return Err(QuotaRecordError::DuplicateAttempt(entry.attempt_id));
        }
        verify_entry(store, run_id, &entry)?;
        entries.push(entry);
    }
    let revision = QuotaRevision {
        schema_version: RECORD_SCHEMA_VERSION,
        revision_id: format!("quota-r{number}"),
        run_id: run_id.to_owned(),
        revision: number,
        previous_revision_id,
        entries,
    };
    let publication = store.publish(&revision)?;
    Ok((publication, revision))
}

/// Builds the return reference for one attempt after re-checking its stored observation.
///
/// A verified account whose values were read gives a `Reported` reference. An unverified account
/// gives a `Partial` reference that keeps its reason. A mismatch, failed query, timeout, or
/// unsupported source gives `Unavailable`, and such a reference never claims attribution.
pub fn return_reference(
    store: &UsageStore,
    revision: &QuotaRevision,
    attempt_id: &str,
) -> Result<QuotaSnapshot, QuotaRecordError> {
    let entry = revision
        .entries
        .iter()
        .find(|entry| entry.attempt_id == attempt_id)
        .ok_or_else(|| QuotaRecordError::NoEntry(attempt_id.to_owned()))?;
    let record = verify_entry(store, &revision.run_id, entry)?;
    let reference = QuotaReference {
        snapshot_id: SnapshotId::new(&revision.revision_id)?,
        observation_digest: ObservationDigest::new(&entry.observation_digest)?,
    };
    let reference = match share_decision(&record.observation) {
        Share::Reported => Availability::Reported { value: reference },
        Share::Partial(reason) => Availability::Partial {
            value: reference,
            reason,
        },
        Share::Unavailable(reason) => Availability::Unavailable { reason },
    };
    Ok(QuotaSnapshot {
        schema_version: QUOTA_SCHEMA_VERSION,
        reference,
    })
}

enum Share {
    Reported,
    Partial(String),
    Unavailable(String),
}

fn share_decision(observation: &QuotaObservation) -> Share {
    match (
        observation.identity.identity_check,
        &observation.acquisition,
    ) {
        (AccountIdentityCheck::Mismatch, _) => Share::Unavailable(MISMATCH_REASON.to_owned()),
        (
            _,
            Acquisition::Failed { reason }
            | Acquisition::TimedOut { reason }
            | Acquisition::Unsupported { reason },
        ) => Share::Unavailable(reason.clone()),
        (AccountIdentityCheck::Verified, _) => Share::Reported,
        (AccountIdentityCheck::Unverified, _) => Share::Partial(UNVERIFIED_REASON.to_owned()),
    }
}

/// Reads the stored observation behind an entry and checks its attempt, digest, and validity.
fn verify_entry(
    store: &UsageStore,
    run_id: &str,
    entry: &QuotaEntry,
) -> Result<QuotaObservationRecord, QuotaRecordError> {
    let record: QuotaObservationRecord = store.read(run_id, &entry.observation_id)?;
    if record.attempt_id != entry.attempt_id || record_digest(&record)? != entry.observation_digest
    {
        return Err(QuotaRecordError::EntryMismatch(
            entry.observation_id.clone(),
        ));
    }
    record.observation.validate()?;
    Ok(record)
}
