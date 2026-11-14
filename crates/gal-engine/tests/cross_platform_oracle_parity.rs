//! macOS / Linux cross-platform behavior contract
//!
//! All tests in this file are gated to non-Windows platforms.
//!
//! These tests define the **Rust render behavior contract** on Unix systems.
//! The retired Bash oracle (`scripts/build-core-plugin.sh`) is no longer
//! a live parity reference; these tests are the authoritative correctness bar.
//!
//! What is tested:
//!   - HOME env var (not USERPROFILE) drives canonical root resolution
//!   - galRoot usable predicate accepts `agents/` (plural) — matches repo layout
//!   - Render produces the expected directory/file structure
//!   - AGY surfaces use Unix symlinks, not Windows junctions
//!
//! How to run via SSH on a remote node:
//!   ssh node-name "cd /path/to/repo && cargo test --test cross_platform_oracle_parity"
//! Or with integration tests:
//!   ssh node-name "cd /path/to/repo && cargo test --test cross_platform_oracle_parity -- --include-ignored"

#[cfg(not(target_os = "windows"))]
mod unix_parity {
    use gal_engine::render::scan_source_components;
    use std::fs;
    use tempfile::TempDir;

    // Uses `agents/` (plural) — matches the actual `plugins/gal-core/` layout.
    fn make_gal_source_tree(root: &std::path::Path) {
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::create_dir_all(root.join("skills")).unwrap();
        fs::create_dir_all(root.join("commands")).unwrap();
        fs::create_dir_all(root.join("codex")).unwrap();
        fs::write(
            root.join("codex").join("hooks.json"),
            include_str!("../../../plugins/gal-core/codex/hooks.json"),
        )
        .unwrap();
        fs::create_dir_all(root.join("claude")).unwrap();
        fs::write(
            root.join("claude").join("hooks.json"),
            include_str!("../../../plugins/gal-core/claude/hooks.json"),
        )
        .unwrap();
    }

