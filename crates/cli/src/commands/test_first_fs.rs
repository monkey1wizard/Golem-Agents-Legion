//! `test_first_fs` — narrow shared owner for durability and atomic-publication
//! primitives shared by the boundary, restore, transition, and probe commands.
//!
//! Provides atomic file publication (candidate creation, sync, atomic rename,
//! directory sync, readback verification) and scoped snapshot manifest/blob capture.

use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// One row in a `.snapshot.tsv` file.
/// Format: `state\tkind\tmode\tsha256-or-dash\trepo-relative-forward-slash-path`
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct SnapshotEntry {
    pub state: String,
    pub kind: String,
    pub mode: String,
    pub sha256_or_dash: String,
    pub path: String,
}

impl SnapshotEntry {
    pub fn to_tsv_line(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\n",
            self.state, self.kind, self.mode, self.sha256_or_dash, self.path
        )
    }

    #[allow(dead_code)]
    pub fn parse_tsv_line(line: &str) -> Result<Self, String> {
        let trimmed = line.trim_end_matches('\r').trim_end_matches('\n');
        let parts: Vec<&str> = trimmed.split('\t').collect();
        if parts.len() != 5 {
            return Err(format!("snapshot TSV line requires 5 fields: {line:?}"));
        }
        Ok(Self {
            state: parts[0].to_string(),
            kind: parts[1].to_string(),
            mode: parts[2].to_string(),
            sha256_or_dash: parts[3].to_string(),
            path: parts[4].to_string(),
        })
    }
}

pub fn format_snapshot_tsv(entries: &[SnapshotEntry]) -> String {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.path.cmp(&b.path));
    let mut out = String::new();
    for entry in sorted {
        out.push_str(&entry.to_tsv_line());
    }
    out
}

pub fn parse_snapshot_tsv(content: &str) -> Result<Vec<SnapshotEntry>, String> {
    let mut entries = Vec::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        entries.push(SnapshotEntry::parse_tsv_line(line)?);
    }
    Ok(entries)
}

/// Helper to sync parent directory across Unix and Windows. This is the one
/// real implementation; cleanup and transition call it rather than keeping
/// their own copies, which were silent no-ops on Windows.
pub(crate) fn sync_dir(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    {
        let dir = std::fs::File::open(path)
            .map_err(|e| format!("cannot open directory for sync {}: {e}", path.display()))?;
        dir.sync_all()
            .map_err(|e| format!("cannot sync directory {}: {e}", path.display()))?;
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_READ_ATTRIBUTES: u32 = 0x0080;
        const FILE_SHARE_READ: u32 = 1;
        const FILE_SHARE_WRITE: u32 = 2;
        const FILE_SHARE_DELETE: u32 = 4;
        const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x02000000;

        let dir = OpenOptions::new()
            .access_mode(FILE_READ_ATTRIBUTES)
            .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
            .custom_flags(FILE_FLAG_BACKUP_SEMANTICS)
            .open(path)
            .map_err(|e| format!("cannot open directory for sync {}: {e}", path.display()))?;
        if let Err(e) = dir.sync_all() {
            // On Windows Win32 API, FlushFileBuffers on a directory handle fails with
            // ERROR_ACCESS_DENIED (5) by design.
            if e.raw_os_error() != Some(5) {
                return Err(format!("cannot sync directory {}: {e}", path.display()));
            }
        }
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        let dir = std::fs::File::open(path)
            .map_err(|e| format!("cannot open directory for sync {}: {e}", path.display()))?;
        dir.sync_all()
            .map_err(|e| format!("cannot sync directory {}: {e}", path.display()))?;
        Ok(())
    }
}

