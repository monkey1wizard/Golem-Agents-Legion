//! Accounting run identity, attempt links, and shared-session interval binding.
//!
//! A run groups every attempt that belongs to one execution. Its identity is
//! stable across resume, so the coordinator can reuse it. Each attempt also
//! records how it links to an earlier attempt. A host session that two runs
//! both read is split into source intervals. An interval that more than one
//! run covers stays ambiguous and is never assigned to a single run.
//!
//! This module performs no I/O. Persistence belongs to the coordinator and the
//! usage store.

use dispatch::host_session::HostSessionRef;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Schema version written for run identities and attempt manifests.
pub const RUN_MANIFEST_VERSION: u32 = 1;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BindingError {
    #[error("accounting run field `{0}` is empty")]
    EmptyField(&'static str),
    #[error("accounting run version {0} is not supported")]
    UnsupportedVersion(u32),
    #[error("host session `{0}` is not a valid host session reference")]
    InvalidHostSession(String),
    #[error("attempt link points to the attempt itself")]
    SelfLink,
    #[error("session interval for `{0}` must end after it starts")]
    InvertedInterval(String),
    #[error("attempt `{attempt_id}` does not belong to run `{run_id}`")]
    AttemptNotBound { run_id: String, attempt_id: String },
}

/// Stable identity of one accounting run. Every attempt of the execution shares it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunIdentity {
    pub version: u32,
    pub run_id: String,
    /// Canonical project worktree root, resolved by the caller.
    pub canonical_worktree: String,
    pub plan_scope: String,
    /// Host session reference such as `codex:<session>`, when one is bound.
    pub host_session: Option<String>,
    /// Host window or pane label, when the host exposes one.
    pub host_window: Option<String>,
}

impl RunIdentity {
    pub fn validate(&self) -> Result<(), BindingError> {
        if self.version != RUN_MANIFEST_VERSION {
            return Err(BindingError::UnsupportedVersion(self.version));
        }
        for (name, value) in [
            ("run_id", &self.run_id),
            ("canonical_worktree", &self.canonical_worktree),
            ("plan_scope", &self.plan_scope),
        ] {
            if value.trim().is_empty() {
                return Err(BindingError::EmptyField(name));
            }
        }
        if let Some(window) = &self.host_window {
            if window.trim().is_empty() {
                return Err(BindingError::EmptyField("host_window"));
            }
        }
        if let Some(session) = &self.host_session {
            HostSessionRef::parse(session)
                .map_err(|_| BindingError::InvalidHostSession(session.clone()))?;
        }
        Ok(())
    }

    /// Hash fixed-order, length-prefixed fields. Optional fields carry a
    /// presence byte, so an absent window never equals an empty one.
    pub fn digest(&self) -> Result<String, BindingError> {
        self.validate()?;
        let mut hash = Sha256::new();
        hash.update(b"GAL-ACCOUNTING-RUN\0");
        hash.update(self.version.to_be_bytes());
        push_field(&mut hash, &self.run_id);
        push_field(&mut hash, &self.canonical_worktree);
        push_field(&mut hash, &self.plan_scope);
        push_optional(&mut hash, self.host_session.as_deref());
        push_optional(&mut hash, self.host_window.as_deref());
        Ok(format!("{:x}", hash.finalize()))
    }
}

/// How one attempt relates to earlier attempts of the same run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum AttemptLink {
    /// First attempt of a new execution, or a standalone dispatch.
    Initial,
    /// A retry after a failed attempt of the same run.
    Retry { previous_attempt_id: String },
    /// A resume after an interrupted attempt of the same run.
    Resume { previous_attempt_id: String },
}

/// Recorded manifest for one attempt. It names the run identity it belongs to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RunManifest {
    pub identity: RunIdentity,
    pub attempt_id: String,
    pub link: AttemptLink,
}

impl RunManifest {
    /// Manifest for a standalone dispatch, which is always a first attempt.
    pub fn standalone(identity: RunIdentity, attempt_id: impl Into<String>) -> Self {
        Self {
            identity,
            attempt_id: attempt_id.into(),
            link: AttemptLink::Initial,
        }
    }

    pub fn validate(&self) -> Result<(), BindingError> {
        self.identity.validate()?;
        if self.attempt_id.trim().is_empty() {
            return Err(BindingError::EmptyField("attempt_id"));
        }
        match &self.link {
            AttemptLink::Initial => {}
            AttemptLink::Retry {
                previous_attempt_id,
            }
            | AttemptLink::Resume {
                previous_attempt_id,
            } => {
                if previous_attempt_id.trim().is_empty() {
                    return Err(BindingError::EmptyField("previous_attempt_id"));
                }
                if previous_attempt_id == &self.attempt_id {
                    return Err(BindingError::SelfLink);
                }
            }
        }
        Ok(())
    }

