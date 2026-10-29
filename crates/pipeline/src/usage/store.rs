//! Git-common store for immutable accounting records.
//!
//! Layout under the Git common directory:
//!
//! ```text
//! gal/usage/v1/<run-id>/.lock
//! gal/usage/v1/<run-id>/<kind>/<record-id>.record.json
//! gal/usage/v1/<run-id>/<kind>/<record-id>.manifest.json
//! ```
//!
//! A record is visible only once its manifest is renamed into place, so a
//! reader enumerates manifests and never sees a partial record. Publishers hold
//! an exclusive `fs2` lock on the run directory before identity checks and
//! before writing temporary files. Existing records are never replaced.

use super::{
    digest_bytes,
    evidence::SnapshotEvidence,
    reconcile::{
        reconcile, ObservationRecord, ReconcileError, ReconcileInput, Reconciliation,
        RECONCILIATION_VERSION,
    },
    RecordKind, StoredRecord, UsageSnapshot, RECORD_SCHEMA_VERSION,
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
};
use thiserror::Error;

const STORE_COMPONENTS: [&str; 3] = ["gal", "usage", "v1"];
const LOCK_FILE: &str = ".lock";
const RECORD_SUFFIX: &str = ".record.json";
const MANIFEST_SUFFIX: &str = ".manifest.json";
const MAX_ID_LEN: usize = 128;

/// Largest page a reader may request, so one call stays bounded.
pub const MAX_PAGE_LIMIT: usize = 256;

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("usage store I/O failed at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("`{0}` is not a safe usage record identifier")]
    InvalidId(String),
    #[error("path escapes the usage store: {}", .0.display())]
    SymlinkEscape(PathBuf),
    #[error("record `{0}` already exists with different content")]
    Conflict(String),
    #[error("record `{0}` is not published")]
    NotFound(String),
    #[error("record `{0}` does not match its published digest")]
    DigestMismatch(String),
    #[error("record `{id}` is malformed: {reason}")]
    Malformed { id: String, reason: String },
    #[error("page limit {0} is outside 1..={MAX_PAGE_LIMIT}")]
    InvalidPageLimit(usize),
    #[error("snapshot `{0}` has no published evidence")]
    EvidenceMissing(String),
    #[error("evidence for snapshot `{snapshot_id}` does not match the digest of observation `{observation_id}`")]
    EvidenceMismatch {
        snapshot_id: String,
        observation_id: String,
    },
    #[error("snapshot `{snapshot_id}` cannot be reconciled: {source}")]
    Reconcile {
        snapshot_id: String,
        #[source]
        source: ReconcileError,
    },
}

/// Digest of the bytes that [`UsageStore::publish`] writes for `record`.
pub fn record_digest<R: StoredRecord>(record: &R) -> Result<String, StoreError> {
    let bytes =
        serde_json::to_vec(record).map_err(|e| malformed(record.record_id(), e.to_string()))?;
    Ok(digest_bytes(&bytes))
}

/// Digest of one observation, computed as [`reconcile`] computes its manifest.
fn observation_digest(record: &ObservationRecord) -> Result<String, StoreError> {
    let bytes = serde_json::to_vec(&record.observation)
        .map_err(|e| malformed(&record.observation_id, e.to_string()))?;
    Ok(digest_bytes(&bytes))
}

/// Outcome of a successful publication.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Publication {
    /// The record and its manifest were written by this call.
    Published,
    /// Identical content was already published, so nothing was added.
    Replayed,
}

/// Published manifest reference, without the record body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub record_id: String,
    pub digest: String,
}

/// One bounded page of manifests in record-id order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestPage {
    pub entries: Vec<ManifestEntry>,
    /// Pass this value as `after` to fetch the next page. `None` ends the listing.
    pub next_after: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    kind: RecordKind,
    run_id: String,
    record_id: String,
    digest: String,
}

/// Handle to the usage store under one Git common directory.
#[derive(Debug, Clone)]
pub struct UsageStore {
    common_dir: PathBuf,
}