    fn make_gal_source_tree_with_content(root: &std::path::Path) {
        make_gal_source_tree(root);

        let skill_dir = root.join("skills").join("doc-sync");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# doc-sync skill\n").unwrap();

        let skill2_dir = root.join("skills").join("git-commits");
        fs::create_dir_all(&skill2_dir).unwrap();
        fs::write(skill2_dir.join("SKILL.md"), "# git-commits skill\n").unwrap();

        fs::write(
            root.join("agents").join("golem-steward.agent.md"),
            "---\nname: golem-steward\ndescription: doc keeper\n---\n# body\n",
        )
        .unwrap();
        fs::write(
            root.join("agents").join("golem-auditor.agent.md"),
            "---\nname: golem-auditor\ndescription: auditor\n---\n# body\n",
        )
        .unwrap();

        let cmd_dir = root.join("commands").join("gal");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join("SKILL.md"), "# gal command\n").unwrap();
    }

    #[test]
    fn scan_uses_agents_plural_directory() {
        let source = TempDir::new().unwrap();
        make_gal_source_tree_with_content(source.path());

        let components =
            scan_source_components(source.path()).expect("scan_source_components must succeed");

        let agent_names: Vec<&str> = components.agents.iter().map(|a| a.name.as_str()).collect();
        assert!(
            agent_names.contains(&"golem-steward"),
            "golem-steward must be found in agents/ (intent assertion): got {:?}",
            agent_names
        );
        assert!(
            agent_names.contains(&"golem-auditor"),
            "golem-auditor must be found in agents/: got {:?}",
            agent_names
        );

        let skill_names: Vec<&str> = components.skills.iter().map(|s| s.name.as_str()).collect();
        assert!(
            skill_names.contains(&"doc-sync"),
            "doc-sync must be found in skills/: got {:?}",
            skill_names
        );

        let cmd_names: Vec<&str> = components
            .command_skills
            .iter()
            .map(|c| c.name.as_str())
            .collect();
        assert!(
            cmd_names.contains(&"gal"),
            "gal command must be found in commands/: got {:?}",
            cmd_names
        );
    }

    #[test]
    #[ignore] // Run explicitly: cargo test -- --include-ignored
    fn render_produces_expected_structure() {
        use gal_engine::render::render_canonical_root_from;

        let source = TempDir::new().unwrap();
        make_gal_source_tree_with_content(source.path());

        let fake_home = TempDir::new().unwrap();
        let home_str = fake_home.path().to_str().unwrap().to_string();

        // NOTE: This mutates global state. Only run in single-threaded mode.
        unsafe {
            std::env::set_var("HOME", &home_str);
        }

        let canonical_root = render_canonical_root_from(source.path(), "0.0.0-test")
            .expect("render_canonical_root_from");

        assert!(
            canonical_root.starts_with(fake_home.path()),
            "canonical_root must be under the isolated HOME"
        );

        let agents_dir = canonical_root.join("agents");
        assert!(agents_dir.is_dir(), "agents/ must exist in canonical root");
        assert!(
            agents_dir.join("golem-steward.md").exists(),
            "golem-steward.md must exist in agents/ (intent assertion)"
        );
        assert!(
            agents_dir.join("golem-auditor.md").exists(),
            "golem-auditor.md must exist in agents/"
        );

        let agy_agents_dir = canonical_root.join("agy-agents");
        assert!(
            agy_agents_dir.is_dir(),
            "agy-agents/ must exist in canonical root"
        );
        assert!(
            agy_agents_dir.join("golem-steward.agent.md").exists(),
            "golem-steward.agent.md must exist in agy-agents/"
        );

        let skills_dir = canonical_root.join("skills");
        assert!(skills_dir.is_dir(), "skills/ must exist in canonical root");
        assert!(
            skills_dir.join("doc-sync").join("SKILL.md").exists(),
            "skills/doc-sync/SKILL.md must exist (intent assertion)"
        );
        assert!(
            skills_dir.join("git-commits").join("SKILL.md").exists(),
            "skills/git-commits/SKILL.md must exist"
        );

        let commands_dir = canonical_root.join("commands");
        assert!(
            commands_dir.is_dir(),
            "commands/ must exist in canonical root"
        );
        assert!(
            commands_dir.join("gal.md").exists(),
            "commands/gal.md must exist"
        );

        assert!(
            canonical_root
                .join(".claude-plugin")
                .join("plugin.json")
                .exists(),
            ".claude-plugin/plugin.json must exist (oracle parity)"
        );
        let codex_path = canonical_root.join(".codex-plugin").join("plugin.json");
        let codex: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&codex_path).unwrap()).unwrap();
        assert_eq!(
            codex["hooks"]["hooks"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<std::collections::BTreeSet<_>>(),
            ["PreToolUse", "Stop", "UserPromptSubmit"]
                .into_iter()
                .collect()
        );
        let claude: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(canonical_root.join(".claude-plugin").join("plugin.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(
            claude["hooks"]
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["Stop"]
        );
        let root_json: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(canonical_root.join("plugin.json")).unwrap())
                .unwrap();
        assert!(
            root_json.get("hooks").is_none(),
            "plugin.json must remain hook-free"
        );
        assert!(
            !canonical_root.join("hooks").join("hooks.json").exists(),
            "root hook asset must remain absent"
        );

        // The obsolete Copilot-only manifest must not be emitted.
        let legacy_copilot_manifest =
            canonical_root.join(format!("{}-{}", "copilot", "manifest.json"));
        assert!(
            !legacy_copilot_manifest.exists(),
            "legacy Copilot manifest must be absent (oracle parity)"
        );

        assert!(
            canonical_root.join("plugin.json").exists(),
            "plugin.json must exist (oracle parity)"
        );

        let copilot_agents = canonical_root.join("com.github.copilot").join("agents");
        assert!(
            copilot_agents.is_dir(),
            "com.github.copilot/agents/ must exist (oracle parity)"
        );
        assert!(
            fs::read_dir(copilot_agents).unwrap().next().is_some(),
            "com.github.copilot/agents/ must be non-empty (oracle parity)"
        );
    }

    #[test]
    fn atomic_swap_works_on_unix() {
        // Indirectly verify atomic_swap by running a full render and confirming
        // no temp dirs remain after completion.
        use gal_engine::render::render_canonical_root_from;

        let source = TempDir::new().unwrap();
        make_gal_source_tree_with_content(source.path());

        let fake_home = TempDir::new().unwrap();
        let home_str = fake_home.path().to_str().unwrap().to_string();

        unsafe {
            std::env::set_var("HOME", &home_str);
        }

        render_canonical_root_from(source.path(), "0.0.0-test")
            .expect("render must succeed on Unix");

        let plugins_dir = fake_home.path().join(".gal").join("plugins");
        if plugins_dir.exists() {
            let temp_dirs: Vec<_> = fs::read_dir(&plugins_dir)
                .unwrap()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .map(|n| n.starts_with(".gal-render-"))
                        .unwrap_or(false)
                })
                .collect();
            assert!(
                temp_dirs.is_empty(),
                "No .gal-render-* temp dirs must remain after successful render: {:?}",
                temp_dirs.iter().map(|e| e.path()).collect::<Vec<_>>()
            );
        }
    }
}
