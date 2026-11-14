//! Durable, deterministic coordinator state for the Codex guarded continuation.
//!
//! This module contains no filesystem or process I/O. The CLI owns persistence
//! and supplies ownership observations before reducing an event.

use crate::usage::binding::{RunBindingOutcome, RunIdentity};
use dispatch::{host_attempt_state::HostTerminalState, host_session::HostSessionRef};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Execution compatibility profile. Host detection must happen outside this
/// reducer through the trusted hook handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ContinuationProfile {
    #[default]
    LegacyInteractive,
    CodexStopV1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "binding")]
pub enum NextAction {
    StartTask {
        task_id: String,
        phase: String,
    },
    ResumeTask {
        task_id: String,
        phase: String,
    },
    AwaitOrchestrator {
        checkpoint_id: String,
    },
    RecoverAttempt {
        attempt_id: String,
    },
    RetryTask {
        task_id: String,
        phase: String,
        attempt: u32,
    },
    Complete,
    Blocked {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttemptOwnership {
    Live,
    Dead,
    Unknown,
    NotApplicable,
}

/// Bounded process observation supplied by the CLI's injected recovery adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerObservation {
    pub verdict: AttemptOwnership,
    pub observed_pid: Option<u32>,
    pub observed_birth: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveAttempt {
    pub attempt_id: String,
    pub task_id: String,
    pub phase: String,
    pub process_id: Option<u32>,
    pub process_birth: Option<String>,
    pub dispatch_claimed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_binding: Option<RouteBinding>,
}

/// Versioned digest identifying the route selected for one attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteBinding {
    pub version: u32,
    pub digest: String,
}

impl RouteBinding {
    const VERSION: u32 = 1;

