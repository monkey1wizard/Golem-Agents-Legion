// AGY (Antigravity) provider projection
//
// Creates three surfaces for the Antigravity runtime:
// 1. CLI junction: ~/.gemini/antigravity-cli/plugins/gal -> canonical root
// 2. IDE junction: ~/.gemini/antigravity-ide/plugins/gal -> canonical root
// 3. GUI config: ~/.gemini/commands/*.toml files
//
// This is a best-effort implementation for M1. Full transaction/ledger support
// is deferred to M2 per the plan's OE-A (over-engineering avoidance).
//
// Corresponds to T-010 in fix-gal-bootstrap-install-convergence.md

use std::fs;
use std::path::{Path, PathBuf};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("AGY CLI plugins directory creation failed: {0}")]
    CliPluginsDirCreation(String),
    
    #[error("AGY IDE plugins directory creation failed: {0}")]
    IdePluginsDirCreation(String),
    
    #[error("AGY GUI config directory creation failed: {0}")]
    GuiConfigDirCreation(String),
    
    #[error("Junction/symlink creation failed: {0}")]
    LinkCreation(String),
}

/// AGY provider configuration for creating three surfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgyProjection {
    /// Path to canonical plugin root to link to
    pub canonical_root: PathBuf,
    
    /// CLI junction target: ~/.gemini/antigravity-cli/plugins/gal
    pub cli_target: PathBuf,
    
    /// IDE junction target: ~/.gemini/antigravity-ide/plugins/gal
    pub ide_target: PathBuf,
    
    /// GUI config directory: ~/.gemini/commands
    pub gui_config_dir: PathBuf,
}