/// Atomically publish raw bytes to `path` with fsync and readback verification.
pub fn atomic_publish_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("cannot create parent dir {}: {e}", parent.display()))?;
    }

    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("tmp");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let tmp_name = format!("{file_name}.tmp.{timestamp}");
    let candidate_path = parent.join(&tmp_name);

    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate_path)
            .map_err(|e| {
                format!(
                    "cannot create candidate file {}: {e}",
                    candidate_path.display()
                )
            })?;
        file.write_all(bytes).map_err(|e| {
            format!(
                "cannot write candidate file {}: {e}",
                candidate_path.display()
            )
        })?;
        file.sync_all().map_err(|e| {
            format!(
                "cannot sync candidate file {}: {e}",
                candidate_path.display()
            )
        })?;
    }

    sync_dir(parent)?;

    fs::rename(&candidate_path, path).map_err(|e| {
        let _ = fs::remove_file(&candidate_path);
        format!(
            "cannot rename candidate {} to {}: {e}",
            candidate_path.display(),
            path.display()
        )
    })?;

    sync_dir(parent)?;

    let read_back =
        fs::read(path).map_err(|e| format!("readback failed for {}: {e}", path.display()))?;
    if read_back != bytes {
        return Err(format!(
            "readback mismatch for {}: written {} bytes, read back {} bytes",
            path.display(),
            bytes.len(),
            read_back.len()
        ));
    }

    Ok(())
}

/// Atomically publish a UTF-8 string to `path`.
#[allow(dead_code)]
pub fn atomic_publish_str(path: &Path, content: &str) -> Result<(), String> {
    atomic_publish_bytes(path, content.as_bytes())
}

/// Bind a caller-supplied `--receipt` destination without writing anything.
///
/// Callers that mutate the repository before publishing use this first, so a
/// rejected path is discovered before the mutation rather than after it.
pub fn validate_receipt_destination(
    repo_root: &Path,
    relative: &Path,
) -> Result<ValidatedRepoPath, String> {
    ValidatedRepoPath::new(
        repo_root,
        relative,
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("receipt path validation failed: {e}"))
}

/// Atomically publish a receipt at a caller-supplied repo-relative path.
///
/// Every `--receipt` destination goes through here so the write side is bound
/// by the same validator the read side already uses. Joining an operator string
/// onto the repo root is not equivalent: `Path::join` drops the base for an
/// absolute path, and a `..` component walks out of the tree, so the plain join
/// this replaces was an unconstrained file-write primitive. Unlike
/// `publish_created`, an existing receipt is a normal overwrite — a receipt is
/// rewritten on every run — but it must still be a regular file, never a link
/// or a directory.
///
/// The binding covers the path and its parents, not the leaf identity across
/// the write. `atomic_publish_bytes` publishes through its own temporary file
/// and rename, so the leaf identity legitimately changes on every successful
/// overwrite and there is no exposed seam at which to rebind it. Claiming a
/// post-write identity guarantee here would be claiming more than the write
/// primitive gives.
pub fn publish_receipt(repo_root: &Path, relative: &Path, bytes: &[u8]) -> Result<(), String> {
    let binding = validate_receipt_destination(repo_root, relative)?;
    let full = repo_root.join(relative);
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create receipt directory: {e}"))?;
    }
    binding
        .bind_materialized_parents()
        .map_err(|e| format!("receipt parent binding failed: {e}"))?;
    atomic_publish_bytes(&full, bytes)
}

/// Determine mode string for a regular file.
fn file_mode_string(_path: &Path, meta: &fs::Metadata) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        format!("{:o}", meta.permissions().mode())
    }
    #[cfg(not(unix))]
    {
        let _ = meta;
        "100644".to_string()
    }
}

pub fn validate_safe_component(s: &str, name: &str) -> Result<(), String> {
    if s.is_empty()
        || s.contains('/')
        || s.contains('\\')
        || s.contains("..")
        || s.contains(':')
        || s.contains('\0')
        || s.trim() != s
    {
        return Err(format!(
            "invalid artifact identity: invalid component for {name}: '{s}'"
        ));
    }
    Ok(())
}

pub fn is_lower_hex_64(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

pub fn validate_artifact_identity(
    plan_slug: &str,
    task_id: &str,
    generation: u32,
    contract_digest: &str,
) -> Result<(), String> {
    validate_safe_component(plan_slug, "plan_slug")?;
    if !task_id.starts_with("T-")
        || task_id.len() <= 2
        || !task_id[2..].chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!(
            "invalid artifact identity: task_id must be T-<digits>, got '{task_id}'"
        ));
    }
    if generation == 0 {
        return Err("invalid artifact identity: generation must be positive u32".to_string());
    }
    if !is_lower_hex_64(contract_digest) {
        return Err(format!(
            "invalid artifact identity: contract_digest must be 64 lowercase hex characters, got '{contract_digest}'"
        ));
    }
    Ok(())
}

