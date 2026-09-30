//! Fail-closed repo path validation and identity management.
//!
//! Enforces lexical containment, no-follow component identity, shape validation,
//! fail-closed metadata uncertainty, and identity recheck tokens.

use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

/// Structured error for repository path validation and identity verification.
#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum ValidatedRepoPathError {
    #[error("Invalid path: {reason}")]
    InvalidPath { reason: String },

    #[error("Uncertain identity: {reason}")]
    UncertainIdentity { reason: String },

    #[error("Changed identity: {reason}")]
    ChangedIdentity { reason: String },
}

/// Validation modes supported by `ValidatedRepoPath`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidatedRepoPathMode {
    /// Task and evidence members: regular file or non-existent path.
    RegularFileOrMissing,
    /// Existing cleanup roots under their expected parent directory.
    ValidatedDirectChildDirectory { expected_parent: PathBuf },
}

/// Unique filesystem identity token for a path component on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentityToken {
    pub dev: Option<u64>,
    pub ino: Option<u64>,
    pub volume_serial: Option<u64>,
    pub file_id: Option<[u8; 16]>,
}

impl FileIdentityToken {
    pub fn from_path(path: &Path, meta: &fs::Metadata) -> Result<Self, ValidatedRepoPathError> {
        #[cfg(unix)]
        {
            let _ = meta;
            let _ = path;
            use std::os::unix::fs::MetadataExt;
            Ok(Self {
                dev: Some(meta.dev()),
                ino: Some(meta.ino()),
                volume_serial: None,
                file_id: None,
            })
        }
        #[cfg(windows)]
        {
            let _ = meta;
            use std::fs::OpenOptions;
            use std::os::windows::fs::OpenOptionsExt;
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                FileIdInfo, GetFileInformationByHandleEx, FILE_FLAG_BACKUP_SEMANTICS,
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_ID_INFO, FILE_READ_ATTRIBUTES,
                FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
            };

            // FILE_FLAG_OPEN_REPARSE_POINT keeps this query no-follow, so a link is
            // identified as the link itself rather than its target.
            // FILE_FLAG_BACKUP_SEMANTICS is required to open a directory handle at all.
            let file = OpenOptions::new()
                .access_mode(FILE_READ_ATTRIBUTES)
                .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
                .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS)
                .open(path)
                .map_err(|e| ValidatedRepoPathError::UncertainIdentity {
                    reason: format!(
                        "Failed to open handle for identity query on {}: {}",
                        path.display(),
                        e
                    ),
                })?;

            let handle = file.as_raw_handle();
            // SAFETY: `handle` is live for the call and `info` is a correctly sized,
            // aligned writable FILE_ID_INFO buffer.
            let mut info: FILE_ID_INFO = unsafe { std::mem::zeroed() };
            let ret = unsafe {
                GetFileInformationByHandleEx(
                    handle as _,
                    FileIdInfo,
                    (&mut info as *mut FILE_ID_INFO).cast(),
                    std::mem::size_of::<FILE_ID_INFO>() as u32,
                )
            };
            if ret == 0 {
                return Err(ValidatedRepoPathError::UncertainIdentity {
                    reason: format!(
                        "GetFileInformationByHandleEx(FileIdInfo) failed for {}: {}",
                        path.display(),
                        io::Error::last_os_error()
                    ),
                });
            }

            Ok(Self {
                dev: None,
                ino: None,
                volume_serial: Some(info.VolumeSerialNumber),
                file_id: Some(info.FileId.Identifier),
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            let _ = meta;
            Err(ValidatedRepoPathError::UncertainIdentity {
                reason: "No trustworthy file identity API is available on this platform".into(),
            })
        }
    }
}

/// Recorded identity for each component in the path chain.
#[derive(Debug, Clone)]
pub struct ComponentIdentity {
    pub path: PathBuf,
    pub exists: bool,
    pub is_dir: bool,
    pub is_file: bool,
    pub identity_token: Option<FileIdentityToken>,
}

