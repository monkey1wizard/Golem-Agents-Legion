use gal_engine::config::GalConfig;
use gal_engine::install::{run_install, run_uninstall};
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
fn run_uninstall_removes_rust_managed_provider_outputs() {
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

    let gal_home = fake_home.path().join(".gal");
    fs::create_dir_all(gal_home.join("config")).unwrap();
    fs::write(
        gal_home.join("install-state.json"),
        serde_json::json!({
            "selectedRuntimes": ["copilot", "antigravity", "codex", "claude"],
            "primaryRuntime": "copilot"
        })
        .to_string(),
    )
    .unwrap();

    let config = GalConfig {
        dev_mode: Some(true),
        gal_root: Some(source.path().display().to_string()),
        install_mode: None,
    };

    run_install(&config).unwrap();

    assert!(gal_home.join("plugins").join("gal").exists());
    assert!(gal_home.join("dist").join("providers").exists());
    assert!(fake_home.path().join(".copilot").join("installed-plugins").join("gal-copilot").join("gal").exists());
    assert!(fake_home.path().join(".claude").join("skills").join("gal").exists());

    run_uninstall().unwrap();

    assert!(!gal_home.join("plugins").join("gal").exists());
    assert!(!gal_home.join("dist").join("providers").exists());
    assert!(!fake_home.path().join(".copilot").join("installed-plugins").join("gal-copilot").join("gal").exists());
    assert!(!fake_home.path().join(".claude").join("skills").join("gal").exists());
    assert!(gal_home.join("ledger.json").is_file());
}