fn validated_read(repo_root: &Path, rel_path: &Path) -> Result<(Vec<u8>, fs::Metadata), String> {
    let binding = ValidatedRepoPath::new(
        repo_root,
        rel_path,
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("ValidatedRepoPath error for '{}': {e}", rel_path.display()))?;
    if !binding.exists() {
        return Err(format!("required file is missing: {}", rel_path.display()));
    }
    binding
        .recheck()
        .map_err(|e| format!("identity check failed for '{}': {e}", rel_path.display()))?;
    let metadata = fs::symlink_metadata(binding.full_path())
        .map_err(|e| format!("metadata error for '{}': {e}", rel_path.display()))?;
    let bytes = fs::read(binding.full_path())
        .map_err(|e| format!("cannot read '{}': {e}", rel_path.display()))?;
    binding.recheck().map_err(|e| {
        format!(
            "identity changed while reading '{}': {e}",
            rel_path.display()
        )
    })?;
    Ok((bytes, metadata))
}

fn snapshot_root(
    plan_slug: &str,
    task_id: &str,
    generation: u32,
    contract_digest: &str,
) -> PathBuf {
    PathBuf::from(".dev")
        .join("pipeline")
        .join("snapshots")
        .join(plan_slug)
        .join(task_id)
        .join(format!("g{generation}-c{contract_digest}"))
}

fn write_staged_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    atomic_publish_bytes(path, bytes)
}

fn remove_staging(path: &Path) {
    let _ = fs::remove_dir_all(path);
}