/// Validated repository path with recorded identity chain and recheck capability.
#[derive(Debug)]
pub struct ValidatedRepoPath {
    repo_root: PathBuf,
    rel_path: PathBuf,
    full_path: PathBuf,
    mode: ValidatedRepoPathMode,
    chain_identities: Vec<ComponentIdentity>,
}

fn is_symlink_or_reparse(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT = 0x400
        if (meta.file_attributes() & 0x400) != 0 {
            return true;
        }
    }
    false
}

fn validate_expected_parent(expected_parent: &Path) -> Result<(), ValidatedRepoPathError> {
    let raw = expected_parent.to_string_lossy();
    if raw.is_empty()
        || expected_parent.is_absolute()
        || raw.starts_with('/')
        || raw.starts_with('\\')
        || raw.starts_with("//")
        || raw.starts_with("\\\\")
    {
        return Err(ValidatedRepoPathError::InvalidPath {
            reason: format!(
                "expected_parent must be a non-empty repository-relative path: {}",
                expected_parent.display()
            ),
        });
    }
    let bytes = raw.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return Err(ValidatedRepoPathError::InvalidPath {
            reason: format!(
                "expected_parent contains a drive prefix: {}",
                expected_parent.display()
            ),
        });
    }
    for part in raw.split(&['/', '\\'][..]) {
        if part.is_empty() || part == "." || part == ".." {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!(
                    "expected_parent contains an invalid component: {}",
                    expected_parent.display()
                ),
            });
        }
    }
    if !expected_parent
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(ValidatedRepoPathError::InvalidPath {
            reason: format!(
                "expected_parent contains a non-normal component: {}",
                expected_parent.display()
            ),
        });
    }
    Ok(())
}