impl UsageStore {
    /// Opens the store for a Git common directory. Nothing is created yet.
    pub fn open(common_dir: &Path) -> Result<Self, StoreError> {
        let common_dir = fs::canonicalize(common_dir).map_err(|e| io_error(common_dir, e))?;
        let metadata = fs::metadata(&common_dir).map_err(|e| io_error(&common_dir, e))?;
        if !metadata.is_dir() {
            return Err(io_error(
                &common_dir,
                io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "common dir is not a directory",
                ),
            ));
        }
        Ok(Self { common_dir })
    }

    /// Publishes a record. Identical content replays idempotently, and
    /// different content for an already published identity fails.
    pub fn publish<R: StoredRecord>(&self, record: &R) -> Result<Publication, StoreError> {
        let run_id = record.run_id();
        let record_id = record.record_id();
        validate_id(run_id)?;
        validate_id(record_id)?;
        let bytes = serde_json::to_vec(record).map_err(|e| malformed(record_id, e.to_string()))?;
        let digest = digest_bytes(&bytes);

        let run_dir = self.ensure_dir(&[run_id])?;
        let _lock = RunLock::acquire(&run_dir)?;
        let kind_dir = self.ensure_dir(&[run_id, R::KIND.dir_name()])?;
        let record_path = kind_dir.join(format!("{record_id}{RECORD_SUFFIX}"));
        let manifest_path = kind_dir.join(format!("{record_id}{MANIFEST_SUFFIX}"));

        if let Some(manifest) = read_manifest(&manifest_path, record_id)? {
            check_manifest_identity(&manifest, R::KIND, run_id, record_id)?;
            if manifest.digest != digest {
                return Err(StoreError::Conflict(record_id.to_owned()));
            }
            let prior = read_regular_file(&record_path, record_id)?
                .ok_or_else(|| malformed(record_id, "manifest has no record"))?;
            if digest_bytes(&prior) != digest {
                return Err(StoreError::DigestMismatch(record_id.to_owned()));
            }
            return Ok(Publication::Replayed);
        }

        // A record without a manifest is a publication that crashed before its
        // final rename. Identical bytes complete it. Different bytes conflict.
        let outcome = match read_regular_file(&record_path, record_id)? {
            Some(prior) => {
                if digest_bytes(&prior) != digest {
                    return Err(StoreError::Conflict(record_id.to_owned()));
                }
                Publication::Replayed
            }
            None => {
                let temp = temp_path(&kind_dir, record_id, RECORD_SUFFIX);
                write_then_rename(&temp, &record_path, &bytes)?;
                Publication::Published
            }
        };

        let manifest = Manifest {
            kind: R::KIND,
            run_id: run_id.to_owned(),
            record_id: record_id.to_owned(),
            digest,
        };
        let manifest_bytes =
            serde_json::to_vec(&manifest).map_err(|e| malformed(record_id, e.to_string()))?;
        let temp = temp_path(&kind_dir, record_id, MANIFEST_SUFFIX);
        write_then_rename(&temp, &manifest_path, &manifest_bytes)?;
        Ok(outcome)
    }

    /// Reads a published record and verifies it against its manifest digest.
    pub fn read<R: StoredRecord>(&self, run_id: &str, record_id: &str) -> Result<R, StoreError> {
        validate_id(run_id)?;
        validate_id(record_id)?;
        let kind_dir = self
            .dir_chain(&[run_id, R::KIND.dir_name()], false)?
            .ok_or_else(|| StoreError::NotFound(record_id.to_owned()))?;

        let manifest = read_manifest(&kind_dir.join(manifest_name(record_id)), record_id)?
            .ok_or_else(|| StoreError::NotFound(record_id.to_owned()))?;
        check_manifest_identity(&manifest, R::KIND, run_id, record_id)?;

        let bytes = read_regular_file(&kind_dir.join(record_name(record_id)), record_id)?
            .ok_or_else(|| malformed(record_id, "manifest has no record"))?;
        if digest_bytes(&bytes) != manifest.digest {
            return Err(StoreError::DigestMismatch(record_id.to_owned()));
        }

        let record: R =
            serde_json::from_slice(&bytes).map_err(|e| malformed(record_id, e.to_string()))?;
        if record.run_id() != run_id || record.record_id() != record_id {
            return Err(malformed(
                record_id,
                "record identity differs from its manifest",
            ));
        }
        if record.schema_version() > RECORD_SCHEMA_VERSION {
            return Err(malformed(
                record_id,
                "schema version is newer than this build",
            ));
        }
        Ok(record)
    }

    /// Lists up to `limit` published manifests of kind `R` after `after`.
    ///
    /// Records are ordered by identifier, so passing `next_after` returns every
    /// record exactly once across pages.
    pub fn list_page<R: StoredRecord>(
        &self,
        run_id: &str,
        after: Option<&str>,
        limit: usize,
    ) -> Result<ManifestPage, StoreError> {
        if !(1..=MAX_PAGE_LIMIT).contains(&limit) {
            return Err(StoreError::InvalidPageLimit(limit));
        }
        validate_id(run_id)?;
        let Some(kind_dir) = self.dir_chain(&[run_id, R::KIND.dir_name()], false)? else {
            return Ok(ManifestPage {
                entries: Vec::new(),
                next_after: None,
            });
        };

        let mut ids = Vec::new();
        for entry in fs::read_dir(&kind_dir).map_err(|e| io_error(&kind_dir, e))? {
            let entry = entry.map_err(|e| io_error(&kind_dir, e))?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Some(id) = name.strip_suffix(MANIFEST_SUFFIX) else {
                continue;
            };
            let file_type = entry.file_type().map_err(|e| io_error(&entry.path(), e))?;
            if file_type.is_symlink() {
                return Err(StoreError::SymlinkEscape(entry.path()));
            }
            if !file_type.is_file() {
                return Err(malformed(id, "manifest path is not a regular file"));
            }
            ids.push(id.to_owned());
        }
        ids.sort();

        let start = after.map_or(0, |cursor| ids.partition_point(|id| id.as_str() <= cursor));
        let end = (start + limit).min(ids.len());
        let mut entries = Vec::with_capacity(end - start);
        for id in &ids[start..end] {
            let manifest = read_manifest(&kind_dir.join(manifest_name(id)), id)?
                .ok_or_else(|| StoreError::NotFound(id.clone()))?;
            check_manifest_identity(&manifest, R::KIND, run_id, id)?;
            entries.push(ManifestEntry {
                record_id: id.clone(),
                digest: manifest.digest,
            });
        }
        let next_after = (end < ids.len()).then(|| ids[end - 1].clone());
        Ok(ManifestPage {
            entries,
            next_after,
        })
    }

    /// Rebuilds the reconciliation of a published snapshot from its evidence.
    ///
    /// Every observation in the evidence must carry the digest that the
    /// snapshot lists, and no listed observation may be missing. Any failure
    /// returns an error that names the identifier, so no partial total escapes.
    pub fn rebuild_reconciliation(
        &self,
        run_id: &str,
        snapshot_id: &str,
    ) -> Result<Reconciliation, StoreError> {
        let snapshot = self.read::<UsageSnapshot>(run_id, snapshot_id)?;
        let evidence = match self.read::<SnapshotEvidence>(run_id, snapshot_id) {
            Err(StoreError::NotFound(_)) => {
                return Err(StoreError::EvidenceMissing(snapshot_id.to_owned()));
            }
            other => other?,
        };
        if snapshot.reconciliation_version != RECONCILIATION_VERSION {
            return Err(malformed(
                snapshot_id,
                "reconciliation version differs from this build",
            ));
        }

        let mut listed: BTreeMap<&str, &str> = BTreeMap::new();
        for entry in &snapshot.observations {
            if listed
                .insert(entry.observation_id.as_str(), entry.digest.as_str())
                .is_some()
            {
                return Err(malformed(
                    snapshot_id,
                    "snapshot lists an observation twice",
                ));
            }
        }
        for record in &evidence.observations {
            let digest = observation_digest(record)?;
            if listed.remove(record.observation_id.as_str()) != Some(digest.as_str()) {
                return Err(StoreError::EvidenceMismatch {
                    snapshot_id: snapshot_id.to_owned(),
                    observation_id: record.observation_id.clone(),
                });
            }
        }
        if let Some((observation_id, _)) = listed.first_key_value() {
            return Err(StoreError::EvidenceMismatch {
                snapshot_id: snapshot_id.to_owned(),
                observation_id: (*observation_id).to_owned(),
            });
        }

        let reconciled = reconcile(ReconcileInput {
            run_id: evidence.run_id,
            cutoff_unix_ms: evidence.cutoff_unix_ms,
            observations: evidence.observations,
            intervals: evidence.intervals,
        })
        .map_err(|source| StoreError::Reconcile {
            snapshot_id: snapshot_id.to_owned(),
            source,
        })?;
        Ok(reconciled)
    }

    fn ensure_dir(&self, parts: &[&str]) -> Result<PathBuf, StoreError> {
        self.dir_chain(parts, true)?
            .ok_or_else(|| io_error(&self.common_dir, io::ErrorKind::NotFound.into()))
    }

    /// Walks the store path under the common directory. Each component must be
    /// a real directory, and the final path must resolve inside the common dir.
    fn dir_chain(&self, parts: &[&str], create: bool) -> Result<Option<PathBuf>, StoreError> {
        let mut current = self.common_dir.clone();
        for part in STORE_COMPONENTS.iter().chain(parts) {
            current.push(part);
            match fs::symlink_metadata(&current) {
                Ok(metadata) => check_real_dir(&current, &metadata)?,
                Err(e) if e.kind() == io::ErrorKind::NotFound && create => {
                    match fs::create_dir(&current) {
                        Ok(()) => {}
                        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                            let metadata = fs::symlink_metadata(&current)
                                .map_err(|e| io_error(&current, e))?;
                            check_real_dir(&current, &metadata)?;
                        }
                        Err(e) => return Err(io_error(&current, e)),
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                Err(e) => return Err(io_error(&current, e)),
            }
        }
        let resolved = fs::canonicalize(&current).map_err(|e| io_error(&current, e))?;
        if !resolved.starts_with(&self.common_dir) {
            return Err(StoreError::SymlinkEscape(current));
        }
        Ok(Some(current))
    }
}

