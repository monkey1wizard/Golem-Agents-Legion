//! Typed, side-effect-free executor executable lookup.
//!
//! Lookup records only the operation and candidate paths. It never includes
//! PATH contents, settings, credentials, or platform error messages.

use std::io::{self, ErrorKind};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeStage {
    ExplicitPath,
    PathDirectory,
    Metadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeEvidence {
    pub operation: String,
    pub candidate: Option<PathBuf>,
    pub resolved_path: Option<PathBuf>,
    pub os_error_kind: Option<ErrorKind>,
    pub stage: ProbeStage,
    /// A fixed safe summary; never an OS error string or caller-provided text.
    pub reason: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability {
    Available {
        path: PathBuf,
        evidence: Vec<ProbeEvidence>,
    },
    Denied {
        evidence: Vec<ProbeEvidence>,
    },
    Unknown {
        evidence: Vec<ProbeEvidence>,
    },
    Missing {
        evidence: Vec<ProbeEvidence>,
    },
}

impl Availability {
    /// Only a resolved executable may proceed to the next readiness check.
    pub fn permits_attempt(&self) -> bool {
        matches!(self, Self::Available { .. })
    }
}

#[derive(Debug, Clone)]
pub struct SearchSpec<'a> {
    pub operation: &'a str,
    pub name: &'a str,
    pub explicit_paths: &'a [PathBuf],
    pub path: Option<&'a str>,
    pub path_ext: Option<&'a str>,
    pub windows: bool,
}

/// Resolve a launch path while retaining evidence for every inspected candidate.
/// `metadata` is injectable so permission failures can be tested without ACL edits.
pub fn resolve_with<F>(spec: &SearchSpec<'_>, mut metadata: F) -> Availability
where
    F: FnMut(&Path) -> io::Result<std::fs::Metadata>,
{
    let mut evidence = Vec::new();
    let mut denied = false;
    let mut incomplete = false;
    let mut candidates = Vec::new();

    for path in spec.explicit_paths {
        candidates.push((path.clone(), ProbeStage::ExplicitPath));
    }

    let name_path = Path::new(spec.name);
    if name_path.is_absolute() || name_path.components().count() > 1 {
        candidates.push((name_path.to_path_buf(), ProbeStage::ExplicitPath));
    } else {
        let extensions = if spec.windows {
            windows_extensions(spec.name, spec.path_ext)
        } else {
            vec![String::new()]
        };
        match spec.path {
            Some(path_value) => {
                for dir in std::env::split_paths(path_value) {
                    for extension in &extensions {
                        candidates.push((
                            dir.join(format!("{}{extension}", spec.name)),
                            ProbeStage::PathDirectory,
                        ));
                    }
                }
            }
            None => incomplete = true,
        }
    }

    for (candidate, stage) in candidates {
        match metadata(&candidate) {
            Ok(info) if info.is_file() => {
                let mut found = evidence;
                found.push(ProbeEvidence {
                    operation: spec.operation.to_owned(),
                    candidate: Some(candidate.clone()),
                    resolved_path: Some(candidate.clone()),
                    os_error_kind: None,
                    stage: ProbeStage::Metadata,
                    reason: "candidate is a file",
                });
                return Availability::Available {
                    path: candidate,
                    evidence: found,
                };
            }
            Ok(_) => {
                // A successful metadata call that finds a directory (or other
                // non-file) does not prove that the executable is absent.
                incomplete = true;
                evidence.push(ProbeEvidence {
                    operation: spec.operation.to_owned(),
                    candidate: Some(candidate),
                    resolved_path: None,
                    os_error_kind: None,
                    stage,
                    reason: "candidate is not a file",
                });
            }
            Err(error) => {
                let kind = error.kind();
                denied |= kind == ErrorKind::PermissionDenied;
                incomplete |= kind != ErrorKind::NotFound;
                evidence.push(ProbeEvidence {
                    operation: spec.operation.to_owned(),
                    candidate: Some(candidate),
                    resolved_path: None,
                    os_error_kind: Some(kind),
                    stage,
                    reason: reason_for(kind),
                });
            }
        }
    }

    if denied {
        Availability::Denied { evidence }
    } else if incomplete {
        Availability::Unknown { evidence }
    } else {
        Availability::Missing { evidence }
    }
}

pub fn resolve(spec: &SearchSpec<'_>) -> Availability {
    resolve_with(spec, |path| std::fs::metadata(path))
}

fn reason_for(kind: ErrorKind) -> &'static str {
    match kind {
        ErrorKind::NotFound => "candidate was not found",
        ErrorKind::PermissionDenied => "candidate access was denied",
        _ => "candidate inspection failed",
    }
}

fn windows_extensions(name: &str, path_ext: Option<&str>) -> Vec<String> {
    let supported = [".exe", ".com", ".cmd", ".bat", ".ps1"];
    if Path::new(name).extension().is_some() {
        return vec![String::new()];
    }
    let configured: Vec<String> = path_ext
        .unwrap_or(".COM;.EXE;.BAT;.CMD;.PS1")
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_ascii_lowercase())
        .collect();
    supported
        .iter()
        .filter(|ext| configured.iter().any(|item| item == **ext))
        .map(|ext| (*ext).to_owned())
        .collect()
}

