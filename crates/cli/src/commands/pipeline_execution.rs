use std::path::{Path, PathBuf};
use std::{fs, process::Command};

use fs2::FileExt;
use sha2::{Digest, Sha256};

#[derive(Debug)]
pub(crate) enum GenerationError {
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    Build {
        status: String,
        stderr: String,
    },
    InvalidExisting {
        path: PathBuf,
        expected: String,
        observed: String,
    },
}

impl std::fmt::Display for GenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io {
                operation,
                path,
                message,
            } => write!(f, "{operation} {}: {message}", path.display()),
            Self::Build { status, stderr } => write!(f, "cargo build failed ({status}): {stderr}"),
            Self::InvalidExisting {
                path,
                expected,
                observed,
            } => write!(
                f,
                "existing generation {} has hash {observed}, expected {expected}",
                path.display()
            ),
        }
    }
}

fn io_error(operation: &'static str, path: &Path, error: std::io::Error) -> GenerationError {
    GenerationError::Io {
        operation,
        path: path.to_path_buf(),
        message: error.to_string(),
    }
}

fn sha256(path: &Path) -> Result<String, GenerationError> {
    let bytes = fs::read(path).map_err(|error| io_error("read executable", path, error))?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Build and publish a private executable generation for one canonical worktree.
pub(crate) fn build_private_generation(
    worktree: &Path,
    manifest: &Path,
) -> Result<PathBuf, GenerationError> {
    let worktree = fs::canonicalize(worktree)
        .map_err(|error| io_error("canonicalize worktree", worktree, error))?;
    let manifest = fs::canonicalize(manifest)
        .map_err(|error| io_error("canonicalize manifest", manifest, error))?;
    if !manifest.starts_with(&worktree) {
        return Err(GenerationError::Io {
            operation: "validate manifest containment",
            path: manifest,
            message: "manifest is outside the worktree".into(),
        });
    }

    let root = worktree.join("target/gal-pipeline");
    let cargo_target = root.join("cargo-target");
    let generations = root.join("bin");
    fs::create_dir_all(&cargo_target)
        .map_err(|error| io_error("create private Cargo target", &cargo_target, error))?;
    fs::create_dir_all(&generations)
        .map_err(|error| io_error("create generation directory", &generations, error))?;
    let lock_path = root.join("build.lock");
    let lock = fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| io_error("open build lock", &lock_path, error))?;
    lock.lock_exclusive()
        .map_err(|error| io_error("lock worktree build", &lock_path, error))?;

    let result = build_and_publish(&worktree, &manifest, &cargo_target, &generations);
    let unlock_result = FileExt::unlock(&lock)
        .map_err(|error| io_error("unlock worktree build", &lock_path, error));
    result.and_then(|path| {
        unlock_result?;
        Ok(path)
    })
}

