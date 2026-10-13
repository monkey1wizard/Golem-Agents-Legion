//! Cross-platform filesystem primitives.
//!
//! OS-divergent operations shared by the render and projection layers:
//! directory links (NTFS junction on Windows / symlink on Unix),
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
            return Err(io::Error::other(stderr.into_owned()));
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
            return Err(io::Error::other(stderr.into_owned()));
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
        use std::os::windows::fs::MetadataExt;

        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

        path.symlink_metadata()
            .map(|metadata| {
                metadata.file_type().is_symlink()
                    || (metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT) != 0
            })
            .unwrap_or(false)
    }
    #[cfg(not(windows))]
    {
        path.is_symlink()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[cfg(windows)]
    #[test]
    fn ordinary_directory_is_not_a_junction() {
        let temp = TempDir::new().unwrap();
        let ordinary_dir = temp.path().join("plain-dir");
        std::fs::create_dir(&ordinary_dir).unwrap();
        assert!(!is_symlink_or_junction(&ordinary_dir));
    }

    #[test]
    fn sync_dir_in_place_overwrites_and_prunes() {
        use std::fs;
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");
        // src: new content + a nested file.
        fs::create_dir_all(src.join("commands")).unwrap();
        fs::write(src.join("plugin.json"), "NEW").unwrap();
        fs::write(src.join("commands").join("gal.md"), "gal dispatch-script").unwrap();
        // dst: stale content (old file value + an extraneous file to prune).
        fs::create_dir_all(dst.join("commands")).unwrap();
        fs::write(dst.join("plugin.json"), "OLD").unwrap();
        fs::write(
            dst.join("commands").join("gal.md"),
            "scripts/gal.ps1 dispatch",
        )
        .unwrap();
        fs::write(dst.join("stale.txt"), "remove me").unwrap();

        sync_dir_in_place(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(dst.join("plugin.json")).unwrap(), "NEW");
        assert_eq!(
            fs::read_to_string(dst.join("commands").join("gal.md")).unwrap(),
            "gal dispatch-script"
        );
        assert!(
            !dst.join("stale.txt").exists(),
            "extraneous dst entry must be pruned"
        );
    }

    #[test]
    fn atomic_swap_normal_path_replaces_root() {
        use std::fs;
        let temp = TempDir::new().unwrap();
        let temp_dir = temp.path().join("render");
        let canonical = temp.path().join("canonical");
        fs::create_dir_all(&temp_dir).unwrap();
        fs::write(temp_dir.join("f.txt"), "new").unwrap();
        fs::create_dir_all(&canonical).unwrap();
        fs::write(canonical.join("old.txt"), "old").unwrap();

        atomic_swap(&temp_dir, &canonical).unwrap();
        assert_eq!(fs::read_to_string(canonical.join("f.txt")).unwrap(), "new");
        assert!(!canonical.join("old.txt").exists(), "old root replaced");
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
        match fs::rename(canonical_root, &backup_path) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::PermissionDenied => {
                // The canonical root cannot be renamed because a process holds an open
                // handle on the directory (e.g. a running plugin host keeps its
                // marketplace-source dir open — `gal install` from inside Claude Code).
                // The atomic backup-move-restore path is impossible, so fall back to a
                // NON-atomic in-place content sync: overwrite files and prune stale
                // entries. File-level writes succeed where a whole-dir rename fails.
                sync_dir_in_place(temp_dir, canonical_root)
                    .map_err(|e| AtomicSwapError::MoveFailed(e.to_string()))?;
                let _ = fs::remove_dir_all(temp_dir);
                return Ok(());
            }
            Err(e) => return Err(AtomicSwapError::BackupFailed(e.to_string())),
        }
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

/// Recursively sync `src` content into `dst` in place: overwrite every `src` file/dir
/// into `dst`, then remove `dst` entries absent from `src`. Non-atomic — the fallback
/// for [`atomic_swap`] when the canonical root is held by an open handle and cannot be
/// renamed. Only needs file-level writes, which succeed while a dir rename is denied.
fn sync_dir_in_place(src: &Path, dst: &Path) -> io::Result<()> {
    use std::fs;
    fs::create_dir_all(dst)?;
    // 1. Copy/overwrite every src entry into dst.
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            sync_dir_in_place(&from, &to)?;
        } else {
            // A directory currently at `to` must go before copying a file over it.
            if to.is_dir() && !is_symlink_or_junction(&to) {
                fs::remove_dir_all(&to)?;
            }
            fs::copy(&from, &to)?;
        }
    }
    // 2. Prune dst entries not present in src (best-effort; ignore locked entries).
    for entry in fs::read_dir(dst)? {
        let entry = entry?;
        if src.join(entry.file_name()).exists() {
            continue;
        }
        let p = entry.path();
        let _ = if is_symlink_or_junction(&p) {
            remove_dir_link(&p).or_else(|_| fs::remove_file(&p))
        } else if entry.file_type()?.is_dir() {
            fs::remove_dir_all(&p)
        } else {
            fs::remove_file(&p)
        };
    }
    Ok(())
}
