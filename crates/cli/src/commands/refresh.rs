//! `gal refresh` — idempotent machine-level rebuild of the canonical plugin root,
//! parent Claude marketplace manifest, and four machine-runtime projections from
//! the installed GAL source.
//!
//! User machine setup step that renders the installed GAL into the canonical root.
//!
//! Boundary: writes only GAL's own `~/.gal` tree (canonical + marketplace.json).
//! Does NOT write Claude-managed `~/.claude/plugins/cache` or `installed_plugins.json`.
//! Claude re-copies via restart / re-add after a refresh.
//!
//! Internal binary subcommand; NOT a public `/gal` slash command.

use super::doctor::required_shared_skill_names;
use gal_engine::render::{materialize_embedded_source_into, render_canonical_root_to, RenderError};
use gal_engine::ExitCode;
use gal_foundation::health::HealthCheck;
use gal_foundation::paths::{gal_home, user_home};
use projection::{
    machine_skill_options, run_update_commands, run_update_skills, AdapterError, ProjectionReport,
    SkillsProjectionHealthCheck,
};
use std::ffi::OsStr;
use std::path::Path;

/// All errors that `refresh_with` can return.
#[derive(Debug)]
pub(crate) enum RefreshError {
    NoHome,
    Render(RenderError),
    Projection(AdapterError),
    Io(std::io::Error),
    Json(serde_json::Error),
    RejectedRoot(String),
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefreshError::NoHome => write!(f, "could not determine home directory"),
            RefreshError::Render(e) => write!(f, "render error: {e}"),
            RefreshError::Projection(e) => write!(f, "projection error: {e}"),
            RefreshError::Io(e) => write!(f, "I/O error: {e}"),
            RefreshError::Json(e) => write!(f, "JSON error: {e}"),
            RefreshError::RejectedRoot(message) => write!(f, "rejected destination: {message}"),
        }
    }
}

impl From<RenderError> for RefreshError {
    fn from(e: RenderError) -> Self {
        RefreshError::Render(e)
    }
}

impl From<AdapterError> for RefreshError {
    fn from(e: AdapterError) -> Self {
        RefreshError::Projection(e)
    }
}

impl From<std::io::Error> for RefreshError {
    fn from(e: std::io::Error) -> Self {
        RefreshError::Io(e)
    }
}

impl From<serde_json::Error> for RefreshError {
    fn from(e: serde_json::Error) -> Self {
        RefreshError::Json(e)
    }
}

/// The canonical content of `~/.gal/plugins/.claude-plugin/marketplace.json`.
///
/// No `version` key — Claude uses the plugin content hash as the cache key
/// once git-source marketplace auto-refresh is available (pending public release).
pub(crate) fn marketplace_json() -> serde_json::Value {
    serde_json::json!({
        "name": "gal",
        "owner": { "name": "GAL" },
        "description": "GAL workflow command plugin — planning, pipeline, agents, and cross-runtime projection.",
        "plugins": [
            {
                "name": "gal",
                "source": "./gal",
                "description": "GAL workflow commands and golem agents."
            }
        ]
    })
}

/// Core refresh logic. Separated from the CLI entry so it is unit-testable.
///
/// Steps:
/// 1. Render canonical plugin root (`~/.gal/plugins/gal`).
/// 2. Project skills + commands to four machine runtimes
///    (copilot/codex/agy/opencode); Claude reads the canonical root.
/// 3. Write `~/.gal/plugins/.claude-plugin/marketplace.json`.
///
/// Deterministic: re-rendered content is byte-identical, including the
/// per-runtime plugin manifests — their `version` field is a content-digest
/// stamp (`<gal_version>-g<digest>`) of the portable render tree, not a
/// timestamp, so an unchanged source yields an unchanged `~/.gal` tree
/// between runs.
/// Fail-safe via the a2 `ManagedArtifactRegistry` lockfile guard — never deletes
/// non-GAL plugins.
pub(crate) fn refresh_with(source_root: &Path) -> Result<(), RefreshError> {
    // Use the same home-dir source as gal_foundation::paths helpers so render
    // and projection resolve to the same root (reads USERPROFILE/HOME, not the
    // Windows profile API, which makes this testable via env-var injection).
    let home = user_home().ok_or(RefreshError::NoHome)?;
    refresh_to_home(source_root, &home, true)
}

