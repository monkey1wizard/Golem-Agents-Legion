//! Manual verification script for AGY three-surface creation (TP-013)
//!
//! This script demonstrates that the AGY provider can create all three surfaces
//! with necessary files as required by the best-effort implementation for M1.
//!
//! Run with: cargo run --example verify_agy_surfaces

use gal_core::providers::agy::AgyProjection;
use std::fs;
use tempfile::TempDir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("TP-013: Verifying AGY three-surface creation (best-effort)");
    println!("============================================================\n");
    
    // Create a temporary canonical root with minimal structure
    let temp_canonical = TempDir::new()?;
    let canonical_root = temp_canonical.path().to_path_buf();
    
    println!("1. Creating minimal canonical root structure...");
    let commands_dir = canonical_root.join("commands");
    fs::create_dir_all(commands_dir.join("gal"))?;
    fs::create_dir_all(commands_dir.join("gal-init"))?;
    fs::create_dir_all(commands_dir.join("planning"))?;
    
    let agents_dir = canonical_root.join("agents");
    fs::create_dir_all(&agents_dir)?;
    fs::write(agents_dir.join("golem-dockeeper.agent.md"), "# Dockeeper\n")?;
    
    let skills_dir = canonical_root.join("skills");
    fs::create_dir_all(skills_dir.join("doc-sync"))?;
    fs::write(skills_dir.join("doc-sync").join("SKILL.md"), "# Doc Sync\n")?;
    
    println!("   ✓ Canonical root created at: {}", canonical_root.display());
    
    // Create temporary directories for AGY surfaces
    let temp_home = TempDir::new()?;
    let cli_plugins_dir = temp_home.path().join(".gemini").join("antigravity-cli").join("plugins");
    let ide_plugins_dir = temp_home.path().join(".gemini").join("antigravity-ide").join("plugins");
    let gui_config_dir = temp_home.path().join(".gemini").join("commands");
    
    println!("\n2. Configuring AGY projection...");
    let mut projection = AgyProjection::new(canonical_root.clone())?;
    projection.cli_target = cli_plugins_dir.join("gal");
    projection.ide_target = ide_plugins_dir.join("gal");
    projection.gui_config_dir = gui_config_dir.clone();
    
    println!("   ✓ CLI target: {}", projection.cli_target.display());
    println!("   ✓ IDE target: {}", projection.ide_target.display());
    println!("   ✓ GUI config dir: {}", projection.gui_config_dir.display());
    
    // Apply the projection
    println!("\n3. Applying AGY projection (creating three surfaces)...");
    projection.apply()?;
    println!("   ✓ Projection applied successfully");
    
    // Verify CLI junction
    println!("\n4. Verifying Surface 1: CLI junction...");
    if projection.cli_target.exists() {
        println!("   ✓ CLI junction exists: {}", projection.cli_target.display());
        if projection.cli_target.is_dir() {
            println!("   ✓ CLI junction is a directory");
        }
    } else {
        println!("   ✗ CLI junction NOT found");
        return Err("CLI junction verification failed".into());
    }
    
    // Verify IDE junction
    println!("\n5. Verifying Surface 2: IDE junction...");
    if projection.ide_target.exists() {
        println!("   ✓ IDE junction exists: {}", projection.ide_target.display());
        if projection.ide_target.is_dir() {
            println!("   ✓ IDE junction is a directory");
        }
    } else {
        println!("   ✗ IDE junction NOT found");
        return Err("IDE junction verification failed".into());
    }
    
    // Verify GUI config directory and files
    println!("\n6. Verifying Surface 3: GUI config files...");
    if projection.gui_config_dir.exists() {
        println!("   ✓ GUI config directory exists: {}", projection.gui_config_dir.display());
        
        let expected_configs = projection.expected_gui_configs();
        println!("   ✓ Expected {} TOML config files", expected_configs.len());
        
        for config_path in &expected_configs {
            if config_path.exists() {
                println!("   ✓ Found: {}", config_path.file_name().unwrap().to_string_lossy());
                
                // Check TOML content
                let content = fs::read_to_string(config_path)?;
                if content.contains("[command]") && content.contains("name =") {
                    println!("      (valid TOML structure)");
                } else {
                    println!("      ⚠ Warning: TOML structure may be incomplete");
                }
            } else {
                println!("   ✗ Missing: {}", config_path.file_name().unwrap().to_string_lossy());
            }
        }
        
        if expected_configs.is_empty() {
            println!("   ⚠ No TOML files expected (no matching commands in canonical root)");
        }
    } else {
        println!("   ✗ GUI config directory NOT found");
        return Err("GUI config directory verification failed".into());
    }
    
    // Final verification check
    println!("\n7. Running final verification check...");
    if projection.verify_surfaces_exist() {
        println!("   ✓ All three surfaces verified successfully");
    } else {
        println!("   ✗ Surface verification failed");
        return Err("Final verification failed".into());
    }
    
    println!("\n============================================================");
    println!("TP-013 PASSED: AGY three surfaces exist with necessary files");
    println!("============================================================\n");
    println!("Note: This is a best-effort implementation for M1.");
    println!("Full transaction/ledger support is deferred to M2 per OE-A.");
    
    Ok(())
}