/// Exclusive lock on one run directory. The lock releases when dropped.
struct RunLock(File);

impl RunLock {
    fn acquire(run_dir: &Path) -> Result<Self, StoreError> {
        let path = run_dir.join(LOCK_FILE);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(StoreError::SymlinkEscape(path));
            }
            Ok(_) => {}
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(io_error(&path, e)),
        }
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| io_error(&path, e))?;
        FileExt::lock_exclusive(&file).map_err(|e| io_error(&path, e))?;
        Ok(Self(file))
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}

fn check_real_dir(path: &Path, metadata: &fs::Metadata) -> Result<(), StoreError> {
    if metadata.file_type().is_symlink() {
        return Err(StoreError::SymlinkEscape(path.to_owned()));
    }
    if !metadata.is_dir() {
        return Err(io_error(
            path,
            io::Error::new(
                io::ErrorKind::NotADirectory,
                "store path is not a directory",
            ),
        ));
    }
    Ok(())
}

fn check_manifest_identity(
    manifest: &Manifest,
    kind: RecordKind,
    run_id: &str,
    record_id: &str,
) -> Result<(), StoreError> {
    if manifest.kind != kind || manifest.run_id != run_id || manifest.record_id != record_id {
        return Err(malformed(
            record_id,
            "manifest identity differs from its path",
        ));
    }
    Ok(())
}

