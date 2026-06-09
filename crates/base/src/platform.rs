//! Cross-platform filesystem primitives.
//!
//! OS-divergent operations previously duplicated across render + providers
//! (R-00/T-004): directory links (NTFS junction on Windows / symlink on Unix),
//! link removal, link detection, and the canonical-root atomic swap. Behavior is
//! byte-identical to the prior inline implementations; callers keep their own
//! error-message prefixes by mapping the returned error.

use std::io;
use std::path::Path;
use thiserror::Error;

/// Create a directory link `link` -> `target`.
///
/// Windows: NTFS directory junction via `mklink /J`. Unix: `symlink`.
pub fn create_dir_link(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd")
            .args([
                "/C",
                "mklink",
                "/J",
                &link.to_string_lossy(),
                &target.to_string_lossy(),
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::new(io::ErrorKind::Other, stderr.into_owned()));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(target, link)
    }
}

/// Remove a directory link.
///
/// Windows: `rmdir` (removes a junction without touching the target). Unix:
/// `remove_file` (removes the symlink). Callers keep their own existence guards.
pub fn remove_dir_link(path: &Path) -> io::Result<()> {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("cmd")
            .args(["/C", "rmdir", &path.to_string_lossy()])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(io::Error::new(io::ErrorKind::Other, stderr.into_owned()));
        }
        Ok(())
    }
    #[cfg(not(windows))]
    {
        std::fs::remove_file(path)
    }
}

/// Return `true` if `path` is a symlink or NTFS junction, even when the target is absent.
pub fn is_symlink_or_junction(path: &Path) -> bool {
    #[cfg(windows)]
    {
        // On Windows, metadata() follows junctions; symlink_metadata() does not.
        path.symlink_metadata()
            .map(|m| m.file_type().is_dir() || m.file_type().is_symlink())
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        path.is_symlink()
    }
}

/// Error from [`atomic_swap`]; callers map this to their own domain error type
/// (and re-add any message prefix) to preserve existing error text.
#[derive(Debug, Error)]
pub enum AtomicSwapError {
    #[error("canonical root has no parent")]
    NoParent,
    #[error("{0}")]
    BackupFailed(String),
    #[error("{0}")]
    MoveFailed(String),
}

/// Atomic swap: move `temp_dir` onto `canonical_root`.
///
/// Uses a backup-move-restore pattern for kill-mid-swap recovery. Platform-
/// agnostic (`fs::rename`), grouped here as the canonical-root swap primitive.
pub fn atomic_swap(temp_dir: &Path, canonical_root: &Path) -> Result<(), AtomicSwapError> {
    use std::fs;

    let parent = canonical_root.parent().ok_or(AtomicSwapError::NoParent)?;

    // Create backup path.
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let backup_name = format!(".gal-plugin-backup-{}", uuid);
    let backup_path = parent.join(backup_name);

    let had_existing_root = canonical_root.exists();

    // Backup existing root if it exists.
    if had_existing_root {
        fs::rename(canonical_root, &backup_path)
            .map_err(|e| AtomicSwapError::BackupFailed(e.to_string()))?;
    }

    // Move temp to canonical.
    match fs::rename(temp_dir, canonical_root) {
        Ok(()) => {
            // Success: remove backup.
            if backup_path.exists() {
                let _ = fs::remove_dir_all(&backup_path);
            }
            Ok(())
        }
        Err(e) => {
            // Failure: restore backup.
            if had_existing_root && backup_path.exists() && !canonical_root.exists() {
                let _ = fs::rename(&backup_path, canonical_root);
            }
            Err(AtomicSwapError::MoveFailed(e.to_string()))
        }
    }
}