/// Refresh into an explicit home root. Every generated destination is derived
/// from this root, so acceptance runs do not inherit machine projection paths.
pub(crate) fn refresh_with_root(source_root: &Path, home: &Path) -> Result<(), RefreshError> {
    validate_refresh_root(home)?;
    refresh_to_home(source_root, home, false)
}

fn refresh_to_home(
    source_root: &Path,
    home: &Path,
    include_machine_local: bool,
) -> Result<(), RefreshError> {
    let plugin_root = home.join(".gal").join("plugins").join("gal");
    let canonical = render_canonical_root_to(
        source_root,
        &plugin_root,
        include_machine_local,
        env!("CARGO_PKG_VERSION"),
    )?;
    println!("  canonical root: {}", canonical.display());

    let opts = machine_skill_options(source_root, &home)?;
    let skill_report = run_update_skills(&opts)?;
    let cmd_report = run_update_commands(&opts)?;

    let written = skill_report.written_files.len() + cmd_report.written_files.len();
    let created = skill_report.created_links.len() + cmd_report.created_links.len();
    let warnings = skill_report.warnings.len() + cmd_report.warnings.len();
    for w in skill_report
        .warnings
        .iter()
        .chain(cmd_report.warnings.iter())
    {
        eprintln!("  warning: {}", w.message);
    }
    println!("  projection: {written} written, {created} links created, {warnings} warning(s)");

    write_marketplace_json(home)?;

    write_agents_marketplace_json(home)?;

    // Step 4 — Codex shared-skill-surface summary (read-only; reuses the same
    // required-skill inventory health check `gal doctor` runs).
    print_codex_surface_summary(&canonical, home, &skill_report, &cmd_report);

    Ok(())
}

fn validate_refresh_root(root: &Path) -> Result<(), RefreshError> {
    if !root.is_absolute() {
        return Err(RefreshError::RejectedRoot(format!(
            "refresh root must be absolute: {}",
            root.display()
        )));
    }
    let normalized = canonicalize_with_missing_tail(root).map_err(|error| {
        RefreshError::RejectedRoot(format!("could not resolve refresh root: {error}"))
    })?;
    let profile = operating_system_profile().ok_or(RefreshError::NoHome)?;
    let protected = [
        profile.join(".gal").join("bin"),
        profile.join(".gal").join("plugins").join("gal"),
        profile.join(".agents").join("skills"),
        profile
            .join(".claude")
            .join("plugins")
            .join("cache")
            .join("gal"),
        profile.join(".gal").join("config"),
    ];
    for path in protected {
        let path = canonicalize_with_missing_tail(&path).map_err(RefreshError::Io)?;
        if normalized == path || normalized.starts_with(&path) || path.starts_with(&normalized) {
            return Err(RefreshError::RejectedRoot(format!(
                "refresh root overlaps a shared destination: {}",
                path.display()
            )));
        }
    }
    if !root.exists() {
        return Err(RefreshError::RejectedRoot(format!(
            "refresh root must exist: {}",
            root.display()
        )));
    }
    Ok(())
}

fn canonicalize_with_missing_tail(path: &Path) -> std::io::Result<std::path::PathBuf> {
    let mut existing = path;
    let mut tail = Vec::new();
    while !existing.exists() {
        let name = existing.file_name().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "path has no existing ancestor",
            )
        })?;
        tail.push(name.to_os_string());
        existing = existing.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "path has no existing ancestor",
            )
        })?;
    }
    let mut resolved = std::fs::canonicalize(existing)?;
    for part in tail.into_iter().rev() {
        resolved.push(part);
    }
    Ok(resolved)
}

fn checkout_root_from_cwd(cwd: &Path) -> Option<std::path::PathBuf> {
    let source = gal_engine::render::resolve_source_from_cwd(cwd)?;
    let local_gal = gal_home()?;
    let source = canonicalize_with_missing_tail(&source).ok()?;
    let local_gal = canonicalize_with_missing_tail(&local_gal).ok()?;
    (!source.starts_with(local_gal)).then_some(source)
}