fn build_and_publish(
    worktree: &Path,
    manifest: &Path,
    cargo_target: &Path,
    generations: &Path,
) -> Result<PathBuf, GenerationError> {
    let cargo_worktree = strip_windows_verbatim_prefix(worktree);
    let cargo_manifest = strip_windows_verbatim_prefix(manifest);
    let cargo_target_path = strip_windows_verbatim_prefix(cargo_target);
    let output = Command::new("cargo")
        .current_dir(cargo_worktree)
        .arg("build")
        .arg("--manifest-path")
        .arg(cargo_manifest)
        .arg("--bin")
        .arg("gal")
        .env("CARGO_TARGET_DIR", cargo_target_path)
        .output()
        .map_err(|error| io_error("start cargo build", Path::new("cargo"), error))?;
    if !output.status.success() {
        return Err(GenerationError::Build {
            status: output.status.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }

    #[cfg(windows)]
    let built = cargo_target.join("debug/gal.exe");
    #[cfg(not(windows))]
    let built = cargo_target.join("debug/gal");
    let digest = sha256(&built)?;
    let destination = generations.join(&digest);
    #[cfg(windows)]
    let executable_name = "gal.exe";
    #[cfg(not(windows))]
    let executable_name = "gal";
    let executable = destination.join(executable_name);

    if destination.exists() {
        let observed = sha256(&executable)?;
        if observed != digest {
            return Err(GenerationError::InvalidExisting {
                path: executable,
                expected: digest,
                observed,
            });
        }
        return Ok(executable);
    }

    let staging = generations.join(format!(".staging-{}-{}", std::process::id(), digest));
    if staging.exists() {
        fs::remove_dir_all(&staging)
            .map_err(|error| io_error("remove stale staging directory", &staging, error))?;
    }
    fs::create_dir(&staging)
        .map_err(|error| io_error("create staging directory", &staging, error))?;
    let staged_executable = staging.join(executable_name);
    let publish = (|| {
        fs::copy(&built, &staged_executable)
            .map_err(|error| io_error("copy executable to staging", &staged_executable, error))?;
        let staged_hash = sha256(&staged_executable)?;
        if staged_hash != digest {
            return Err(GenerationError::InvalidExisting {
                path: staged_executable.clone(),
                expected: digest.clone(),
                observed: staged_hash,
            });
        }
        fs::rename(&staging, &destination)
            .map_err(|error| io_error("publish immutable generation", &destination, error))?;
        Ok(executable.clone())
    })();
    if publish.is_err() && staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    publish
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ExecutionLane {
    Source,
    Downstream,
}

impl ExecutionLane {
    pub(crate) fn diagnostic(self) -> &'static str {
        match self {
            Self::Source => {
                "execution lane: source worktree; use its worktree-private GAL executable"
            }
            Self::Downstream => {
                "execution lane: downstream repository; use its approved GAL executable"
            }
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum IdentityError {
    Missing(PathBuf),
    Canonicalize {
        path: PathBuf,
        message: String,
    },
    CaseAlias {
        requested: PathBuf,
        canonical: PathBuf,
    },
    OutsideApprovedRoots(PathBuf),
    OtherWorktree(PathBuf),
    SymlinkOrJunctionEscape {
        path: PathBuf,
        canonical: PathBuf,
    },
}

impl std::fmt::Display for IdentityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(path) => write!(f, "path is missing: {}", path.display()),
            Self::Canonicalize { path, message } => {
                write!(f, "cannot canonicalize {}: {message}", path.display())
            }
            Self::CaseAlias {
                requested,
                canonical,
            } => write!(
                f,
                "case alias: {} resolves as {}",
                requested.display(),
                canonical.display()
            ),
            Self::OutsideApprovedRoots(path) => {
                write!(f, "path is outside approved roots: {}", path.display())
            }
            Self::OtherWorktree(path) => {
                write!(f, "path belongs to another worktree: {}", path.display())
            }
            Self::SymlinkOrJunctionEscape { path, canonical } => write!(
                f,
                "symlink or junction escape: {} resolves to {}",
                path.display(),
                canonical.display()
            ),
        }
    }
}

fn canonical(path: &Path) -> Result<PathBuf, IdentityError> {
    std::fs::canonicalize(path)
        .map(|resolved| normalize_windows_prefix(resolved))
        .map_err(|error| {
            if !path.exists() {
                IdentityError::Missing(path.to_path_buf())
            } else {
                IdentityError::Canonicalize {
                    path: path.to_path_buf(),
                    message: error.to_string(),
                }
            }
        })
}

fn normalize_windows_prefix(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(stripped) = value.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path
}

pub(crate) fn strip_windows_verbatim_prefix(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let value = path.to_string_lossy();
        if let Some(stripped) = value.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path.to_path_buf()
}

/// Render a path for execution bindings and receipts: drop the Windows
/// verbatim prefix, then use `/` separators. Every writer and comparer of
/// `ExecutionBinding` paths and receipt `executable_path` uses this form.
pub(crate) fn binding_path_text(path: &Path) -> String {
    strip_windows_verbatim_prefix(path)
        .to_string_lossy()
        .replace('\\', "/")
}

/// True when `dir` is the root of a GAL source checkout, identified by the
/// CLI package name in `crates/cli/Cargo.toml`.
pub(crate) fn is_gal_source_checkout(dir: &Path) -> bool {
    let manifest = dir.join("crates/cli/Cargo.toml");
    let Ok(contents) = fs::read_to_string(manifest) else {
        return false;
    };
    let mut in_package = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_package = line == "[package]";
        } else if in_package {
            if let Some((key, value)) = line.split_once('=') {
                if key.trim() == "name" {
                    return value.trim().trim_matches('"') == "gal-cli";
                }
            }
        }
    }
    false
}

/// Resolve the canonical worktree root for a starting directory: the nearest
/// ancestor that is a GAL source checkout, else the git top-level of `start`,
/// else `start` itself.
pub(crate) fn resolve_worktree_root(start: &Path) -> PathBuf {
    if let Some(root) = start.ancestors().find(|dir| is_gal_source_checkout(dir)) {
        return root.to_path_buf();
    }
    let toplevel = Command::new("git")
        .arg("-C")
        .arg(strip_windows_verbatim_prefix(start))
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty());
    if let Some(toplevel) = toplevel {
        if let Ok(canonical) = fs::canonicalize(&toplevel) {
            return strip_windows_verbatim_prefix(&canonical);
        }
    }
    start.to_path_buf()
}