fn read_manifest(path: &Path, record_id: &str) -> Result<Option<Manifest>, StoreError> {
    let Some(bytes) = read_regular_file(path, record_id)? else {
        return Ok(None);
    };
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|e| malformed(record_id, e.to_string()))
}

/// Reads a regular file. A missing file yields `None`, and a symlink is rejected.
fn read_regular_file(path: &Path, record_id: &str) -> Result<Option<Vec<u8>>, StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(StoreError::SymlinkEscape(path.to_owned()))
        }
        Ok(metadata) if !metadata.is_file() => {
            Err(malformed(record_id, "store entry is not a regular file"))
        }
        Ok(_) => fs::read(path).map(Some).map_err(|e| io_error(path, e)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(io_error(path, e)),
    }
}

/// Writes a temporary file, syncs it, and renames it to `target`. The caller
/// holds the run lock and has confirmed that `target` does not exist.
fn write_then_rename(temp: &Path, target: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(temp)
            .map_err(|e| io_error(temp, e))?;
        file.write_all(bytes).map_err(|e| io_error(temp, e))?;
        file.sync_all().map_err(|e| io_error(temp, e))?;
    }
    fs::rename(temp, target).map_err(|e| io_error(target, e))
}

fn temp_path(kind_dir: &Path, record_id: &str, suffix: &str) -> PathBuf {
    kind_dir.join(format!(".{record_id}{suffix}.tmp"))
}

fn record_name(record_id: &str) -> String {
    format!("{record_id}{RECORD_SUFFIX}")
}

fn manifest_name(record_id: &str) -> String {
    format!("{record_id}{MANIFEST_SUFFIX}")
}

/// Accepts ASCII letters, digits, `-`, `_`, and `.`, with a leading
/// alphanumeric. Windows device names are rejected because they cannot name files.
fn validate_id(id: &str) -> Result<(), StoreError> {
    let shaped = !id.is_empty()
        && id.len() <= MAX_ID_LEN
        && id.starts_with(|c: char| c.is_ascii_alphanumeric())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
        && !id.contains("..");
    if shaped && !is_windows_device_name(id) {
        return Ok(());
    }
    Err(StoreError::InvalidId(id.to_owned()))
}

fn is_windows_device_name(id: &str) -> bool {
    let stem = id.split('.').next().unwrap_or(id).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || stem
            .strip_prefix("COM")
            .or_else(|| stem.strip_prefix("LPT"))
            .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

fn io_error(path: &Path, source: io::Error) -> StoreError {
    StoreError::Io {
        path: path.to_owned(),
        source,
    }
}

fn malformed(id: &str, reason: impl Into<String>) -> StoreError {
    StoreError::Malformed {
        id: id.to_owned(),
        reason: reason.into(),
    }
}
