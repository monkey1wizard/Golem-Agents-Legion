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
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CoordinatorError {}

impl CoordinatorState {
    pub const SCHEMA_VERSION: u32 = 2;

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

    /// Select guarded mode only from a caller-validated activation digest.
    pub fn activate_codex_stop_v1(
        &mut self,
        activation_digest: String,
    ) -> Result<(), CoordinatorError> {
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
        self.active_attempt = Some(ActiveAttempt {
            attempt_id,
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
            process_id: None,
            process_birth: None,
            dispatch_claimed: true,
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
        self.reject_pending_host_work()?;
        if self.profile == ContinuationProfile::CodexStopV1 {
            self.execution_binding_digest()?;
        }
        let active = self
            .active_attempt
            .as_ref()
            .ok_or(CoordinatorError::AttemptMissing)?;
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
        if self.has_pending_host_checkpoint() {
            return Ok(AttemptOwnership::NotApplicable);
        }
        if self.profile == ContinuationProfile::CodexStopV1 {
            self.execution_binding_digest()?;
        }
        let attempt = self
            .active_attempt
            .as_ref()
            .ok_or(CoordinatorError::AttemptMissing)?;
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
        self.reject_pending_host_work()?;
        let active = self
            .active_attempt
            .as_ref()
            .ok_or(CoordinatorError::AttemptMissing)?;
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