impl ValidatedRepoPath {
    /// Validate a repository-relative path under `repo_root` using `mode`.
    pub fn new(
        repo_root: &Path,
        rel_path: &Path,
        mode: ValidatedRepoPathMode,
    ) -> Result<Self, ValidatedRepoPathError> {
        if !repo_root.is_absolute() {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!("repo_root is not an absolute path: {}", repo_root.display()),
            });
        }

        let canonical_repo_root = match repo_root.canonicalize() {
            Ok(p) => p,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(ValidatedRepoPathError::InvalidPath {
                    reason: format!("repo_root does not exist: {}", repo_root.display()),
                });
            }
            Err(e) => {
                return Err(ValidatedRepoPathError::UncertainIdentity {
                    reason: format!(
                        "Failed to query metadata for repo_root {}: {}",
                        repo_root.display(),
                        e
                    ),
                });
            }
        };

        // Validate repo_root metadata
        let root_meta = match fs::symlink_metadata(&canonical_repo_root) {
            Ok(m) => m,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Err(ValidatedRepoPathError::InvalidPath {
                    reason: format!(
                        "repo_root does not exist: {}",
                        canonical_repo_root.display()
                    ),
                });
            }
            Err(e) => {
                return Err(ValidatedRepoPathError::UncertainIdentity {
                    reason: format!(
                        "Failed to query metadata for repo_root {}: {}",
                        canonical_repo_root.display(),
                        e
                    ),
                });
            }
        };

        if is_symlink_or_reparse(&root_meta) {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!(
                    "repo_root is a symlink or reparse point: {}",
                    canonical_repo_root.display()
                ),
            });
        }
        if !root_meta.is_dir() {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!(
                    "repo_root is not a directory: {}",
                    canonical_repo_root.display()
                ),
            });
        }

        // Lexical check on rel_path
        let raw_str = rel_path.to_string_lossy();
        if raw_str.trim().is_empty() {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: "rel_path is empty".into(),
            });
        }

        if rel_path.is_absolute() || raw_str.starts_with('/') || raw_str.starts_with('\\') {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!("Path is absolute: {}", raw_str),
            });
        }

        let bytes = raw_str.as_bytes();
        if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!("Path contains drive letter prefix: {}", raw_str),
            });
        }

        if raw_str.starts_with("//") || raw_str.starts_with("\\\\") {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!("Path contains UNC prefix: {}", raw_str),
            });
        }

        // Segment checking for . and .. and empty segments
        for part in raw_str.split(&['/', '\\'][..]) {
            if part == "." || part == ".." || part.is_empty() {
                return Err(ValidatedRepoPathError::InvalidPath {
                    reason: format!("Path contains invalid component '{}': {}", part, raw_str),
                });
            }
        }

        // Component check via Path::components
        for comp in rel_path.components() {
            if !matches!(comp, Component::Normal(_)) {
                return Err(ValidatedRepoPathError::InvalidPath {
                    reason: format!("Path contains non-normal component {:?}: {}", comp, raw_str),
                });
            }
        }

        if let ValidatedRepoPathMode::ValidatedDirectChildDirectory { expected_parent } = &mode {
            validate_expected_parent(expected_parent)?;
            if rel_path.parent().unwrap_or_else(|| Path::new("")) != expected_parent {
                return Err(ValidatedRepoPathError::InvalidPath {
                    reason: format!(
                        "Parent path {:?} does not match expected parent {:?}",
                        rel_path.parent().unwrap_or_else(|| Path::new("")),
                        expected_parent
                    ),
                });
            }
        }

        let full_path = canonical_repo_root.join(rel_path);
        if !full_path.starts_with(&canonical_repo_root) {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!(
                    "Path {} escapes repo root {}",
                    full_path.display(),
                    canonical_repo_root.display()
                ),
            });
        }

        // Build chain identities
        let mut chain_identities = Vec::new();
        let root_token = FileIdentityToken::from_path(&canonical_repo_root, &root_meta)?;
        chain_identities.push(ComponentIdentity {
            path: canonical_repo_root.clone(),
            exists: true,
            is_dir: true,
            is_file: false,
            identity_token: Some(root_token),
        });

        let segments: Vec<&Path> = rel_path
            .components()
            .map(|c| Path::new(c.as_os_str()))
            .collect();
        let total = segments.len();
        let mut current_path = canonical_repo_root.clone();
        let mut ancestor_missing = false;

        for (i, seg) in segments.iter().enumerate() {
            let is_leaf = i == total - 1;
            current_path = current_path.join(seg);

            if ancestor_missing {
                if is_leaf {
                    match mode {
                        ValidatedRepoPathMode::RegularFileOrMissing => {
                            chain_identities.push(ComponentIdentity {
                                path: current_path.clone(),
                                exists: false,
                                is_dir: false,
                                is_file: false,
                                identity_token: None,
                            });
                        }
                        ValidatedRepoPathMode::ValidatedDirectChildDirectory { .. } => {
                            return Err(ValidatedRepoPathError::InvalidPath {
                                reason: format!(
                                    "Target directory {} does not exist (ancestor missing)",
                                    current_path.display()
                                ),
                            });
                        }
                    }
                } else {
                    chain_identities.push(ComponentIdentity {
                        path: current_path.clone(),
                        exists: false,
                        is_dir: false,
                        is_file: false,
                        identity_token: None,
                    });
                }
                continue;
            }

            let meta_result = fs::symlink_metadata(&current_path);
            match meta_result {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    if is_leaf {
                        match mode {
                            ValidatedRepoPathMode::RegularFileOrMissing => {
                                chain_identities.push(ComponentIdentity {
                                    path: current_path.clone(),
                                    exists: false,
                                    is_dir: false,
                                    is_file: false,
                                    identity_token: None,
                                });
                            }
                            ValidatedRepoPathMode::ValidatedDirectChildDirectory { .. } => {
                                return Err(ValidatedRepoPathError::InvalidPath {
                                    reason: format!(
                                        "Target directory {} does not exist",
                                        current_path.display()
                                    ),
                                });
                            }
                        }
                    } else {
                        ancestor_missing = true;
                        chain_identities.push(ComponentIdentity {
                            path: current_path.clone(),
                            exists: false,
                            is_dir: false,
                            is_file: false,
                            identity_token: None,
                        });
                    }
                }
                Err(e) => {
                    return Err(ValidatedRepoPathError::UncertainIdentity {
                        reason: format!(
                            "Failed to read metadata for {}: {}",
                            current_path.display(),
                            e
                        ),
                    });
                }
                Ok(meta) => {
                    if is_symlink_or_reparse(&meta) {
                        return Err(ValidatedRepoPathError::InvalidPath {
                            reason: format!(
                                "Path component {} is a symlink, junction, or reparse point",
                                current_path.display()
                            ),
                        });
                    }

                    if !is_leaf {
                        if !meta.is_dir() {
                            return Err(ValidatedRepoPathError::InvalidPath {
                                reason: format!(
                                    "Ancestor component {} is not a directory",
                                    current_path.display()
                                ),
                            });
                        }
                        let token = FileIdentityToken::from_path(&current_path, &meta)?;
                        chain_identities.push(ComponentIdentity {
                            path: current_path.clone(),
                            exists: true,
                            is_dir: true,
                            is_file: false,
                            identity_token: Some(token),
                        });
                    } else {
                        match &mode {
                            ValidatedRepoPathMode::RegularFileOrMissing => {
                                if meta.is_dir() {
                                    return Err(ValidatedRepoPathError::InvalidPath {
                                        reason: format!(
                                            "Target path {} is a directory in regular file mode",
                                            current_path.display()
                                        ),
                                    });
                                }
                                if !meta.is_file() {
                                    return Err(ValidatedRepoPathError::InvalidPath {
                                        reason: format!(
                                            "Target path {} is not a regular file",
                                            current_path.display()
                                        ),
                                    });
                                }
                                let token = FileIdentityToken::from_path(&current_path, &meta)?;
                                chain_identities.push(ComponentIdentity {
                                    path: current_path.clone(),
                                    exists: true,
                                    is_dir: false,
                                    is_file: true,
                                    identity_token: Some(token),
                                });
                            }
                            ValidatedRepoPathMode::ValidatedDirectChildDirectory { .. } => {
                                if !meta.is_dir() {
                                    return Err(ValidatedRepoPathError::InvalidPath {
                                        reason: format!(
                                            "Target path {} is not a directory in direct-child directory mode",
                                            current_path.display()
                                        ),
                                    });
                                }
                                let token = FileIdentityToken::from_path(&current_path, &meta)?;
                                chain_identities.push(ComponentIdentity {
                                    path: current_path.clone(),
                                    exists: true,
                                    is_dir: true,
                                    is_file: false,
                                    identity_token: Some(token),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(ValidatedRepoPath {
            repo_root: canonical_repo_root,
            rel_path: rel_path.to_path_buf(),
            full_path,
            mode,
            chain_identities,
        })
    }

    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    pub fn rel_path(&self) -> &Path {
        &self.rel_path
    }

    pub fn full_path(&self) -> &Path {
        &self.full_path
    }

    pub fn mode(&self) -> &ValidatedRepoPathMode {
        &self.mode
    }

    pub fn chain_identities(&self) -> &[ComponentIdentity] {
        &self.chain_identities
    }

    pub fn exists(&self) -> bool {
        self.chain_identities
            .last()
            .map(|c| c.exists)
            .unwrap_or(false)
    }

    pub fn is_file(&self) -> bool {
        self.chain_identities
            .last()
            .map(|c| c.is_file)
            .unwrap_or(false)
    }

    pub fn is_dir(&self) -> bool {
        self.chain_identities
            .last()
            .map(|c| c.is_dir)
            .unwrap_or(false)
    }

    /// Immediately recheck the component chain against disk state.
    pub fn recheck(&self) -> Result<(), ValidatedRepoPathError> {
        for comp in &self.chain_identities {
            match fs::symlink_metadata(&comp.path) {
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    if comp.exists {
                        return Err(ValidatedRepoPathError::ChangedIdentity {
                            reason: format!(
                                "Path component {} existed previously but was removed",
                                comp.path.display()
                            ),
                        });
                    }
                }
                Err(e) => {
                    return Err(ValidatedRepoPathError::UncertainIdentity {
                        reason: format!(
                            "Failed to read metadata for {} during recheck: {}",
                            comp.path.display(),
                            e
                        ),
                    });
                }
                Ok(meta) => {
                    if !comp.exists {
                        return Err(ValidatedRepoPathError::ChangedIdentity {
                            reason: format!(
                                "Path component {} was missing previously but now exists",
                                comp.path.display()
                            ),
                        });
                    }

                    if is_symlink_or_reparse(&meta) {
                        return Err(ValidatedRepoPathError::InvalidPath {
                            reason: format!(
                                "Path component {} became a symlink, junction, or reparse point during recheck",
                                comp.path.display()
                            ),
                        });
                    }

                    if comp.is_dir && !meta.is_dir() {
                        return Err(ValidatedRepoPathError::ChangedIdentity {
                            reason: format!(
                                "Directory {} changed to non-directory",
                                comp.path.display()
                            ),
                        });
                    }

                    if comp.is_file && !meta.is_file() {
                        return Err(ValidatedRepoPathError::ChangedIdentity {
                            reason: format!("File {} changed to non-file", comp.path.display()),
                        });
                    }

                    let new_token = FileIdentityToken::from_path(&comp.path, &meta)?;
                    if let Some(ref old_token) = comp.identity_token {
                        if old_token != &new_token {
                            return Err(ValidatedRepoPathError::ChangedIdentity {
                                reason: format!(
                                    "Identity token swapped for path component {}",
                                    comp.path.display()
                                ),
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Consume this authorization after all previously missing parent directories
    /// have been materialized. The leaf must retain its original state.
    pub fn bind_materialized_parents(self) -> Result<Self, ValidatedRepoPathError> {
        self.rebind(RebindOperation::MaterializedParents)
    }

    /// Consume this authorization after creating its previously missing regular-file leaf.
    pub fn bind_created_leaf(self) -> Result<Self, ValidatedRepoPathError> {
        self.rebind(RebindOperation::CreatedLeaf)
    }

    /// Bind a prevalidated sibling candidate before a native replacement operation.
    pub fn bind_replacement_candidate(
        self,
        candidate: Self,
    ) -> Result<ValidatedReplacement, ValidatedRepoPathError> {
        self.recheck()?;
        candidate.recheck()?;
        require_same_parent(&self, &candidate, "replacement candidate")?;
        if !self.exists()
            || !self.is_file()
            || self.full_path == candidate.full_path
            || !candidate.is_file()
        {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason:
                    "Replacement requires distinct existing regular-file target and candidate leaves"
                        .into(),
            });
        }
        let candidate_token = candidate.leaf_token()?.clone();
        let previous_token = self.leaf_token()?.clone();
        if previous_token == candidate_token {
            return Err(ValidatedRepoPathError::ChangedIdentity {
                reason: "Replacement candidate identity equals the old leaf identity".into(),
            });
        }
        Ok(ValidatedReplacement {
            target: self,
            candidate_path: candidate.full_path,
            candidate_token,
            previous_token,
        })
    }

    /// Bind an absent, one-component destination under the source's validated parent.
    pub fn bind_same_parent_relocation_target(
        self,
        destination: Self,
    ) -> Result<ValidatedRelocation, ValidatedRepoPathError> {
        self.recheck()?;
        destination.recheck()?;
        require_same_parent(&self, &destination, "relocation target")?;
        if !self.exists() || destination.exists() || self.full_path == destination.full_path {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: "Relocation requires an existing source and distinct absent destination"
                    .into(),
            });
        }
        let destination_name = destination.rel_path.file_name().ok_or_else(|| {
            ValidatedRepoPathError::InvalidPath {
                reason: "Relocation destination has no leaf name".into(),
            }
        })?;
        if Path::new(destination_name).components().count() != 1
            || !matches!(
                Path::new(destination_name).components().next(),
                Some(Component::Normal(_))
            )
        {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: "Relocation destination must be one normal component".into(),
            });
        }
        let source_token = self.leaf_token()?.clone();
        let source_is_dir = self.is_dir();
        let source_is_file = self.is_file();
        Ok(ValidatedRelocation {
            source: self,
            destination,
            source_token,
            source_is_dir,
            source_is_file,
        })
    }

    /// Bind the current leaf and parent chain before granting removal authority.
    pub fn bind_removal(self) -> Result<ValidatedRemoval, ValidatedRepoPathError> {
        self.recheck()?;
        if !self.exists() {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: "Removal binding requires an existing leaf".into(),
            });
        }
        Ok(ValidatedRemoval { target: self })
    }

    /// Validate one no-follow cleanup child without granting recursive traversal.
    pub fn validate_child(
        &self,
        child_name: &Path,
    ) -> Result<ValidatedChild, ValidatedRepoPathError> {
        self.recheck()?;
        if !self.is_dir()
            || !matches!(
                self.mode,
                ValidatedRepoPathMode::ValidatedDirectChildDirectory { .. }
            )
        {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: "Child validation requires a validated cleanup directory".into(),
            });
        }
        validate_single_normal_component(child_name, "cleanup child")?;
        let child_rel = self.rel_path.join(child_name);
        let child_full = self.repo_root.join(&child_rel);
        let metadata = fs::symlink_metadata(&child_full).map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                ValidatedRepoPathError::ChangedIdentity {
                    reason: format!("Cleanup child {} disappeared", child_full.display()),
                }
            } else {
                ValidatedRepoPathError::UncertainIdentity {
                    reason: format!(
                        "Failed to query cleanup child {}: {}",
                        child_full.display(),
                        error
                    ),
                }
            }
        })?;
        if is_symlink_or_reparse(&metadata) {
            return Err(ValidatedRepoPathError::InvalidPath {
                reason: format!(
                    "Cleanup child {} is a symlink, junction, or reparse point",
                    child_full.display()
                ),
            });
        }
        if metadata.is_file() {
            return Self::new(
                &self.repo_root,
                &child_rel,
                ValidatedRepoPathMode::RegularFileOrMissing,
            )
            .map(ValidatedChild::RegularFile);
        }
        if metadata.is_dir() {
            return Self::new(
                &self.repo_root,
                &child_rel,
                ValidatedRepoPathMode::ValidatedDirectChildDirectory {
                    expected_parent: self.rel_path.clone(),
                },
            )
            .map(ValidatedChild::Directory);
        }
        Err(ValidatedRepoPathError::InvalidPath {
            reason: format!("Cleanup child {} is a special entry", child_full.display()),
        })
    }

    fn leaf_token(&self) -> Result<&FileIdentityToken, ValidatedRepoPathError> {
        self.chain_identities
            .last()
            .and_then(|component| component.identity_token.as_ref())
            .ok_or_else(|| ValidatedRepoPathError::InvalidPath {
                reason: format!(
                    "Path {} has no bound leaf identity",
                    self.full_path.display()
                ),
            })
    }

    fn rebind(mut self, operation: RebindOperation<'_>) -> Result<Self, ValidatedRepoPathError> {
        let total = self.chain_identities.len();
        let mut rebound = Vec::with_capacity(total);
        for (index, recorded) in self.chain_identities.iter().enumerate() {
            let is_leaf = index + 1 == total;
            match fs::symlink_metadata(&recorded.path) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    let allowed = if is_leaf {
                        match operation {
                            RebindOperation::MaterializedParents => !recorded.exists,
                            RebindOperation::Removed => recorded.exists,
                            RebindOperation::CreatedLeaf | RebindOperation::ExactLeaf { .. } => {
                                false
                            }
                        }
                    } else {
                        false
                    };
                    if !allowed {
                        return Err(ValidatedRepoPathError::ChangedIdentity {
                            reason: format!(
                                "Path component {} is absent at an invalid transition boundary",
                                recorded.path.display()
                            ),
                        });
                    }
                    rebound.push(ComponentIdentity {
                        path: recorded.path.clone(),
                        exists: false,
                        is_dir: false,
                        is_file: false,
                        identity_token: None,
                    });
                }
                Err(error) => {
                    return Err(ValidatedRepoPathError::UncertainIdentity {
                        reason: format!(
                            "Failed to query {} during identity transition: {}",
                            recorded.path.display(),
                            error
                        ),
                    });
                }
                Ok(metadata) => {
                    if is_symlink_or_reparse(&metadata) {
                        return Err(ValidatedRepoPathError::InvalidPath {
                            reason: format!(
                                "Path component {} is a symlink, junction, or reparse point during identity transition",
                                recorded.path.display()
                            ),
                        });
                    }
                    let token = FileIdentityToken::from_path(&recorded.path, &metadata)?;
                    if !is_leaf {
                        if !metadata.is_dir() {
                            return Err(ValidatedRepoPathError::ChangedIdentity {
                                reason: format!(
                                    "Parent component {} is not a directory",
                                    recorded.path.display()
                                ),
                            });
                        }
                        if recorded.exists {
                            require_recorded_identity(recorded, &token)?;
                        } else if !matches!(operation, RebindOperation::MaterializedParents) {
                            return Err(ValidatedRepoPathError::ChangedIdentity {
                                reason: format!(
                                    "Previously missing parent {} materialized outside its binding transition",
                                    recorded.path.display()
                                ),
                            });
                        }
                        rebound.push(ComponentIdentity {
                            path: recorded.path.clone(),
                            exists: true,
                            is_dir: true,
                            is_file: false,
                            identity_token: Some(token),
                        });
                        continue;
                    }

                    let (expected_dir, expected_file) = match operation {
                        RebindOperation::MaterializedParents => {
                            if !recorded.exists {
                                return Err(ValidatedRepoPathError::ChangedIdentity {
                                    reason: format!(
                                        "Leaf {} was created during parent materialization",
                                        recorded.path.display()
                                    ),
                                });
                            }
                            require_recorded_identity(recorded, &token)?;
                            (recorded.is_dir, recorded.is_file)
                        }
                        RebindOperation::CreatedLeaf => {
                            if recorded.exists
                                || !matches!(self.mode, ValidatedRepoPathMode::RegularFileOrMissing)
                            {
                                return Err(ValidatedRepoPathError::InvalidPath {
                                    reason: "Created-leaf binding requires a previously missing regular-file leaf"
                                        .into(),
                                });
                            }
                            (false, true)
                        }
                        RebindOperation::ExactLeaf {
                            token: expected,
                            is_dir,
                            is_file,
                        } => {
                            if &token != expected {
                                return Err(ValidatedRepoPathError::ChangedIdentity {
                                    reason: format!(
                                        "Result identity for {} does not equal the bound operation identity",
                                        recorded.path.display()
                                    ),
                                });
                            }
                            (is_dir, is_file)
                        }
                        RebindOperation::Removed => {
                            return Err(ValidatedRepoPathError::ChangedIdentity {
                                reason: format!(
                                    "Removed leaf {} still exists",
                                    recorded.path.display()
                                ),
                            });
                        }
                    };
                    if expected_dir != metadata.is_dir() || expected_file != metadata.is_file() {
                        return Err(ValidatedRepoPathError::ChangedIdentity {
                            reason: format!(
                                "Result kind for {} does not match the bound operation kind",
                                recorded.path.display()
                            ),
                        });
                    }
                    rebound.push(ComponentIdentity {
                        path: recorded.path.clone(),
                        exists: true,
                        is_dir: expected_dir,
                        is_file: expected_file,
                        identity_token: Some(token),
                    });
                }
            }
        }
        self.chain_identities = rebound;
        Ok(self)
    }
}