/// Capture snapshot manifest and blobs for the declared phase paths.
/// Writes manifest to `.dev/pipeline/snapshots/<slug>/<task>/g<gen>-c<contract_digest>/<phase>.snapshot.tsv`
/// and blobs to `blobs/<sha256>.bin`.
pub fn capture_snapshot_manifest_and_blobs(
    repo_root: &Path,
    plan_slug: &str,
    task_id: &str,
    generation: u32,
    contract_digest: &str,
    snapshot_phase: &str,
    paths: &[String],
) -> Result<PathBuf, String> {
    validate_artifact_identity(plan_slug, task_id, generation, contract_digest)?;
    validate_safe_component(snapshot_phase, "snapshot_phase")?;
    if !matches!(
        snapshot_phase,
        "implementation-commit" | "pre-implement" | "state-recording" | "post-test" | "post-audit"
    ) {
        return Err(format!(
            "invalid artifact identity: unknown snapshot phase '{snapshot_phase}'"
        ));
    }

    let task_root = PathBuf::from(".dev")
        .join("pipeline")
        .join("snapshots")
        .join(plan_slug)
        .join(task_id);
    let mut selected_generation = generation;
    let mut rel_snapshot_dir =
        snapshot_root(plan_slug, task_id, selected_generation, contract_digest);
    let existing_manifest = rel_snapshot_dir.join(format!("{snapshot_phase}.snapshot.tsv"));
    if ValidatedRepoPath::new(
        repo_root,
        &existing_manifest,
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("snapshot path validation failed: {e}"))?
    .exists()
    {
        let (old, _) = validated_read(repo_root, &existing_manifest)?;
        // Same-generation capture is idempotent. A changed capture is assigned a
        // fresh generation so the old evidence remains immutable.
        let mut expected = Vec::new();
        for raw_path in paths {
            let norm = normalize_snapshot_path(raw_path)?;
            let (bytes, meta) = match validated_read(repo_root, Path::new(&norm)) {
                Ok(v) => v,
                Err(e) if e.contains("required file is missing") => {
                    expected.push(SnapshotEntry {
                        state: "missing".into(),
                        kind: "-".into(),
                        mode: "-".into(),
                        sha256_or_dash: "-".into(),
                        path: norm,
                    });
                    continue;
                }
                Err(e) => return Err(e),
            };
            let digest = format!("{:x}", Sha256::digest(&bytes));
            expected.push(SnapshotEntry {
                state: "present".into(),
                kind: "file".into(),
                mode: file_mode_string(Path::new(&norm), &meta),
                sha256_or_dash: digest,
                path: norm,
            });
        }
        if old == format_snapshot_tsv(&expected).as_bytes() {
            return Ok(repo_root.join(existing_manifest));
        }
        selected_generation = selected_generation
            .checked_add(1)
            .ok_or("snapshot generation overflow")?;
        rel_snapshot_dir = snapshot_root(plan_slug, task_id, selected_generation, contract_digest);
    }

    let rel_manifest_path = rel_snapshot_dir.join(format!("{snapshot_phase}.snapshot.tsv"));
    let staging = repo_root.join(&task_root).join(format!(
        ".staging-g{}-{}",
        selected_generation,
        std::process::id()
    ));
    remove_staging(&staging);
    fs::create_dir_all(staging.join("blobs"))
        .map_err(|e| format!("cannot create snapshot staging directory: {e}"))?;
    let mut entries = Vec::new();
    let fail_after_blob = std::env::var("GAL_TEST_FIRST_SNAPSHOT_FAIL_AFTER_BLOB")
        .or_else(|_| std::env::var("GAL_TEST_FIRST_FAIL_AFTER_BLOB"))
        .ok()
        .and_then(|v| v.parse::<usize>().ok());
    let mut published_blobs = 0usize;

    for raw_path in paths {
        let norm_path = normalize_snapshot_path(raw_path)?;
        match validated_read(repo_root, Path::new(&norm_path)) {
            Ok((bytes, meta)) => {
                let sha256_hex = format!("{:x}", Sha256::digest(&bytes));
                let mode = file_mode_string(Path::new(&norm_path), &meta);
                write_staged_file(
                    &staging.join("blobs").join(format!("{sha256_hex}.bin")),
                    &bytes,
                )
                .inspect_err(|_| {
                    remove_staging(&staging);
                })?;
                published_blobs += 1;
                if fail_after_blob == Some(published_blobs) {
                    remove_staging(&staging);
                    return Err("injected snapshot publication failure".to_string());
                }

                entries.push(SnapshotEntry {
                    state: "present".to_string(),
                    kind: "file".to_string(),
                    mode,
                    sha256_or_dash: sha256_hex,
                    path: norm_path,
                });
            }
            Err(e) if e.contains("required file is missing") => {
                entries.push(SnapshotEntry {
                    state: "missing".to_string(),
                    kind: "-".to_string(),
                    mode: "-".to_string(),
                    sha256_or_dash: "-".to_string(),
                    path: norm_path,
                });
            }
            Err(e) => {
                remove_staging(&staging);
                return Err(e);
            }
        }
    }

    let manifest_content = format_snapshot_tsv(&entries);
    write_staged_file(
        &staging.join(format!("{snapshot_phase}.snapshot.tsv")),
        manifest_content.as_bytes(),
    )
    .inspect_err(|_| {
        remove_staging(&staging);
    })?;
    sync_dir(&staging).inspect_err(|_| {
        remove_staging(&staging);
    })?;
    if let Err(e) = fs::create_dir_all(repo_root.join(&task_root)) {
        remove_staging(&staging);
        return Err(format!("cannot create snapshot parent: {e}"));
    }
    let final_dir = repo_root.join(&rel_snapshot_dir);
    if final_dir.exists() {
        remove_staging(&staging);
        return Err(format!(
            "snapshot generation already exists: {}",
            final_dir.display()
        ));
    }
    fs::rename(repo_root.join(&staging), &final_dir).map_err(|e| {
        remove_staging(&staging);
        format!("cannot promote snapshot generation: {e}")
    })?;
    sync_dir(final_dir.parent().ok_or("snapshot parent missing")?)?;

    Ok(repo_root.join(&rel_manifest_path))
}

fn normalize_snapshot_path(raw_path: &str) -> Result<String, String> {
    let path = raw_path.replace('\\', "/");
    let path = path.strip_prefix("./").unwrap_or(&path);
    if path.is_empty()
        || path.starts_with('/')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err(format!("invalid snapshot member path '{raw_path}'"));
    }
    Ok(path.to_string())
}