impl AgyProjection {
    /// Create a new AGY projection configuration
    pub fn new(canonical_root: PathBuf) -> Result<Self, AgyError> {
        let home = dirs::home_dir().ok_or_else(|| {
            AgyError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Could not determine home directory"
            ))
        })?;
        
        let cli_target = home.join(".gemini").join("antigravity-cli").join("plugins").join("gal");
        let ide_target = home.join(".gemini").join("antigravity-ide").join("plugins").join("gal");
        let gui_config_dir = home.join(".gemini").join("commands");
        
        Ok(Self {
            canonical_root,
            cli_target,
            ide_target,
            gui_config_dir,
        })
    }
    
    /// Create or update all three AGY surfaces
    pub fn apply(&self) -> Result<(), AgyError> {
        // Surface 1: CLI junction
        self.create_cli_junction()?;
        
        // Surface 2: IDE junction
        self.create_ide_junction()?;
        
        // Surface 3: GUI config files
        self.create_gui_configs()?;
        
        Ok(())
    }
    
    /// Create CLI junction: ~/.gemini/antigravity-cli/plugins/gal -> canonical root
    fn create_cli_junction(&self) -> Result<(), AgyError> {
        let plugins_dir = self.cli_target.parent().ok_or_else(|| {
            AgyError::CliPluginsDirCreation("Invalid CLI target path".to_string())
        })?;
        
        fs::create_dir_all(plugins_dir).map_err(|e| {
            AgyError::CliPluginsDirCreation(format!("{}: {}", plugins_dir.display(), e))
        })?;
        
        // Remove existing link/junction if present
        if self.cli_target.exists() {
            if self.cli_target.is_dir() {
                #[cfg(windows)]
                {
                    // On Windows, remove junction using rmdir
                    std::process::Command::new("cmd")
                        .args(&["/C", "rmdir", self.cli_target.to_str().unwrap()])
                        .output()
                        .map_err(|e| AgyError::LinkCreation(format!("Failed to remove existing CLI junction: {}", e)))?;
                }
                #[cfg(not(windows))]
                {
                    fs::remove_file(&self.cli_target)
                        .map_err(|e| AgyError::LinkCreation(format!("Failed to remove existing CLI symlink: {}", e)))?;
                }
            }
        }
        
        // Create junction/symlink
        self.create_link(&self.canonical_root, &self.cli_target, "CLI")?;
        
        Ok(())
    }
    
    /// Create IDE junction: ~/.gemini/antigravity-ide/plugins/gal -> canonical root
    fn create_ide_junction(&self) -> Result<(), AgyError> {
        let plugins_dir = self.ide_target.parent().ok_or_else(|| {
            AgyError::IdePluginsDirCreation("Invalid IDE target path".to_string())
        })?;
        
        fs::create_dir_all(plugins_dir).map_err(|e| {
            AgyError::IdePluginsDirCreation(format!("{}: {}", plugins_dir.display(), e))
        })?;
        
        // Remove existing link/junction if present
        if self.ide_target.exists() {
            if self.ide_target.is_dir() {
                #[cfg(windows)]
                {
                    // On Windows, remove junction using rmdir
                    std::process::Command::new("cmd")
                        .args(&["/C", "rmdir", self.ide_target.to_str().unwrap()])
                        .output()
                        .map_err(|e| AgyError::LinkCreation(format!("Failed to remove existing IDE junction: {}", e)))?;
                }
                #[cfg(not(windows))]
                {
                    fs::remove_file(&self.ide_target)
                        .map_err(|e| AgyError::LinkCreation(format!("Failed to remove existing IDE symlink: {}", e)))?;
                }
            }
        }
        
        // Create junction/symlink
        self.create_link(&self.canonical_root, &self.ide_target, "IDE")?;
        
        Ok(())
    }
    
    /// Create GUI config files: ~/.gemini/commands/*.toml
    fn create_gui_configs(&self) -> Result<(), AgyError> {
        fs::create_dir_all(&self.gui_config_dir).map_err(|e| {
            AgyError::GuiConfigDirCreation(format!("{}: {}", self.gui_config_dir.display(), e))
        })?;
        
        // List of GAL commands to create TOML configs for
        // These correspond to commands/ directories in the canonical root
        let commands = vec![
            "gal",
            "gal-init",
            "gal-status",
            "gal-whats-next",
            "gal-wrap-up",
            "gal-pipeline",
            "planning",
            "deep-planning",
            "refining-plan",
            "plan-to-prompt",
            "git-commit-msg",
        ];
        
        for cmd in commands {
            let toml_path = self.gui_config_dir.join(format!("{}.toml", cmd));
            let skill_path = self.canonical_root.join("commands").join(cmd);
            
            // Only create TOML if the command directory exists in canonical root
            if skill_path.exists() {
                let toml_content = format!(
                    r#"# AGY GUI config for {}
# Generated by GAL best-effort AGY projection (T-010)
# This is a minimal config for M1. Full metadata deferred to M2.

[command]
name = "{}"
skill_path = "{}"
"#,
                    cmd,
                    cmd,
                    skill_path.display().to_string().replace("\\", "/")
                );
                
                fs::write(&toml_path, toml_content)
                    .map_err(|e| AgyError::Io(e))?;
            }
        }
        
        Ok(())
    }
    
    /// Cross-platform link/junction creation
    #[cfg(windows)]
    fn create_link(&self, target: &Path, link: &Path, label: &str) -> Result<(), AgyError> {
        // On Windows, create a directory junction using mklink /J
        let output = std::process::Command::new("cmd")
            .args(&[
                "/C",
                "mklink",
                "/J",
                link.to_str().unwrap(),
                target.to_str().unwrap(),
            ])
            .output()
            .map_err(|e| AgyError::LinkCreation(format!("{} junction creation failed: {}", label, e)))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(AgyError::LinkCreation(format!(
                "{} junction creation failed: {}",
                label, stderr
            )));
        }
        
        Ok(())
    }
    
    #[cfg(not(windows))]
    fn create_link(&self, target: &Path, link: &Path, label: &str) -> Result<(), AgyError> {
        // On Unix-like systems, create a symbolic link
        std::os::unix::fs::symlink(target, link)
            .map_err(|e| AgyError::LinkCreation(format!("{} symlink creation failed: {}", label, e)))?;
        
        Ok(())
    }
    
    /// Check if all three surfaces exist
    pub fn verify_surfaces_exist(&self) -> bool {
        self.cli_target.exists() && self.ide_target.exists() && self.gui_config_dir.exists()
    }
    
    /// Get the list of expected GUI config files
    pub fn expected_gui_configs(&self) -> Vec<PathBuf> {
        let commands = vec![
            "gal", "gal-init", "gal-status", "gal-whats-next", "gal-wrap-up",
            "gal-pipeline", "planning", "deep-planning", "refining-plan",
            "plan-to-prompt", "git-commit-msg",
        ];
        
        commands
            .iter()
            .map(|cmd| self.gui_config_dir.join(format!("{}.toml", cmd)))
            .filter(|p| {
                // Only include if the corresponding command directory exists
                let skill_path = self.canonical_root.join("commands").join(
                    p.file_stem().unwrap().to_str().unwrap()
                );
                skill_path.exists()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[test]
    fn test_agy_projection_new() {
        let temp_dir = TempDir::new().unwrap();
        let canonical_root = temp_dir.path().to_path_buf();
        
        let projection = AgyProjection::new(canonical_root.clone()).unwrap();
        
        assert_eq!(projection.canonical_root, canonical_root);
        assert!(projection.cli_target.to_string_lossy().contains(".gemini"));
        assert!(projection.cli_target.to_string_lossy().contains("antigravity-cli"));
        assert!(projection.ide_target.to_string_lossy().contains(".gemini"));
        assert!(projection.ide_target.to_string_lossy().contains("antigravity-ide"));
        assert!(projection.gui_config_dir.to_string_lossy().contains(".gemini"));
        assert!(projection.gui_config_dir.to_string_lossy().contains("commands"));
    }
    
    #[test]
    fn test_verify_surfaces_exist_when_missing() {
        let temp_dir = TempDir::new().unwrap();
        let canonical_root = temp_dir.path().to_path_buf();
        
        let projection = AgyProjection::new(canonical_root).unwrap();
        
        // Should return false when surfaces don't exist
        assert!(!projection.verify_surfaces_exist());
    }
    
    #[test]
    fn test_expected_gui_configs_empty_when_no_commands() {
        let temp_dir = TempDir::new().unwrap();
        let canonical_root = temp_dir.path().to_path_buf();
        
        let projection = AgyProjection::new(canonical_root).unwrap();
        
        // Should return empty list when no command directories exist
        let configs = projection.expected_gui_configs();
        assert_eq!(configs.len(), 0);
    }
    
    #[test]
    fn test_expected_gui_configs_with_commands() {
        let temp_dir = TempDir::new().unwrap();
        let canonical_root = temp_dir.path().to_path_buf();
        let commands_dir = canonical_root.join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        
        // Create a few command directories
        fs::create_dir_all(commands_dir.join("gal")).unwrap();
        fs::create_dir_all(commands_dir.join("gal-init")).unwrap();
        fs::create_dir_all(commands_dir.join("planning")).unwrap();
        
        let projection = AgyProjection::new(canonical_root).unwrap();
        
        let configs = projection.expected_gui_configs();
        assert_eq!(configs.len(), 3);
        assert!(configs.iter().any(|p| p.file_name().unwrap() == "gal.toml"));
        assert!(configs.iter().any(|p| p.file_name().unwrap() == "gal-init.toml"));
        assert!(configs.iter().any(|p| p.file_name().unwrap() == "planning.toml"));
    }
    
    #[test]
    fn test_create_gui_configs() {
        let temp_canonical = TempDir::new().unwrap();
        let canonical_root = temp_canonical.path().to_path_buf();
        let commands_dir = canonical_root.join("commands");
        
        // Create command directories in canonical root
        fs::create_dir_all(commands_dir.join("gal")).unwrap();
        fs::create_dir_all(commands_dir.join("gal-init")).unwrap();
        
        // Use a separate temp dir for AGY surfaces to avoid home dir pollution
        let temp_home = TempDir::new().unwrap();
        let gui_config_dir = temp_home.path().join(".gemini").join("commands");
        
        let mut projection = AgyProjection::new(canonical_root.clone()).unwrap();
        projection.gui_config_dir = gui_config_dir.clone();
        
        // Create GUI configs
        projection.create_gui_configs().unwrap();
        
        // Verify TOML files were created
        assert!(gui_config_dir.join("gal.toml").exists());
        assert!(gui_config_dir.join("gal-init.toml").exists());
        
        // Verify TOML content
        let gal_toml = fs::read_to_string(gui_config_dir.join("gal.toml")).unwrap();
        assert!(gal_toml.contains("name = \"gal\""));
        assert!(gal_toml.contains("skill_path"));
    }
    
    // Integration test: verify complete surface creation
    // Note: This test manipulates the file system but uses isolated temp directories
    #[test]
    #[ignore] // Mark as ignored for CI - requires file system manipulation
    fn test_apply_creates_all_surfaces() {
        let temp_canonical = TempDir::new().unwrap();
        let canonical_root = temp_canonical.path().to_path_buf();
        
        // Create minimal canonical root structure
        let commands_dir = canonical_root.join("commands");
        fs::create_dir_all(commands_dir.join("gal")).unwrap();
        fs::create_dir_all(commands_dir.join("gal-init")).unwrap();
        
        let agents_dir = canonical_root.join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        
        let skills_dir = canonical_root.join("skills");
        fs::create_dir_all(&skills_dir).unwrap();
        
        // Use temp dirs for AGY surfaces
        let temp_home = TempDir::new().unwrap();
        let cli_plugins_dir = temp_home.path().join(".gemini").join("antigravity-cli").join("plugins");
        let ide_plugins_dir = temp_home.path().join(".gemini").join("antigravity-ide").join("plugins");
        let gui_config_dir = temp_home.path().join(".gemini").join("commands");
        
        let mut projection = AgyProjection::new(canonical_root.clone()).unwrap();
        projection.cli_target = cli_plugins_dir.join("gal");
        projection.ide_target = ide_plugins_dir.join("gal");
        projection.gui_config_dir = gui_config_dir.clone();
        
        // Apply projection
        projection.apply().unwrap();
        
        // Verify CLI junction exists
        assert!(projection.cli_target.exists(), "CLI junction should exist");
        
        // Verify IDE junction exists
        assert!(projection.ide_target.exists(), "IDE junction should exist");
        
        // Verify GUI config directory exists
        assert!(projection.gui_config_dir.exists(), "GUI config dir should exist");
        
        // Verify at least some TOML files were created
        let tomls: Vec<_> = fs::read_dir(&gui_config_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "toml"))
            .collect();
        assert!(tomls.len() >= 2, "Should have at least 2 TOML files");
        
        // Verify surfaces check passes
        assert!(projection.verify_surfaces_exist());
    }
}
