//! R-05 install-family parity tests.
//!
//! These tests define the missing behavior the Rust install path must own
//! before the legacy install/build scripts can be deleted.

use gal_engine::config::GalConfig;
use gal_engine::install::run_install;
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn make_gal_source_tree_with_content(root: &Path) {
    fs::create_dir_all(root.join("agents")).unwrap();
    fs::create_dir_all(root.join("skills").join("doc-sync")).unwrap();
    fs::create_dir_all(root.join("commands").join("gal")).unwrap();

    fs::write(
        root.join("agents").join("golem-dockeeper.agent.md"),
        "---\nname: golem-dockeeper\ndescription: doc keeper\n---\n# body\n",
    )
    .unwrap();
    fs::write(
        root.join("skills").join("doc-sync").join("SKILL.md"),
        "# doc-sync skill\n",
    )
    .unwrap();
    fs::write(
        root.join("commands").join("gal").join("SKILL.md"),
        "# gal command\n",
    )
    .unwrap();
}

#[test]
fn run_install_writes_provider_lifecycle_state_for_primary_runtimes() {
    let fake_home = TempDir::new().unwrap();
    let source = TempDir::new().unwrap();
    make_gal_source_tree_with_content(source.path());

    #[cfg(windows)]
    unsafe {
        std::env::set_var("USERPROFILE", fake_home.path());
        std::env::set_var("HOME", fake_home.path());
    }

    #[cfg(not(windows))]
    unsafe {
        std::env::set_var("HOME", fake_home.path());
    }

    let resolved_home = base::paths::user_home().expect("home_dir should resolve in isolated test");
    assert_eq!(
        resolved_home,
        fake_home.path(),
        "base::paths::user_home() must point at the isolated test home"
    );

    let gal_home = fake_home.path().join(".gal");
    fs::create_dir_all(gal_home.join("config")).unwrap();
    fs::write(
        gal_home.join("install-state.json"),
        serde_json::to_string_pretty(&json!({
            "selectedRuntimes": ["copilot", "antigravity", "codex", "claude"],
            "primaryRuntime": "copilot"
        }))
        .unwrap(),
    )
    .unwrap();

    let config = GalConfig {
        dev_mode: Some(true),
        gal_root: Some(source.path().display().to_string()),
        install_mode: None,
    };

    let report = run_install(&config).expect("run_install should succeed in isolated home");
    assert!(report.canonical_root.exists(), "canonical root must be rendered");
    assert!(
        report
            .canonical_root
            .join(".codex-plugin")
            .join("plugin.json")
            .is_file(),
        "codex plugin manifest must be rendered"
    );

    let generated_providers = gal_home.join("dist").join("providers");
    assert!(
        generated_providers.join("copilot").join("managed.json").is_file(),
        "copilot managed state must be written"
    );
    assert!(
        generated_providers.join("claude").join("managed.json").is_file(),
        "claude managed state must be written"
    );
    assert!(
        generated_providers.join("codex").join("managed.json").is_file(),
        "codex managed state must be written"
    );
    assert!(
        generated_providers.join("agy").join("managed.json").is_file(),
        "agy managed state must be written"
    );

    assert!(
        gal_home
            .join("plugins")
            .join(".claude-plugin")
            .join("marketplace.json")
            .is_file(),
        "claude marketplace manifest must be written"
    );
    assert!(
        gal_home
            .join("plugins")
            .join(".agents")
            .join("plugins")
            .join("marketplace.json")
            .is_file(),
        "codex marketplace manifest must be written"
    );
    assert!(
        fake_home
            .path()
            .join(".copilot")
            .join("installed-plugins")
            .join("gal-copilot")
            .join("gal")
            .join("copilot-manifest.json")
            .is_file(),
        "copilot projection must exist after install"
    );
}