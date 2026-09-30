//! Durable, deterministic coordinator state for the Codex guarded continuation.
//!
//! This module contains no filesystem or process I/O. The CLI owns persistence
//! and supplies ownership observations before reducing an event.

use serde::{Deserialize, Serialize};
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
    pub commit: Option<String>,
    pub kind: CheckpointKind,
    pub consumed_receipt_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckpointKind {
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
    pub commit: Option<String>,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RetryState {
    pub attempt: u32,
    pub max_attempts: u32,
    pub last_failure: Option<String>,
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
    pub activation_digest: Option<String>,
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
}

impl fmt::Display for CoordinatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for CoordinatorError {}

impl CoordinatorState {
    pub const SCHEMA_VERSION: u32 = 1;

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
            activation_digest: None,
            next_action: NextAction::StartTask { task_id, phase },
            consumed_checkpoint_receipts: Vec::new(),
        }
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
        if self.profile != ContinuationProfile::CodexStopV1 {
            return Err(CoordinatorError::InvalidTransition(
                "semantic checkpoint requires CodexStopV1",
            ));
        }
        self.revision = self.revision.saturating_add(1);
        self.checkpoint = Some(SemanticCheckpoint {
            checkpoint_id: checkpoint_id.clone(),
            revision: self.revision,
            task_id: self.task_id.clone(),
            phase: self.phase.clone(),
            prompt_sha256: self.prompt_sha256.clone(),
            commit,
            kind,
            consumed_receipt_id: None,
        });
        self.next_action = NextAction::AwaitOrchestrator { checkpoint_id };
        Ok(self.revision)
    }

    /// Consume a receipt once, bound to the exact checkpoint revision and evidence.
    pub fn consume_checkpoint_receipt(
        &mut self,
        receipt: CheckpointReceipt,
    ) -> Result<u64, CoordinatorError> {
        if self
            .consumed_checkpoint_receipts
            .contains(&receipt.receipt_id)
        {
            return Err(CoordinatorError::ReceiptReplay);
        }
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
