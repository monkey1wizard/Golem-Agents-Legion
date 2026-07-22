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

// resolve_source_from_cwd — additive cwd-first source resolution.

#[test]
fn resolve_source_from_cwd_repo_checkout_layout_at_repo_root() {
    use tempfile::TempDir;
    // Repo-checkout layout: <repo>/plugins/gal-core holds the source markers;
    // cwd is the repo root itself.
    let repo = TempDir::new().unwrap();
    let source_root = repo.path().join("plugins").join("gal-core");
    make_source_markers(&source_root);

    let resolved = resolve_source_from_cwd(repo.path());
    assert_eq!(resolved.as_deref(), Some(source_root.as_path()));
}

#[test]
fn resolve_source_from_cwd_finds_repo_root_from_a_nested_working_directory() {
    use tempfile::TempDir;
    let repo = TempDir::new().unwrap();
    let source_root = repo.path().join("plugins").join("gal-core");
    make_source_markers(&source_root);
    let nested_cwd = repo.path().join("crates").join("cli").join("src");
    fs::create_dir_all(&nested_cwd).unwrap();

    let resolved = resolve_source_from_cwd(&nested_cwd);
    assert_eq!(resolved.as_deref(), Some(source_root.as_path()));
}

#[test]
fn resolve_source_from_cwd_accepts_flat_layout_at_cwd_itself() {
    use tempfile::TempDir;
    // No plugins/gal-core wrapper; cwd itself carries the source markers.
    let dir = TempDir::new().unwrap();
    make_source_markers(dir.path());

    let resolved = resolve_source_from_cwd(dir.path());
    assert_eq!(resolved.as_deref(), Some(dir.path()));
}

#[test]
fn resolve_source_from_cwd_none_when_no_markers_in_any_ancestor() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let cwd = dir.path().join("some").join("unrelated").join("dir");
    fs::create_dir_all(&cwd).unwrap();

    assert!(resolve_source_from_cwd(&cwd).is_none());
}

/// Regression: resolve_source_from_cwd is additive — it must not change any of
/// resolve_source_from_exe_dir's existing layout-resolution behavior.
#[test]
fn resolve_source_from_exe_dir_behavior_unchanged_by_cwd_resolver() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    make_source_markers(dir.path());
    assert_eq!(
        resolve_source_from_exe_dir(dir.path()).as_deref(),
        Some(dir.path()),
        "flat-layout exe-dir resolution must be unaffected by the new cwd resolver"
    );

    let isolated = TempDir::new().unwrap();
    let exe_dir = isolated.path().join("isolated").join("bin");
    fs::create_dir_all(&exe_dir).unwrap();
    assert!(
        resolve_source_from_exe_dir(&exe_dir).is_none(),
        "no-markers exe-dir resolution must still return None"
    );
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
    assert_eq!(
        json["description"],
        "Golem Agents Legion plugin for GitHub Copilot CLI"
    );
    assert!(json["version"].as_str().unwrap().starts_with("1.0.0-"));

    // Verify components structure
    let components_obj = json["components"].as_object().unwrap();
    assert_eq!(components_obj["agents"], "agents/");
    assert_eq!(components_obj["skills"], "skills/");
    assert_eq!(components_obj["commands"], "commands/");
    assert_eq!(components_obj["mcpConfig"], ".mcp.json");
}

// ─── bin/ exposure tests ─────────────────────────

#[test]
fn test_render_bin_exposure_copies_binary() {
    use tempfile::TempDir;

    let src_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();

    // Create a fake "binary" file to copy.
    let fake_exe = src_dir.path().join("gal_fake");
    fs::write(&fake_exe, b"fake binary content").unwrap();

    let result = render_bin_exposure(&fake_exe, dest_dir.path());
    assert!(
        result.is_ok(),
        "render_bin_exposure must succeed: {:?}",
        result
    );

    let bin_name = if cfg!(windows) { "gal.exe" } else { "gal" };
    let target = dest_dir.path().join("bin").join(bin_name);
    assert!(
        target.exists(),
        "bin/{bin_name} must exist after render_bin_exposure"
    );
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
        target.to_string_lossy().ends_with("/gal") || target.to_string_lossy() == "gal",
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

// ─── orphan temp dir tests ───────────────────────

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
    assert!(
        orphans.is_empty(),
        "expected no orphans, got: {:?}",
        orphans
    );
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

// ─── render_personal_layer: merge_personal_skills ───────────────────────

/// Helper: create a skill directory with a SKILL.md stub.
fn make_skill_dir(parent: &std::path::Path, name: &str) -> PathBuf {
    let dir = parent.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("SKILL.md"), format!("# {name}")).unwrap();
    dir
}