#[cfg(windows)]
fn operating_system_profile() -> Option<std::path::PathBuf> {
    use std::{ffi::OsString, os::windows::ffi::OsStringExt};
    #[repr(C)]
    struct Guid {
        d1: u32,
        d2: u16,
        d3: u16,
        d4: [u8; 8],
    }
    #[link(name = "shell32")]
    extern "system" {
        fn SHGetKnownFolderPath(
            id: *const Guid,
            flags: u32,
            token: isize,
            path: *mut *mut u16,
        ) -> i32;
    }
    #[link(name = "ole32")]
    extern "system" {
        fn CoTaskMemFree(ptr: *mut std::ffi::c_void);
    }
    const PROFILE: Guid = Guid {
        d1: 0x5e6c858f,
        d2: 0x0e22,
        d3: 0x4760,
        d4: [0x9a, 0xfe, 0xea, 0x33, 0x17, 0xb6, 0x71, 0x73],
    };
    let mut ptr = std::ptr::null_mut();
    if unsafe { SHGetKnownFolderPath(&PROFILE, 0, 0, &mut ptr) } < 0 {
        return None;
    }
    let mut len = 0;
    while unsafe { *ptr.add(len) } != 0 {
        len += 1;
    }
    let result = OsString::from_wide(unsafe { std::slice::from_raw_parts(ptr, len) });
    unsafe {
        CoTaskMemFree(ptr.cast());
    }
    Some(result.into())
}