#[cfg(test)]
mod availability_contract {
    use super::*;

    #[test]
    fn windows_extensions_preserve_launch_order_and_skip_extensionless_shims() {
        assert_eq!(
            windows_extensions("codex", Some(".ps1;.cmd;.exe;.bat;.com")),
            vec![".exe", ".com", ".cmd", ".bat", ".ps1"]
        );
        assert_eq!(windows_extensions("codex.exe", Some(".EXE;.CMD")), vec![""]);
    }

    #[test]
    fn metadata_failure_evidence_is_redacted_and_typed() {
        let candidates = vec![PathBuf::from("secret-sentinel/codex")];
        let spec = SearchSpec {
            operation: "resolve codex",
            name: "codex",
            explicit_paths: &candidates,
            path: None,
            path_ext: None,
            windows: false,
        };
        let result = resolve_with(&spec, |_| Err(io::Error::from(ErrorKind::PermissionDenied)));
        let Availability::Denied { evidence } = result else {
            panic!("expected denial")
        };
        assert_eq!(evidence[0].operation, "resolve codex");
        assert_eq!(
            evidence[0].candidate.as_deref(),
            Some(Path::new("secret-sentinel/codex"))
        );
        assert_eq!(evidence[0].os_error_kind, Some(ErrorKind::PermissionDenied));
        assert!(!evidence[0].reason.contains("secret-sentinel"));
    }

    #[test]
    fn denied_candidate_does_not_hide_later_usable_path_with_spaces() {
        let dir = tempfile::tempdir().unwrap();
        let spaced = dir.path().join("directory with spaces");
        std::fs::create_dir(&spaced).unwrap();
        let usable = spaced.join("codex");
        std::fs::write(&usable, b"fixture").unwrap();
        let denied = PathBuf::from("denied-candidate/codex");
        let candidates = vec![denied.clone(), usable.clone()];
        let spec = SearchSpec {
            operation: "resolve codex",
            name: "codex",
            explicit_paths: &candidates,
            path: None,
            path_ext: None,
            windows: false,
        };
        let result = resolve_with(&spec, |path| {
            if path == denied {
                Err(io::Error::from(ErrorKind::PermissionDenied))
            } else {
                std::fs::metadata(path)
            }
        });
        let Availability::Available { path, evidence } = result else {
            panic!("later usable candidate should win")
        };
        assert_eq!(path, usable);
        assert_eq!(evidence[0].os_error_kind, Some(ErrorKind::PermissionDenied));
        assert_eq!(evidence[1].resolved_path.as_deref(), Some(path.as_path()));
    }

    #[test]
    fn absent_and_incomplete_searches_are_distinct() {
        let no_candidates = [];
        let complete = SearchSpec {
            operation: "resolve absent",
            name: "absent",
            explicit_paths: &no_candidates,
            path: Some(""),
            path_ext: None,
            windows: false,
        };
        assert!(matches!(resolve(&complete), Availability::Missing { .. }));
        let incomplete = SearchSpec {
            path: None,
            ..complete
        };
        assert!(matches!(resolve(&incomplete), Availability::Unknown { .. }));
    }

    #[test]
    fn path_search_keeps_denial_and_reports_only_proven_absence_as_missing() {
        let root = tempfile::tempdir().unwrap();
        let denied_dir = root.path().join("denied directory");
        let usable_dir = root.path().join("usable directory");
        std::fs::create_dir(&denied_dir).unwrap();
        std::fs::create_dir(&usable_dir).unwrap();
        let usable = usable_dir.join("tool");
        std::fs::write(&usable, b"fixture").unwrap();
        let path = std::env::join_paths([&denied_dir, &usable_dir]).unwrap();
        let path = path.to_string_lossy().into_owned();
        let spec = SearchSpec {
            operation: "find tool",
            name: "tool",
            explicit_paths: &[],
            path: Some(&path),
            path_ext: None,
            windows: false,
        };
        let result = resolve_with(&spec, |candidate| {
            if candidate == denied_dir.join("tool") {
                Err(io::Error::from(ErrorKind::PermissionDenied))
            } else {
                std::fs::metadata(candidate)
            }
        });
        let Availability::Available { path, evidence } = result else {
            panic!("usable PATH candidate should win")
        };
        assert_eq!(path, usable);
        assert_eq!(evidence[0].os_error_kind, Some(ErrorKind::PermissionDenied));

        let absent_path = path_string(root.path());
        let absent_spec = SearchSpec {
            path: Some(&absent_path),
            ..spec
        };
        let absent = resolve(&absent_spec);
        assert!(matches!(absent, Availability::Missing { .. }));
    }

    fn path_string(path: &Path) -> String {
        std::env::join_paths([path])
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn non_not_found_failures_make_the_search_unknown() {
        let candidates = vec![PathBuf::from("tool")];
        let spec = SearchSpec {
            operation: "find tool",
            name: "tool",
            explicit_paths: &candidates,
            path: None,
            path_ext: None,
            windows: false,
        };
        let result = resolve_with(&spec, |_| Err(io::Error::from(ErrorKind::Other)));
        assert!(matches!(result, Availability::Unknown { .. }));
    }
}