#[test]
fn render_personal_layer_merge_no_collision_appends_skill() {
    use tempfile::TempDir;
    let local_skills = TempDir::new().unwrap();
    make_skill_dir(local_skills.path(), "my-skill");

    let mut components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    merge_personal_skills(&mut components, local_skills.path());
    assert_eq!(components.skills.len(), 1);
    assert_eq!(components.skills[0].name, "my-skill");
}

#[test]
fn render_personal_layer_merge_collision_core_wins() {
    use tempfile::TempDir;
    let local_skills = TempDir::new().unwrap();
    make_skill_dir(local_skills.path(), "doc-sync"); // collides with core
    make_skill_dir(local_skills.path(), "personal-only");

    let core_skill_path = local_skills.path().join("__core_doc_sync").join("SKILL.md");
    fs::create_dir_all(core_skill_path.parent().unwrap()).unwrap();
    fs::write(&core_skill_path, "# doc-sync core").unwrap();

    let mut components = ScannedComponents {
        skills: vec![SkillEntry {
            name: "doc-sync".to_string(),
            source_path: core_skill_path,
        }],
        command_skills: vec![],
        agents: vec![],
    };
    merge_personal_skills(&mut components, local_skills.path());

    // doc-sync must still be the core version (only one entry)
    let doc_sync_entries: Vec<_> = components
        .skills
        .iter()
        .filter(|s| s.name == "doc-sync")
        .collect();
    assert_eq!(
        doc_sync_entries.len(),
        1,
        "core-wins: exactly one doc-sync entry"
    );
    assert!(
        doc_sync_entries[0]
            .source_path
            .to_string_lossy()
            .contains("__core_doc_sync"),
        "core-wins: doc-sync must point to core source"
    );
    // personal-only must be appended
    assert!(
        components.skills.iter().any(|s| s.name == "personal-only"),
        "non-colliding personal skill must be present"
    );
}

#[test]
fn claude_plugin_manifest_has_no_version() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    manifests::render_claude_plugin_manifest(dir.path(), &components).unwrap();
    let raw = fs::read_to_string(dir.path().join("plugin.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(
        json.get("version").is_none(),
        "Claude manifest must not carry a version: {raw}"
    );
    assert_eq!(json["name"], "gal");
    // Component fields are PATHS in Claude's schema; name arrays make Claude reject
    // the manifest. They must be omitted so Claude auto-discovers commands/, agents/,
    // skills/ from the plugin dir.
    assert!(
        json.get("commands").is_none(),
        "commands key must be omitted: {raw}"
    );
    assert!(
        json.get("agents").is_none(),
        "agents key must be omitted: {raw}"
    );
    assert!(
        json.get("skills").is_none(),
        "skills key must be omitted: {raw}"
    );
}

#[test]
fn agy_plugin_manifest_has_no_version() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    manifests::render_agy_plugin_manifest(dir.path(), &components).unwrap();
    let raw = fs::read_to_string(dir.path().join("plugin.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(
        json.get("version").is_none(),
        "agy manifest must not carry a version: {raw}"
    );
    assert_eq!(json["name"], "gal");
}

#[test]
fn codex_copilot_manifests_keep_dynamic_version() {
    // Regression: the version removal must not strip codex/copilot dynamic versions.
    let v = manifests::generate_plugin_version();
    assert!(
        v.starts_with("1.0.0-"),
        "dynamic version retains timestamped form: {v}"
    );
}

#[test]
fn render_personal_layer_merge_no_skill_md_skips_dir() {
    use tempfile::TempDir;
    let local_skills = TempDir::new().unwrap();
    // dir without SKILL.md
    fs::create_dir_all(local_skills.path().join("incomplete-skill")).unwrap();
    // dir with SKILL.md
    make_skill_dir(local_skills.path(), "valid-skill");

    let mut components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    merge_personal_skills(&mut components, local_skills.path());
    assert_eq!(components.skills.len(), 1);
    assert_eq!(components.skills[0].name, "valid-skill");
}

#[test]
fn render_personal_layer_merge_nonexistent_root_is_failsafe() {
    let mut components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    merge_personal_skills(
        &mut components,
        Path::new("/nonexistent/local/skills/xyz999"),
    );
    assert!(
        components.skills.is_empty(),
        "unreadable root must produce no skills"
    );
}

