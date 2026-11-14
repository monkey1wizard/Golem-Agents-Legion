use super::EvidenceRecord;
use crate::usage::{ManifestPage, Publication, StoreError, UsageStore};
use std::path::Path;

/// Immutable governance events, separate from the usage store.
#[derive(Debug, Clone)]
pub struct EvidenceStore(UsageStore);

impl EvidenceStore {
    pub fn open(common_dir: &Path) -> Result<Self, StoreError> {
        UsageStore::open_with_root(common_dir, ["gal", "evidence", "v1"]).map(Self)
    }
    pub fn publish(&self, record: &EvidenceRecord) -> Result<Publication, StoreError> {
        self.0.publish(record)
    }
    pub fn read(&self, run_id: &str, event_id: &str) -> Result<EvidenceRecord, StoreError> {
        self.0.read(run_id, event_id)
    }
    pub fn list_page(
        &self,
        run_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ManifestPage, StoreError> {
        self.0.list_page::<EvidenceRecord>(run_id, after, limit)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrozenInputs {
    pub run_id: String,
    pub evidence_ids: std::collections::BTreeMap<String, String>,
    pub usage_evidence_ids: std::collections::BTreeMap<String, String>,
    pub snapshot_ids: std::collections::BTreeMap<String, String>,
    pub observations: Vec<crate::usage::ObservationRecord>,
    pub intervals: Vec<crate::usage::ElapsedInterval>,
}

#[derive(Debug, thiserror::Error)]
pub enum FreezeError {
    #[error("integrity-failure: {0}")]
    Integrity(String),
    #[error("{0}")]
    Store(#[from] StoreError),
}

pub fn freeze_inputs(common_dir: &Path, run_id: &str) -> Result<FrozenInputs, FreezeError> {
    use crate::usage::{SnapshotEvidence, UsageSnapshot};
    let evidence = EvidenceStore::open(common_dir)?;
    let usage = UsageStore::open(common_dir)?;
    let mut frozen = FrozenInputs {
        run_id: run_id.to_owned(),
        evidence_ids: Default::default(),
        usage_evidence_ids: Default::default(),
        snapshot_ids: Default::default(),
        observations: Vec::new(),
        intervals: Vec::new(),
    };
    let mut cursor = None;
    loop {
        let page = evidence.list_page(run_id, cursor.as_deref(), 256)?;
        for entry in page.entries {
            evidence.read(run_id, &entry.record_id).map_err(integrity)?;
            frozen.evidence_ids.insert(entry.record_id, entry.digest);
        }
        cursor = page.next_after;
        if cursor.is_none() {
            break;
        }
    }
    let mut cursor = None;
    let mut observations = std::collections::BTreeMap::new();
    let mut intervals = std::collections::BTreeMap::new();
    loop {
        let page = usage.list_page::<SnapshotEvidence>(run_id, cursor.as_deref(), 256)?;
        for entry in page.entries {
            let record = usage
                .read::<SnapshotEvidence>(run_id, &entry.record_id)
                .map_err(integrity)?;
            for observation in record.observations {
                match observations.entry(observation.observation_id.clone()) {
                    std::collections::btree_map::Entry::Vacant(e) => {
                        e.insert(observation);
                    }
                    std::collections::btree_map::Entry::Occupied(e) if e.get() == &observation => {}
                    std::collections::btree_map::Entry::Occupied(e) => {
                        return Err(FreezeError::Integrity(e.key().clone()))
                    }
                }
            }
            for interval in record.intervals {
                match intervals.entry(interval.attempt_id.clone()) {
                    std::collections::btree_map::Entry::Vacant(e) => {
                        e.insert(interval);
                    }
                    std::collections::btree_map::Entry::Occupied(e) if e.get() == &interval => {}
                    std::collections::btree_map::Entry::Occupied(e) => {
                        return Err(FreezeError::Integrity(e.key().clone()))
                    }
                }
            }
            frozen
                .usage_evidence_ids
                .insert(entry.record_id, entry.digest);
        }
        cursor = page.next_after;
        if cursor.is_none() {
            break;
        }
    }
    let mut cursor = None;
    loop {
        let page = usage.list_page::<UsageSnapshot>(run_id, cursor.as_deref(), 256)?;
        for entry in page.entries {
            usage
                .rebuild_reconciliation(run_id, &entry.record_id)
                .map_err(integrity)?;
            frozen.snapshot_ids.insert(entry.record_id, entry.digest);
        }
        cursor = page.next_after;
        if cursor.is_none() {
            break;
        }
    }
    frozen.observations = observations.into_values().collect();
    frozen.intervals = intervals.into_values().collect();
    Ok(frozen)
}

fn integrity(error: StoreError) -> FreezeError {
    FreezeError::Integrity(error.to_string())
}

impl FrozenInputs {
    pub fn verify(&self, common_dir: &Path) -> Result<(), FreezeError> {
        use crate::usage::store::record_digest;
        use crate::usage::{SnapshotEvidence, UsageSnapshot};
        let evidence = EvidenceStore::open(common_dir)?;
        let usage = UsageStore::open(common_dir)?;
        for (id, digest) in &self.evidence_ids {
            let record = evidence.read(&self.run_id, id).map_err(integrity)?;
            if record_digest(&record)? != *digest {
                return Err(FreezeError::Integrity(id.clone()));
            }
        }
        for (id, digest) in &self.usage_evidence_ids {
            let record = usage
                .read::<SnapshotEvidence>(&self.run_id, id)
                .map_err(integrity)?;
            if record_digest(&record)? != *digest {
                return Err(FreezeError::Integrity(id.clone()));
            }
        }
        for (id, digest) in &self.snapshot_ids {
            let record = usage
                .read::<UsageSnapshot>(&self.run_id, id)
                .map_err(integrity)?;
            if record_digest(&record)? != *digest {
                return Err(FreezeError::Integrity(id.clone()));
            }
            usage
                .rebuild_reconciliation(&self.run_id, id)
                .map_err(integrity)?;
        }
        Ok(())
    }
    pub fn reconcile(
        &self,
        cutoff_unix_ms: u64,
    ) -> Result<crate::usage::Reconciliation, crate::usage::ReconcileError> {
        crate::usage::reconcile::reconcile(crate::usage::ReconcileInput {
            run_id: self.run_id.clone(),
            cutoff_unix_ms,
            observations: self.observations.clone(),
            intervals: self.intervals.clone(),
        })
    }
}