#[cfg(unix)]
fn operating_system_profile() -> Option<std::path::PathBuf> {
    use std::{ffi::CStr, os::unix::ffi::OsStrExt};

    let mut buffer = vec![0u8; 1024];
    loop {
        let mut entry = std::mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = std::ptr::null_mut();
        // The entry and buffer remain alive until the returned path is copied.
        let status = unsafe {
            libc::getpwuid_r(
                libc::getuid(),
                entry.as_mut_ptr(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
                &mut result,
            )
        };
        if status == libc::ERANGE {
            let new_len = buffer.len().checked_mul(2)?;
            buffer.try_reserve(new_len - buffer.len()).ok()?;
            buffer.resize(new_len, 0);
            continue;
        }
        if status != 0 || result.is_null() {
            return None;
        }
        // A successful lookup initializes entry and its strings in buffer.
        let home = unsafe { entry.assume_init().pw_dir };
        if home.is_null() {
            return None;
        }
        let bytes = unsafe { CStr::from_ptr(home) }.to_bytes();
        return Some(std::ffi::OsStr::from_bytes(bytes).into());
    }
}

#[cfg(not(any(windows, unix)))]
fn operating_system_profile() -> Option<std::path::PathBuf> {
    None
}

fn print_codex_surface_summary(
    canonical_root: &Path,
    home: &Path,
    skill_report: &ProjectionReport,
    cmd_report: &ProjectionReport,
) {
    println!(
        "{}",
        build_codex_surface_summary(canonical_root, home, skill_report, cmd_report)
    );
}

/// Build a read-only Codex shared-skill-surface summary: required-skill
/// filesystem status, zombie-cleanup count, and budget footprint (all sourced
/// from [`SkillsProjectionHealthCheck`], the same machinery `gal doctor` uses),
/// followed by a fixed auto-detect-first verification instruction. Pure and
/// independently testable (extracted from [`print_codex_surface_summary`] so
/// content can be asserted without capturing real stdout).
///
/// Deliberately reports only filesystem-observable facts. `gal refresh`
/// cannot observe whether Codex actually surfaced a skill in its current
/// thread — claiming that would be a surfaced-success claim this function
/// must never make.
fn build_codex_surface_summary(
    canonical_root: &Path,
    home: &Path,
    skill_report: &ProjectionReport,
    cmd_report: &ProjectionReport,
) -> String {
    let mut lines = vec!["Codex skill surface:".to_string()];

    let required_names = required_shared_skill_names(canonical_root);
    let shared_skills_root = home.join(".agents").join("skills");
    let lockfile_path = Some(home.join(".gal").join("state").join("plugins.lock.json"));
    let findings = SkillsProjectionHealthCheck::with_inventory(
        shared_skills_root,
        required_names,
        lockfile_path,
    )
    .check();
    let (budget_findings, skill_findings): (Vec<_>, Vec<_>) = findings
        .into_iter()
        .partition(|f| f.message.contains("description footprint"));

    if skill_findings.is_empty() {
        lines.push("  required skills: all present and materialized".to_string());
    } else {
        for finding in &skill_findings {
            lines.push(format!("  {finding}"));
        }
    }
    if budget_findings.is_empty() {
        lines.push("  budget: within the initial-list context budget".to_string());
    } else {
        for finding in &budget_findings {
            lines.push(format!("  {finding}"));
        }
    }

    let zombies_removed = skill_report.removed_paths.len() + cmd_report.removed_paths.len();
    lines.push(format!(
        "  zombie cleanup: {zombies_removed} stale entr{} removed",
        if zombies_removed == 1 { "y" } else { "ies" }
    ));

    lines.push(
        "  Codex auto-detects skill changes; if not appearing, restart Codex or open a fresh \
         thread, then verify via `/skills` or `$gal-status`."
            .to_string(),
    );

    lines.join("\n")
}

/// Write `~/.gal/plugins/.claude-plugin/marketplace.json` atomically.
fn write_marketplace_json(home: &Path) -> Result<(), RefreshError> {
    let plugins_root = home.join(".gal").join("plugins");
    let dir = plugins_root.join(".claude-plugin");
    std::fs::create_dir_all(&dir)?;

    let target = dir.join("marketplace.json");
    let content = serde_json::to_string_pretty(&marketplace_json())?;

    // Atomic write: tmp sibling → rename.
    let tmp = target.with_extension("json.tmp");
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;

    println!("  marketplace: {}", target.display());
    Ok(())
}

/// The canonical content of `~/.gal/plugins/.agents/plugins/marketplace.json` —
/// the standard Agent Plugins marketplace manifest, parallel to
/// [`marketplace_json`]'s Claude-specific one.
///
/// No `version` key, for the same reason as [`marketplace_json`].
pub(crate) fn agents_marketplace_json() -> serde_json::Value {
    serde_json::json!({
        "name": "gal",
        "plugins": [
            {
                "name": "gal",
                "source": { "source": "local", "path": "./gal" },
                "interface": { "displayName": "Golem Agents Legion" }
            }
        ]
    })
}

/// Write `~/.gal/plugins/.agents/plugins/marketplace.json` atomically.
fn write_agents_marketplace_json(home: &Path) -> Result<(), RefreshError> {
    let plugins_root = home.join(".gal").join("plugins");
    let dir = plugins_root.join(".agents").join("plugins");
    std::fs::create_dir_all(&dir)?;

    let target = dir.join("marketplace.json");
    let content = serde_json::to_string_pretty(&agents_marketplace_json())?;

    // Atomic write: tmp sibling → rename.
    let tmp = target.with_extension("json.tmp");
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &target)?;

    println!("  agents marketplace: {}", target.display());
    Ok(())
}

fn refresh_source_not_found_message() -> String {
    "gal refresh: this binary has no embedded contract; reinstall GAL through a package manager."
        .to_string()
}

/// Strip the Windows verbatim `\\?\` prefix (and map `\\?\UNC\` back to `\\`)
/// that `std::fs::canonicalize` adds but `cmd` builtins (`mklink` / `rmdir`)
/// reject. No-op on a string without the prefix, so it is safe on all platforms.
fn strip_verbatim_prefix(s: &str) -> String {
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest.to_string()
    } else {
        s.to_string()
    }
}

/// Resolve `p` to an absolute, native-separator path so the projection layer's
/// Windows junction calls (`cmd /C mklink /J` / `rmdir`) cannot mis-parse a
/// relative `/segment` (e.g. `./plugins/gal-core`) as a command switch.
/// Falls back to the input path if canonicalization fails.
fn normalize_source_root(p: std::path::PathBuf) -> std::path::PathBuf {
    match std::fs::canonicalize(&p) {
        Ok(c) => std::path::PathBuf::from(strip_verbatim_prefix(&c.to_string_lossy())),
        Err(_) => p,
    }
}

