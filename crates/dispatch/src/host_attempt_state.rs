use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
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
    #[error("host attempt custody is already held")]
    CustodyClaimed,
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
    Abandoned,
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
            Self::Abandoned => "abandoned",
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
            "abandoned" => Self::Abandoned,
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

    pub fn custody_lock_path(&self) -> Result<PathBuf, HostAttemptStateError> {
        self.validate()?;
        Self::custody_lock_path_for(
            &self.worktree.join(".dev/pipeline").join(&self.task),
            &self.attempt_id,
        )
    }

    pub fn custody_lock_path_for(
        task_scope_dir: &Path,
        attempt_id: &str,
    ) -> Result<PathBuf, HostAttemptStateError> {
        Ok(Self::manifest_path_for(task_scope_dir, attempt_id)?.with_extension("custody-lock"))
    }

    pub fn acquire_custody_lock(&self) -> Result<HostAttemptCustodyGuard, HostAttemptStateError> {
        let lock_path = self.custody_lock_path()?;
        HostAttemptCustodyGuard::try_acquire(lock_path)
    }

    pub fn acquire_custody_lock_at(
        &self,
        task_scope_dir: &Path,
    ) -> Result<HostAttemptCustodyGuard, HostAttemptStateError> {
        self.validate()?;
        let expected = self.worktree.join(".dev/pipeline");
        if !task_scope_dir.starts_with(&expected)
            || task_scope_dir
                .components()
                .any(|part| matches!(part, std::path::Component::ParentDir))
        {
            return Err(HostAttemptStateError::Malformed);
        }
        let lock_path = Self::custody_lock_path_for(task_scope_dir, &self.attempt_id)?;
        HostAttemptCustodyGuard::try_acquire(lock_path)
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

/// Exclusive operating-system custody of one host-attempt manifest.
pub struct HostAttemptCustodyGuard {
    file: File,
    path: PathBuf,
}

impl HostAttemptCustodyGuard {
    fn try_acquire(path: PathBuf) -> Result<Self, HostAttemptStateError> {
        let parent = path.parent().ok_or(HostAttemptStateError::Malformed)?;
        std::fs::create_dir_all(parent).map_err(|e| HostAttemptStateError::Io(e.to_string()))?;
        let file = open_custody_file(&path).map_err(|e| {
            #[cfg(windows)]
            if e.raw_os_error() == Some(32) {
                return HostAttemptStateError::CustodyClaimed;
            }
            HostAttemptStateError::Io(e.to_string())
        })?;
        file.try_lock_exclusive().map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                HostAttemptStateError::CustodyClaimed
            } else {
                HostAttemptStateError::Io(e.to_string())
            }
        })?;
        Ok(Self { file, path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(windows)]
fn open_custody_file(path: &Path) -> std::io::Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .share_mode(0)
        .open(path)
}

#[cfg(not(windows))]
fn open_custody_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
}

impl Drop for HostAttemptCustodyGuard {
    fn drop(&mut self) {
        let _ = self.file.unlock();
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, Write};
    use std::process::{Command, Stdio};

    fn attempt_state(worktree: &Path) -> HostAttemptState {
        HostAttemptState {
            attempt_id: "attempt-1".to_string(),
            worktree: worktree.to_path_buf(),
            prompt: worktree.join("prompt.md"),
            task: "task-1".to_string(),
            phase: "implement".to_string(),
            profile: "default".to_string(),
            run_id: "run-1".to_string(),
            host: "codex".to_string(),
            mode: "headless".to_string(),
            spec_path: worktree.join("spec.md"),
            spec_sha256: "a".repeat(64),
            receipt_path: None,
            log_path: worktree.join("log.json"),
            snapshot_path: worktree.join("snapshot.json"),
            started_at: "2026-10-23T00:00:00Z".to_string(),
            model: "model".to_string(),
            effort: None,
            tier: None,
            tier_source: None,
            override_reason: None,
            policy_applied: false,
            prerequisite_refs: Vec::new(),
            failure_history: Vec::new(),
            observed_model: None,
            observed_effort: None,
            prompt_sha256: None,
            receipt_sha256: None,
            log_sha256: None,
            snapshot_sha256: None,
            worktree_status_sha256: None,
            worktree_diff_sha256: None,
        }
    }

