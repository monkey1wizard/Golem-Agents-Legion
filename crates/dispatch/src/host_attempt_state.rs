use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Immutable identity and evidence paths for one host-subagent attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostAttemptState {
    pub attempt_id: String,
    pub worktree: PathBuf,
    pub prompt: PathBuf,
    pub task: String,
    pub phase: String,
    pub profile: String,
    pub run_id: String,
    pub host: String,
    pub mode: String,
    pub spec_path: PathBuf,
    pub spec_sha256: String,
    pub receipt_path: Option<PathBuf>,
    pub log_path: PathBuf,
    pub snapshot_path: PathBuf,
    pub started_at: String,
    pub model: String,
    pub effort: Option<String>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub tier_source: Option<String>,
    #[serde(default)]
    pub override_reason: Option<String>,
    #[serde(default)]
    pub policy_applied: bool,
    #[serde(default)]
    pub prerequisite_refs: Vec<String>,
    #[serde(default)]
    pub failure_history: Vec<String>,
    #[serde(default)]
    pub observed_model: Option<String>,
    #[serde(default)]
    pub observed_effort: Option<String>,
    #[serde(default)]
    pub prompt_sha256: Option<String>,
    #[serde(default)]
    pub receipt_sha256: Option<String>,
    #[serde(default)]
    pub log_sha256: Option<String>,
    #[serde(default)]
    pub snapshot_sha256: Option<String>,
    #[serde(default)]
    pub worktree_status_sha256: Option<String>,
    #[serde(default)]
    pub worktree_diff_sha256: Option<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostAttemptStateError {
    #[error("host attempt manifest is malformed")]
    Malformed,
    #[error("host attempt manifest does not match the requested identity")]
    IdentityMismatch,
    #[error("host attempt manifest already exists")]
    AlreadyExists,
    #[error("host attempt close is already claimed")]
    CloseClaimed,
    #[error("host attempt manifest digest is invalid")]
    DigestMismatch,
    #[error("host attempt manifest I/O failed: {0}")]
    Io(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostTerminalState {
    Completed,
    HostSessionInvalid,
    NoWriteback,
    LeaseOwnerMismatch,
    LeaseMissing,
    IdentityMismatch,
    TerminalReplay,
    PlacementFailed,
    ModelTierMismatch,
}
impl HostTerminalState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::HostSessionInvalid => "host-session-invalid",
            Self::NoWriteback => "no-writeback",
            Self::LeaseOwnerMismatch => "lease-owner-mismatch",
            Self::LeaseMissing => "lease-missing",
            Self::IdentityMismatch => "identity-mismatch",
            Self::TerminalReplay => "terminal-replay",
            Self::PlacementFailed => "placement-failed",
            Self::ModelTierMismatch => "model-tier-mismatch",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "completed" => Self::Completed,
            "host-session-invalid" => Self::HostSessionInvalid,
            "no-writeback" => Self::NoWriteback,
            "lease-owner-mismatch" => Self::LeaseOwnerMismatch,
            "lease-missing" => Self::LeaseMissing,
            "identity-mismatch" => Self::IdentityMismatch,
            "terminal-replay" => Self::TerminalReplay,
            "placement-failed" => Self::PlacementFailed,
            "model-tier-mismatch" => Self::ModelTierMismatch,
            _ => return None,
        })
    }
}

pub fn workdir_unchanged(
    before: Option<&str>,
    after: Option<&str>,
    before_head: Option<&str>,
    after_head: Option<&str>,
) -> bool {
    matches!((before, after, before_head, after_head), (Some(a), Some(b), Some(c), Some(d)) if a == b && c == d)
}

