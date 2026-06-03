//! Canonical root rendering: atomic temp+swap, directory scanning, provider projections.
//!
//! Renders the superset canonical plugin root from either:
//! - Dev mode: galRoot source directory
//! - Normal mode: installed/packaged location
//!
//! Uses atomic temp directory + swap to ensure kill-mid-render recovery.

use crate::config::GalConfig;
use crate::mode::GalMode;
use std::fs;
use std::path::{Path, PathBuf};

/// Error types for rendering operations.
#[derive(Debug)]
pub enum RenderError {
    /// I/O error during rendering.
    Io(std::io::Error),
    /// Source root not found or not accessible.
    SourceRootNotFound(PathBuf),
    /// Invalid source structure (missing required components).
    InvalidSourceStructure(String),
    /// Atomic swap failed.
    AtomicSwapFailed(String),
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Io(e) => write!(f, "I/O error: {}", e),
            RenderError::SourceRootNotFound(p) => {
                write!(f, "Source root not found: {}", p.display())
            }
            RenderError::InvalidSourceStructure(msg) => {
                write!(f, "Invalid source structure: {}", msg)
            }
            RenderError::AtomicSwapFailed(msg) => write!(f, "Atomic swap failed: {}", msg),
        }
    }
}

impl std::error::Error for RenderError {}

impl From<std::io::Error> for RenderError {
    fn from(e: std::io::Error) -> Self {
        RenderError::Io(e)
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

    // Scan agent/ directory for agents
    let agents_dir = source_root.join("agent");
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
            // Dev mode: use galRoot from config
            config
                .gal_root
                .as_ref()
                .map(PathBuf::from)
                .ok_or_else(|| {
                    RenderError::InvalidSourceStructure(
                        "Dev mode requires galRoot in config".to_string(),
                    )
                })?
        }
        GalMode::Normal => {
            // Normal mode: use installed/packaged location
            // For now, use current directory (will be refined in later tasks)
            std::env::current_dir()?
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

    // Create temp directory for atomic rendering
    let temp_dir = create_temp_render_dir(&canonical_root)?;

    // Render to temp directory
    render_to_temp(&temp_dir, &source_root, &components)?;

    // Atomic swap: temp -> canonical
    atomic_swap(&temp_dir, &canonical_root)?;

    Ok(canonical_root)
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

    fs::create_dir_all(&skills_dir)?;
    fs::create_dir_all(&commands_dir)?;
    fs::create_dir_all(&agents_dir)?;
    fs::create_dir_all(&agy_agents_dir)?;

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

    // Copy agents
    for agent in &components.agents {
        // Render to both agents/ (filtered) and agy-agents/ (unfiltered)
        let filtered_target = agents_dir.join(format!("{}.md", agent.name));
        let unfiltered_target = agy_agents_dir.join(format!("{}.agent.md", agent.name));

        // For now, just copy as-is (filtering logic will be added later)
        fs::copy(&agent.source_path, &filtered_target)?;
        fs::copy(&agent.source_path, &unfiltered_target)?;
    }

    // Copy other components (mcp.json, etc.) if they exist
    let mcp_source = source_root.join("mcp.json");
    if mcp_source.exists() {
        let mcp_target = temp_dir.join(".mcp.json");
        fs::copy(&mcp_source, &mcp_target)?;
    }

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
}
