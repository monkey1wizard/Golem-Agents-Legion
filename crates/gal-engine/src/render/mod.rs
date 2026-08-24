//! Canonical root rendering: atomic temp+swap, directory scanning, provider projections.
//!
//! Renders the superset canonical plugin root from a resolved source root.
//! Uses atomic temp directory + swap to ensure kill-mid-render recovery.

use std::fs;
use std::path::{Path, PathBuf};

mod manifests;
#[cfg(test)]
mod tests;
use manifests::*;

/// Error types for rendering operations.
#[derive(Debug)]
pub enum RenderError {
    /// I/O error during rendering.
    Io(std::io::Error),
    /// JSON serialization error.
    Json(serde_json::Error),
    /// Source root not found or not accessible.
    SourceRootNotFound(PathBuf),
    /// Invalid source structure (missing required components).
    InvalidSourceStructure(String),
    /// Atomic swap failed.
    AtomicSwapFailed(String),
    /// Host-OS native gal binary not found — render aborted (fail-loud).
    BinaryNotFound(PathBuf),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Io(e) => write!(f, "I/O error: {}", e),
            RenderError::Json(e) => write!(f, "JSON error: {}", e),
            RenderError::SourceRootNotFound(p) => {
                write!(f, "Source root not found: {}", p.display())
            }
            RenderError::InvalidSourceStructure(msg) => {
                write!(f, "Invalid source structure: {}", msg)
            }
            RenderError::AtomicSwapFailed(msg) => write!(f, "Atomic swap failed: {}", msg),
            RenderError::BinaryNotFound(p) => write!(
                f,
                "gal binary not found at {}: cannot produce plugin bin/ (fail-loud)",
                p.display()
            ),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        RenderError::Io(e)
    }
}

impl From<serde_json::Error> for RenderError {
    fn from(e: serde_json::Error) -> Self {
        RenderError::Json(e)
    }
}

/// Metadata about a skill discovered during directory scan.
#[derive(Debug, Clone)]
pub struct SkillEntry {
    pub name: String,
    pub source_path: PathBuf,
}

/// Metadata about a command skill discovered during directory scan.
#[derive(Debug, Clone)]
pub struct CommandSkillEntry {
    pub name: String,
    pub source_path: PathBuf,
}

/// Metadata about an agent discovered during directory scan.
#[derive(Debug, Clone)]
pub struct AgentEntry {
    pub name: String,
    pub source_path: PathBuf,
}

/// Result of scanning source directories.
#[derive(Debug)]
pub struct ScannedComponents {
    pub skills: Vec<SkillEntry>,
    pub command_skills: Vec<CommandSkillEntry>,
    pub agents: Vec<AgentEntry>,
}

/// Scan the source directory for skills, command skills, and agents.
pub fn scan_source_components(source_root: &Path) -> Result<ScannedComponents, RenderError> {
    let mut skills = Vec::new();
    let mut command_skills = Vec::new();
    let mut agents = Vec::new();

    // Scan skills/ directory
    let skills_dir = source_root.join("skills");
    if skills_dir.exists() {
        for entry in fs::read_dir(&skills_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let skill_file = entry.path().join("SKILL.md");
                if skill_file.exists() {
                    let name = entry.file_name().to_string_lossy().to_string();
                    skills.push(SkillEntry {
                        name,
                        source_path: skill_file,
                    });
                }
            }
        }
    }
    skills.sort_by(|a, b| a.name.cmp(&b.name));

    // Scan commands/ directory for command skills
    let commands_dir = source_root.join("commands");
    if commands_dir.exists() {
        for entry in fs::read_dir(&commands_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let skill_file = entry.path().join("SKILL.md");
                let template_file = entry.path().join("SKILL.template.md");
                let source_path = if skill_file.exists() {
                    Some(skill_file)
                } else if template_file.exists() {
                    Some(template_file)
                } else {
                    None
                };

                if let Some(source_path) = source_path {
                    let name = entry.file_name().to_string_lossy().to_string();
                    command_skills.push(CommandSkillEntry { name, source_path });
                }
            }
        }
    }
    command_skills.sort_by(|a, b| a.name.cmp(&b.name));

    // Scan agents/ directory for agents
    let agents_dir = source_root.join("agents");
    if agents_dir.exists() {
        for entry in fs::read_dir(&agents_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".agent.md") {
                        let base_name = name.trim_end_matches(".agent.md").to_string();
                        agents.push(AgentEntry {
                            name: base_name,
                            source_path: entry.path(),
                        });
                    }
                }
            }
        }
    }
    agents.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(ScannedComponents {
        skills,
        command_skills,
        agents,
    })
}