/// Worktree root for the current process directory.
pub(crate) fn current_worktree_root() -> std::io::Result<PathBuf> {
    std::env::current_dir().map(|cwd| resolve_worktree_root(&cwd))
}

fn contains(root: &Path, path: &Path) -> bool {
    path.starts_with(root)
}

fn is_other_worktree(path: &Path, worktree: &Path) -> bool {
    path.components().any(|component| {
        ["worktrees", ".worktrees"].iter().any(|name| {
            component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(name)
        })
    }) && !contains(worktree, path)
}

pub(crate) struct ExecutionIdentity {
    pub(crate) worktree: PathBuf,
    pub(crate) executable: PathBuf,
    pub(crate) approved_roots: Vec<PathBuf>,
    pub(crate) lane: ExecutionLane,
}

pub(crate) fn identify(
    worktree: &Path,
    executable: &Path,
    approved_roots: &[PathBuf],
) -> Result<ExecutionIdentity, IdentityError> {
    let canonical_worktree = canonical(worktree)?;
    let canonical_executable = canonical(executable)?;
    let canonical_roots = approved_roots
        .iter()
        .map(|root| canonical(root))
        .collect::<Result<Vec<_>, _>>()?;

    for (requested, resolved) in [
        (worktree, &canonical_worktree),
        (executable, &canonical_executable),
    ] {
        let requested_text = requested.to_string_lossy();
        let resolved_text = resolved.to_string_lossy();
        if requested.is_absolute()
            && requested_text.eq_ignore_ascii_case(&resolved_text)
            && requested != resolved
        {
            return Err(IdentityError::CaseAlias {
                requested: requested.to_path_buf(),
                canonical: resolved.clone(),
            });
        }
    }
    if executable.is_absolute() {
        let requested = normalize_windows_prefix(executable.to_path_buf());
        if requested != canonical_executable {
            return Err(IdentityError::SymlinkOrJunctionEscape {
                path: executable.to_path_buf(),
                canonical: canonical_executable,
            });
        }
    }
    if !canonical_roots
        .iter()
        .any(|root| contains(root, &canonical_executable))
    {
        return Err(IdentityError::OutsideApprovedRoots(canonical_executable));
    }
    if is_other_worktree(&canonical_executable, &canonical_worktree) {
        return Err(IdentityError::OtherWorktree(canonical_executable));
    }
    let lane = if contains(&canonical_worktree, &canonical_executable) {
        ExecutionLane::Source
    } else {
        ExecutionLane::Downstream
    };
    Ok(ExecutionIdentity {
        worktree: canonical_worktree,
        executable: canonical_executable,
        approved_roots: canonical_roots,
        lane,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn cargo_paths_strip_windows_verbatim_prefix() {
        let path = Path::new(r"\\?\C:\worktree\target");
        #[cfg(windows)]
        assert_eq!(
            strip_windows_verbatim_prefix(path),
            PathBuf::from(r"C:\worktree\target")
        );
        #[cfg(not(windows))]
        assert_eq!(strip_windows_verbatim_prefix(path), path);
    }

    #[test]
    fn binding_path_text_strips_verbatim_prefix_and_uses_forward_slashes() {
        #[cfg(windows)]
        assert_eq!(
            binding_path_text(Path::new(r"\\?\C:\worktree\target\gal.exe")),
            "C:/worktree/target/gal.exe"
        );
        assert_eq!(
            binding_path_text(Path::new("/worktree/target/gal")),
            "/worktree/target/gal"
        );
    }

    fn git(dir: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .current_dir(dir)
            .args(args)
            .output()
            .is_ok_and(|output| output.status.success())
    }

    #[test]
    fn worktree_root_resolves_source_checkout_from_subdirectory() {
        let tmp = TempDir::new().unwrap();
        let root = fs::canonicalize(tmp.path()).unwrap();
        fs::create_dir_all(root.join("crates/cli")).unwrap();
        fs::write(
            root.join("crates/cli/Cargo.toml"),
            "[package]\nname = \"gal-cli\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        let nested = root.join("docs/nested");
        fs::create_dir_all(&nested).unwrap();
        assert_eq!(resolve_worktree_root(&nested), root);
        assert_eq!(resolve_worktree_root(&root.join("crates")), root);
        assert_eq!(resolve_worktree_root(&root), root);
    }

    #[test]
    fn worktree_root_falls_back_to_git_toplevel_then_start() {
        let tmp = TempDir::new().unwrap();
        let repo = tmp.path().join("downstream");
        let nested = repo.join("src/deep");
        fs::create_dir_all(&nested).unwrap();
        assert!(git(&repo, &["init", "-q"]), "git init failed");
        let expected = strip_windows_verbatim_prefix(&fs::canonicalize(&repo).unwrap());
        assert_eq!(resolve_worktree_root(&nested), expected);

        let plain = tmp.path().join("plain");
        fs::create_dir_all(&plain).unwrap();
        if !git(&plain, &["rev-parse", "--show-toplevel"]) {
            assert_eq!(resolve_worktree_root(&plain), plain);
        }

        let unrelated = tmp.path().join("unrelated");
        fs::create_dir_all(unrelated.join("crates/cli")).unwrap();
        fs::write(
            unrelated.join("crates/cli/Cargo.toml"),
            "[package]\nname = \"unrelated-cli\"\n",
        )
        .unwrap();
        assert!(!is_gal_source_checkout(&unrelated));
    }

    #[test]
    fn pipeline_execution_classifies_source_and_downstream_lanes() {
        let tmp = TempDir::new().unwrap();
        let source = tmp.path().join("source");
        let downstream = tmp.path().join("downstream-repo");
        let downstream_bin = tmp.path().join("approved-bin");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::create_dir_all(&downstream).unwrap();
        std::fs::create_dir_all(&downstream_bin).unwrap();
        let source_exe = source.join("gal");
        let downstream_exe = downstream_bin.join("gal");
        std::fs::write(&source_exe, "source").unwrap();
        std::fs::write(&downstream_exe, "downstream").unwrap();
        let identity = identify(&source, &source_exe, &[tmp.path().to_path_buf()]).unwrap();
        assert_eq!(identity.lane, ExecutionLane::Source);
        assert_eq!(
            identity.worktree,
            normalize_windows_prefix(std::fs::canonicalize(&source).unwrap())
        );
        assert_eq!(
            identity.executable,
            normalize_windows_prefix(std::fs::canonicalize(&source_exe).unwrap())
        );
        assert_eq!(
            identify(&downstream, &downstream_exe, &[tmp.path().to_path_buf()])
                .unwrap()
                .lane,
            ExecutionLane::Downstream
        );
        assert_eq!(
            ExecutionLane::Source.diagnostic(),
            "execution lane: source worktree; use its worktree-private GAL executable"
        );
        assert_eq!(
            ExecutionLane::Downstream.diagnostic(),
            "execution lane: downstream repository; use its approved GAL executable"
        );
        #[cfg(windows)]
        {
            let alias = PathBuf::from(source_exe.to_string_lossy().replace("source", "SOURCE"));
            assert!(matches!(
                identify(&source, &alias, &[tmp.path().to_path_buf()]),
                Err(IdentityError::CaseAlias { .. })
            ));
        }
    }

    #[test]
    fn pipeline_execution_rejects_missing_and_outside_root_paths() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("root");
        let outside = tmp.path().join("outside");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(&outside, "gal").unwrap();
        assert!(matches!(
            identify(&root, &root.join("missing"), &[root.clone()]),
            Err(IdentityError::Missing(_))
        ));
        assert!(matches!(
            identify(&root, &outside, &[root.clone()]),
            Err(IdentityError::OutsideApprovedRoots(_))
        ));
    }

    #[test]
    fn pipeline_execution_rejects_other_worktree_and_links() {
        let tmp = TempDir::new().unwrap();
        let first = tmp.path().join("first");
        let second = tmp.path().join("worktrees").join("second");
        std::fs::create_dir_all(&first).unwrap();
        std::fs::create_dir_all(&second).unwrap();
        let exe = second.join("gal");
        std::fs::write(&exe, "gal").unwrap();
        assert!(matches!(
            identify(&first, &exe, &[tmp.path().to_path_buf()]),
            Err(IdentityError::OtherWorktree(_))
        ));
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            let alias = first.join("linked-gal");
            symlink(&exe, &alias).unwrap();
            assert!(matches!(
                identify(&first, &alias, &[tmp.path().to_path_buf()]),
                Err(IdentityError::SymlinkOrJunctionEscape { .. })
            ));
        }
        #[cfg(windows)]
        {
            let link_root = first.join("junction");
            let status = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link_root)
                .arg(&second)
                .status()
                .expect("cmd must be available on Windows");
            assert!(status.success(), "could not create junction fixture");
            let junction_exe = link_root.join("gal");
            assert!(matches!(
                identify(&first, &junction_exe, &[tmp.path().to_path_buf()]),
                Err(IdentityError::SymlinkOrJunctionEscape { .. })
            ));
        }
    }
}