    fn validate(&self) -> Result<(), CoordinatorError> {
        if self.version != Self::VERSION
            || self.digest.len() != 64
            || !self.digest.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(CoordinatorError::InvalidTransition(
                "route binding is invalid",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticCheckpoint {
    pub checkpoint_id: String,
    pub revision: u64,
    pub task_id: String,
    pub phase: String,
    pub prompt_sha256: String,
    pub execution_binding_sha256: String,
    pub commit: Option<String>,
    pub kind: CheckpointKind,
    pub consumed_receipt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostCheckpointIdentity {
    pub checkpoint_id: String,
    pub checkpoint_revision: u64,
    pub run_id: String,
    pub attempt_id: String,
    pub session_ref: Option<String>,
    pub terminal_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_binding: Option<RouteBinding>,
}

/// Typed request checked under the caller's coordinator revision lock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResumeHostSubagent {
    pub checkpoint_id: String,
    pub expected_revision: u64,
    pub run_id: String,
    pub task_id: String,
    pub phase: String,
    pub attempt_id: String,
    pub session_ref: String,
    pub prompt_sha256: String,
    pub execution_binding_sha256: String,
    pub commit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub route_binding: Option<RouteBinding>,
}

/// In-memory authority to close one validated checkpoint. Never deserialize it.
#[derive(Debug)]
pub struct HostCloseCustody {
    binding: ResumeHostSubagent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
    HostSubagent,
    TaskQuality,
    BoundaryWidening,
    GoalBackward,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointReceipt {
    pub receipt_id: String,
    pub checkpoint_id: String,
    pub revision: u64,
    pub prompt_sha256: String,
    pub execution_binding_sha256: String,
    pub commit: Option<String>,
    pub accepted: bool,
}

/// A design stop request bound to the state observed by the caller.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignStopEvent {
    pub expected_revision: u64,
    pub reason: String,
    pub active_attempt: Option<ActiveAttempt>,
    pub checkpoint: Option<SemanticCheckpoint>,
    pub host_subagent: Option<HostCheckpointIdentity>,
    pub execution_binding_sha256: String,
}

/// Durable marker that distinguishes a terminal design stop from other blocked causes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignStop {
    pub reason: String,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RetryState {
    pub attempt: u32,
    pub max_attempts: u32,
    pub last_failure: Option<String>,
}

/// Immutable identity for the executable allowed to perform guarded work.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionBinding {
    pub version: u32,
    pub binding_scope: String,
    pub worktree_root: String,
    pub executable_path: String,
    pub executable_sha256: String,
}

impl ExecutionBinding {
    pub const VERSION: u32 = 1;

    /// Hash fixed-order, length-prefixed UTF-8 fields. This encoding does not
    /// depend on locale, JSON key ordering, or platform path separators.
    pub fn digest(&self) -> Result<String, CoordinatorError> {
        if self.version != Self::VERSION
            || [
                self.binding_scope.as_str(),
                self.worktree_root.as_str(),
                self.executable_path.as_str(),
                self.executable_sha256.as_str(),
            ]
            .iter()
            .any(|v| v.is_empty())
        {
            return Err(CoordinatorError::InvalidTransition(
                "execution binding is invalid",
            ));
        }
        let mut hash = Sha256::new();
        hash.update(b"GAL-EXECUTION-BINDING\0");
        hash.update(self.version.to_be_bytes());
        for value in [
            &self.binding_scope,
            &self.worktree_root,
            &self.executable_path,
            &self.executable_sha256,
        ] {
            let bytes = value.as_bytes();
            hash.update((bytes.len() as u64).to_be_bytes());
            hash.update(bytes);
        }
        Ok(format!("{:x}", hash.finalize()))
    }
}

/// Complete serializable state persisted by the plan-scoped CLI driver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoordinatorState {
    pub schema_version: u32,
    pub revision: u64,
    pub profile: ContinuationProfile,
    pub plan_scope: String,
    pub prompt_sha256: String,
    pub task_id: String,
    pub phase: String,
    pub active_attempt: Option<ActiveAttempt>,
    pub retry: RetryState,
    pub last_verified_evidence: Option<String>,
    pub pending_projection_id: Option<String>,
    pub checkpoint: Option<SemanticCheckpoint>,
    #[serde(default)]
    pub host_subagent: Option<HostCheckpointIdentity>,
    pub activation_digest: Option<String>,
    #[serde(default)]
    pub execution_binding: Option<ExecutionBinding>,
    #[serde(default)]
    pub pinned_host: Option<String>,
    #[serde(default)]
    pub pinned_mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub design_stop: Option<DesignStop>,
    /// Accounting run this state belongs to. Legacy state has none, and the
    /// key is omitted while unset, so existing state bytes do not change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accounting_run: Option<RunIdentity>,
    pub next_action: NextAction,
    pub consumed_checkpoint_receipts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CoordinatorError {
    InvalidTransition(&'static str),
    CheckpointMissing,
    CheckpointMismatch,
    ReceiptReplay,
    ReceiptBindingMismatch,
    AttemptMissing,
    DispatchAlreadyClaimed,
    BindingMissing,
    UnsafeRebind,
    AccountingRunConflict,
    DesignStopMismatch,
    DesignStopActive,
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CoordinatorError {}

impl CoordinatorState {
    pub const SCHEMA_VERSION: u32 = 2;

    fn reject_design_stop(&self) -> Result<(), CoordinatorError> {
        if self.design_stop.is_some() {
            Err(CoordinatorError::DesignStopActive)
        } else {
            Ok(())
        }
    }

    pub fn new_legacy(
        plan_scope: impl Into<String>,
        prompt_sha256: impl Into<String>,
        task_id: impl Into<String>,
        phase: impl Into<String>,
    ) -> Self {
        let task_id = task_id.into();
        let phase = phase.into();
        Self {
            schema_version: Self::SCHEMA_VERSION,
            revision: 0,
            profile: ContinuationProfile::LegacyInteractive,
            plan_scope: plan_scope.into(),
            prompt_sha256: prompt_sha256.into(),
            task_id: task_id.clone(),
            phase: phase.clone(),
            active_attempt: None,
            retry: RetryState::default(),
            last_verified_evidence: None,
            pending_projection_id: None,
            checkpoint: None,
            host_subagent: None,
            activation_digest: None,
            execution_binding: None,
            pinned_host: None,
            pinned_mode: None,
            design_stop: None,
            accounting_run: None,
            next_action: NextAction::StartTask { task_id, phase },
            consumed_checkpoint_receipts: Vec::new(),
        }
    }

    /// Upgrade a legacy serialized state in memory. Active legacy work remains
    /// unbound and therefore cannot enter guarded operations.
    pub fn migrate_schema(&mut self) -> Result<(), CoordinatorError> {
        if self.schema_version > Self::SCHEMA_VERSION {
            return Err(CoordinatorError::InvalidTransition(
                "unsupported coordinator schema",
            ));
        }
        self.schema_version = Self::SCHEMA_VERSION;
        Ok(())
    }

    /// Initialize or safely replace the binding at guarded entry. Callers must
    /// only use this transition after validating the executable identity.
    pub fn guarded_entry(&mut self, binding: ExecutionBinding) -> Result<String, CoordinatorError> {
        self.reject_design_stop()?;
        let digest = binding.digest()?;
        let safe = self.active_attempt.is_none()
            && self
                .checkpoint
                .as_ref()
                .is_none_or(|c| c.consumed_receipt_id.is_some())
            && self.pending_projection_id.is_none();
        if !safe {
            return Err(CoordinatorError::UnsafeRebind);
        }
        if self.execution_binding.as_ref() != Some(&binding) {
            self.execution_binding = Some(binding);
            self.revision = self.revision.saturating_add(1);
        }
        self.schema_version = Self::SCHEMA_VERSION;
        Ok(digest)
    }

    /// Accounting run recorded for this state, if one was bound.
    pub fn accounting_run_id(&self) -> Option<&str> {
        self.accounting_run.as_ref().map(|run| run.run_id.as_str())
    }

    /// Record the accounting run for this execution. The first call allocates
    /// it and advances the revision. A resume presents the same identity, which
    /// changes nothing. Any other identity conflicts with the recorded run and
    /// leaves the state unchanged.
    pub fn bind_accounting_run(
        &mut self,
        identity: RunIdentity,
    ) -> Result<RunBindingOutcome, CoordinatorError> {
        self.reject_design_stop()?;
        identity.validate().map_err(|_| {
            CoordinatorError::InvalidTransition("accounting run identity is invalid")
        })?;
        match &self.accounting_run {
            None => {
                self.accounting_run = Some(identity);
                self.revision = self.revision.saturating_add(1);
                Ok(RunBindingOutcome::Allocated)
            }
            Some(recorded) if *recorded == identity => Ok(RunBindingOutcome::Reused),
            Some(_) => Err(CoordinatorError::AccountingRunConflict),
        }
    }

    fn execution_binding_digest(&self) -> Result<String, CoordinatorError> {
        self.execution_binding
            .as_ref()
            .ok_or(CoordinatorError::BindingMissing)?
            .digest()
    }

    /// Bind a design stop to the complete coordinator state observed by the caller.
    pub fn bind_design_stop(
        &self,
        reason: impl Into<String>,
    ) -> Result<DesignStopEvent, CoordinatorError> {
        Ok(DesignStopEvent {
            expected_revision: self.revision,
            reason: reason.into(),
            active_attempt: self.active_attempt.clone(),
            checkpoint: self.checkpoint.clone(),
            host_subagent: self.host_subagent.clone(),
            execution_binding_sha256: self.execution_binding_digest()?,
        })
    }

    /// Persist a terminal design stop in the pure state reducer.
    pub fn record_design_stop(&mut self, event: DesignStopEvent) -> Result<u64, CoordinatorError> {
        if self.design_stop.is_some() {
            return Err(CoordinatorError::DesignStopActive);
        }
        if event.reason.trim().is_empty() {
            return Err(CoordinatorError::InvalidTransition(
                "design stop reason is empty",
            ));
        }
        if event.expected_revision != self.revision
            || event.active_attempt != self.active_attempt
            || event.checkpoint != self.checkpoint
            || event.host_subagent != self.host_subagent
        {
            return Err(CoordinatorError::DesignStopMismatch);
        }
        if event.execution_binding_sha256 != self.execution_binding_digest()? {
            return Err(CoordinatorError::ReceiptBindingMismatch);
        }

        self.revision = self.revision.saturating_add(1);
        self.design_stop = Some(DesignStop {
            reason: event.reason.clone(),
            revision: self.revision,
        });
        self.next_action = NextAction::Blocked {
            reason: event.reason,
        };
        Ok(self.revision)
    }

    /// Select guarded mode only from a caller-validated activation digest.
    pub fn activate_codex_stop_v1(
        &mut self,
        activation_digest: String,
    ) -> Result<(), CoordinatorError> {
        self.reject_design_stop()?;
        if activation_digest.trim().is_empty() {
            return Err(CoordinatorError::InvalidTransition(
                "activation digest is empty",
            ));
        }
        self.profile = ContinuationProfile::CodexStopV1;
        self.activation_digest = Some(activation_digest);
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    /// Claim an attempt before dispatch. Persist this revision before the CLI
    /// performs the external dispatch, preventing restart from dispatching twice.
    pub fn claim_attempt(&mut self, attempt_id: String) -> Result<u64, CoordinatorError> {
        self.claim_attempt_with_route(attempt_id, None)
    }

    pub fn claim_attempt_routed(
        &mut self,
        attempt_id: String,
        route: RouteBinding,
    ) -> Result<u64, CoordinatorError> {
        route.validate()?;
        self.claim_attempt_with_route(attempt_id, Some(route))
    }

    fn claim_attempt_with_route(
        &mut self,
        attempt_id: String,
        route_binding: Option<RouteBinding>,
    ) -> Result<u64, CoordinatorError> {
        self.reject_design_stop()?;
        if self.profile == ContinuationProfile::CodexStopV1 {
            self.execution_binding_digest()?;
        }
        if self
            .active_attempt
            .as_ref()
            .is_some_and(|a| a.dispatch_claimed)
        {
            return Err(CoordinatorError::DispatchAlreadyClaimed);
        }
        if let Some(active) = self.active_attempt.as_ref() {
            if active.route_binding != route_binding {
                return Err(CoordinatorError::ReceiptBindingMismatch);
            }
        }
        self.active_attempt = Some(ActiveAttempt {
            attempt_id,
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
            process_id: None,
            process_birth: None,
            dispatch_claimed: true,
            route_binding,
        });
        self.next_action = NextAction::ResumeTask {
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
        };
        self.revision = self.revision.saturating_add(1);
        Ok(self.revision)
    }

    pub fn bind_process(
        &mut self,
        process_id: u32,
        process_birth: Option<String>,
    ) -> Result<u64, CoordinatorError> {
        self.reject_design_stop()?;
        self.reject_pending_host_work()?;
        let attempt = self
            .active_attempt
            .as_mut()
            .ok_or(CoordinatorError::AttemptMissing)?;
        attempt.process_id = Some(process_id);
        attempt.process_birth = process_birth;
        self.revision = self.revision.saturating_add(1);
        Ok(self.revision)
    }

    /// Mark the claimed dispatch terminal after the CLI has recorded its outcome.
    pub fn finish_attempt(
        &mut self,
        attempt_id: &str,
        verified_evidence: Option<String>,
    ) -> Result<u64, CoordinatorError> {
        self.reject_design_stop()?;
        self.reject_pending_host_work()?;
        if self.profile == ContinuationProfile::CodexStopV1 {
            self.execution_binding_digest()?;
        }
        let active = self
            .active_attempt
            .as_ref()
            .ok_or(CoordinatorError::AttemptMissing)?;
        if let Some(route) = &active.route_binding {
            route.validate()?;
        }
        if active.attempt_id != attempt_id {
            return Err(CoordinatorError::InvalidTransition(
                "attempt identity mismatch",
            ));
        }
        self.active_attempt = None;
        if let Some(evidence) = verified_evidence {
            self.last_verified_evidence = Some(evidence);
        }
        self.revision = self.revision.saturating_add(1);
        Ok(self.revision)
    }

    /// Resolve a persisted attempt without ever reclaiming a live or uncertain owner.
    pub fn recover_owner(
        &mut self,
        observation: OwnerObservation,
    ) -> Result<AttemptOwnership, CoordinatorError> {
        self.reject_design_stop()?;
        if self.has_pending_host_checkpoint() {
            let active_route = self
                .active_attempt
                .as_ref()
                .and_then(|attempt| attempt.route_binding.as_ref());
            let checkpoint_route = self
                .host_subagent
                .as_ref()
                .and_then(|identity| identity.route_binding.as_ref());
            if let Some(route) = active_route {
                route.validate()?;
            }
            if let Some(route) = checkpoint_route {
                route.validate()?;
            }
            if active_route != checkpoint_route {
                return Err(CoordinatorError::ReceiptBindingMismatch);
            }
            return Ok(AttemptOwnership::NotApplicable);
        }
        if self.profile == ContinuationProfile::CodexStopV1 {
            self.execution_binding_digest()?;
        }
        let attempt = self
            .active_attempt
            .as_ref()
            .ok_or(CoordinatorError::AttemptMissing)?;
        if let Some(route) = &attempt.route_binding {
            route.validate()?;
        }
        let ownership = match observation.verdict {
            AttemptOwnership::Live
                if attempt.process_id == observation.observed_pid
                    && attempt.process_birth == observation.observed_birth =>
            {
                AttemptOwnership::Live
            }
            AttemptOwnership::Live => AttemptOwnership::Unknown,
            other => other,
        };
        match ownership {
            AttemptOwnership::Live => {
                self.next_action = NextAction::ResumeTask {
                    task_id: attempt.task_id.clone(),
                    phase: attempt.phase.clone(),
                }
            }
            AttemptOwnership::Dead => {
                self.next_action = NextAction::RecoverAttempt {
                    attempt_id: attempt.attempt_id.clone(),
                }
            }
            AttemptOwnership::Unknown => {
                self.next_action = NextAction::Blocked {
                    reason: "attempt-owner-unknown".into(),
                }
            }
            AttemptOwnership::NotApplicable => {
                return Err(CoordinatorError::InvalidTransition(
                    "owner observation is not applicable",
                ))
            }
        }
        self.revision = self.revision.saturating_add(1);
        Ok(ownership)
    }

    pub fn checkpoint(
        &mut self,
        checkpoint_id: String,
        kind: CheckpointKind,
        commit: Option<String>,
    ) -> Result<u64, CoordinatorError> {
        self.reject_design_stop()?;
        self.reject_pending_host_work()?;
        if kind == CheckpointKind::HostSubagent {
            return Err(CoordinatorError::InvalidTransition(
                "host checkpoint requires identity",
            ));
        }
        if self.profile != ContinuationProfile::CodexStopV1 {
            return Err(CoordinatorError::InvalidTransition(
                "semantic checkpoint requires CodexStopV1",
            ));
        }
        if let Some(route) = self
            .active_attempt
            .as_ref()
            .and_then(|attempt| attempt.route_binding.as_ref())
        {
            route.validate()?;
        }
        let execution_binding_sha256 = self.execution_binding_digest()?;
        self.revision = self.revision.saturating_add(1);
        self.checkpoint = Some(SemanticCheckpoint {
            checkpoint_id: checkpoint_id.clone(),
            revision: self.revision,
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
            prompt_sha256: self.prompt_sha256.clone(),
            execution_binding_sha256,
            commit,
            kind,
            consumed_receipt_id: None,
        });
        self.host_subagent = None;
        self.next_action = NextAction::AwaitOrchestrator { checkpoint_id };
        Ok(self.revision)
    }

    pub fn has_pending_host_checkpoint(&self) -> bool {
        self.checkpoint.as_ref().is_some_and(|c| {
            c.kind == CheckpointKind::HostSubagent && c.consumed_receipt_id.is_none()
        })
    }

    fn reject_pending_host_work(&self) -> Result<(), CoordinatorError> {
        if self.has_pending_host_checkpoint() {
            return Err(CoordinatorError::InvalidTransition(
                "host checkpoint requires typed resume",
            ));
        }
        Ok(())
    }

    /// Publish after claiming the host attempt and before returning to the host.
    pub fn checkpoint_host_subagent(
        &mut self,
        checkpoint_id: String,
        run_id: String,
        commit: Option<String>,
    ) -> Result<u64, CoordinatorError> {
        self.reject_design_stop()?;
        self.reject_pending_host_work()?;
        let active = self
            .active_attempt
            .as_ref()
            .ok_or(CoordinatorError::AttemptMissing)?;
        if let Some(route) = &active.route_binding {
            route.validate()?;
        }
        if self.profile != ContinuationProfile::CodexStopV1
            || self.pinned_mode.as_deref() != Some("host-subagent")
            || !matches!(self.pinned_host.as_deref(), Some("codex" | "claude"))
            || checkpoint_id.trim().is_empty()
            || run_id.trim().is_empty()
            || active.attempt_id.trim().is_empty()
            || !active.dispatch_claimed
            || active.task_id != self.task_id
            || active.phase != self.phase
            || self
                .checkpoint
                .as_ref()
                .is_some_and(|c| c.consumed_receipt_id.is_none())
        {
            return Err(CoordinatorError::ReceiptBindingMismatch);
        }
        let execution_binding_sha256 = self.execution_binding_digest()?;
        let identity = HostCheckpointIdentity {
            checkpoint_id: checkpoint_id.clone(),
            checkpoint_revision: self.revision.saturating_add(1),
            run_id,
            attempt_id: active.attempt_id.clone(),
            session_ref: None,
            terminal_digest: None,
            route_binding: active.route_binding.clone(),
        };
        self.revision = self.revision.saturating_add(1);
        self.checkpoint = Some(SemanticCheckpoint {
            checkpoint_id: checkpoint_id.clone(),
            revision: self.revision,
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
            prompt_sha256: self.prompt_sha256.clone(),
            execution_binding_sha256,
            commit,
            kind: CheckpointKind::HostSubagent,
            consumed_receipt_id: None,
        });
        self.host_subagent = Some(identity);
        self.next_action = NextAction::AwaitOrchestrator { checkpoint_id };
        Ok(self.revision)
    }

    /// Validate custody before any close side effect. The caller also validates
    /// the immutable manifest and holds the revision lock through persistence.
    pub fn resume_host_subagent(
        &self,
        binding: ResumeHostSubagent,
    ) -> Result<HostCloseCustody, CoordinatorError> {
        self.reject_design_stop()?;
        let checkpoint = self
            .checkpoint
            .as_ref()
            .ok_or(CoordinatorError::CheckpointMissing)?;
        if checkpoint.consumed_receipt_id.is_some() {
            return Err(CoordinatorError::ReceiptReplay);
        }
        if checkpoint.kind != CheckpointKind::HostSubagent
            || binding.checkpoint_id != checkpoint.checkpoint_id
            || binding.expected_revision != self.revision
            || checkpoint.revision > self.revision
            || self.next_action
                != (NextAction::AwaitOrchestrator {
                    checkpoint_id: binding.checkpoint_id.clone(),
                })
        {
            return Err(CoordinatorError::CheckpointMismatch);
        }
        let identity = self
            .host_subagent
            .as_ref()
            .ok_or(CoordinatorError::CheckpointMismatch)?;
        if identity.checkpoint_id != checkpoint.checkpoint_id
            || identity.checkpoint_revision != checkpoint.revision
        {
            return Err(CoordinatorError::CheckpointMismatch);
        }
        let active = self
            .active_attempt
            .as_ref()
            .ok_or(CoordinatorError::AttemptMissing)?;
        if let Some(route) = &active.route_binding {
            route.validate()?;
        }
        if identity.route_binding != active.route_binding {
            return Err(CoordinatorError::CheckpointMismatch);
        }
        let session = HostSessionRef::parse(&binding.session_ref)
            .map_err(|_| CoordinatorError::ReceiptBindingMismatch)?;
        if self.profile != ContinuationProfile::CodexStopV1
            || self.pinned_mode.as_deref() != Some("host-subagent")
            || self.pinned_host.as_deref() != Some(session.host.as_str())
            || binding.run_id != identity.run_id
            || binding.attempt_id != identity.attempt_id
            || binding.attempt_id != active.attempt_id
            || !active.dispatch_claimed
            || binding.task_id != self.task_id
            || binding.task_id != checkpoint.task_id
            || binding.task_id != active.task_id
            || binding.phase != self.phase
            || binding.phase != checkpoint.phase
            || binding.phase != active.phase
            || binding.prompt_sha256 != self.prompt_sha256
            || binding.prompt_sha256 != checkpoint.prompt_sha256
            || binding.execution_binding_sha256 != checkpoint.execution_binding_sha256
            || binding.execution_binding_sha256 != self.execution_binding_digest()?
            || binding.commit != checkpoint.commit
            || binding.route_binding != identity.route_binding
            || binding.route_binding != active.route_binding
            || identity
                .session_ref
                .as_ref()
                .is_some_and(|s| s != &binding.session_ref)
        {
            return Err(CoordinatorError::ReceiptBindingMismatch);
        }
        Ok(HostCloseCustody { binding })
    }

    /// Attach only evidence already validated by the CLI close transaction.
    /// Persisting this step permits exact recovery without repeating placement.
    pub fn attach_host_subagent_terminal(
        &mut self,
        custody: HostCloseCustody,
        terminal: HostTerminalState,
        terminal_digest: String,
    ) -> Result<HostCloseCustody, CoordinatorError> {
        self.reject_design_stop()?;
        let mut custody = self.resume_host_subagent(custody.binding)?;
        let identity = self.host_subagent.as_mut().unwrap();
        if identity
            .terminal_digest
            .as_ref()
            .is_some_and(|d| d != &terminal_digest)
            || terminal_digest.len() != 64
            || !terminal_digest.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(CoordinatorError::ReceiptBindingMismatch);
        }
        if terminal != HostTerminalState::Completed {
            self.retry.last_failure = Some(terminal.as_str().into());
            self.next_action = NextAction::Blocked {
                reason: format!("host-close:{}", terminal.as_str()),
            };
            self.revision = self.revision.saturating_add(1);
            return Err(CoordinatorError::InvalidTransition(
                "host close did not complete",
            ));
        }
        identity.session_ref = Some(custody.binding.session_ref.clone());
        identity.terminal_digest = Some(terminal_digest);
        self.revision = self.revision.saturating_add(1);
        custody.binding.expected_revision = self.revision;
        Ok(custody)
    }

    pub fn consume_host_subagent_checkpoint(
        &mut self,
        custody: HostCloseCustody,
        terminal_digest: &str,
    ) -> Result<u64, CoordinatorError> {
        self.reject_design_stop()?;
        let custody = self.resume_host_subagent(custody.binding)?;
        let checkpoint = self.checkpoint.as_mut().unwrap();
        if self
            .host_subagent
            .as_ref()
            .and_then(|h| h.terminal_digest.as_deref())
            != Some(terminal_digest)
        {
            return Err(CoordinatorError::ReceiptBindingMismatch);
        }
        let receipt_id = format!(
            "host-terminal:{}:{terminal_digest}",
            custody.binding.attempt_id
        );
        if self.consumed_checkpoint_receipts.contains(&receipt_id) {
            return Err(CoordinatorError::ReceiptReplay);
        }
        checkpoint.consumed_receipt_id = Some(receipt_id.clone());
        self.consumed_checkpoint_receipts.push(receipt_id.clone());
        self.last_verified_evidence = Some(receipt_id);
        self.active_attempt = None;
        self.next_action = NextAction::ResumeTask {
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
        };
        self.revision = self.revision.saturating_add(1);
        Ok(self.revision)
    }

    /// Consume a receipt once, bound to the exact checkpoint revision and evidence.
    pub fn consume_checkpoint_receipt(
        &mut self,
        receipt: CheckpointReceipt,
    ) -> Result<u64, CoordinatorError> {
        self.reject_design_stop()?;
        self.reject_pending_host_work()?;
        if self
            .consumed_checkpoint_receipts
            .contains(&receipt.receipt_id)
        {
            return Err(CoordinatorError::ReceiptReplay);
        }
        let current_binding_digest = self.execution_binding_digest()?;
        let checkpoint = self
            .checkpoint
            .as_mut()
            .ok_or(CoordinatorError::CheckpointMissing)?;
        if checkpoint.consumed_receipt_id.is_some() {
            return Err(CoordinatorError::ReceiptReplay);
        }
        if checkpoint.checkpoint_id != receipt.checkpoint_id
            || checkpoint.revision != receipt.revision
        {
            return Err(CoordinatorError::CheckpointMismatch);
        }
        if checkpoint.prompt_sha256 != receipt.prompt_sha256 || checkpoint.commit != receipt.commit
        {
            return Err(CoordinatorError::ReceiptBindingMismatch);
        }
        if checkpoint.execution_binding_sha256 != receipt.execution_binding_sha256
            || current_binding_digest != receipt.execution_binding_sha256
        {
            return Err(CoordinatorError::ReceiptBindingMismatch);
        }
        if !receipt.accepted {
            self.retry.last_failure = Some("orchestrator-rejected-checkpoint".into());
            self.next_action = NextAction::RetryTask {
                task_id: self.task_id.clone(),
                phase: self.phase.clone(),
                attempt: self.retry.attempt.saturating_add(1),
            };
        } else {
            self.last_verified_evidence = Some(format!(
                "checkpoint:{};receipt:{}",
                checkpoint.checkpoint_id, receipt.receipt_id
            ));
            self.next_action = NextAction::ResumeTask {
                task_id: self.task_id.clone(),
                phase: self.phase.clone(),
            };
        }
        checkpoint.consumed_receipt_id = Some(receipt.receipt_id.clone());
        self.consumed_checkpoint_receipts.push(receipt.receipt_id);
        self.revision = self.revision.saturating_add(1);
        Ok(self.revision)
    }

    pub fn record_retry(&mut self, failure: String) -> u64 {
        if self.design_stop.is_some() {
            return self.revision;
        }
        if self.has_pending_host_checkpoint() {
            self.next_action = NextAction::Blocked {
                reason: "host-checkpoint-pending".into(),
            };
            self.retry.last_failure = Some(failure);
            self.revision = self.revision.saturating_add(1);
            return self.revision;
        }
        self.retry.attempt = self.retry.attempt.saturating_add(1);
        self.retry.last_failure = Some(failure);
        self.revision = self.revision.saturating_add(1);
        if self.retry.max_attempts > 0 && self.retry.attempt >= self.retry.max_attempts {
            self.next_action = NextAction::Blocked {
                reason: "retry-limit-exhausted".into(),
            };
        } else {
            self.next_action = NextAction::RetryTask {
                task_id: self.task_id.clone(),
                phase: self.phase.clone(),
                attempt: self.retry.attempt,
            };
        }
        self.revision
    }

    /// Produce a bounded display string. Input size never controls output size.
    pub fn bounded_status(&self) -> String {
        const LIMIT: usize = 512;
        let action = match &self.next_action {
            NextAction::StartTask { task_id, phase } => {
                format!("start:{}:{}", compact(task_id, 64), compact(phase, 32))
            }
            NextAction::ResumeTask { task_id, phase } => {
                format!("resume:{}:{}", compact(task_id, 64), compact(phase, 32))
            }
            NextAction::AwaitOrchestrator { checkpoint_id } => {
                format!("await:{}", compact(checkpoint_id, 96))
            }
            NextAction::RecoverAttempt { attempt_id } => {
                format!("recover:{}", compact(attempt_id, 96))
            }
            NextAction::RetryTask {
                task_id,
                phase,
                attempt,
            } => format!(
                "retry:{}:{}:{attempt}",
                compact(task_id, 64),
                compact(phase, 32)
            ),
            NextAction::Complete => "complete".into(),
            NextAction::Blocked { reason } => format!("blocked:{}", compact(reason, 96)),
        };
        let mut status = format!(
            "profile={:?} revision={} task={} phase={} action={}",
            self.profile,
            self.revision,
            compact(&self.task_id, 80),
            compact(&self.phase, 40),
            action
        );
        if status.len() > LIMIT {
            let mut boundary = LIMIT;
            while !status.is_char_boundary(boundary) {
                boundary -= 1;
            }
            status.truncate(boundary);
        }
        status
    }
}

fn compact(value: &str, max_chars: usize) -> String {
    let mut out: String = value.chars().take(max_chars).collect();
    if value.chars().count() > max_chars {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod design_stop_tests {
    use super::*;

    fn state_with_checkpoint() -> CoordinatorState {
        let mut state = CoordinatorState::new_legacy("plan", "prompt", "task", "implement");
        state.activate_codex_stop_v1("activation".into()).unwrap();
        state
            .guarded_entry(ExecutionBinding {
                version: ExecutionBinding::VERSION,
                binding_scope: "task".into(),
                worktree_root: "worktree".into(),
                executable_path: "codex".into(),
                executable_sha256: "digest".into(),
            })
            .unwrap();
        state.claim_attempt("attempt".into()).unwrap();
        state
            .checkpoint("checkpoint".into(), CheckpointKind::TaskQuality, None)
            .unwrap();
        state
    }

    fn state_with_host_checkpoint() -> CoordinatorState {
        let mut state = CoordinatorState::new_legacy("plan", "prompt", "task", "implement");
        state.activate_codex_stop_v1("activation".into()).unwrap();
        state
            .guarded_entry(ExecutionBinding {
                version: ExecutionBinding::VERSION,
                binding_scope: "task".into(),
                worktree_root: "worktree".into(),
                executable_path: "codex".into(),
                executable_sha256: "digest".into(),
            })
            .unwrap();
        state.pinned_host = Some("codex".into());
        state.pinned_mode = Some("host-subagent".into());
        state.claim_attempt("attempt".into()).unwrap();
        state
            .checkpoint_host_subagent("host-checkpoint".into(), "run".into(), None)
            .unwrap();
        state
    }

    #[test]
    fn one_shot_route_claim_recovery_and_terminal_expiry() {
        let mut state = CoordinatorState::new_legacy("plan", "prompt", "task", "implement");
        state.activate_codex_stop_v1("activation".into()).unwrap();
        state
            .guarded_entry(ExecutionBinding {
                version: ExecutionBinding::VERSION,
                binding_scope: "task".into(),
                worktree_root: "worktree".into(),
                executable_path: "codex".into(),
                executable_sha256: "digest".into(),
            })
            .unwrap();
        let legacy = serde_json::to_value(&state).unwrap();
        let loaded: CoordinatorState = serde_json::from_value(legacy.clone()).unwrap();
        assert_eq!(serde_json::to_value(loaded).unwrap(), legacy);

        let route = RouteBinding {
            version: 1,
            digest: "a".repeat(64),
        };
        state
            .claim_attempt_routed("attempt".into(), route.clone())
            .unwrap();
        let claimed = serde_json::to_value(&state).unwrap();
        assert_eq!(
            claimed["active_attempt"]["route_binding"]["digest"],
            route.digest
        );

        let ownership = state
            .recover_owner(OwnerObservation {
                verdict: AttemptOwnership::Dead,
                observed_pid: None,
                observed_birth: None,
            })
            .unwrap();
        assert_eq!(ownership, AttemptOwnership::Dead);
        assert_eq!(
            serde_json::to_value(&state).unwrap()["active_attempt"]["route_binding"]["digest"],
            route.digest
        );
        state.finish_attempt("attempt", None).unwrap();
        assert!(serde_json::to_value(&state).unwrap()["active_attempt"].is_null());
        state.claim_attempt("retry".into()).unwrap();
        assert!(serde_json::to_value(&state).unwrap()["active_attempt"]
            .get("route_binding")
            .is_none());

        let mut stale: CoordinatorState = serde_json::from_value(claimed).unwrap();
        let mut stale_json = serde_json::to_value(&stale).unwrap();
        stale_json["active_attempt"]["route_binding"]["digest"] =
            serde_json::Value::String("invalid".into());
        stale = serde_json::from_value(stale_json).unwrap();
        assert_eq!(
            stale.recover_owner(OwnerObservation {
                verdict: AttemptOwnership::Dead,
                observed_pid: None,
                observed_birth: None,
            }),
            Err(CoordinatorError::InvalidTransition(
                "route binding is invalid"
            ))
        );
    }

    #[test]
    fn design_stop_survives_retry_and_checkpoint_consumption() {
        let mut state = state_with_checkpoint();
        let event = state.bind_design_stop("design-defect:blocked").unwrap();
        state.record_design_stop(event).unwrap();
        let stopped = state.clone();

        assert_eq!(
            state.record_retry("ordinary retry".into()),
            stopped.revision
        );
        assert_eq!(state, stopped);
        let checkpoint = stopped.checkpoint.as_ref().unwrap();
        assert_eq!(
            state.consume_checkpoint_receipt(CheckpointReceipt {
                receipt_id: "receipt".into(),
                checkpoint_id: checkpoint.checkpoint_id.clone(),
                revision: checkpoint.revision,
                prompt_sha256: checkpoint.prompt_sha256.clone(),
                execution_binding_sha256: checkpoint.execution_binding_sha256.clone(),
                commit: checkpoint.commit.clone(),
                accepted: true,
            }),
            Err(CoordinatorError::DesignStopActive)
        );
        assert_eq!(state, stopped);
        assert_eq!(
            state.claim_attempt("second-attempt".into()),
            Err(CoordinatorError::DesignStopActive)
        );
        assert_eq!(state, stopped);
        assert!(matches!(
            &state.next_action,
            NextAction::Blocked { reason } if reason == "design-defect:blocked"
        ));
        assert_eq!(
            state.design_stop.as_ref().unwrap().revision,
            stopped.revision
        );
        assert!(state.active_attempt.is_some());
        assert!(state.checkpoint.is_some());
        assert!(state.execution_binding.is_some());
    }

    #[test]
    fn design_stop_rejects_stale_revision_and_wrong_binding_without_mutation() {
        let mut state = state_with_checkpoint();
        let event = state.bind_design_stop("design-defect:blocked").unwrap();
        let before = state.clone();
        let mut stale = event.clone();
        stale.expected_revision = stale.expected_revision.saturating_add(1);
        assert_eq!(
            state.record_design_stop(stale),
            Err(CoordinatorError::DesignStopMismatch)
        );
        assert_eq!(state, before);

        let mut wrong_binding = event;
        wrong_binding.execution_binding_sha256 = "wrong-binding".into();
        assert_eq!(
            state.record_design_stop(wrong_binding),
            Err(CoordinatorError::ReceiptBindingMismatch)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn design_stop_preserves_host_attempt_and_custody_on_retry() {
        let mut state = state_with_host_checkpoint();
        let event = state.bind_design_stop("design-defect:blocked").unwrap();
        state.record_design_stop(event).unwrap();
        let stopped = state.clone();

        assert_eq!(
            state.record_retry("ordinary retry".into()),
            stopped.revision
        );
        assert_eq!(state, stopped);
        assert_eq!(state.active_attempt.as_ref().unwrap().attempt_id, "attempt");
        assert_eq!(
            state.host_subagent.as_ref().unwrap().checkpoint_id,
            "host-checkpoint"
        );
        assert_eq!(state.checkpoint.as_ref().unwrap().consumed_receipt_id, None);
        assert!(state.execution_binding.is_some());
    }

    #[test]
    fn design_stop_state_reads_legacy_serialized_state() {
        let state = state_with_checkpoint();
        let mut value = serde_json::to_value(state).unwrap();
        value.as_object_mut().unwrap().remove("design_stop");
        let decoded: CoordinatorState = serde_json::from_value(value).unwrap();
        assert!(decoded.design_stop.is_none());
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelEvent {
    Claim(String),
    Finish(String),
    BindProcess,
    Observe(OwnerObservation),
    Publish,
    Receipt(CheckpointReceipt),
    CloseHost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelOutcome {
    Revision(Result<u64, CoordinatorError>),
    Ownership(Result<AttemptOwnership, CoordinatorError>),
}

#[derive(Debug, Clone)]
pub struct ModelViolation {
    pub property: &'static str,
    pub initial_state: CoordinatorState,
    pub steps: Vec<String>,
    pub expected: String,
    pub actual: String,
}

impl fmt::Display for ModelViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}: initial={:?}; steps={:?}; expected={}; actual={}",
            self.property, self.initial_state, self.steps, self.expected, self.actual
        )
    }
}
impl std::error::Error for ModelViolation {}

#[derive(Debug, Clone)]
pub struct BoundedModelReport {
    pub attempt_counts: [usize; 4],
    pub checkpoint_counts: [usize; 4],
    pub host_counts: [usize; 4],
    pub witnesses: std::collections::BTreeSet<String>,
    pub elapsed: std::time::Duration,
}

pub fn check_transition_invariants(
    before: &CoordinatorState,
    event: &ModelEvent,
    outcome: &ModelOutcome,
    after: &CoordinatorState,
) -> Result<(), ModelViolation> {
    let mut failure: Option<(&str, String)> = None;
    match event {
        ModelEvent::Claim(_)
            if before
                .active_attempt
                .as_ref()
                .is_some_and(|a| a.dispatch_claimed) =>
        {
            if *outcome != ModelOutcome::Revision(Err(CoordinatorError::DispatchAlreadyClaimed))
                || before != after
            {
                failure = Some(("P1", "duplicate claim rejected with unchanged state".into()));
            }
        }
        ModelEvent::Claim(id) if before.active_attempt.is_none() => {
            if !matches!(outcome, ModelOutcome::Revision(Ok(_)))
                || !after
                    .active_attempt
                    .as_ref()
                    .is_some_and(|a| a.attempt_id == *id && a.dispatch_claimed)
            {
                failure = Some(("P1", "claim succeeds after release".into()));
            }
        }
        ModelEvent::Finish(id)
            if !before.has_pending_host_checkpoint()
                && before
                    .active_attempt
                    .as_ref()
                    .is_some_and(|a| a.attempt_id == *id) =>
        {
            if !matches!(outcome, ModelOutcome::Revision(Ok(_))) || after.active_attempt.is_some() {
                failure = Some(("P1", "completion releases the matching attempt".into()));
            }
        }
        ModelEvent::Observe(observation) => {
            if before.has_pending_host_checkpoint() {
                if *outcome != ModelOutcome::Ownership(Ok(AttemptOwnership::NotApplicable))
                    || before != after
                {
                    failure = Some((
                        "P2",
                        "host checkpoint retains state and returns NotApplicable".into(),
                    ));
                }
            } else if let Some(attempt) = &before.active_attempt {
                let expected = match observation.verdict {
                    AttemptOwnership::Live
                        if attempt.process_id == observation.observed_pid
                            && attempt.process_birth == observation.observed_birth =>
                    {
                        AttemptOwnership::Live
                    }
                    AttemptOwnership::Live => AttemptOwnership::Unknown,
                    other => other,
                };
                let valid = if expected == AttemptOwnership::NotApplicable {
                    *outcome
                        == ModelOutcome::Ownership(Err(CoordinatorError::InvalidTransition(
                            "owner observation is not applicable",
                        )))
                        && before == after
                } else {
                    let action = match expected {
                        AttemptOwnership::Live => NextAction::ResumeTask {
                            task_id: attempt.task_id.clone(),
                            phase: attempt.phase.clone(),
                        },
                        AttemptOwnership::Dead => NextAction::RecoverAttempt {
                            attempt_id: attempt.attempt_id.clone(),
                        },
                        _ => NextAction::Blocked {
                            reason: "attempt-owner-unknown".into(),
                        },
                    };
                    *outcome == ModelOutcome::Ownership(Ok(expected))
                        && after.active_attempt == before.active_attempt
                        && after.next_action == action
                };
                if !valid {
                    failure = Some((
                        "P2",
                        format!("ownership {expected:?}, matching action, and retained attempt"),
                    ));
                }
            } else if *outcome != ModelOutcome::Ownership(Err(CoordinatorError::AttemptMissing))
                || before != after
            {
                failure = Some(("P2", "missing attempt rejected with unchanged state".into()));
            }
        }
        ModelEvent::Receipt(receipt) => {
            let current = before.checkpoint.as_ref();
            let valid = current.is_some_and(|c| {
                c.kind != CheckpointKind::HostSubagent
                    && c.consumed_receipt_id.is_none()
                    && c.checkpoint_id == receipt.checkpoint_id
                    && c.revision == receipt.revision
                    && c.prompt_sha256 == receipt.prompt_sha256
                    && c.execution_binding_sha256 == receipt.execution_binding_sha256
                    && c.commit == receipt.commit
            }) && !before
                .consumed_checkpoint_receipts
                .contains(&receipt.receipt_id);
            let good = if valid {
                matches!(outcome, ModelOutcome::Revision(Ok(_)))
                    && if receipt.accepted {
                        matches!(&after.next_action, NextAction::ResumeTask { .. })
                    } else {
                        matches!(&after.next_action, NextAction::RetryTask { .. })
                    }
                    && after.checkpoint.as_ref().is_some_and(|c| {
                        c.consumed_receipt_id.as_ref() == Some(&receipt.receipt_id)
                    })
                    && after
                        .consumed_checkpoint_receipts
                        .contains(&receipt.receipt_id)
            } else {
                matches!(outcome, ModelOutcome::Revision(Err(_))) && before == after
            };
            if !good {
                failure = Some((
                    "P3",
                    if valid {
                        "consume the current publication once"
                    } else {
                        "reject replay, stale receipt, or host receipt without mutation"
                    }
                    .into(),
                ));
            }
        }
        ModelEvent::CloseHost
            if before
                .checkpoint
                .as_ref()
                .is_some_and(|c| c.consumed_receipt_id.is_some()) =>
        {
            if !matches!(outcome, ModelOutcome::Revision(Err(_))) || before != after {
                failure = Some((
                    "P3",
                    "deny renewed host close authority without mutation".into(),
                ));
            }
        }
        ModelEvent::CloseHost => {
            if !matches!(outcome, ModelOutcome::Revision(Ok(_)))
                || !after
                    .checkpoint
                    .as_ref()
                    .is_some_and(|c| c.consumed_receipt_id.is_some())
                || after.active_attempt.is_some()
            {
                failure = Some((
                    "P3",
                    "close host checkpoint once and release attempt".into(),
                ));
            }
        }
        _ => {}
    }
    match failure {
        Some((property, expected)) => Err(ModelViolation {
            property,
            initial_state: before.clone(),
            steps: vec![format!("{event:?}")],
            expected,
            actual: format!("outcome={outcome:?}; state={after:?}"),
        }),
        None => Ok(()),
    }
}

#[derive(Clone)]
struct ModelBranch {
    state: CoordinatorState,
    publications: Vec<SemanticCheckpoint>,
    steps: Vec<String>,
}

fn model_start() -> CoordinatorState {
    let mut state = CoordinatorState::new_legacy("plan", "prompt", "task", "implement");
    state.activate_codex_stop_v1("activation".into()).unwrap();
    state
        .guarded_entry(ExecutionBinding {
            version: ExecutionBinding::VERSION,
            binding_scope: "task".into(),
            worktree_root: "worktree".into(),
            executable_path: "codex".into(),
            executable_sha256: "digest".into(),
        })
        .unwrap();
    state
}

fn model_observations() -> Vec<OwnerObservation> {
    [
        (AttemptOwnership::Live, Some(1234), Some("birth")),
        (AttemptOwnership::Live, Some(9999), Some("birth")),
        (AttemptOwnership::Live, Some(1234), Some("other")),
        (AttemptOwnership::Dead, None, None),
        (AttemptOwnership::Unknown, None, None),
        (AttemptOwnership::NotApplicable, None, None),
    ]
    .into_iter()
    .map(|(verdict, observed_pid, birth)| OwnerObservation {
        verdict,
        observed_pid,
        observed_birth: birth.map(str::to_owned),
    })
    .collect()
}

fn model_receipt(c: &SemanticCheckpoint, id: &str, accepted: bool) -> CheckpointReceipt {
    CheckpointReceipt {
        receipt_id: id.into(),
        checkpoint_id: c.checkpoint_id.clone(),
        revision: c.revision,
        prompt_sha256: c.prompt_sha256.clone(),
        execution_binding_sha256: c.execution_binding_sha256.clone(),
        commit: c.commit.clone(),
        accepted,
    }
}

fn model_events(group: usize, branch: &ModelBranch) -> Vec<ModelEvent> {
    match group {
        0 => {
            let mut events = vec![
                ModelEvent::Claim("A".into()),
                ModelEvent::Claim("B".into()),
                ModelEvent::Finish("A".into()),
                ModelEvent::Finish("B".into()),
                ModelEvent::BindProcess,
            ];
            events.extend(model_observations().into_iter().map(ModelEvent::Observe));
            events
        }
        1 => {
            let mut events = vec![ModelEvent::Publish];
            for c in branch.publications.iter().rev().take(2) {
                for id in ["receipt-A", "receipt-B"] {
                    for accepted in [true, false] {
                        events.push(ModelEvent::Receipt(model_receipt(c, id, accepted)));
                    }
                }
            }
            events
        }
        _ => {
            let observations = model_observations();
            let mut events: Vec<_> = [0, 3, 4, 5]
                .into_iter()
                .map(|i| ModelEvent::Observe(observations[i].clone()))
                .collect();
            events.push(ModelEvent::Receipt(model_receipt(
                branch.state.checkpoint.as_ref().unwrap(),
                "receipt-A",
                true,
            )));
            events.push(ModelEvent::CloseHost);
            events
        }
    }
}

/// Compare the ordinary checkpoint choices with the canonical event identities.
pub fn check_checkpoint_event_choices(
    publications: &[SemanticCheckpoint],
    choices: &[ModelEvent],
) -> bool {
    let branch = ModelBranch {
        state: model_start(),
        publications: publications.to_vec(),
        steps: Vec::new(),
    };
    let expected = model_events(1, &branch);
    choices.len() == expected.len()
        && expected
            .iter()
            .all(|event| choices.iter().filter(|choice| *choice == event).count() == 1)
}

fn model_apply(state: &mut CoordinatorState, event: &ModelEvent) -> (ModelOutcome, Vec<String>) {
    let mut calls = vec![format!("{event:?}")];
    let result = match event {
        ModelEvent::Claim(id) => ModelOutcome::Revision(state.claim_attempt(id.clone())),
        ModelEvent::Finish(id) => ModelOutcome::Revision(state.finish_attempt(id, None)),
        ModelEvent::BindProcess => {
            ModelOutcome::Revision(state.bind_process(1234, Some("birth".into())))
        }
        ModelEvent::Observe(o) => ModelOutcome::Ownership(state.recover_owner(o.clone())),
        ModelEvent::Publish => ModelOutcome::Revision(state.checkpoint(
            "checkpoint".into(),
            CheckpointKind::TaskQuality,
            None,
        )),
        ModelEvent::Receipt(r) => {
            ModelOutcome::Revision(state.consume_checkpoint_receipt(r.clone()))
        }
        ModelEvent::CloseHost => {
            let c = state.checkpoint.as_ref().unwrap();
            let request = ResumeHostSubagent {
                checkpoint_id: c.checkpoint_id.clone(),
                expected_revision: state.revision,
                run_id: "run".into(),
                task_id: state.task_id.clone(),
                phase: state.phase.clone(),
                attempt_id: "A".into(),
                session_ref: "codex:child-session".into(),
                prompt_sha256: c.prompt_sha256.clone(),
                execution_binding_sha256: c.execution_binding_sha256.clone(),
                commit: c.commit.clone(),
                route_binding: None,
            };
            calls = vec![format!("resume_host_subagent({request:?})")];
            let result = state
                .resume_host_subagent(request)
                .and_then(|custody| {
                    calls.push("attach_host_subagent_terminal(Completed, digest)".into());
                    state.attach_host_subagent_terminal(
                        custody,
                        HostTerminalState::Completed,
                        "a".repeat(64),
                    )
                })
                .and_then(|custody| {
                    calls.push("consume_host_subagent_checkpoint(digest)".into());
                    state.consume_host_subagent_checkpoint(custody, &"a".repeat(64))
                });
            ModelOutcome::Revision(result)
        }
    };
    (result, calls)
}

pub fn verify_bounded_model() -> Result<BoundedModelReport, ModelViolation> {
    let start = std::time::Instant::now();
    let mut report = BoundedModelReport {
        attempt_counts: [0; 4],
        checkpoint_counts: [0; 4],
        host_counts: [0; 4],
        witnesses: Default::default(),
        elapsed: Default::default(),
    };
    for group in 0..3 {
        let mut initial = model_start();
        if group == 2 {
            initial.pinned_host = Some("codex".into());
            initial.pinned_mode = Some("host-subagent".into());
            initial.claim_attempt("A".into()).unwrap();
            initial.bind_process(1234, Some("birth".into())).unwrap();
            initial
                .checkpoint_host_subagent("host".into(), "run".into(), None)
                .unwrap();
        }
        let mut level = vec![ModelBranch {
            state: initial.clone(),
            publications: vec![],
            steps: vec![],
        }];
        for depth in 0..4 {
            let mut next = Vec::new();
            for branch in level {
                let events = model_events(group, &branch);
                let identities: std::collections::HashSet<_> =
                    events.iter().map(|event| format!("{event:?}")).collect();
                let expected = match group {
                    0 => 11,
                    1 => 1 + 4 * branch.publications.len().min(2),
                    _ => 6,
                };
                if events.len() != expected
                    || identities.len() != expected
                    || (group == 1
                        && !check_checkpoint_event_choices(&branch.publications, &events))
                {
                    return Err(ModelViolation {
                        property: "enumeration",
                        initial_state: initial.clone(),
                        steps: branch.steps,
                        expected: format!("{expected} distinct eligible events"),
                        actual: format!("{} events, {} identities", events.len(), identities.len()),
                    });
                }
                for event in events {
                    let mut child = branch.clone();
                    let before = child.state.clone();
                    let (outcome, calls) = model_apply(&mut child.state, &event);
                    if let Err(mut violation) =
                        check_transition_invariants(&before, &event, &outcome, &child.state)
                    {
                        violation.initial_state = initial.clone();
                        violation.steps = child.steps;
                        violation.steps.extend(calls);
                        return Err(violation);
                    }
                    child.steps.extend(calls);
                    child.steps.push(format!("outcome: {outcome:?}"));
                    match (&event, &outcome) {
                        (
                            ModelEvent::Claim(id),
                            ModelOutcome::Revision(Err(CoordinatorError::DispatchAlreadyClaimed)),
                        ) => {
                            let active = before.active_attempt.as_ref().unwrap();
                            report.witnesses.insert(
                                if active.attempt_id == *id {
                                    "claim-same"
                                } else {
                                    "claim-different"
                                }
                                .into(),
                            );
                        }
                        (ModelEvent::Claim(_), ModelOutcome::Revision(Ok(_)))
                            if child.steps.windows(2).any(|s| {
                                s[0].starts_with("Finish")
                                    && s[1].starts_with("outcome: Revision(Ok(")
                            }) =>
                        {
                            report.witnesses.insert("reclaim".into());
                        }
                        (ModelEvent::Observe(o), ModelOutcome::Ownership(Ok(result))) => {
                            report
                                .witnesses
                                .insert(format!("ownership-{:?}-{:?}", o.verdict, result));
                        }
                        (ModelEvent::Receipt(r), ModelOutcome::Revision(Ok(_))) => {
                            report.witnesses.insert(format!("receipt-{}", r.accepted));
                        }
                        (ModelEvent::Receipt(r), ModelOutcome::Revision(Err(_))) => {
                            if before.has_pending_host_checkpoint() {
                                report.witnesses.insert("host-receipt-rejected".into());
                            } else if before
                                .checkpoint
                                .as_ref()
                                .is_some_and(|c| c.revision != r.revision)
                            {
                                report.witnesses.insert("stale-receipt".into());
                            } else {
                                report.witnesses.insert("receipt-replay".into());
                            }
                        }
                        (ModelEvent::CloseHost, ModelOutcome::Revision(Ok(_))) => {
                            report.witnesses.insert("host-close".into());
                        }
                        (ModelEvent::CloseHost, ModelOutcome::Revision(Err(_))) => {
                            report.witnesses.insert("host-close-replay".into());
                        }
                        _ => {}
                    }
                    if matches!(event, ModelEvent::Publish)
                        && matches!(outcome, ModelOutcome::Revision(Ok(_)))
                    {
                        child
                            .publications
                            .push(child.state.checkpoint.clone().unwrap());
                    }
                    next.push(child);
                }
            }
            match group {
                0 => report.attempt_counts[depth] = next.len(),
                1 => report.checkpoint_counts[depth] = next.len(),
                _ => report.host_counts[depth] = next.len(),
            }
            level = next;
        }
    }
    report.elapsed = start.elapsed();
    Ok(report)
}
