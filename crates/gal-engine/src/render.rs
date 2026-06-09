//! Canonical root rendering: atomic temp+swap, directory scanning, provider projections.
//!
//! Renders the superset canonical plugin root from either:
//! - Dev mode: galRoot source directory
//! - Normal mode: installed/packaged location
//!
//! Uses atomic temp directory + swap to ensure kill-mid-render recovery.

use crate::config::GalConfig;
use crate::mode::{resolve_gal_source_root, GalMode};
use std::fs;
use std::path::{Path, PathBuf};

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
    /// Host-OS native gal binary not found — render aborted (fail-loud, R-04).
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
                "gal binary not found at {}: cannot produce plugin bin/ (fail-loud, R-04)",
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
                    let name = entry
                        .file_name()
                        .to_string_lossy()
                        .to_string();
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
                    let name = entry
                        .file_name()
                        .to_string_lossy()
                        .to_string();
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

/// Render the canonical plugin root atomically.
///
/// Uses temp directory + atomic swap to ensure kill-mid-render recovery.
pub fn render_canonical_root(
    config: &GalConfig,
    mode: GalMode,
) -> Result<PathBuf, RenderError> {
    // Determine source root based on mode
    let source_root = match mode {
        GalMode::Dev => {
            // Dev mode: resolve galRoot (handles both gal-core form and repo-root form — R-10/RC-6).
            let gal_root = config.gal_root.as_deref().ok_or_else(|| {
                RenderError::InvalidSourceStructure(
                    "Dev mode requires galRoot in config".to_string(),
                )
            })?;
            resolve_gal_source_root(gal_root)
                .map_err(|e| RenderError::InvalidSourceStructure(format!("{e}")))?
        }
        GalMode::Normal => {
            // Normal mode: resolve the packaged source relative to the installed
            // binary, NOT the process working directory (FU-01). A package-manager
            // install ships the GAL source alongside the executable; the user may
            // invoke `gal` from any directory, so current_dir() is wrong.
            resolve_packaged_source_root()?
        }
    };

    if !source_root.exists() {
        return Err(RenderError::SourceRootNotFound(source_root));
    }

    // Scan source components
    let components = scan_source_components(&source_root)?;

    // Validate that doc-sync and golem-dockeeper exist (intent assertion for BUG-A)
    let has_doc_sync = components.skills.iter().any(|s| s.name == "doc-sync");
    let has_dockeeper = components.agents.iter().any(|a| a.name == "golem-dockeeper");
    
    if !has_doc_sync {
        return Err(RenderError::InvalidSourceStructure(
            "Missing required skill: doc-sync".to_string(),
        ));
    }
    
    if !has_dockeeper {
        return Err(RenderError::InvalidSourceStructure(
            "Missing required agent: golem-dockeeper".to_string(),
        ));
    }

    // Determine canonical root path
    let canonical_root = get_canonical_plugin_root();

    // Clean any orphan .gal-render-* temp dirs before creating a new one (R-05).
    if let Some(plugins_parent) = canonical_root.parent() {
        let _ = clean_orphan_temp_dirs(plugins_parent);
    }

    // Create temp directory for atomic rendering
    let temp_dir = create_temp_render_dir(&canonical_root)?;

    // Render to temp directory
    render_to_temp(&temp_dir, &source_root, &components)?;

    // Atomic swap: temp -> canonical
    atomic_swap(&temp_dir, &canonical_root)?;

    Ok(canonical_root)
}

/// Resolve the packaged source root for normal mode (FU-01).
///
/// In a package-manager install the GAL source travels with the binary, so we
/// resolve it relative to the executable location — never the process working
/// directory, which is wherever the user happened to invoke `gal` from.
///
/// Returns a clear error if no candidate layout next to the binary looks like a
/// GAL source root; it does not silently fall back to the working directory.
fn resolve_packaged_source_root() -> Result<PathBuf, RenderError> {
    let exe = std::env::current_exe()?;
    let exe_dir = exe.parent().ok_or_else(|| {
        RenderError::InvalidSourceStructure("Executable path has no parent directory".to_string())
    })?;
    resolve_source_from_exe_dir(exe_dir)
        .ok_or_else(|| RenderError::SourceRootNotFound(exe_dir.to_path_buf()))
}