/// Merge personal skills from `local_skills_root` into `components.skills`.
///
/// Core-wins collision policy: if a skill name already exists in
/// `components.skills` (from the core source scan), the personal skill with
/// that same name is silently skipped. Non-colliding personal skills are
/// appended and flow through `render_to_temp` identically to core skills.
/// Unreadable `local_skills_root` is treated as fail-safe (no personal skills).
pub(crate) fn merge_personal_skills(components: &mut ScannedComponents, local_skills_root: &Path) {
    let existing: std::collections::HashSet<String> =
        components.skills.iter().map(|s| s.name.clone()).collect();

    let entries = match fs::read_dir(local_skills_root) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut personal: Vec<SkillEntry> = Vec::new();
    for entry in entries.flatten() {
        let Ok(ft) = entry.file_type() else {
            continue;
        };
        if !ft.is_dir() {
            continue;
        }
        let skill_file = entry.path().join("SKILL.md");
        if !skill_file.exists() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if existing.contains(&name) {
            continue; // core-wins: personal skill with colliding name is skipped
        }
        personal.push(SkillEntry {
            name,
            source_path: skill_file,
        });
    }
    personal.sort_by(|a, b| a.name.cmp(&b.name));
    components.skills.extend(personal);
}

/// Merge personal MCP servers from `local_mcp_path` into the rendered `.mcp.json`.
///
/// Core-wins: if the `"servers"` map in the target already contains a server name, the
/// personal entry with that name is skipped (`Map::entry().or_insert_with()` semantics).
/// Any I/O or parse error leaves the target file unchanged (fail-safe).
pub(crate) fn merge_personal_mcp(target_mcp: &Path, local_mcp_path: &Path) {
    let core_str = match fs::read_to_string(target_mcp) {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut core: serde_json::Value = match serde_json::from_str(&core_str) {
        Ok(v) => v,
        Err(_) => return,
    };
    let personal_str = match fs::read_to_string(local_mcp_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let personal: serde_json::Value = match serde_json::from_str(&personal_str) {
        Ok(v) => v,
        Err(_) => return,
    };
    if let (Some(core_servers), Some(personal_servers)) = (
        core.get_mut("servers").and_then(|v| v.as_object_mut()),
        personal.get("servers").and_then(|v| v.as_object()),
    ) {
        for (name, value) in personal_servers {
            core_servers
                .entry(name.clone())
                .or_insert_with(|| value.clone());
        }
    }
    if let Ok(merged) = serde_json::to_string_pretty(&core) {
        let _ = fs::write(target_mcp, merged);
    }
}

/// Render the canonical plugin root atomically from a resolved source root.
///
/// Thin wrapper over [`render_canonical_root_to`] targeting the home-derived
/// canonical root with machine-local content included (personal layer + host
/// binary) — the standard local install/refresh behavior. Byte-identical to the
/// pre-split behavior.
pub fn render_canonical_root_from(source_root: &Path) -> Result<PathBuf, RenderError> {
    render_canonical_root_to(source_root, &get_canonical_plugin_root(), true)
}

/// Render the canonical plugin root atomically to an explicit `plugin_root`.
///
/// `include_machine_local` controls whether machine-local content is injected:
/// - `true`  — personal layer (skills / plugin / MCP, config-gated) plus host
///   `gal` binary exposure. Used by the local install/refresh path.
/// - `false` — pure portable source render: no personal layer and no `bin/`.
///   Used to build a public marketplace snapshot that must never leak
///   machine-local content or bundle a platform-specific binary.
pub fn render_canonical_root_to(
    source_root: &Path,
    plugin_root: &Path,
    include_machine_local: bool,
) -> Result<PathBuf, RenderError> {
    if !source_root.exists() {
        return Err(RenderError::SourceRootNotFound(source_root.to_path_buf()));
    }

    // Scan source components
    let mut components = scan_source_components(source_root)?;

    // Required core components must survive render: doc-sync skill + golem-steward agent.
    let has_doc_sync = components.skills.iter().any(|s| s.name == "doc-sync");
    let has_steward = components.agents.iter().any(|a| a.name == "golem-steward");

    if !has_doc_sync {
        return Err(RenderError::InvalidSourceStructure(
            "Missing required skill: doc-sync".to_string(),
        ));
    }

    if !has_steward {
        return Err(RenderError::InvalidSourceStructure(
            "Missing required agent: golem-steward".to_string(),
        ));
    }

    // Optionally merge personal skills + personal plugin components (machine-local
    // layer). Presence-based: a file present at the personal skills root is the
    // opt-in signal, not a config flag (the flag was a double opt-in — dir AND
    // flag both required — and has been removed). Fail-safe: an absent dir
    // silently skips the personal layer so the render always completes with at
    // least the core components.
    if include_machine_local {
        if let Some(local_skills) =
            gal_foundation::paths::gal_local_skills_root().filter(|p| p.exists())
        {
            merge_personal_skills(&mut components, &local_skills);
        }
    }

    // Target the explicit plugin root: home-derived canonical for `from`, or an
    // arbitrary staging dir for a marketplace snapshot.
    let canonical_root = plugin_root.to_path_buf();

    // Clean any orphan .gal-render-* temp dirs before creating a new one.
    if let Some(plugins_parent) = canonical_root.parent() {
        let _ = clean_orphan_temp_dirs(plugins_parent);
    }

    // Create temp directory for atomic rendering
    let temp_dir = create_temp_render_dir(&canonical_root)?;

    // Render to temp directory
    render_to_temp(&temp_dir, source_root, &components, include_machine_local)?;

    // Atomic swap: temp -> canonical
    atomic_swap(&temp_dir, &canonical_root)?;

    Ok(canonical_root)
}

/// Pure source resolution given the executable's directory. Separated from
/// `current_exe()` resolution so the layout logic is unit-testable without
/// depending on the real running binary.
///
/// Tries, in order: a flat layout (binary and source side by side), an
/// FHS-style layout (`<prefix>/bin/gal` + `<prefix>/share/gal`), then a bounded
/// walk up the ancestor chain (covers `bin/` nesting and dev `target/` layouts).
///
/// Public so that `doctor.rs` can use it to locate the `packaging/` directory
/// alongside the source root (release gate).
pub fn resolve_source_from_exe_dir(exe_dir: &Path) -> Option<PathBuf> {
    // 1. Flat layout: gal(.exe) and skills/agent/commands in the same directory.
    if looks_like_source_root(exe_dir) {
        return Some(exe_dir.to_path_buf());
    }

    // 2. FHS layout: <prefix>/bin/gal next to <prefix>/share/gal.
    if let Some(prefix) = exe_dir.parent() {
        let fhs = prefix.join("share").join("gal");
        if looks_like_source_root(&fhs) {
            return Some(fhs);
        }
    }

    // 3. Walk up ancestors (binary inside a nested bin/, or dev target/ tree).
    let mut cursor = exe_dir;
    while let Some(parent) = cursor.parent() {
        if looks_like_source_root(parent) {
            return Some(parent.to_path_buf());
        }
        cursor = parent;
    }

    None
}

/// Does this path look like a GAL source root? Requires the three marker
/// directories that the renderer scans: `skills/`, `agents/`, `commands/`.
fn looks_like_source_root(path: &Path) -> bool {
    path.join("skills").is_dir() && path.join("agents").is_dir() && path.join("commands").is_dir()
}

/// Pure cwd-based source resolution: walk `cwd` and each of its ancestors,
/// checking at every level both `<level>/plugins/gal-core` (the repo-checkout
/// layout) and `<level>` itself (the flat layout) against
/// [`looks_like_source_root`]. Returns on the first hit, `None` if the walk
/// reaches the filesystem root without one.
///
/// Additive: does not change [`resolve_source_from_exe_dir`] or
/// [`render_canonical_root`] in any way — this is a new, independent recovery
/// path for `gal refresh` invoked from a repo checkout, not a replacement for
/// the packaged-binary resolution.
pub fn resolve_source_from_cwd(cwd: &Path) -> Option<PathBuf> {
    let mut cursor = Some(cwd);
    while let Some(level) = cursor {
        let repo_checkout = level.join("plugins").join("gal-core");
        if looks_like_source_root(&repo_checkout) {
            return Some(repo_checkout);
        }
        if looks_like_source_root(level) {
            return Some(level.to_path_buf());
        }
        cursor = level.parent();
    }
    None
}

/// Idempotently materialize the build-time embedded gal-core payload into
/// `~/.gal/embedded-src` and return that path. This is the Cargo-channel
/// fallback: `cargo install --git` produces a bare binary with no source payload
/// beside it, so neither [`resolve_source_from_cwd`] nor
/// [`resolve_source_from_exe_dir`] finds a root. The payload is baked into the
/// binary at build time (see [`crate::embedded`]); this writes it to disk once
/// and reuses it.
///
/// Idempotent by per-file byte comparison (no version key — the workspace ships
/// a single static `0.1.0`, so a version key would never invalidate). When the
/// on-disk copy already matches the embedded bytes it is left untouched; only a
/// mismatch triggers an atomic rebuild (extract to a `.tmp` sibling, then swap).
/// Returns `None` when the binary embedded an empty tree (non-git build) or
/// `~/.gal` cannot be resolved.
///
/// Concurrency-safe: each caller extracts into its own process-unique staging
/// directory (never a shared name another concurrent `gal` process could
/// collide with or delete out from under), then hands the swap to
/// [`gal_foundation::platform::atomic_swap`] — never a direct
/// `remove_dir_all` of the canonical target. If the swap loses a race to a
/// concurrent winner, only the caller's own staging tree is removed; the
/// canonical target is rechecked byte-for-byte and accepted as success when
/// it already matches (a concurrent winner materialized the same content).
pub fn materialize_embedded_source() -> Option<PathBuf> {
    if !crate::embedded::is_populated() {
        return None;
    }
    let home = gal_foundation::paths::gal_home()?;
    let target = home.join("embedded-src");
    if embedded_matches_disk(&target) {
        return Some(target);
    }
    // Rebuild atomically: extract to a caller-unique staging sibling, swap in.
    let staging_name = format!(
        "embedded-src.staging-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    );
    let staging = home.join(staging_name);
    fs::create_dir_all(&staging).ok()?;
    if crate::embedded::extract_to(&staging).is_err() {
        let _ = fs::remove_dir_all(&staging);
        return None;
    }
    match atomic_swap(&staging, &target) {
        Ok(()) => Some(target),
        Err(_) => {
            // Lost the swap race (or another platform-level swap failure).
            // Clean up only our own staging tree — never touch the canonical
            // target directly. A concurrent winner may have produced
            // byte-identical content; recheck before reporting failure.
            //
            // The recheck is retried briefly: a concurrent winner's OWN
            // `atomic_swap` briefly renames the target out to a backup path
            // before renaming its staging in (see `atomic_swap`'s
            // backup-move-restore sequence), so an immediate single recheck
            // can observe that transient window and see the target as
            // (temporarily) missing even though the winner's swap completes
            // moments later. A handful of short-sleep retries resolves this
            // without an unbounded wait.
            let _ = fs::remove_dir_all(&staging);
            for attempt in 0..20 {
                if embedded_matches_disk(&target) {
                    return Some(target);
                }
                if attempt < 19 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }
            None
        }
    }
}

/// True when every embedded file already exists on disk under `target` with
/// identical bytes. A missing/changed file (or a non-directory `target`) is a
/// mismatch that forces a rebuild.
fn embedded_matches_disk(target: &Path) -> bool {
    fn walk(dir: &include_dir::Dir, target: &Path) -> bool {
        for file in dir.files() {
            // `file.path()` is relative to the embed root.
            match fs::read(target.join(file.path())) {
                Ok(bytes) if bytes == file.contents() => {}
                _ => return false,
            }
        }
        dir.dirs().all(|d| walk(d, target))
    }
    if !target.is_dir() {
        return false;
    }
    walk(&crate::embedded::EMBEDDED_GAL_CORE, target)
}

/// Get the canonical plugin root path.
///
/// Uses the shared `gal_foundation::paths` resolution so install/render honor the same
/// home-directory semantics as the rest of GAL.
fn get_canonical_plugin_root() -> PathBuf {
    gal_foundation::paths::gal_plugin_root("gal")
        .unwrap_or_else(|| PathBuf::from(".").join(".gal").join("plugins").join("gal"))
}

/// Create a temp directory for atomic rendering.
///
/// Domain validation (canonical root must have a parent) stays here; the staging
/// mechanism delegates to `gal_foundation::render::create_temp_render_dir`.
fn create_temp_render_dir(canonical_root: &Path) -> Result<PathBuf, RenderError> {
    let parent = canonical_root.parent().ok_or_else(|| {
        RenderError::InvalidSourceStructure("Canonical root has no parent".to_string())
    })?;

    Ok(gal_foundation::render::create_temp_render_dir(parent)?)
}

/// Render all components to the temp directory.
fn render_to_temp(
    temp_dir: &Path,
    source_root: &Path,
    components: &ScannedComponents,
    include_machine_local: bool,
) -> Result<(), RenderError> {
    // Create directory structure
    let skills_dir = temp_dir.join("skills");
    let commands_dir = temp_dir.join("commands");
    let agents_dir = temp_dir.join("agents");
    let agy_agents_dir = temp_dir.join("agy-agents");
    let claude_plugin_dir = temp_dir.join(".claude-plugin");
    let codex_plugin_dir = temp_dir.join(".codex-plugin");
    let rules_dir = temp_dir.join("rules");

    fs::create_dir_all(&skills_dir)?;
    fs::create_dir_all(&commands_dir)?;
    fs::create_dir_all(&agents_dir)?;
    fs::create_dir_all(&agy_agents_dir)?;
    fs::create_dir_all(&claude_plugin_dir)?;
    fs::create_dir_all(&codex_plugin_dir)?;
    fs::create_dir_all(&rules_dir)?;

    // Copy skills
    for skill in &components.skills {
        let target_skill_dir = skills_dir.join(&skill.name);
        fs::create_dir_all(&target_skill_dir)?;
        let target_file = target_skill_dir.join("SKILL.md");
        fs::copy(&skill.source_path, &target_file)?;
    }

    // Copy command skills (rendered as flat command files)
    for cmd_skill in &components.command_skills {
        let target_file = commands_dir.join(format!("{}.md", cmd_skill.name));
        fs::copy(&cmd_skill.source_path, &target_file)?;
    }

    // Copy agents with frontmatter filtering
    for agent in &components.agents {
        // Render to agents/ (Claude-filtered)
        let filtered_target = agents_dir.join(format!("{}.md", agent.name));
        let filtered_content = filter_agent_for_claude(&agent.source_path)?;
        fs::write(&filtered_target, filtered_content)?;

        // Render to agy-agents/ (unfiltered)
        let unfiltered_target = agy_agents_dir.join(format!("{}.agent.md", agent.name));
        fs::copy(&agent.source_path, &unfiltered_target)?;
    }

    // Render Claude plugin manifest
    render_claude_plugin_manifest(&claude_plugin_dir, components)?;

    // Render Codex plugin manifest
    render_codex_plugin_manifest(&codex_plugin_dir)?;

    // Render Copilot manifest
    render_copilot_manifest(temp_dir, components)?;

    // Render AGY plugin manifest
    render_agy_plugin_manifest(temp_dir, components)?;

    // Render instruction corpus
    render_instruction_corpus(&rules_dir, source_root)?;

    // Copy MCP config if it exists
    let mcp_source = source_root.join("mcp.json");
    if mcp_source.exists() {
        let mcp_target = temp_dir.join(".mcp.json");
        fs::copy(&mcp_source, &mcp_target)?;

        // Optionally merge personal MCP servers (machine-local layer). Presence-based:
        // a personal mcp.json present is the opt-in signal, not a config flag.
        // Core-wins: personal server with same name as core is skipped.
        // Fail-safe: missing/bad personal mcp.json leaves .mcp.json unchanged.
        if include_machine_local {
            if let Some(local_mcp) =
                gal_foundation::paths::gal_local_mcp_path().filter(|p| p.exists())
            {
                merge_personal_mcp(&mcp_target, &local_mcp);
            }
        }
    }

    // Expose host-OS-native gal binary in bin/ — machine-local installs only.
    // A public marketplace snapshot must never bundle a platform-specific binary,
    // so it is skipped when include_machine_local is false.
    // Fail-loud if the binary is missing — no half-product.
    if include_machine_local {
        let exe = std::env::current_exe()?;
        render_bin_exposure(&exe, temp_dir)?;
    }

    Ok(())
}

/// Copy the host-OS-native `gal` binary into `<dest>/bin/`.
///
/// Produces:
/// - Windows: `bin/gal.exe`
/// - Unix:    `bin/gal` (with `+x` permissions set)
///
/// No shell wrappers (`.sh`/`.ps1`) are ever created.
/// Fails loud if `exe_path` is not a regular file.
pub fn render_bin_exposure(exe_path: &Path, dest: &Path) -> Result<PathBuf, RenderError> {
    if !exe_path.is_file() {
        return Err(RenderError::BinaryNotFound(exe_path.to_path_buf()));
    }

    let bin_dir = dest.join("bin");
    fs::create_dir_all(&bin_dir)?;

    let bin_name = if cfg!(windows) { "gal.exe" } else { "gal" };
    let target = bin_dir.join(bin_name);

    fs::copy(exe_path, &target)?;

    // On Unix, ensure the binary is executable.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&target)?.permissions();
        perms.set_mode(perms.mode() | 0o111);
        fs::set_permissions(&target, perms)?;
    }

    Ok(target)
}

/// Scan `plugins_parent` for orphan `.gal-render-*` temp directories.
///
/// These are left behind when a render is interrupted before the atomic swap.
/// Returns paths that exist as directories and match the `.gal-render-*` prefix.
pub fn scan_orphan_temp_dirs(plugins_parent: &Path) -> Vec<PathBuf> {
    let Ok(rd) = fs::read_dir(plugins_parent) else {
        return Vec::new();
    };
    let mut orphans: Vec<PathBuf> = rd
        .flatten()
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(".gal-render-"))
                .unwrap_or(false)
                && e.path().is_dir()
        })
        .map(|e| e.path())
        .collect();
    orphans.sort();
    orphans
}

/// Remove all orphan `.gal-render-*` temp directories under `plugins_parent`.
///
/// Returns the count of directories successfully removed.
pub fn clean_orphan_temp_dirs(plugins_parent: &Path) -> usize {
    scan_orphan_temp_dirs(plugins_parent)
        .iter()
        .filter(|p| fs::remove_dir_all(p).is_ok())
        .count()
}

/// Atomic swap: move temp directory to canonical root.
///
/// Delegates to `gal_foundation::platform::atomic_swap`, mapping its error
/// back to `RenderError` with the existing message text preserved.
fn atomic_swap(temp_dir: &Path, canonical_root: &Path) -> Result<(), RenderError> {
    use gal_foundation::platform::AtomicSwapError;
    gal_foundation::platform::atomic_swap(temp_dir, canonical_root).map_err(|e| match e {
        AtomicSwapError::NoParent => {
            RenderError::InvalidSourceStructure("Canonical root has no parent".to_string())
        }
        AtomicSwapError::BackupFailed(msg) => {
            RenderError::AtomicSwapFailed(format!("Backup failed: {}", msg))
        }
        AtomicSwapError::MoveFailed(msg) => {
            RenderError::AtomicSwapFailed(format!("Move failed: {}", msg))
        }
    })
}