#[test]
fn render_personal_layer_merge_preserves_sort_order() {
    use tempfile::TempDir;
    let local_skills = TempDir::new().unwrap();
    make_skill_dir(local_skills.path(), "zzz-skill");
    make_skill_dir(local_skills.path(), "aaa-skill");

    let mut components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    merge_personal_skills(&mut components, local_skills.path());
    assert_eq!(components.skills.len(), 2);
    assert_eq!(components.skills[0].name, "aaa-skill");
    assert_eq!(components.skills[1].name, "zzz-skill");
}

// ─── render_personal_layer: merge_personal_mcp ──────────────────────────

fn make_mcp_json(dir: &std::path::Path, servers: &[(&str, &str)]) -> PathBuf {
    let mut srv = serde_json::Map::new();
    for (name, url) in servers {
        srv.insert(
            name.to_string(),
            serde_json::json!({ "type": "http", "url": url }),
        );
    }
    let obj = serde_json::json!({ "servers": serde_json::Value::Object(srv) });
    let path = dir.join("mcp.json");
    fs::write(&path, serde_json::to_string_pretty(&obj).unwrap()).unwrap();
    path
}

#[test]
fn render_personal_mcp_no_collision_appends_server() {
    use tempfile::TempDir;
    let core_dir = TempDir::new().unwrap();
    let local_dir = TempDir::new().unwrap();
    let target = make_mcp_json(core_dir.path(), &[("core-srv", "https://core.example")]);
    let local = make_mcp_json(
        local_dir.path(),
        &[("personal-srv", "https://personal.example")],
    );

    merge_personal_mcp(&target, &local);

    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    let servers = merged["servers"].as_object().unwrap();
    assert!(
        servers.contains_key("core-srv"),
        "core server must be retained"
    );
    assert!(
        servers.contains_key("personal-srv"),
        "personal server must be added"
    );
    assert_eq!(servers.len(), 2);
}

#[test]
fn render_personal_mcp_collision_core_wins() {
    use tempfile::TempDir;
    let core_dir = TempDir::new().unwrap();
    let local_dir = TempDir::new().unwrap();
    let target = make_mcp_json(
        core_dir.path(),
        &[("shared-srv", "https://core.example/core")],
    );
    let local = make_mcp_json(
        local_dir.path(),
        &[
            ("shared-srv", "https://personal.example/personal"),
            ("extra-srv", "https://personal.example/extra"),
        ],
    );

    merge_personal_mcp(&target, &local);

    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    let servers = merged["servers"].as_object().unwrap();
    assert_eq!(
        servers["shared-srv"]["url"], "https://core.example/core",
        "core-wins: core url must not be overwritten"
    );
    assert!(
        servers.contains_key("extra-srv"),
        "non-colliding personal server must be added"
    );
    assert_eq!(servers.len(), 2);
}

#[test]
fn render_personal_mcp_missing_local_file_is_failsafe() {
    use tempfile::TempDir;
    let core_dir = TempDir::new().unwrap();
    let target = make_mcp_json(core_dir.path(), &[("core-srv", "https://core.example")]);
    let before = fs::read_to_string(&target).unwrap();

    merge_personal_mcp(&target, Path::new("/nonexistent/local/mcp.json"));

    assert_eq!(fs::read_to_string(&target).unwrap(), before);
}

#[test]
fn render_personal_mcp_invalid_local_json_is_failsafe() {
    use tempfile::TempDir;
    let core_dir = TempDir::new().unwrap();
    let bad_dir = TempDir::new().unwrap();
    let target = make_mcp_json(core_dir.path(), &[("core-srv", "https://core.example")]);
    let before = fs::read_to_string(&target).unwrap();

    let bad_local = bad_dir.path().join("mcp.json");
    fs::write(&bad_local, b"{ not valid json").unwrap();

    merge_personal_mcp(&target, &bad_local);

    assert_eq!(fs::read_to_string(&target).unwrap(), before);
}

#[test]
fn render_personal_mcp_wrong_servers_key_in_local_is_noop() {
    use tempfile::TempDir;
    let core_dir = TempDir::new().unwrap();
    let local_dir = TempDir::new().unwrap();
    let target = make_mcp_json(core_dir.path(), &[("core-srv", "https://core.example")]);
    let before = fs::read_to_string(&target).unwrap();

    // personal file uses "mcpServers" instead of "servers" → noop
    let local = local_dir.path().join("mcp.json");
    fs::write(
        &local,
        r#"{"mcpServers": {"p": {"type": "http", "url": "x"}}}"#,
    )
    .unwrap();

    merge_personal_mcp(&target, &local);

    assert_eq!(fs::read_to_string(&target).unwrap(), before);
}