/// Pure source resolution given the executable's directory. Extracted from
/// [`resolve_packaged_source_root`] so the layout logic is unit-testable without
/// depending on the real `current_exe()`.
///
/// Tries, in order: a flat layout (binary and source side by side), an
/// FHS-style layout (`<prefix>/bin/gal` + `<prefix>/share/gal`), then a bounded
/// walk up the ancestor chain (covers `bin/` nesting and dev `target/` layouts).
///
/// Public so that `doctor.rs` can use it to locate the `scripts/packaging/` directory
/// alongside the source root (release gate, T-021).
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
/// Mirrors the `galRoot` usable predicate used for dev-mode validation.
fn looks_like_source_root(path: &Path) -> bool {
    path.join("skills").is_dir()
        && path.join("agents").is_dir()
        && path.join("commands").is_dir()
}

/// Get the canonical plugin root path.
fn get_canonical_plugin_root() -> PathBuf {
    // ~/.gal/plugins/gal/
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .unwrap_or_else(|_| ".".to_string());
    Path::new(&home).join(".gal").join("plugins").join("gal")
}

/// Create a temp directory for atomic rendering.
fn create_temp_render_dir(canonical_root: &Path) -> Result<PathBuf, RenderError> {
    let parent = canonical_root
        .parent()
        .ok_or_else(|| {
            RenderError::InvalidSourceStructure("Canonical root has no parent".to_string())
        })?;

    fs::create_dir_all(parent)?;

    // Create temp dir with unique name
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let temp_name = format!(".gal-render-{}", uuid);
    let temp_dir = parent.join(temp_name);

    fs::create_dir(&temp_dir)?;

    Ok(temp_dir)
}

/// Render all components to the temp directory.
fn render_to_temp(
    temp_dir: &Path,
    source_root: &Path,
    components: &ScannedComponents,
) -> Result<(), RenderError> {
    // Create directory structure
    let skills_dir = temp_dir.join("skills");
    let commands_dir = temp_dir.join("commands");
    let agents_dir = temp_dir.join("agents");
    let agy_agents_dir = temp_dir.join("agy-agents");
    let claude_plugin_dir = temp_dir.join(".claude-plugin");
    let rules_dir = temp_dir.join("rules");

    fs::create_dir_all(&skills_dir)?;
    fs::create_dir_all(&commands_dir)?;
    fs::create_dir_all(&agents_dir)?;
    fs::create_dir_all(&agy_agents_dir)?;
    fs::create_dir_all(&claude_plugin_dir)?;
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
    }

    // Expose host-OS-native gal binary in bin/ (R-04 / T-004).
    // Fail-loud if the binary is missing — no half-product.
    let exe = std::env::current_exe()?;
    render_bin_exposure(&exe, temp_dir)?;

    Ok(())
}

/// Copy the host-OS-native `gal` binary into `<dest>/bin/`.
///
/// Produces:
/// - Windows: `bin/gal.exe`
/// - Unix:    `bin/gal` (with `+x` permissions set)
///
/// No shell wrappers (`.sh`/`.ps1`) are ever created.
/// Fails loud if `exe_path` is not a regular file (R-04).
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

/// Scan `plugins_parent` for orphan `.gal-render-*` temp directories (R-05).
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

/// Remove all orphan `.gal-render-*` temp directories under `plugins_parent` (R-05).
///
/// Returns the count of directories successfully removed.
pub fn clean_orphan_temp_dirs(plugins_parent: &Path) -> usize {
    scan_orphan_temp_dirs(plugins_parent)
        .iter()
        .filter(|p| fs::remove_dir_all(p).is_ok())
        .count()
}

/// Filter agent frontmatter for Claude compatibility.
/// Keeps only Claude-compatible keys: name, description, model, effort, maxTurns, 
/// tools, disallowedTools, skills, memory, background, isolation (if worktree).
fn filter_agent_for_claude(agent_path: &Path) -> Result<String, RenderError> {
    let content = fs::read_to_string(agent_path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() < 3 || lines[0] != "---" {
        // No frontmatter, return as-is
        return Ok(content);
    }

    // Find closing ---
    let mut closing_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if *line == "---" {
            closing_idx = Some(i);
            break;
        }
    }

    let closing_idx = match closing_idx {
        Some(idx) => idx,
        None => return Ok(content), // No closing, return as-is
    };

    let allowed_keys = [
        "name", "description", "model", "effort", "maxTurns", 
        "tools", "disallowedTools", "skills", "memory", "background", "isolation"
    ];

    let mut filtered_frontmatter = Vec::new();
    for line in &lines[1..closing_idx] {
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            if allowed_keys.contains(&key) {
                // Special handling for isolation
                if key == "isolation" {
                    let value = line[colon_pos + 1..].trim();
                    if value == "worktree" || value == "\"worktree\"" || value == "'worktree'" {
                        filtered_frontmatter.push(line.to_string());
                    }
                } else {
                    filtered_frontmatter.push(line.to_string());
                }
            }
        }
    }

    let body = &lines[closing_idx + 1..];
    let mut result = Vec::new();
    result.push("---".to_string());
    result.extend(filtered_frontmatter);
    result.push("---".to_string());
    result.extend(body.iter().map(|s| s.to_string()));

    Ok(result.join("\n"))
}