/// Verify an already-published snapshot without creating or replacing files.
pub fn verify_snapshot_manifest_and_blobs(
    repo_root: &Path,
    plan_slug: &str,
    task_id: &str,
    generation: u32,
    contract_digest: &str,
    snapshot_phase: &str,
    paths: &[String],
) -> Result<PathBuf, String> {
    validate_artifact_identity(plan_slug, task_id, generation, contract_digest)?;
    validate_safe_component(snapshot_phase, "snapshot_phase")?;
    if !matches!(
        snapshot_phase,
        "implementation-commit" | "pre-implement" | "state-recording" | "post-test" | "post-audit"
    ) {
        return Err(format!(
            "invalid artifact identity: unknown snapshot phase '{snapshot_phase}'"
        ));
    }
    let root = snapshot_root(plan_slug, task_id, generation, contract_digest);
    let manifest = root.join(format!("{snapshot_phase}.snapshot.tsv"));
    let (manifest_bytes, _) = validated_read(repo_root, &manifest)?;
    let entries = parse_snapshot_tsv(
        std::str::from_utf8(&manifest_bytes).map_err(|_| "manifest is not UTF-8")?,
    )?;
    let expected: std::collections::HashSet<String> = paths
        .iter()
        .map(|p| normalize_snapshot_path(p))
        .collect::<Result<_, _>>()?;
    if entries.len() != expected.len() || entries.iter().any(|e| !expected.contains(&e.path)) {
        return Err(
            "snapshot manifest path set does not match the active phase allowlist".to_string(),
        );
    }
    for entry in entries {
        let path = normalize_snapshot_path(&entry.path)?;
        if entry.state == "missing" {
            let member = ValidatedRepoPath::new(
                repo_root,
                Path::new(&path),
                ValidatedRepoPathMode::RegularFileOrMissing,
            )
            .map_err(|e| format!("snapshot path validation failed: {e}"))?;
            if entry.kind != "-"
                || entry.mode != "-"
                || entry.sha256_or_dash != "-"
                || member.exists()
            {
                return Err(format!("invalid missing snapshot row for '{path}'"));
            }
            continue;
        }
        if entry.state != "present"
            || entry.kind != "file"
            || !is_lower_hex_64(&entry.sha256_or_dash)
        {
            return Err(format!("invalid snapshot row for '{path}'"));
        }
        let (bytes, meta) = validated_read(repo_root, Path::new(&path))?;
        if format!("{:x}", Sha256::digest(&bytes)) != entry.sha256_or_dash
            || file_mode_string(Path::new(&path), &meta) != entry.mode
        {
            return Err(format!("snapshot member mismatch for '{path}'"));
        }
        let blob = root
            .join("blobs")
            .join(format!("{}.bin", entry.sha256_or_dash));
        let (blob_bytes, _) = validated_read(repo_root, &blob)?;
        if blob_bytes != bytes
            || format!("{:x}", Sha256::digest(&blob_bytes)) != entry.sha256_or_dash
        {
            return Err(format!("snapshot blob mismatch for '{path}'"));
        }
    }
    Ok(repo_root.join(manifest))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Compose plan-task IDs at runtime — never hardcode `T-NN` literals in
    // source (naming-gate provenance rule: plan-task IDs belong only in .dev).
    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    #[test]
    fn validate_safe_component_accepts_valid() {
        assert!(validate_safe_component("my-slug", "slug").is_ok());
        assert!(validate_safe_component(&tid(1), "task").is_ok());
        assert!(validate_safe_component("pre-test", "phase").is_ok());
    }

    #[test]
    fn validate_safe_component_rejects_traversal_and_invalid() {
        assert!(validate_safe_component("../foo", "slug").is_err());
        assert!(validate_safe_component("foo/bar", "task").is_err());
        assert!(validate_safe_component("foo\\bar", "phase").is_err());
        assert!(validate_safe_component("c:foo", "slug").is_err());
        assert!(validate_safe_component("", "task").is_err());
        assert!(validate_safe_component(" foo ", "phase").is_err());
    }

    #[test]
    fn capture_snapshot_rejects_invalid_contract_digest() {
        let repo = Path::new(".");
        let res = capture_snapshot_manifest_and_blobs(
            repo,
            "slug",
            &tid(1),
            1,
            "invalid_digest",
            "pre-test",
            &[],
        );
        assert!(res.is_err());
    }
}
