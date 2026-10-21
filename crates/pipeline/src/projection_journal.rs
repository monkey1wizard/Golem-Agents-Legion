//! Crash-safe journal for the three authoritative pipeline projections.
//!
//! The journal stores a recovery recipe only. The plan, prompt, and state files
//! remain the authority; recovery changes a target only when its bytes match
//! the recorded old or new hash.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct Projection {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boundary {
    BeforeJournalPrepared,
    JournalPrepared,
    BeforeLock(usize),
    LockAcquired(usize),
    BeforeTargetReplaced(usize),
    TargetReplaced(usize),
    BeforeCommit,
    Committed,
}

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("projection journal I/O failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("projection journal serialization failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("projection target has an unexpected hash: {0}")]
    ThirdHash(PathBuf),
    #[error("projection target list must contain exactly three distinct paths")]
    InvalidTargets,
    #[error("injected interruption at {0:?}")]
    Interrupted(Boundary),
}

#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    version: u8,
    committed: bool,
    targets: Vec<Entry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Entry {
    path: PathBuf,
    old_hash: Option<String>,
    new_hash: String,
    staged: String,
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), JournalError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let temp = parent.join(format!(".{name}.projection-tmp"));
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
    }
    replace_file(&temp, path)?;
    Ok(())
}

#[cfg(windows)]
fn replace_file(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    if to.exists() {
        fs::remove_file(to)?;
    }
    fs::rename(from, to)
}

#[cfg(not(windows))]
fn replace_file(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    fs::rename(from, to)
}

fn lock_paths(entries: &[Entry]) -> Vec<PathBuf> {
    let mut paths: Vec<_> = entries
        .iter()
        .map(|e| {
            let mut os = e.path.as_os_str().to_os_string();
            os.push(".projection-lock");
            PathBuf::from(os)
        })
        .collect();
    paths.sort();
    paths
}

struct Locks(Vec<PathBuf>);
impl Drop for Locks {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

fn acquire_locks(
    entries: &[Entry],
    mut hook: impl FnMut(Boundary) -> Result<(), JournalError>,
) -> Result<Locks, JournalError> {
    let mut locks = Locks(Vec::new());
    for (index, path) in lock_paths(entries).into_iter().enumerate() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        hook(Boundary::BeforeLock(index))?;
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?
            .write_all(b"projection\n")?;
        locks.0.push(path);
        hook(Boundary::LockAcquired(index))?;
    }
    Ok(locks)
}

/// Apply all three projections. `hook` provides deterministic interruption
/// points for recovery testing and may be omitted with [`apply`].
pub fn apply_with_hook(
    journal_path: &Path,
    projections: [Projection; 3],
    mut hook: impl FnMut(Boundary) -> Result<(), JournalError>,
) -> Result<(), JournalError> {
    let mut projections = projections.to_vec();
    projections.sort_by(|a, b| a.path.cmp(&b.path));
    if projections.windows(2).any(|w| w[0].path == w[1].path) {
        return Err(JournalError::InvalidTargets);
    }
    let mut entries = Vec::new();
    for p in projections {
        let old_hash = match fs::read(&p.path) {
            Ok(bytes) => Some(hash(&bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        entries.push(Entry {
            path: p.path,
            old_hash,
            new_hash: hash(&p.bytes),
            staged: STANDARD.encode(p.bytes),
        });
    }
    let mut journal = Journal {
        version: 1,
        committed: false,
        targets: entries,
    };
    hook(Boundary::BeforeJournalPrepared)?;
    write_atomic(journal_path, &serde_json::to_vec(&journal)?)?;
    hook(Boundary::JournalPrepared)?;
    let _locks = acquire_locks(&journal.targets, &mut hook)?;
    replace_targets(&journal, &mut hook)?;
    hook(Boundary::BeforeCommit)?;
    journal.committed = true;
    write_atomic(journal_path, &serde_json::to_vec(&journal)?)?;
    hook(Boundary::Committed)?;
    Ok(())
}

pub fn apply(journal_path: &Path, projections: [Projection; 3]) -> Result<(), JournalError> {
    apply_with_hook(journal_path, projections, |_| Ok(()))
}

fn replace_targets(
    journal: &Journal,
    hook: &mut impl FnMut(Boundary) -> Result<(), JournalError>,
) -> Result<(), JournalError> {
    for (index, entry) in journal.targets.iter().enumerate() {
        let current = match fs::read(&entry.path) {
            Ok(bytes) => Some(hash(&bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.into()),
        };
        if current.as_deref() == Some(&entry.new_hash) {
            continue;
        }
        if current != entry.old_hash {
            return Err(JournalError::ThirdHash(entry.path.clone()));
        }
        let bytes = STANDARD
            .decode(&entry.staged)
            .map_err(|_| JournalError::InvalidTargets)?;
        hook(Boundary::BeforeTargetReplaced(index))?;
        write_atomic(&entry.path, &bytes)?;
        hook(Boundary::TargetReplaced(index))?;
    }
    Ok(())
}

/// Recover an existing journal idempotently. A third-party target hash is
/// preserved, and the journal remains available to keep gates blocked.
pub fn recover(journal_path: &Path) -> Result<(), JournalError> {
    let bytes = fs::read(journal_path)?;
    let mut journal: Journal = serde_json::from_slice(&bytes)?;
    if journal.version != 1 || journal.targets.len() != 3 {
        return Err(JournalError::InvalidTargets);
    }
    let _locks = acquire_locks(&journal.targets, |_| Ok(()))?;
    replace_targets(&journal, &mut |_| Ok(()))?;
    if !journal.committed {
        journal.committed = true;
        write_atomic(journal_path, &serde_json::to_vec(&journal)?)?;
    }
    Ok(())
}