/// Render Claude plugin manifest (.claude-plugin/plugin.json).
fn render_claude_plugin_manifest(
    claude_plugin_dir: &Path,
    components: &ScannedComponents,
) -> Result<(), RenderError> {
    use std::collections::HashMap;

    let mut manifest = HashMap::new();
    manifest.insert("name", "gal");
    manifest.insert("version", "1.0.0");
    manifest.insert("description", "GAL - Golem Agents Legion");

    // List agents
    let agent_names: Vec<String> = components
        .agents
        .iter()
        .map(|a| a.name.clone())
        .collect();
    
    let manifest_json = serde_json::json!({
        "name": "gal",
        "version": "1.0.0",
        "description": "GAL - Golem Agents Legion",
        "agents": agent_names,
        "skills": components.skills.iter().map(|s| &s.name).collect::<Vec<_>>(),
    });

    let manifest_path = claude_plugin_dir.join("plugin.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest_json)?)?;

    Ok(())
}

/// Generate a plugin version string: 1.0.0-timestamp.hash
/// Format matches the PowerShell oracle: timestamp from current time, hash from package content.
/// For initial implementation, we use a simplified version with timestamp + UUID-based suffix.
fn generate_plugin_version() -> String {
    let formatted_time = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let uuid_suffix = uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>();
    
    format!("1.0.0-{}.{}", formatted_time, uuid_suffix)
}

/// Render Copilot manifest (copilot-manifest.json at root).
/// Fixes BUG-02: Copilot will not load commands/ unless the path is explicitly defined
/// in a root-level manifest.
fn render_copilot_manifest(
    temp_dir: &Path,
    _components: &ScannedComponents,
) -> Result<(), RenderError> {
    // Generate version string: 1.0.0-timestamp.hash
    let version = generate_plugin_version();

    let manifest_json = serde_json::json!({
        "name": "gal",
        "displayName": "Golem Agents Legion",
        "version": version,
        "description": "Golem Agents Legion plugin for GitHub Copilot CLI",
        "components": {
            "agents": "agents/",
            "skills": "skills/",
            "commands": "commands/",
            "mcpConfig": ".mcp.json"
        }
    });

    let manifest_path = temp_dir.join("copilot-manifest.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest_json)?)?;

    Ok(())
}

/// Render AGY plugin manifest (plugin.json at root).
fn render_agy_plugin_manifest(
    temp_dir: &Path,
    components: &ScannedComponents,
) -> Result<(), RenderError> {
    let manifest_json = serde_json::json!({
        "name": "gal",
        "version": "1.0.0",
        "description": "GAL - Golem Agents Legion",
        "agents": components.agents.iter().map(|a| format!("agy-agents/{}.agent.md", a.name)).collect::<Vec<_>>(),
        "skills": components.skills.iter().map(|s| format!("skills/{}", s.name)).collect::<Vec<_>>(),
    });

    let manifest_path = temp_dir.join("plugin.json");
    fs::write(&manifest_path, serde_json::to_string_pretty(&manifest_json)?)?;

    Ok(())
}

/// Render instruction corpus (rules/gal.md) from conventions/ and workflows/.
fn render_instruction_corpus(
    rules_dir: &Path,
    source_root: &Path,
) -> Result<(), RenderError> {
    let mut corpus = String::new();
    corpus.push_str("# GAL Instruction Corpus\n\n");
    corpus.push_str("## Conventions\n\n");

    // Collect conventions
    let conventions_dir = source_root.join("conventions");
    if conventions_dir.exists() {
        for entry in fs::read_dir(&conventions_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".md") {
                        let content = fs::read_to_string(entry.path())?;
                        corpus.push_str(&format!("### {}\n\n", name));
                        corpus.push_str(&content);
                        corpus.push_str("\n\n");
                    }
                }
            }
        }
    }

    corpus.push_str("## Workflows\n\n");

    // Collect workflows
    let workflows_dir = source_root.join("workflows");
    if workflows_dir.exists() {
        for entry in fs::read_dir(&workflows_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".md") {
                        let content = fs::read_to_string(entry.path())?;
                        corpus.push_str(&format!("### {}\n\n", name));
                        corpus.push_str(&content);
                        corpus.push_str("\n\n");
                    }
                }
            }
        }
    }

    let corpus_path = rules_dir.join("gal.md");
    fs::write(&corpus_path, corpus)?;

    Ok(())
}