#[derive(Debug)]
pub struct ValidatedReplacement {
    target: ValidatedRepoPath,
    candidate_path: PathBuf,
    candidate_token: FileIdentityToken,
    previous_token: FileIdentityToken,
}

/// A consuming proof that binds a leaf and its parent chain before unlink.
#[derive(Debug)]
pub struct ValidatedRemoval {
    target: ValidatedRepoPath,
}

impl ValidatedRemoval {
    /// The only path authorized for the bound unlink operation.
    pub fn unlink_path(&self) -> &Path {
        self.target.full_path()
    }

    /// Verify that the bound leaf is absent and every parent identity is unchanged.
    pub fn verify(self) -> Result<(), ValidatedRepoPathError> {
        self.target.rebind(RebindOperation::Removed).map(|_| ())
    }
}

impl ValidatedReplacement {
    pub fn target_path(&self) -> &Path {
        self.target.full_path()
    }

    pub fn candidate_path(&self) -> &Path {
        &self.candidate_path
    }

    pub fn verify(self) -> Result<ValidatedRepoPath, ValidatedRepoPathError> {
        match fs::symlink_metadata(&self.candidate_path) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(ValidatedRepoPathError::UncertainIdentity {
                    reason: format!(
                        "Failed to verify consumed replacement candidate {}: {}",
                        self.candidate_path.display(),
                        error
                    ),
                });
            }
            Ok(_) => {
                return Err(ValidatedRepoPathError::ChangedIdentity {
                    reason: format!(
                        "Replacement candidate {} still exists after replacement",
                        self.candidate_path.display()
                    ),
                });
            }
        }
        if self.previous_token == self.candidate_token {
            return Err(ValidatedRepoPathError::ChangedIdentity {
                reason: "Replacement did not change the old leaf identity".into(),
            });
        }
        self.target.rebind(RebindOperation::ExactLeaf {
            token: &self.candidate_token,
            is_dir: false,
            is_file: true,
        })
    }
}