    /// Digest of the manifest, including its identity and attempt link.
    pub fn digest(&self) -> Result<String, BindingError> {
        self.validate()?;
        let identity_digest = self.identity.digest()?;
        let (kind, previous) = match &self.link {
            AttemptLink::Initial => ("initial", None),
            AttemptLink::Retry {
                previous_attempt_id,
            } => ("retry", Some(previous_attempt_id.as_str())),
            AttemptLink::Resume {
                previous_attempt_id,
            } => ("resume", Some(previous_attempt_id.as_str())),
        };
        let mut hash = Sha256::new();
        hash.update(b"GAL-ACCOUNTING-MANIFEST\0");
        push_field(&mut hash, &identity_digest);
        push_field(&mut hash, &self.attempt_id);
        push_field(&mut hash, kind);
        push_optional(&mut hash, previous);
        Ok(format!("{:x}", hash.finalize()))
    }
}

/// Checks that `manifest` binds `attempt_id` to `run_id`. Records that name an attempt call this first.
pub fn check_attempt_binding(
    manifest: &RunManifest,
    run_id: &str,
    attempt_id: &str,
) -> Result<(), BindingError> {
    if manifest.identity.run_id != run_id || manifest.attempt_id != attempt_id {
        return Err(BindingError::AttemptNotBound {
            run_id: run_id.to_owned(),
            attempt_id: attempt_id.to_owned(),
        });
    }
    Ok(())
}

/// Result of binding a run identity to coordinator state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunBindingOutcome {
    /// No run was recorded, so this execution received a fresh one.
    Allocated,
    /// The same identity was already recorded, so the resume reuses it.
    Reused,
}

/// One source interval of a host session that a run asks to attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionClaim {
    pub run_id: String,
    pub session: String,
    /// Half-open source interval `[start_ms, end_ms)`.
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntervalBinding {
    /// No other run covers any part of this segment.
    Exclusive,
    /// Other runs cover this segment too. No run receives it as its own.
    Ambiguous { competing_run_ids: Vec<String> },
}

/// A disjoint segment of one claim, with its binding status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundInterval {
    pub run_id: String,
    pub session: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub binding: IntervalBinding,
}

/// Split each claim at the boundaries of overlapping claims from other runs.
///
/// Output keeps claim order, and each claim's segments are sorted by start.
/// Claims on different sessions never compete. Overlap between claims of the
/// same run is not ambiguous.
pub fn bind_session_intervals(claims: &[SessionClaim]) -> Result<Vec<BoundInterval>, BindingError> {
    if let Some(claim) = claims.iter().find(|claim| claim.start_ms >= claim.end_ms) {
        return Err(BindingError::InvertedInterval(claim.session.clone()));
    }
    let mut bound = Vec::new();
    for claim in claims {
        let rivals: Vec<&SessionClaim> = claims
            .iter()
            .filter(|other| {
                other.session == claim.session
                    && other.run_id != claim.run_id
                    && other.start_ms < claim.end_ms
                    && claim.start_ms < other.end_ms
            })
            .collect();
        let mut cuts = vec![claim.start_ms, claim.end_ms];
        for rival in &rivals {
            cuts.push(rival.start_ms.clamp(claim.start_ms, claim.end_ms));
            cuts.push(rival.end_ms.clamp(claim.start_ms, claim.end_ms));
        }
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (start_ms, end_ms) = (pair[0], pair[1]);
            let mut competing: Vec<String> = rivals
                .iter()
                .filter(|rival| rival.start_ms <= start_ms && end_ms <= rival.end_ms)
                .map(|rival| rival.run_id.clone())
                .collect();
            competing.sort_unstable();
            competing.dedup();
            let binding = if competing.is_empty() {
                IntervalBinding::Exclusive
            } else {
                IntervalBinding::Ambiguous {
                    competing_run_ids: competing,
                }
            };
            bound.push(BoundInterval {
                run_id: claim.run_id.clone(),
                session: claim.session.clone(),
                start_ms,
                end_ms,
                binding,
            });
        }
    }
    Ok(bound)
}

fn push_field(hash: &mut Sha256, value: &str) {
    hash.update((value.len() as u64).to_be_bytes());
    hash.update(value.as_bytes());
}

fn push_optional(hash: &mut Sha256, value: Option<&str>) {
    hash.update([u8::from(value.is_some())]);
    if let Some(value) = value {
        push_field(hash, value);
    }
}