    #[test]
    fn custody_lock_rejects_outside_and_parent_traversal_scopes_without_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path().join("worktree");
        let state = attempt_state(&worktree);
        let expected = worktree.join(".dev/pipeline");
        let outside = dir.path().join("outside/task-1");
        let traversal = expected.join("../outside/task-1");

        assert!(matches!(
            state.acquire_custody_lock_at(&outside),
            Err(HostAttemptStateError::Malformed)
        ));
        assert!(matches!(
            state.acquire_custody_lock_at(&traversal),
            Err(HostAttemptStateError::Malformed)
        ));
        assert!(!outside.exists());
        assert!(!expected.exists());
    }

    #[test]
    fn custody_lock_rejects_malformed_state_without_artifacts() {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path().join("worktree");
        let mut state = attempt_state(&worktree);
        state.spec_sha256 = "invalid".to_string();
        let task_scope = worktree.join(".dev/pipeline/task-1");

        assert!(matches!(
            state.acquire_custody_lock_at(&task_scope),
            Err(HostAttemptStateError::Malformed)
        ));
        assert!(!task_scope.exists());
        assert!(!worktree.exists());
    }

    #[test]
    fn custody_lock_contention_preserves_fixture_and_reuses_path() {
        let dir = tempfile::tempdir().unwrap();
        let scope = dir.path().join("task");
        let lock_path = HostAttemptState::custody_lock_path_for(&scope, "attempt-1").unwrap();
        let fixture = HostAttemptState::manifest_path_for(&scope, "attempt-1").unwrap();
        std::fs::create_dir_all(fixture.parent().unwrap()).unwrap();
        std::fs::write(&fixture, b"fixture state").unwrap();

        let owner = HostAttemptCustodyGuard::try_acquire(lock_path.clone()).unwrap();
        let blocked = HostAttemptCustodyGuard::try_acquire(lock_path.clone());
        assert!(matches!(
            blocked,
            Err(HostAttemptStateError::CustodyClaimed)
        ));
        assert_eq!(std::fs::read(&fixture).unwrap(), b"fixture state");
        assert_eq!(owner.path(), lock_path);
        drop(owner);

        let reacquired = HostAttemptCustodyGuard::try_acquire(lock_path.clone()).unwrap();
        assert_eq!(reacquired.path(), lock_path);
        drop(reacquired);
        assert!(lock_path.exists());
    }

    #[test]
    fn custody_lock_is_released_after_child_termination() {
        const CHILD_ENV: &str = "DISPATCH_CUSTODY_LOCK_CHILD";
        if let Some(path) = std::env::var_os(CHILD_ENV) {
            let _guard = HostAttemptCustodyGuard::try_acquire(PathBuf::from(path)).unwrap();
            if std::env::var_os("DISPATCH_CUSTODY_LOCK_REACQUIRE").is_some() {
                println!("LOCK_REACQUIRED");
                return;
            }
            println!("LOCK_HELD");
            std::io::stdout().flush().unwrap();
            let mut input = String::new();
            let _ = std::io::stdin().read_line(&mut input);
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let lock_path = HostAttemptState::custody_lock_path_for(dir.path(), "attempt-2").unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host_attempt_state::tests::custody_lock_is_released_after_child_termination",
                "--nocapture",
            ])
            .env(CHILD_ENV, &lock_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut reader = std::io::BufReader::new(stdout);
        let mut line = String::new();
        loop {
            line.clear();
            assert!(
                reader.read_line(&mut line).unwrap() > 0,
                "child exited before locking"
            );
            if line.trim() == "LOCK_HELD" {
                break;
            }
        }

        assert!(matches!(
            HostAttemptCustodyGuard::try_acquire(lock_path.clone()),
            Err(HostAttemptStateError::CustodyClaimed)
        ));
        child.kill().unwrap();
        let _ = child.wait().unwrap();

        let reacquirer = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "host_attempt_state::tests::custody_lock_is_released_after_child_termination",
                "--nocapture",
            ])
            .env(CHILD_ENV, &lock_path)
            .env("DISPATCH_CUSTODY_LOCK_REACQUIRE", "1")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let output = reacquirer.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "reacquirer failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("LOCK_REACQUIRED"));
    }

    #[test]
    fn abandoned_terminal_state_has_stable_literal() {
        assert_eq!(HostTerminalState::Abandoned.as_str(), "abandoned");
        assert_eq!(
            HostTerminalState::parse("abandoned"),
            Some(HostTerminalState::Abandoned)
        );
    }
}