#[derive(Debug)]
pub struct ValidatedRelocation {
    source: ValidatedRepoPath,
    destination: ValidatedRepoPath,
    source_token: FileIdentityToken,
    source_is_dir: bool,
    source_is_file: bool,
}

impl ValidatedRelocation {
    pub fn source_path(&self) -> &Path {
        self.source.full_path()
    }

    pub fn destination_path(&self) -> &Path {
        self.destination.full_path()
    }

    pub fn verify(self) -> Result<ValidatedRepoPath, ValidatedRepoPathError> {
        let source_mode = self.source.mode.clone();
        self.source.rebind(RebindOperation::Removed)?;
        let mut destination = self.destination.rebind(RebindOperation::ExactLeaf {
            token: &self.source_token,
            is_dir: self.source_is_dir,
            is_file: self.source_is_file,
        })?;
        destination.mode = source_mode;
        Ok(destination)
    }
}

#[derive(Debug)]
pub enum ValidatedChild {
    RegularFile(ValidatedRepoPath),
    Directory(ValidatedRepoPath),
}

impl ValidatedChild {
    pub fn path(&self) -> &ValidatedRepoPath {
        match self {
            Self::RegularFile(path) | Self::Directory(path) => path,
        }
    }

    pub fn into_path(self) -> ValidatedRepoPath {
        match self {
            Self::RegularFile(path) | Self::Directory(path) => path,
        }
    }
}