pub(crate) fn check_path_identity(
    current_exe: &Path,
    refresh_path: Option<&OsStr>,
    path: Option<&OsStr>,
    cargo_present: bool,
) -> Result<Option<String>, String> {
    use sha2::{Digest, Sha256};

    let refresh_path = refresh_path.filter(|value| !value.is_empty());
    if cargo_present && refresh_path.is_none() {
        return Err("gal refresh: CARGO is set and GAL_REFRESH_PATH is missing. In the worktree terminal, run:
  $env:Path = \"$PWD\\target\\debug;$env:Path\"
  $env:GAL_REFRESH_PATH = $env:Path".to_string());
    }
    let (source, search_path) = match refresh_path {
        Some(value) => ("GAL_REFRESH_PATH", Some(value)),
        None => ("PATH", path),
    };
    for directory in search_path.into_iter().flat_map(std::env::split_paths) {
        let candidate = directory.join(format!("gal{}", std::env::consts::EXE_SUFFIX));
        let link_metadata = match std::fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(format!("gal refresh: cannot inspect candidate: {error}\n  current_exe: {}\n  PATH source: {source}\n  candidate: {}", current_exe.display(), candidate.display())),
        };
        let metadata = if link_metadata.file_type().is_symlink() {
            std::fs::metadata(&candidate).map_err(|error| {
                format!("gal refresh: cannot inspect candidate: {error}\n  current_exe: {}\n  PATH source: {source}\n  candidate: {}", current_exe.display(), candidate.display())
            })?
        } else {
            link_metadata
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if metadata.is_file() && metadata.permissions().mode() & 0o111 == 0 {
                continue;
            }
        }
        let error_context = || {
            format!(
                "current_exe: {}\n  PATH source: {source}\n  candidate: {}",
                current_exe.display(),
                candidate.display()
            )
        };
        if !metadata.is_file() {
            return Err(format!(
                "gal refresh: candidate is not a regular file\n  {}",
                error_context()
            ));
        }
        let current = std::fs::read(current_exe).map_err(|error| {
            format!(
                "gal refresh: cannot read current executable: {error}\n  {}",
                error_context()
            )
        })?;
        let found = std::fs::read(&candidate).map_err(|error| {
            format!(
                "gal refresh: cannot read candidate: {error}\n  {}",
                error_context()
            )
        })?;
        if Sha256::digest(&current) != Sha256::digest(&found) {
            return Err(format!(
                "gal refresh: executable differs from the first candidate\n  {}",
                error_context()
            ));
        }
        return Ok(None);
    }
    Ok(Some(format!(
        "gal refresh: no gal candidate found in {source}; continuing"
    )))
}

