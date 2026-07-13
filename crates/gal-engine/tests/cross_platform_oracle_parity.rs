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
//!   - MCP serializers produce identical output to Windows (pure logic, no OS dependency)
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

    // -----------------------------------------------------------------------
    // Helper: build a minimal GAL source tree with the real directory names.
    // Uses `agents/` (plural) — matches the actual `plugins/gal-core/` layout.
    // -----------------------------------------------------------------------
    fn make_gal_source_tree(root: &std::path::Path) {
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::create_dir_all(root.join("skills")).unwrap();
        fs::create_dir_all(root.join("commands")).unwrap();
    }

    fn make_gal_source_tree_with_content(root: &std::path::Path) {
        make_gal_source_tree(root);

        // Add a skill with SKILL.md
        let skill_dir = root.join("skills").join("doc-sync");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(skill_dir.join("SKILL.md"), "# doc-sync skill\n").unwrap();

        // Add a second skill
        let skill2_dir = root.join("skills").join("git-commits");
        fs::create_dir_all(&skill2_dir).unwrap();
        fs::write(skill2_dir.join("SKILL.md"), "# git-commits skill\n").unwrap();

        // Add an agent (intent assertion: golem-steward must be present)
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

        // Add a command skill
        let cmd_dir = root.join("commands").join("gal");
        fs::create_dir_all(&cmd_dir).unwrap();
        fs::write(cmd_dir.join("SKILL.md"), "# gal command\n").unwrap();
    }

    // -----------------------------------------------------------------------
    // (unit): scan_source_components uses agents/ directory
    // -----------------------------------------------------------------------
    #[test]
    fn scan_uses_agents_plural_directory() {
        let source = TempDir::new().unwrap();
        make_gal_source_tree_with_content(source.path());

        let components =
            scan_source_components(source.path()).expect("scan_source_components must succeed");

        // Agents found from agents/ (plural)
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

        // Skills found from skills/
        let skill_names: Vec<&str> = components.skills.iter().map(|s| s.name.as_str()).collect();
        assert!(
            skill_names.contains(&"doc-sync"),
            "doc-sync must be found in skills/: got {:?}",
            skill_names
        );

        // Commands found from commands/
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

    // -----------------------------------------------------------------------
    // (unit): AGY surfaces use Unix symlinks (not junctions)
    // -----------------------------------------------------------------------
    #[test]
    fn agy_projection_creates_unix_symlinks() {
        use projection::agy::AgyProjection;

        let temp_canonical = TempDir::new().unwrap();
        let canonical_root = temp_canonical.path().to_path_buf();

        // Create minimal canonical root so apply() has something to link to
        let commands_dir = canonical_root.join("commands");
        fs::create_dir_all(commands_dir.join("gal")).unwrap();

        let temp_home = TempDir::new().unwrap();
        let cli_target = temp_home
            .path()
            .join(".gemini")
            .join("antigravity-cli")
            .join("plugins")
            .join("gal");
        let ide_target = temp_home
            .path()
            .join(".gemini")
            .join("antigravity-ide")
            .join("plugins")
            .join("gal");
        let gui_config_dir = temp_home.path().join(".gemini").join("commands");

        let mut projection = AgyProjection::new(canonical_root.clone()).unwrap();
        projection.cli_target = cli_target.clone();
        projection.ide_target = ide_target.clone();
        projection.gui_config_dir = gui_config_dir;

        projection.apply().expect("AGY apply must succeed on Unix");

        // On Unix, CLI target must be a symlink (not a Windows junction).
        assert!(cli_target.exists(), "CLI target must exist after apply");
        assert!(
            cli_target.is_symlink(),
            "CLI target must be a Unix symlink (not a Windows junction)"
        );

        // Verify symlink points to canonical root
        let link_target = std::fs::read_link(&cli_target).expect("read_link must succeed");
        assert_eq!(
            link_target, canonical_root,
            "CLI symlink must point to canonical root"
        );

        assert!(ide_target.exists(), "IDE target must exist after apply");
        assert!(
            ide_target.is_symlink(),
            "IDE target must be a Unix symlink (not a Windows junction)"
        );
    }

    #[test]
    fn agy_removal_removes_unix_symlinks() {
        use projection::agy::AgyProjection;

        let temp_canonical = TempDir::new().unwrap();
        let canonical_root = temp_canonical.path().to_path_buf();
        fs::create_dir_all(canonical_root.join("commands").join("gal")).unwrap();

        let temp_home = TempDir::new().unwrap();
        let cli_target = temp_home
            .path()
            .join(".gemini")
            .join("antigravity-cli")
            .join("plugins")
            .join("gal");
        let ide_target = temp_home
            .path()
            .join(".gemini")
            .join("antigravity-ide")
            .join("plugins")
            .join("gal");

        let mut projection = AgyProjection::new(canonical_root.clone()).unwrap();
        projection.cli_target = cli_target.clone();
        projection.ide_target = ide_target.clone();
        projection.gui_config_dir = temp_home.path().join(".gemini").join("commands");

        projection.apply().unwrap();
        assert!(cli_target.is_symlink());

        projection.remove().expect("remove must succeed on Unix");

        assert!(
            !cli_target.exists(),
            "CLI symlink must be removed after remove()"
        );
        assert!(
            !ide_target.exists(),
            "IDE symlink must be removed after remove()"
        );
    }

    // -----------------------------------------------------------------------
    // (integration): render produces expected structure
    //
    // This is an isolated-home integration test verifying that the canonical
    // root structure is correct on Unix:
    //   - agents/ directory with filtered .md files
    //   - agy-agents/ directory with unfiltered .agent.md files
    //   - skills/ directory with per-skill subdirs containing SKILL.md
    //   - commands/ directory with .md files
    //   - .claude-plugin/plugin.json
    //   - copilot-manifest.json
    //   - plugin.json
    //
    // Version strings and timestamps are NOT compared (they differ between
    // runs). The test verifies structure by file name.
    // -----------------------------------------------------------------------
    #[test]
    #[ignore] // Run explicitly: cargo test -- --include-ignored
    fn render_produces_expected_structure() {
        use gal_engine::render::render_canonical_root_from;

        // Build a real-ish source tree with the structure matching gal-core
        let source = TempDir::new().unwrap();
        make_gal_source_tree_with_content(source.path());

        // Use an isolated home so render writes to a temp location
        let fake_home = TempDir::new().unwrap();
        let home_str = fake_home.path().to_str().unwrap().to_string();

        // Override HOME for this process
        // NOTE: This mutates global state. Only run in single-threaded mode.
        unsafe {
            std::env::set_var("HOME", &home_str);
        }

        let canonical_root =
            render_canonical_root_from(source.path()).expect("render_canonical_root_from");

        // Verify canonical root is under our fake home
        assert!(
            canonical_root.starts_with(fake_home.path()),
            "canonical_root must be under the isolated HOME"
        );

        // ---- Structure checks (oracle parity) ----

        // agents/ directory with per-agent .md files (Claude-filtered)
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

        // agy-agents/ directory with unfiltered .agent.md files
        let agy_agents_dir = canonical_root.join("agy-agents");
        assert!(
            agy_agents_dir.is_dir(),
            "agy-agents/ must exist in canonical root"
        );
        assert!(
            agy_agents_dir.join("golem-steward.agent.md").exists(),
            "golem-steward.agent.md must exist in agy-agents/"
        );

        // skills/ directory
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

        // commands/ directory
        let commands_dir = canonical_root.join("commands");
        assert!(
            commands_dir.is_dir(),
            "commands/ must exist in canonical root"
        );
        assert!(
            commands_dir.join("gal.md").exists(),
            "commands/gal.md must exist"
        );

        // Claude plugin manifest
        assert!(
            canonical_root
                .join(".claude-plugin")
                .join("plugin.json")
                .exists(),
            ".claude-plugin/plugin.json must exist (oracle parity)"
        );

        // Copilot manifest
        assert!(
            canonical_root.join("copilot-manifest.json").exists(),
            "copilot-manifest.json must exist (oracle parity)"
        );

        // AGY plugin manifest
        assert!(
            canonical_root.join("plugin.json").exists(),
            "plugin.json must exist (oracle parity)"
        );

        // Copilot manifest structure
        let copilot_manifest: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(canonical_root.join("copilot-manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(copilot_manifest["name"], "gal");
        assert_eq!(copilot_manifest["components"]["agents"], "agents/");
        assert_eq!(copilot_manifest["components"]["skills"], "skills/");
        assert_eq!(copilot_manifest["components"]["commands"], "commands/");
    }

    // -----------------------------------------------------------------------
    // (unit): MCP serializers produce same output on Unix as Windows
    //
    // The MCP serializers (claude.rs, copilot.rs) are pure logic with one
    // platform branch: on Windows, stdio servers with env vars get wrapped in
    // PowerShell. On macOS/Linux they must NOT wrap in PowerShell.
    // -----------------------------------------------------------------------
    #[test]
    fn claude_desktop_env_vars_not_wrapped_on_unix() {
        use mcp::serializers::claude_mcp::ClaudeDesktopMcpConfig;
        use mcp::serializers::McpHostConfig;
        use mcp::{McpManifest, McpServer};
        use std::collections::HashMap;

        let mut env = HashMap::new();
        env.insert("API_KEY".to_string(), "secret123".to_string());

        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: Some(env),
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = ClaudeDesktopMcpConfig::from_manifest(&manifest).unwrap();
        let json = config.to_json_pretty().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        let entry = &value["mcpServers"]["test-server"];

        // On Unix: command stays as "node", no PowerShell wrapper
        assert_eq!(
            entry["command"], "node",
            "On Unix, Claude stdio server with env must NOT be wrapped in PowerShell"
        );
        assert_eq!(
            entry["args"],
            serde_json::json!(["index.js"]),
            "Args must not be wrapped in PowerShell on Unix"
        );
    }

    #[test]
    fn copilot_cli_local_server_parity_on_unix() {
        use mcp::serializers::copilot::CopilotCliMcpConfig;
        use mcp::serializers::McpHostConfig;
        use mcp::{McpManifest, McpServer};
        use std::collections::HashMap;

        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            McpServer {
                server_type: Some("stdio".to_string()),
                command: Some("node".to_string()),
                args: Some(vec!["index.js".to_string()]),
                env: None,
                url: None,
                headers: None,
            },
        );

        let manifest = McpManifest {
            servers,
            inputs: None,
        };

        let config = CopilotCliMcpConfig::from_manifest(&manifest).unwrap();
        let json = config.to_json_pretty().unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();

        let entry = &value["mcpServers"]["test-server"];

        // Oracle: Copilot CLI uses type=local, tools=["*"] — same on all platforms
        assert_eq!(
            entry["type"], "local",
            "Copilot type must be 'local' on Unix"
        );
        assert_eq!(
            entry["tools"],
            serde_json::json!(["*"]),
            "Copilot tools must be ['*'] on Unix"
        );
        assert_eq!(entry["command"], "node");
    }

    // -----------------------------------------------------------------------
    // (unit): atomic swap works on Unix filesystem
    // -----------------------------------------------------------------------
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

        render_canonical_root_from(source.path()).expect("render must succeed on Unix");

        // After render, no .gal-render-* temp dirs should remain under the plugins parent
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
