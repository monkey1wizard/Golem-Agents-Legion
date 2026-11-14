//! Observations and intervals that a snapshot was reconciled from.
//!
//! A snapshot lists only observation digests. Its evidence record keeps the
//! observation bytes and elapsed intervals behind those digests, so a later
//! reader can rebuild the same subtotals without rerunning any executor. Rate
//! content stays out of this record, so a rebuild never reprices.

use super::reconcile::{ElapsedInterval, ObservationRecord};
use super::{RecordKind, StoredRecord};
use serde::{Deserialize, Serialize};

/// Reconciliation inputs of one snapshot. It is keyed by the snapshot identifier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotEvidence {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub run_id: String,
    pub cutoff_unix_ms: u64,
    pub observations: Vec<ObservationRecord>,
    pub intervals: Vec<ElapsedInterval>,
}

impl StoredRecord for SnapshotEvidence {
    const KIND: RecordKind = RecordKind::Evidence;

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