#[derive(Clone, Copy)]
enum RebindOperation<'a> {
    MaterializedParents,
    CreatedLeaf,
    ExactLeaf {
        token: &'a FileIdentityToken,
        is_dir: bool,
        is_file: bool,
    },
    Removed,
}

fn require_recorded_identity(
    recorded: &ComponentIdentity,
    current: &FileIdentityToken,
) -> Result<(), ValidatedRepoPathError> {
    if recorded.identity_token.as_ref() != Some(current) {
        return Err(ValidatedRepoPathError::ChangedIdentity {
            reason: format!(
                "Identity token swapped for path component {}",
                recorded.path.display()
            ),
        });
    }
    Ok(())
}

fn require_same_parent(
    left: &ValidatedRepoPath,
    right: &ValidatedRepoPath,
    role: &str,
) -> Result<(), ValidatedRepoPathError> {
    if left.repo_root != right.repo_root
        || left.rel_path.parent() != right.rel_path.parent()
        || left.rel_path.parent().is_none()
    {
        return Err(ValidatedRepoPathError::InvalidPath {
            reason: format!("{role} must be under the same validated parent"),
        });
    }
    Ok(())
}

fn validate_single_normal_component(path: &Path, role: &str) -> Result<(), ValidatedRepoPathError> {
    let raw = path.to_string_lossy();
    let mut components = path.components();
    if raw.is_empty()
        || raw.contains('/')
        || raw.contains('\\')
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
        || raw == "."
        || raw == ".."
    {
        return Err(ValidatedRepoPathError::InvalidPath {
            reason: format!("{role} must be exactly one normal path component"),
        });
    }
    Ok(())
}
