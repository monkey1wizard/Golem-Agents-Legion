//! Immutable governance evidence in the Git common directory.
pub mod evidence;
pub mod store;

pub use evidence::{
    artifact_snapshot, criterion_status, read_design_roster_version, ArtifactSnapshot,
    ArtifactState, ArtifactValidity, CriterionBinding, CriterionDeclaration, CriterionMapping,
    CriterionPayload, CriterionRow, CriterionSecurity, CriterionSource, CriterionStatus,
    CriterionTestPlan, DesignRosterVersion, EvidenceRecord,
};
pub use store::{freeze_inputs, EvidenceStore, FreezeError, FrozenInputs};
