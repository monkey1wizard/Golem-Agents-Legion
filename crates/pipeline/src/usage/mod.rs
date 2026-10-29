//! Durable accounting records: factual snapshots and separate assessments.
//!
//! A snapshot names the observations and the reconciliation version it was
//! built from. An assessment names one snapshot and the exact bytes it judges,
//! so a later review can detect changed evidence. Evidence holds the
//! observations behind a snapshot, so the snapshot's subtotals can be rebuilt.
//! All are stored through [`UsageStore`], which never replaces a published record.

pub mod assessment;
pub mod binding;
pub mod evidence;
pub mod pricing;
pub mod quota;
pub mod reconcile;
pub mod store;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub use evidence::SnapshotEvidence;
pub use reconcile::{
    ClockDomain, ElapsedInterval, ObservationRecord, ReconcileError, ReconcileInput, Reconciliation,
};
pub use store::{ManifestEntry, ManifestPage, Publication, StoreError, UsageStore};

/// Schema version written by this build for stored accounting records.
pub const RECORD_SCHEMA_VERSION: u32 = 1;

/// Kind of a stored record. Each kind has its own directory in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordKind {
    Snapshot,
    Assessment,
    QuotaObservation,
    QuotaRevision,
    Evidence,
}

impl RecordKind {
    /// Directory name under a run directory that holds records of this kind.
    pub fn dir_name(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshots",
            Self::Assessment => "assessments",
            Self::QuotaObservation => "quota_observations",
            Self::QuotaRevision => "quota_revisions",
            Self::Evidence => "evidence",
        }
    }
}

/// A record that the store can publish, read, and page.
pub trait StoredRecord: Serialize + DeserializeOwned {
    const KIND: RecordKind;

    fn schema_version(&self) -> u32;
    fn run_id(&self) -> &str;
    fn record_id(&self) -> &str;
}

/// Digest of one observation that a snapshot includes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ObservationDigest {
    pub observation_id: String,
    pub digest: String,
}

/// Factual snapshot of a run. It names the exact observations it covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageSnapshot {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub run_id: String,
    pub reconciliation_version: u32,
    /// Digest of the rate set used for priced subtotals. `None` when unpriced.
    pub rate_set_digest: Option<String>,
    pub observations: Vec<ObservationDigest>,
}

impl StoredRecord for UsageSnapshot {
    const KIND: RecordKind = RecordKind::Snapshot;

    fn schema_version(&self) -> u32 {
        self.schema_version
    }

    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn record_id(&self) -> &str {
        &self.snapshot_id
    }
}

/// Orchestrator judgment about one snapshot. It never changes the snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsageAssessment {
    pub schema_version: u32,
    pub assessment_id: String,
    pub run_id: String,
    pub snapshot_id: String,
    /// Digest of the snapshot record bytes this assessment judges.
    pub snapshot_digest: String,
    pub classification_rule: String,
    pub recorded_at_unix_ms: u64,
}

impl StoredRecord for UsageAssessment {
    const KIND: RecordKind = RecordKind::Assessment;

    fn schema_version(&self) -> u32 {
        self.schema_version
    }

    fn run_id(&self) -> &str {
        &self.run_id
    }

    fn record_id(&self) -> &str {
        &self.assessment_id
    }
}

/// Lowercase hex SHA-256 of the given bytes.
pub fn digest_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