impl HostAttemptState {
    pub fn validate(&self) -> Result<(), HostAttemptStateError> {
        if !valid_identifier(&self.attempt_id)
            || !valid_identifier(&self.task)
            || !matches!(self.host.as_str(), "claude" | "codex")
            || self.phase.is_empty()
            || self.run_id.is_empty()
            || self.spec_sha256.len() != 64
            || !self.spec_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(HostAttemptStateError::Malformed);
        }
        Ok(())
    }

    pub fn encode_immutable(&self) -> Result<Vec<u8>, HostAttemptStateError> {
        self.validate()?;
        let payload = serde_json::to_vec(self).map_err(|_| HostAttemptStateError::Malformed)?;
        serde_json::to_vec(&ManifestEnvelope {
            state: self.clone(),
            sha256: format!("{:x}", Sha256::digest(payload)),
        })
        .map_err(|_| HostAttemptStateError::Malformed)
    }

    pub fn decode_immutable(bytes: &[u8]) -> Result<Self, HostAttemptStateError> {
        let envelope: ManifestEnvelope =
            serde_json::from_slice(bytes).map_err(|_| HostAttemptStateError::Malformed)?;
        let payload =
            serde_json::to_vec(&envelope.state).map_err(|_| HostAttemptStateError::Malformed)?;
        if format!("{:x}", Sha256::digest(&payload)) != envelope.sha256 {
            return Err(HostAttemptStateError::DigestMismatch);
        }
        let state = envelope.state;
        state.validate()?;
        Ok(state)
    }

    pub fn manifest_path(&self) -> Result<PathBuf, HostAttemptStateError> {
        self.validate()?;
        Self::manifest_path_for(
            &self.worktree.join(".dev/pipeline").join(&self.task),
            &self.attempt_id,
        )
    }

    pub fn manifest_path_for(
        task_scope_dir: &Path,
        attempt_id: &str,
    ) -> Result<PathBuf, HostAttemptStateError> {
        if !valid_identifier(attempt_id) {
            return Err(HostAttemptStateError::Malformed);
        }
        Ok(task_scope_dir
            .join("host-attempts")
            .join(format!("{attempt_id}.json")))
    }

    pub fn write_once_at(&self, task_scope_dir: &Path) -> Result<PathBuf, HostAttemptStateError> {
        let path = Self::manifest_path_for(task_scope_dir, &self.attempt_id)?;
        let expected = self.worktree.join(".dev/pipeline");
        if !task_scope_dir.starts_with(&expected)
            || task_scope_dir
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(HostAttemptStateError::Malformed);
        }
        self.write_once_to(path)
    }

    pub fn write_once(&self) -> Result<PathBuf, HostAttemptStateError> {
        let path = self.manifest_path()?;
        self.write_once_to(path)
    }

    fn write_once_to(&self, path: PathBuf) -> Result<PathBuf, HostAttemptStateError> {
        let parent = path.parent().ok_or(HostAttemptStateError::Malformed)?;
        std::fs::create_dir_all(parent).map_err(|e| HostAttemptStateError::Io(e.to_string()))?;
        let envelope = self.encode_immutable()?;
        let temporary = parent.join(format!(".{}.{}.tmp", self.attempt_id, std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::AlreadyExists {
                    HostAttemptStateError::AlreadyExists
                } else {
                    HostAttemptStateError::Io(e.to_string())
                }
            })?;
        use std::io::Write;
        if let Err(e) = file.write_all(&envelope).and_then(|_| file.sync_all()) {
            drop(file);
            let _ = std::fs::remove_file(&temporary);
            return Err(HostAttemptStateError::Io(e.to_string()));
        }
        drop(file);
        if let Err(e) = std::fs::hard_link(&temporary, &path) {
            let _ = std::fs::remove_file(&temporary);
            return Err(if e.kind() == std::io::ErrorKind::AlreadyExists {
                HostAttemptStateError::AlreadyExists
            } else {
                HostAttemptStateError::Io(e.to_string())
            });
        }
        std::fs::remove_file(&temporary).map_err(|e| HostAttemptStateError::Io(e.to_string()))?;
        Ok(path)
    }

    pub fn load(path: &Path, attempt_id: &str) -> Result<Self, HostAttemptStateError> {
        let bytes = std::fs::read(path).map_err(|e| HostAttemptStateError::Io(e.to_string()))?;
        let state = Self::decode_immutable(&bytes)?;
        if state.attempt_id != attempt_id {
            return Err(HostAttemptStateError::IdentityMismatch);
        }
        Ok(state)
    }

    pub fn claim_close(&self) -> Result<PathBuf, HostAttemptStateError> {
        let claim = self.manifest_path()?.with_extension("close-claim");
        Self::create_close_claim(claim)
    }

    pub fn claim_close_at(&self, task_scope_dir: &Path) -> Result<PathBuf, HostAttemptStateError> {
        let manifest = Self::manifest_path_for(task_scope_dir, &self.attempt_id)?;
        Self::create_close_claim(manifest.with_extension("close-claim"))
    }

    fn create_close_claim(claim: PathBuf) -> Result<PathBuf, HostAttemptStateError> {
        match std::fs::create_dir(&claim) {
            Ok(()) => Ok(claim),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(HostAttemptStateError::CloseClaimed)
            }
            Err(e) => Err(HostAttemptStateError::Io(e.to_string())),
        }
    }

    pub fn matches_identity(
        &self,
        worktree: &std::path::Path,
        task: &str,
        phase: &str,
        host: &str,
    ) -> Result<(), HostAttemptStateError> {
        self.validate()?;
        if self.worktree != worktree
            || self.task != task
            || self.phase != phase
            || self.host != host
        {
            return Err(HostAttemptStateError::IdentityMismatch);
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
struct ManifestEnvelope {
    state: HostAttemptState,
    sha256: String,
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
}