use super::manifests;

// ── filter_agent_for_claude ───────────────────────────────────────────────

#[test]
fn filter_agent_abstract_tools_converted_to_claude_names() {
    let dir = tempfile::tempdir().unwrap();
    let agent_file = dir.path().join("golem-test.agent.md");
    fs::write(
        &agent_file,
        "---\nname: golem-test\ntools: [read, search]\n---\nbody\n",
    )
    .unwrap();
    let out = manifests::filter_agent_for_claude(&agent_file).unwrap();
    assert!(
        out.contains("tools: [Read, Grep, Glob]"),
        "expected claude tool names, got: {out}"
    );
    assert!(
        out.contains("name: golem-test"),
        "name line must be preserved"
    );
}

#[test]
fn filter_agent_edit_token_maps_to_read_grep_glob_edit_write() {
    let dir = tempfile::tempdir().unwrap();
    let agent_file = dir.path().join("golem-edit.agent.md");
    fs::write(&agent_file, "---\nname: golem-edit\ntools: [edit]\n---\n").unwrap();
    let out = manifests::filter_agent_for_claude(&agent_file).unwrap();
    assert!(out.contains("Read"), "edit must expand to Read");
    assert!(out.contains("Edit"), "edit must expand to Edit");
    assert!(out.contains("Write"), "edit must expand to Write");
}

#[test]
fn filter_agent_no_frontmatter_returned_as_is() {
    let dir = tempfile::tempdir().unwrap();
    let agent_file = dir.path().join("bare.md");
    let body = "just body content\n";
    fs::write(&agent_file, body).unwrap();
    let out = manifests::filter_agent_for_claude(&agent_file).unwrap();
    assert_eq!(out, body);
}

#[test]
fn filter_agent_unknown_abstract_tokens_produce_no_tools_line() {
    let dir = tempfile::tempdir().unwrap();
    let agent_file = dir.path().join("golem-noop.agent.md");
    fs::write(
        &agent_file,
        "---\nname: golem-noop\ntools: [unknown_token]\n---\nbody\n",
    )
    .unwrap();
    let out = manifests::filter_agent_for_claude(&agent_file).unwrap();
    assert!(
        !out.contains("tools:"),
        "unknown tokens must not emit tools line"
    );
}

#[test]
fn filter_agent_non_allowed_keys_stripped() {
    let dir = tempfile::tempdir().unwrap();
    let agent_file = dir.path().join("golem-strip.agent.md");
    fs::write(
        &agent_file,
        "---\nname: golem-strip\ncustom_key: value\ntools: [read]\n---\nbody\n",
    )
    .unwrap();
    let out = manifests::filter_agent_for_claude(&agent_file).unwrap();
    assert!(
        !out.contains("custom_key"),
        "non-allowed keys must be stripped"
    );
    assert!(out.contains("name: golem-strip"), "name must remain");
    assert!(out.contains("tools: [Read]"), "tools must be converted");
}

/// `render_canonical_root_to` with `include_machine_local=false` renders the
/// full portable plugin tree (both manifests + component dirs) to an explicit
/// output root and produces NO `bin/` — no bundled host binary. Runs without a
/// `gal` binary present precisely because bin exposure is skipped in the
/// core-only path.
#[test]
fn render_canonical_root_to_core_only_excludes_bin_and_personal() {
    use tempfile::TempDir;

    // Real repo source so render passes doc-sync + steward validation.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("plugins")
        .join("gal-core");
    if !source.exists() {
        return; // skip on machines without a checked-out source
    }

    let out = TempDir::new().unwrap();
    let plugin_root = out.path().join("gal");

    let rendered = render_canonical_root_to(&source, &plugin_root, false)
        .expect("core-only render must succeed without a bundled binary");
    assert_eq!(rendered, plugin_root);

    // Both plugin manifests + component dirs exist.
    assert!(plugin_root
        .join(".claude-plugin")
        .join("plugin.json")
        .exists());
    assert!(plugin_root
        .join(".codex-plugin")
        .join("plugin.json")
        .exists());
    assert!(plugin_root.join("commands").is_dir());
    assert!(plugin_root.join("agents").is_dir());
    assert!(plugin_root.join("skills").is_dir());

    // Guard: a public snapshot render must not bundle a host binary.
    assert!(
        !plugin_root.join("bin").exists(),
        "core-only render must not produce bin/ (no bundled binary)"
    );
}