/// `gal refresh` CLI entry point.
///
/// Renders the installed source root. Prints a one-line header and summary.
pub(crate) fn cmd_refresh(args: &[String]) -> ExitCode {
    let mut root = None;
    let mut extra = args[1..].iter();
    while let Some(arg) = extra.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("Usage: gal refresh [--root <path>]");
                println!("Deploy the embedded contract. For development, from the worktree root in each PowerShell terminal:");
                println!("  $env:Path = \"$PWD\\target\\debug;$env:Path\"");
                println!("  $env:GAL_REFRESH_PATH = $env:Path");
                println!("Then run cargo run --locked --bin gal -- refresh after each change.");
                return ExitCode::Success;
            }
            "--root" if root.is_none() => match extra.next() {
                Some(path) if !path.starts_with('-') => root = Some(std::path::PathBuf::from(path)),
                _ => {
                    eprintln!("gal refresh: --root requires a path argument");
                    return ExitCode::Usage;
                }
            },
            _ => {
                eprintln!("gal refresh: unsupported option '{arg}'. For development, set PATH and GAL_REFRESH_PATH for the worktree, then run cargo run --locked --bin gal -- refresh.");
                return ExitCode::Usage;
            }
        }
    }
    let destination = match root.as_deref() {
        Some(path) => {
            if let Err(error) = validate_refresh_root(path) {
                eprintln!("gal refresh: {error}");
                return ExitCode::Error;
            }
            path.to_path_buf()
        }
        None => match user_home() {
            Some(home) => home,
            None => {
                eprintln!("gal refresh: {}", RefreshError::NoHome);
                return ExitCode::Error;
            }
        },
    };
    if root.is_none() {
        let current_exe = match std::env::current_exe() {
            Ok(path) => path,
            Err(error) => {
                eprintln!("gal refresh: cannot determine current executable: {error}");
                return ExitCode::Error;
            }
        };
        match check_path_identity(
            &current_exe,
            std::env::var_os("GAL_REFRESH_PATH").as_deref(),
            std::env::var_os("PATH").as_deref(),
            cfg!(windows) && std::env::var_os("CARGO").is_some(),
        ) {
            Ok(Some(note)) => eprintln!("{note}"),
            Ok(None) => {}
            Err(error) => {
                eprintln!("{error}");
                return ExitCode::Error;
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        if checkout_root_from_cwd(&cwd).is_some() {
            eprintln!("gal refresh: this directory is a GAL source checkout. To deploy its changes, set PATH and GAL_REFRESH_PATH for the worktree, then run cargo run --locked --bin gal -- refresh.");
        }
    }
    let source_root =
        match materialize_embedded_source_into(&destination.join(".gal/deployed-source")) {
            Ok(Some(root)) => root,
            Ok(None) => {
                eprintln!("{}", refresh_source_not_found_message());
                return ExitCode::Error;
            }
            Err(error) => {
                eprintln!("gal refresh: embedded source materialization failed: {error}");
                return ExitCode::Error;
            }
        };

    // Absolutize + native-normalize before handing to the projection layer.
    // On Windows the projection creates junctions via `cmd /C mklink /J <link>
    // <target>`; a relative, forward-slash source (e.g. `./plugins/gal-core`)
    // makes the cmd builtin mis-parse `/plugins` as a switch
    // ("Invalid switch - plugins"). Junction targets must be absolute anyway.
    let source_root = normalize_source_root(source_root);

    let result = match root.as_deref() {
        Some(root) => refresh_with_root(&source_root, root),
        None => refresh_with(&source_root),
    };
    match result {
        Ok(()) => {
            println!("gal refresh");
            match std::env::current_exe() {
                Ok(exe) => println!("  current_exe: {}", exe.display()),
                Err(error) => println!("  current_exe: unavailable ({error})"),
            }
            println!("  source: {}", source_root.display());
            println!("  destination: {}", destination.display());
            println!("done.");
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal refresh: {e}");
            ExitCode::Error
        }
    }
}

pub(crate) fn cmd_dev_apply(_args: &[String]) -> ExitCode {
    eprintln!("gal dev-apply was removed. From the GAL worktree, set PATH and GAL_REFRESH_PATH, then run: cargo run --locked --bin gal -- refresh [--root <dir>]");
    ExitCode::Usage
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn path_identity_selects_and_checks_first_candidate() {
        let temp = TempDir::new().unwrap();
        let same = temp.path().join("same");
        let different = temp.path().join("different");
        let empty = temp.path().join("empty");
        for dir in [&same, &different, &empty] {
            std::fs::create_dir(dir).unwrap();
        }
        let current = temp.path().join("current");
        std::fs::write(&current, b"same").unwrap();
        let name = format!("gal{}", std::env::consts::EXE_SUFFIX);
        let candidate = same.join(&name);
        std::fs::write(&candidate, b"same").unwrap();
        std::fs::write(different.join(&name), b"different").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            for file in [&candidate, &different.join(&name)] {
                std::fs::set_permissions(file, std::fs::Permissions::from_mode(0o755)).unwrap();
            }
        }
        let check = |refresh: Option<&OsStr>, path: Option<&OsStr>, cargo| {
            check_path_identity(&current, refresh, path, cargo)
        };
        assert_eq!(check(None, Some(same.as_os_str()), false), Ok(None));
        assert!(check(Some(different.as_os_str()), Some(same.as_os_str()), false).is_err());
        assert_eq!(
            check(Some(OsStr::new("")), Some(same.as_os_str()), false),
            Ok(None)
        );
        assert!(check(None, Some(empty.as_os_str()), false)
            .unwrap()
            .is_some());
        assert!(check(None, Some(same.as_os_str()), true).is_err());
        std::fs::create_dir(empty.join(&name)).unwrap();
        assert!(check(None, Some(empty.as_os_str()), false).is_err());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&candidate, std::fs::Permissions::from_mode(0o644)).unwrap();
            assert!(check(None, Some(same.as_os_str()), false)
                .unwrap()
                .is_some());
        }
    }

    // Shared process-global env guard (crate::commands::ENV_GUARD) serialises every
    // env/cwd-mutating test in this binary — refresh and restore must share ONE mutex
    // or they race on global HOME. Poison-tolerant via into_inner at each lock site.
    use crate::commands::ENV_GUARD;

    #[cfg(unix)]
    #[test]
    fn operating_system_profile_ignores_home_environment() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        let original = std::env::var_os("HOME");
        std::env::set_var("HOME", tmp.path());
        let profile = operating_system_profile();
        match original {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }
        let profile = profile.expect("account home should be available");
        assert!(profile.is_absolute());
        assert!(profile.is_dir());
        assert_ne!(profile, tmp.path());
    }

    fn set_home(tmp: &Path) {
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", tmp);
        #[cfg(not(windows))]
        std::env::set_var("HOME", tmp);
    }

    fn source_root() -> PathBuf {
        // Use the real repo's plugins/gal-core as source so render passes doc-sync validation.
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        manifest
            .ancestors()
            .nth(2)
            .unwrap()
            .join("plugins")
            .join("gal-core")
    }

    #[test]
    fn refresh_with_writes_canonical_and_marketplace() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        set_home(tmp.path());

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        refresh_with(&src).expect("refresh_with should succeed");

        // Canonical root written.
        let canonical_skill = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("gal")
            .join("commands")
            .join("gal-pipeline")
            .join("SKILL.md");
        assert!(
            canonical_skill.exists(),
            "canonical SKILL.md must be written"
        );

        // Marketplace written.
        let marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".claude-plugin")
            .join("marketplace.json");
        assert!(marketplace.exists(), "marketplace.json must be written");

        let raw = std::fs::read_to_string(&marketplace).unwrap();
        let val: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(val["name"], "gal");
        assert_eq!(val["plugins"][0]["source"], "./gal");
        assert!(val.get("version").is_none(), "no version key");

        // Standard Agent Plugins marketplace written alongside it.
        let agents_marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".agents")
            .join("plugins")
            .join("marketplace.json");
        assert!(
            agents_marketplace.exists(),
            "agents marketplace.json must be written"
        );

        let agents_raw = std::fs::read_to_string(&agents_marketplace).unwrap();
        let agents_val: serde_json::Value = serde_json::from_str(&agents_raw).unwrap();
        assert_eq!(agents_val["name"], "gal");
        assert_eq!(agents_val["plugins"][0]["source"]["source"], "local");
        assert_eq!(agents_val["plugins"][0]["source"]["path"], "./gal");
        assert_eq!(
            agents_val["plugins"][0]["interface"]["displayName"],
            "Golem Agents Legion"
        );
        assert!(
            agents_val.get("version").is_none(),
            "no version key in agents marketplace.json"
        );

        // Canonical and root mcp.json files carry the empty server roster.
        let canonical_mcp = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("gal")
            .join(".mcp.json");
        assert!(
            canonical_mcp.exists(),
            "canonical .mcp.json must be written"
        );
        let mcp_raw = std::fs::read_to_string(&canonical_mcp).unwrap();
        let canonical_mcp_value: serde_json::Value = serde_json::from_str(&mcp_raw).unwrap();
        assert_eq!(canonical_mcp_value["mcpServers"], serde_json::json!({}));

        let root_mcp = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("gal")
            .join("mcp.json");
        assert!(root_mcp.exists(), "root mcp.json must be written");
        let root_mcp_raw = std::fs::read_to_string(&root_mcp).unwrap();
        let root_mcp_value: serde_json::Value = serde_json::from_str(&root_mcp_raw).unwrap();
        assert_eq!(root_mcp_value["mcpServers"], serde_json::json!({}));
    }

    #[test]
    fn refresh_with_idempotent() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        set_home(tmp.path());

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        refresh_with(&src).expect("first run");

        let marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".claude-plugin")
            .join("marketplace.json");
        let first = std::fs::read(&marketplace).unwrap();

        let agents_marketplace = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join(".agents")
            .join("plugins")
            .join("marketplace.json");
        let agents_first = std::fs::read(&agents_marketplace).unwrap();

        refresh_with(&src).expect("second run");
        let second = std::fs::read(&marketplace).unwrap();
        let agents_second = std::fs::read(&agents_marketplace).unwrap();

        // Scope: both marketplace manifests carry no version field, so they are
        // byte-identical on re-run. The per-runtime plugin manifests carry a
        // content-digest stamp (not a timestamp) and are byte-identical on re-run
        // too, but their determinism is covered by the gal-engine render tests
        // rather than here.
        assert_eq!(
            first, second,
            "marketplace.json must be byte-identical on re-run"
        );
        assert_eq!(
            agents_first, agents_second,
            "agents marketplace.json must be byte-identical on re-run"
        );
    }

    #[test]
    fn strip_verbatim_prefix_cases() {
        assert_eq!(
            strip_verbatim_prefix(r"\\?\C:\Code\plugins\gal-core"),
            r"C:\Code\plugins\gal-core"
        );
        assert_eq!(
            strip_verbatim_prefix(r"\\?\UNC\server\share\x"),
            r"\\server\share\x"
        );
        // No prefix → unchanged (covers the Unix path shape too).
        assert_eq!(
            strip_verbatim_prefix("/home/u/plugins/gal-core"),
            "/home/u/plugins/gal-core"
        );
    }

    #[test]
    fn refresh_with_preserves_non_gal_plugin() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let tmp = TempDir::new().unwrap();
        set_home(tmp.path());

        // Place a non-GAL plugin in ~/.gal/plugins before refresh.
        let external = tmp
            .path()
            .join(".gal")
            .join("plugins")
            .join("my-other-plugin");
        std::fs::create_dir_all(&external).unwrap();
        let sentinel = external.join("sentinel.txt");
        std::fs::write(&sentinel, "user-owned").unwrap();

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        refresh_with(&src).expect("refresh_with should not delete non-GAL plugins");

        assert!(
            sentinel.exists(),
            "non-GAL plugin file must survive refresh"
        );
    }

    #[test]
    fn refresh_source_not_found_message_recommends_reinstall() {
        let message = refresh_source_not_found_message();
        assert!(message.contains("no embedded contract"));
        assert!(message.contains("reinstall"));
    }

    // Codex surface summary — snapshot content, no surfaced-success wording.

    #[test]
    fn codex_surface_summary_has_three_segments_and_no_surfaced_success_claim() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let home_tmp = TempDir::new().unwrap();
        set_home(home_tmp.path());

        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        // Render + project first so the summary reflects a real projected state.
        let canonical =
            gal_engine::render::render_canonical_root_from(&src, env!("CARGO_PKG_VERSION"))
                .unwrap();
        let opts = projection::machine_skill_options(&src, home_tmp.path()).unwrap();
        let skill_report = projection::run_update_skills(&opts).unwrap();
        let cmd_report = projection::run_update_commands(&opts).unwrap();

        let summary =
            build_codex_surface_summary(&canonical, home_tmp.path(), &skill_report, &cmd_report);

        assert!(
            summary.contains("required skills:") || summary.contains("required GAL skill"),
            "must have a required-skill status segment: {summary}"
        );
        assert!(
            summary.contains("budget:") || summary.contains("footprint"),
            "must have a budget segment: {summary}"
        );
        assert!(
            summary.contains("zombie cleanup:"),
            "must have a zombie-cleanup segment: {summary}"
        );
        assert!(
            summary.contains("auto-detects skill changes"),
            "must have the auto-detect-first instruction segment: {summary}"
        );
        assert!(
            summary.contains("restart Codex") || summary.contains("fresh thread"),
            "must name the restart/fresh-thread fallback: {summary}"
        );

        let banned = [
            "surfaced successfully",
            "now visible in Codex",
            "Codex surfaced",
            "successfully surfaced",
        ];
        for phrase in banned {
            assert!(
                !summary.contains(phrase),
                "must never claim surfaced success ('{phrase}'): {summary}"
            );
        }
    }
}