/// Atomic swap: move temp directory to canonical root.
///
/// Uses backup-move-restore pattern for kill-mid-swap recovery.
fn atomic_swap(temp_dir: &Path, canonical_root: &Path) -> Result<(), RenderError> {
    let parent = canonical_root
        .parent()
        .ok_or_else(|| {
            RenderError::InvalidSourceStructure("Canonical root has no parent".to_string())
        })?;

    // Create backup path
    let uuid = uuid::Uuid::new_v4().simple().to_string();
    let backup_name = format!(".gal-plugin-backup-{}", uuid);
    let backup_path = parent.join(backup_name);

    let had_existing_root = canonical_root.exists();

    // Backup existing root if it exists
    if had_existing_root {
        fs::rename(canonical_root, &backup_path)
            .map_err(|e| RenderError::AtomicSwapFailed(format!("Backup failed: {}", e)))?;
    }

    // Move temp to canonical
    let move_result = fs::rename(temp_dir, canonical_root);

    match move_result {
        Ok(()) => {
            // Success: remove backup
            if backup_path.exists() {
                let _ = fs::remove_dir_all(&backup_path);
            }
            Ok(())
        }
        Err(e) => {
            // Failure: restore backup
            if had_existing_root && backup_path.exists() && !canonical_root.exists() {
                let _ = fs::rename(&backup_path, canonical_root);
            }
            Err(RenderError::AtomicSwapFailed(format!(
                "Move failed: {}",
                e
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_source_components() {
        // Scanning a directory that doesn't exist returns empty components
        // (the function uses if exists() checks, so it won't error)
        let result = scan_source_components(Path::new("nonexistent"));
        assert!(result.is_ok());
        let components = result.unwrap();
        assert_eq!(components.skills.len(), 0);
        assert_eq!(components.command_skills.len(), 0);
        assert_eq!(components.agents.len(), 0);
    }

    #[test]
    fn test_get_canonical_plugin_root() {
        let root = get_canonical_plugin_root();
        assert!(root.to_string_lossy().contains(".gal"));
        assert!(root.to_string_lossy().contains("plugins"));
        assert!(root.to_string_lossy().ends_with("gal"));
    }

    /// Create the three marker directories that mark a GAL source root.
    fn make_source_markers(root: &Path) {
        fs::create_dir_all(root.join("skills")).unwrap();
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::create_dir_all(root.join("commands")).unwrap();
    }

    #[test]
    fn test_looks_like_source_root_requires_all_three() {
        use tempfile::TempDir;
        let dir = TempDir::new().unwrap();
        // Only skills/ present → not a source root.
        fs::create_dir_all(dir.path().join("skills")).unwrap();
        assert!(!looks_like_source_root(dir.path()));
        // All three present → source root.
        make_source_markers(dir.path());
        assert!(looks_like_source_root(dir.path()));
    }

    #[test]
    fn test_resolve_source_flat_layout() {
        use tempfile::TempDir;
        // Binary sits in the same directory as skills/agent/commands.
        let dir = TempDir::new().unwrap();
        make_source_markers(dir.path());
        let resolved = resolve_source_from_exe_dir(dir.path());
        assert_eq!(resolved.as_deref(), Some(dir.path()));
    }

    #[test]
    fn test_resolve_source_fhs_layout() {
        use tempfile::TempDir;
        // <prefix>/bin/gal + <prefix>/share/gal/{skills,agent,commands}
        let prefix = TempDir::new().unwrap();
        let bin_dir = prefix.path().join("bin");
        fs::create_dir_all(&bin_dir).unwrap();
        let share_gal = prefix.path().join("share").join("gal");
        make_source_markers(&share_gal);

        let resolved = resolve_source_from_exe_dir(&bin_dir);
        assert_eq!(resolved.as_deref(), Some(share_gal.as_path()));
    }

    #[test]
    fn test_resolve_source_walk_up() {
        use tempfile::TempDir;
        // Source markers at root; binary nested several levels down (dev target/).
        let root = TempDir::new().unwrap();
        make_source_markers(root.path());
        let nested = root.path().join("target").join("debug");
        fs::create_dir_all(&nested).unwrap();

        let resolved = resolve_source_from_exe_dir(&nested);
        assert_eq!(resolved.as_deref(), Some(root.path()));
    }

    #[test]
    fn test_resolve_source_none_when_no_markers() {
        use tempfile::TempDir;
        // Empty tree with no source markers in it or (realistically) any ancestor.
        let dir = TempDir::new().unwrap();
        let exe_dir = dir.path().join("isolated").join("bin");
        fs::create_dir_all(&exe_dir).unwrap();
        assert!(resolve_source_from_exe_dir(&exe_dir).is_none());
    }

    #[test]
    fn test_generate_plugin_version() {
        let version = generate_plugin_version();
        // Version should match format: 1.0.0-timestamp.hash
        assert!(version.starts_with("1.0.0-"));
        let parts: Vec<&str> = version.split('-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "1.0.0");
        // Second part should be timestamp.hash
        let suffix_parts: Vec<&str> = parts[1].split('.').collect();
        assert_eq!(suffix_parts.len(), 2);
        // Timestamp should be 14 digits (yyyyMMddHHmmss)
        assert_eq!(suffix_parts[0].len(), 14);
        // Hash should be 8 characters
        assert_eq!(suffix_parts[1].len(), 8);
    }

    #[test]
    fn test_render_copilot_manifest() {
        use tempfile::TempDir;
        
        let temp_dir = TempDir::new().unwrap();
        let components = ScannedComponents {
            skills: vec![],
            command_skills: vec![],
            agents: vec![],
        };
        
        let result = render_copilot_manifest(temp_dir.path(), &components);
        assert!(result.is_ok());
        
        // Check manifest file exists
        let manifest_path = temp_dir.path().join("copilot-manifest.json");
        assert!(manifest_path.exists());
        
        // Verify content structure
        let content = std::fs::read_to_string(&manifest_path).unwrap();
        let json: serde_json::Value = serde_json::from_str(&content).unwrap();
        
        assert_eq!(json["name"], "gal");
        assert_eq!(json["displayName"], "Golem Agents Legion");
        assert_eq!(json["description"], "Golem Agents Legion plugin for GitHub Copilot CLI");
        assert!(json["version"].as_str().unwrap().starts_with("1.0.0-"));
        
        // Verify components structure
        let components_obj = json["components"].as_object().unwrap();
        assert_eq!(components_obj["agents"], "agents/");
        assert_eq!(components_obj["skills"], "skills/");
        assert_eq!(components_obj["commands"], "commands/");
        assert_eq!(components_obj["mcpConfig"], ".mcp.json");
    }

    // ─── bin/ exposure tests (T-004 / R-04 / TP-04) ─────────────────────────

    #[test]
    fn test_render_bin_exposure_copies_binary() {
        use tempfile::TempDir;

        let src_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        // Create a fake "binary" file to copy.
        let fake_exe = src_dir.path().join("gal_fake");
        fs::write(&fake_exe, b"fake binary content").unwrap();

        let result = render_bin_exposure(&fake_exe, dest_dir.path());
        assert!(result.is_ok(), "render_bin_exposure must succeed: {:?}", result);

        let bin_name = if cfg!(windows) { "gal.exe" } else { "gal" };
        let target = dest_dir.path().join("bin").join(bin_name);
        assert!(target.exists(), "bin/{bin_name} must exist after render_bin_exposure");
        assert!(target.is_file(), "bin/{bin_name} must be a regular file");
    }

    #[test]
    fn test_render_bin_exposure_correct_os_filename() {
        use tempfile::TempDir;

        let src_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        let fake_exe = src_dir.path().join("gal_fake");
        fs::write(&fake_exe, b"fake").unwrap();

        let target = render_bin_exposure(&fake_exe, dest_dir.path()).unwrap();

        #[cfg(windows)]
        assert!(
            target.to_string_lossy().ends_with("gal.exe"),
            "Windows must produce gal.exe, got: {}",
            target.display()
        );
        #[cfg(not(windows))]
        assert!(
            target.to_string_lossy().ends_with("/gal")
                || target.to_string_lossy() == "gal",
            "Unix must produce gal (no extension), got: {}",
            target.display()
        );
    }

    #[test]
    #[cfg(unix)]
    fn test_render_bin_exposure_sets_executable_on_unix() {
        use std::os::unix::fs::PermissionsExt;
        use tempfile::TempDir;

        let src_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        let fake_exe = src_dir.path().join("gal_fake");
        fs::write(&fake_exe, b"fake").unwrap();

        let target = render_bin_exposure(&fake_exe, dest_dir.path()).unwrap();
        let mode = fs::metadata(&target).unwrap().permissions().mode();
        assert!(
            mode & 0o111 != 0,
            "Unix binary must have +x bits set; mode was {:o}",
            mode
        );
    }

    #[test]
    fn test_render_bin_exposure_fail_loud_on_missing_binary() {
        use tempfile::TempDir;

        let dest_dir = TempDir::new().unwrap();
        let missing = Path::new("/absolutely/nonexistent/gal_binary_xyz123");

        let result = render_bin_exposure(missing, dest_dir.path());
        assert!(result.is_err(), "missing binary must fail-loud");
        match result.unwrap_err() {
            RenderError::BinaryNotFound(_) => {}
            other => panic!("expected BinaryNotFound, got: {other}"),
        }
    }

    #[test]
    fn test_render_bin_exposure_no_shell_wrappers() {
        use tempfile::TempDir;

        let src_dir = TempDir::new().unwrap();
        let dest_dir = TempDir::new().unwrap();

        let fake_exe = src_dir.path().join("gal_fake");
        fs::write(&fake_exe, b"fake").unwrap();

        render_bin_exposure(&fake_exe, dest_dir.path()).unwrap();

        let bin_dir = dest_dir.path().join("bin");
        for entry in fs::read_dir(&bin_dir).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            let n = name.to_string_lossy();
            assert!(
                !n.ends_with(".sh") && !n.ends_with(".ps1"),
                "bin/ must not contain shell wrappers; found: {n}"
            );
        }
    }

    // ─── orphan temp dir tests (T-005 / R-05 / TP-06) ───────────────────────

    #[test]
    fn test_scan_orphan_temp_dirs_finds_render_dirs() {
        use tempfile::TempDir;

        let parent = TempDir::new().unwrap();
        // Create orphan render dirs.
        fs::create_dir(parent.path().join(".gal-render-abc123")).unwrap();
        fs::create_dir(parent.path().join(".gal-render-def456")).unwrap();
        // Create a non-orphan dir — must not appear.
        fs::create_dir(parent.path().join("gal")).unwrap();
        // Create a file with the orphan prefix — must not appear (not a dir).
        fs::write(parent.path().join(".gal-render-file"), b"x").unwrap();

        let orphans = scan_orphan_temp_dirs(parent.path());
        assert_eq!(orphans.len(), 2, "expected 2 orphans, got: {:?}", orphans);
        for o in &orphans {
            assert!(
                o.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with(".gal-render-"),
                "unexpected entry in orphans: {}",
                o.display()
            );
        }
    }

    #[test]
    fn test_scan_orphan_temp_dirs_empty_when_none() {
        use tempfile::TempDir;

        let parent = TempDir::new().unwrap();
        fs::create_dir(parent.path().join("gal")).unwrap();
        fs::create_dir(parent.path().join(".gal-plugin-backup-xyz")).unwrap();

        let orphans = scan_orphan_temp_dirs(parent.path());
        assert!(orphans.is_empty(), "expected no orphans, got: {:?}", orphans);
    }

    #[test]
    fn test_scan_orphan_temp_dirs_nonexistent_parent() {
        let orphans = scan_orphan_temp_dirs(Path::new("/nonexistent/no/such/dir/xyz999"));
        assert!(orphans.is_empty());
    }

    #[test]
    fn test_clean_orphan_temp_dirs_removes_and_returns_count() {
        use tempfile::TempDir;

        let parent = TempDir::new().unwrap();
        let orphan1 = parent.path().join(".gal-render-aaa");
        let orphan2 = parent.path().join(".gal-render-bbb");
        fs::create_dir(&orphan1).unwrap();
        fs::create_dir(&orphan2).unwrap();
        // Place a file inside one orphan to verify recursive removal.
        fs::write(orphan1.join("leftover.json"), b"{}").unwrap();

        let cleaned = clean_orphan_temp_dirs(parent.path());
        assert_eq!(cleaned, 2);
        assert!(!orphan1.exists(), "orphan1 must be removed");
        assert!(!orphan2.exists(), "orphan2 must be removed");
    }
}
