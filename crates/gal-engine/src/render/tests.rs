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
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("skills").join("demo")).unwrap();
    fs::write(
        dir.path().join("skills").join("demo").join("SKILL.md"),
        "# demo",
    )
    .unwrap();

    // Same render tree, same gal_version -> same stamp (digest determinism).
    let core_skills = std::collections::HashSet::new();
    let version = generate_plugin_version("1.0.0", dir.path(), dir.path(), &core_skills, b"");
    let version_again = generate_plugin_version("1.0.0", dir.path(), dir.path(), &core_skills, b"");
    assert_eq!(
        version, version_again,
        "an unchanged render tree must yield an identical stamp"
    );

    // Stamp matches the required pattern: <semver>-g<12 lowercase hex>.
    let re = regex::Regex::new(r"^\d+\.\d+\.\d+-g[0-9a-f]{12}$").unwrap();
    assert!(
        re.is_match(&version),
        "stamp does not match the required pattern: {version}"
    );

    // A single changed byte in a digested file changes the stamp.
    fs::write(
        dir.path().join("skills").join("demo").join("SKILL.md"),
        "# demX",
    )
    .unwrap();
    let version_after_change =
        generate_plugin_version("1.0.0", dir.path(), dir.path(), &core_skills, b"");
    assert_ne!(
        version, version_after_change,
        "a single-byte content change must change the stamp"
    );
    let version_after_hook_change = generate_plugin_version(
        "1.0.0",
        dir.path(),
        dir.path(),
        &core_skills,
        b"changed hook bytes",
    );
    assert_ne!(
        version_after_change, version_after_hook_change,
        "Codex hook bytes must contribute to the portable version stamp"
    );
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
fn claude_plugin_manifest_has_version() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    manifests::render_claude_plugin_manifest(dir.path(), &components, "1.0.0-gabc123abc123")
        .unwrap();
    let raw = fs::read_to_string(dir.path().join("plugin.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let version = json["version"].as_str().expect("version must be a string");
    let re = regex::Regex::new(r"^\d+\.\d+\.\d+-g[0-9a-f]{12}$").unwrap();
    assert!(
        re.is_match(version),
        "Claude manifest version must match the stamp pattern: {raw}"
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
fn root_plugin_manifest_has_agent_plugins_shape() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let components = ScannedComponents {
        skills: vec![],
        command_skills: vec![],
        agents: vec![],
    };
    manifests::render_root_plugin_manifest(dir.path(), &components, "1.0.0-gabc123abc123").unwrap();
    let raw = fs::read_to_string(dir.path().join("plugin.json")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert_eq!(
        json["$schema"],
        "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json"
    );
    assert_eq!(json["name"], "gal");
    assert_eq!(json["description"], "GAL - Golem Agents Legion");
    let version = json["version"].as_str().expect("version must be a string");
    let re = regex::Regex::new(r"^\d+\.\d+\.\d+-g[0-9a-f]{12}$").unwrap();
    assert!(
        re.is_match(version),
        "root manifest version must match the stamp pattern: {raw}"
    );
    let mut keys = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, vec!["$schema", "description", "name", "version"]);
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
fn codex_copilot_manifests_keep_dynamic_version() {
    // Regression: machine-local content (the `bin/` binary, a personal skill,
    // a personal MCP server) must never perturb the version stamp — one
    // portable source yields one digest regardless of what a given machine's
    // personal layer adds on top of it.
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    fs::create_dir_all(dir.path().join("skills").join("core-skill")).unwrap();
    fs::write(
        dir.path()
            .join("skills")
            .join("core-skill")
            .join("SKILL.md"),
        "# core",
    )
    .unwrap();
    // Pretty-printed, matching what the render actually writes for
    // `mcp.json`/`.mcp.json`. A compact fixture here would let a digest that
    // hashes raw bytes in one branch and re-serialized bytes in the other pass
    // unnoticed, because compact input survives that round trip byte-identical.
    fs::write(
        dir.path().join("mcp.json"),
        serde_json::to_string_pretty(&serde_json::json!({
            "mcpServers": { "core-srv": { "type": "http", "url": "https://core.example" } }
        }))
        .unwrap(),
    )
    .unwrap();

    let baseline_digest = manifests::compute_portable_digest(
        dir.path(),
        &std::collections::HashSet::new(),
        &std::collections::HashSet::new(),
    );

    // Add a `bin/` file — always excluded structurally, no exclude-set entry needed.
    fs::create_dir_all(dir.path().join("bin")).unwrap();
    fs::write(dir.path().join("bin").join("gal.exe"), b"binary").unwrap();

    // Add a personal skill.
    fs::create_dir_all(dir.path().join("skills").join("personal-skill")).unwrap();
    fs::write(
        dir.path()
            .join("skills")
            .join("personal-skill")
            .join("SKILL.md"),
        "# personal",
    )
    .unwrap();

    // Add a personal MCP server.
    let mut mcp_value: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(dir.path().join("mcp.json")).unwrap()).unwrap();
    mcp_value["mcpServers"]["personal-srv"] =
        serde_json::json!({ "type": "http", "url": "https://personal.example" });
    fs::write(
        dir.path().join("mcp.json"),
        serde_json::to_string_pretty(&mcp_value).unwrap(),
    )
    .unwrap();

    let mut exclude_skills = std::collections::HashSet::new();
    exclude_skills.insert("personal-skill".to_string());
    let mut exclude_mcp = std::collections::HashSet::new();
    exclude_mcp.insert("personal-srv".to_string());

    let digest_with_personal_layer =
        manifests::compute_portable_digest(dir.path(), &exclude_skills, &exclude_mcp);

    assert_eq!(
        baseline_digest, digest_with_personal_layer,
        "a bin/ file, a personal skill, and a personal MCP server must each leave the digest unchanged"
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

/// Build a rendered `.mcp.json` target fixture in the `{"mcpServers": {...}}`
/// shape that `merge_personal_mcp` now reads and writes back.
fn make_mcp_servers_json(dir: &std::path::Path, servers: &[(&str, &str)]) -> PathBuf {
    let mut srv = serde_json::Map::new();
    for (name, url) in servers {
        srv.insert(
            name.to_string(),
            serde_json::json!({ "type": "http", "url": url }),
        );
    }
    let obj = serde_json::json!({ "mcpServers": serde_json::Value::Object(srv) });
    let path = dir.join(".mcp.json");
    fs::write(&path, serde_json::to_string_pretty(&obj).unwrap()).unwrap();
    path
}

#[test]
fn render_personal_mcp_no_collision_appends_server() {
    use tempfile::TempDir;
    let core_dir = TempDir::new().unwrap();
    let local_dir = TempDir::new().unwrap();
    let target = make_mcp_servers_json(core_dir.path(), &[("core-srv", "https://core.example")]);
    let local = make_mcp_json(
        local_dir.path(),
        &[("personal-srv", "https://personal.example")],
    );

    merge_personal_mcp(&target, &local);

    let merged: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    let servers = merged["mcpServers"].as_object().unwrap();
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
    let target = make_mcp_servers_json(
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
    let servers = merged["mcpServers"].as_object().unwrap();
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
    assert!(
        source.exists(),
        "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
        source.display()
    );

    let out = TempDir::new().unwrap();
    let plugin_root = out.path().join("gal");

    let rendered = render_canonical_root_to(&source, &plugin_root, false, "0.0.0-test")
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

#[test]
fn render_canonical_root_to_stamps_all_three_manifests_identically_and_reproducibly() {
    use tempfile::TempDir;

    // Real repo source so render passes doc-sync + steward validation.
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("plugins")
        .join("gal-core");
    assert!(
        source.exists(),
        "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
        source.display()
    );

    let manifest_paths = [
        [".claude-plugin", "plugin.json"],
        [".codex-plugin", "plugin.json"],
        ["", "plugin.json"],
    ];

    let render = |label: &str| -> (PathBuf, TempDir) {
        let out = TempDir::new().unwrap();
        let plugin_root = out.path().join(label);
        render_canonical_root_to(&source, &plugin_root, false, "1.2.3")
            .expect("core-only render must succeed");
        (plugin_root, out)
    };

    let (first_root, _first_out) = render("first");

    // Every manifest from one render carries the same stamp, and the stamp
    // is prefixed by the passed semver — `render_to_temp` must compute the
    // digest once and pass the identical value to all three manifests.
    let versions: Vec<String> = manifest_paths
        .iter()
        .map(|[dir, file]| {
            let path = if dir.is_empty() {
                first_root.join(file)
            } else {
                first_root.join(dir).join(file)
            };
            let raw = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
            let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
            json["version"]
                .as_str()
                .unwrap_or_else(|| panic!("{path:?} missing version: {raw}"))
                .to_string()
        })
        .collect();
    assert!(
        versions.iter().all(|v| v == &versions[0]),
        "all three manifests from one render must carry the same stamp: {versions:?}"
    );
    assert!(
        versions[0].starts_with("1.2.3-g"),
        "stamp must be prefixed by the passed semver: {}",
        versions[0]
    );

    let codex: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(first_root.join(".codex-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
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
    for path in ["hooks/hooks.json", "hooks"] {
        let candidate = first_root.join(path);
        assert!(
            !candidate.exists(),
            "hook asset or unexpected path exists: {path}"
        );
    }
    for path in [".claude-plugin/plugin.json", "plugin.json"] {
        let raw = fs::read_to_string(first_root.join(path)).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        assert!(json.get("hooks").is_none(), "{path} must remain hook-free");
    }

    // A second render from the identical portable source yields byte-identical
    // manifest files (R1 / plan Test Cases bullet 1).
    let (second_root, _second_out) = render("second");
    for [dir, file] in manifest_paths {
        let rel = if dir.is_empty() {
            PathBuf::from(file)
        } else {
            PathBuf::from(dir).join(file)
        };
        let a = fs::read(first_root.join(&rel)).unwrap();
        let b = fs::read(second_root.join(&rel)).unwrap();
        assert_eq!(
            a, b,
            "{rel:?} must be byte-identical across two renders of identical source"
        );
    }
}

#[test]
fn render_copilot_agents_excludes_orchestrated_only_and_rewrites_tools() {
    use tempfile::TempDir;

    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .join("plugins")
        .join("gal-core");
    assert!(
        source.exists(),
        "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
        source.display()
    );

    let out = TempDir::new().unwrap();
    let plugin_root = out.path().join("gal");
    render_canonical_root_to(&source, &plugin_root, false, "0.0.0-test").unwrap();

    let copilot_agents = plugin_root.join("com.github.copilot").join("agents");
    let entries: Vec<_> = fs::read_dir(&copilot_agents)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    let expected: Vec<_> = scan_source_components(&source)
        .unwrap()
        .agents
        .into_iter()
        .filter(|agent| !projection::ORCHESTRATED_ONLY_GOLEM_NAMES.contains(&agent.name.as_str()))
        .map(|agent| format!("{}.agent.md", agent.name))
        .collect();

    let mut entries = entries;
    let mut expected = expected;
    entries.sort();
    expected.sort();
    assert_eq!(entries, expected);
    for name in &expected {
        let agent_name = name.trim_end_matches(".agent.md");
        let source_agent = source.join("agents").join(name);
        let expected_content = projection::tool_map::rewrite_agent_tools_for_claude(
            &fs::read_to_string(source_agent).unwrap(),
        );
        assert_eq!(
            fs::read_to_string(copilot_agents.join(name)).unwrap(),
            expected_content,
            "Copilot agent {agent_name} must use the shared tool transform"
        );
    }
    for name in projection::ORCHESTRATED_ONLY_GOLEM_NAMES {
        assert!(!copilot_agents.join(format!("{name}.agent.md")).exists());
    }
}

// ── render_standard_mcp_manifest ────────────────────────────────────────────

#[test]
fn standard_mcp_manifest_has_exactly_schema_and_mcp_servers_keys() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("mcp.json");

    let mut servers = serde_json::Map::new();
    servers.insert(
        "core-srv".to_string(),
        serde_json::json!({ "type": "http", "url": "https://core.example" }),
    );

    manifests::render_standard_mcp_manifest(&servers, &target).unwrap();

    let raw = fs::read_to_string(&target).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mut keys = parsed
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(keys, vec!["$schema", "mcpServers"], "raw: {raw}");
    assert_eq!(
        parsed["$schema"],
        "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json"
    );
}

#[test]
fn standard_mcp_manifest_http_type_becomes_streamable_http_stdio_sse_pass_through() {
    use tempfile::TempDir;
    let dir = TempDir::new().unwrap();
    let target = dir.path().join("mcp.json");

    let mut servers = serde_json::Map::new();
    servers.insert(
        "http-srv".to_string(),
        serde_json::json!({ "type": "http", "url": "https://core.example" }),
    );
    servers.insert(
        "stdio-srv".to_string(),
        serde_json::json!({ "type": "stdio", "command": "some-cmd" }),
    );
    servers.insert(
        "sse-srv".to_string(),
        serde_json::json!({ "type": "sse", "url": "https://sse.example" }),
    );

    manifests::render_standard_mcp_manifest(&servers, &target).unwrap();

    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&target).unwrap()).unwrap();
    let mapped = parsed["mcpServers"].as_object().unwrap();
    assert_eq!(mapped["http-srv"]["type"], "streamable-http");
    assert_eq!(mapped["http-srv"]["url"], "https://core.example");
    assert_eq!(mapped["stdio-srv"]["type"], "stdio");
    assert_eq!(mapped["stdio-srv"]["command"], "some-cmd");
    assert_eq!(mapped["sse-srv"]["type"], "sse");
    assert_eq!(mapped["sse-srv"]["url"], "https://sse.example");
}

/// Minimal source root satisfying `render_canonical_root_to`'s required-component
/// gate (doc-sync skill + golem-steward agent) plus an optional `mcp.json`.
fn make_minimal_source(root: &Path, mcp_servers: Option<&serde_json::Value>) {
    fs::create_dir_all(root.join("codex")).unwrap();
    fs::write(
        root.join("codex").join("hooks.json"),
        include_str!("../../../../plugins/gal-core/codex/hooks.json"),
    )
    .unwrap();
    let skills_dir = root.join("skills").join("doc-sync");
    fs::create_dir_all(&skills_dir).unwrap();
    fs::write(
        skills_dir.join("SKILL.md"),
        "---\nname: doc-sync\n---\nbody\n",
    )
    .unwrap();

    let agents_dir = root.join("agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::write(
        agents_dir.join("golem-steward.agent.md"),
        "---\nname: golem-steward\n---\nbody\n",
    )
    .unwrap();

    fs::create_dir_all(root.join("commands")).unwrap();

    if let Some(servers) = mcp_servers {
        fs::write(
            root.join("mcp.json"),
            serde_json::to_string_pretty(&serde_json::json!({ "servers": servers })).unwrap(),
        )
        .unwrap();
    }
}

#[test]
fn standard_mcp_json_absent_when_source_mcp_json_is_absent() {
    use tempfile::TempDir;
    let source = TempDir::new().unwrap();
    make_minimal_source(source.path(), None);

    let out = TempDir::new().unwrap();
    let plugin_root = out.path().join("gal");
    render_canonical_root_to(source.path(), &plugin_root, false, "0.0.0-test").unwrap();

    assert!(
        !plugin_root.join(".mcp.json").exists(),
        "no source mcp.json must yield no .mcp.json"
    );
    assert!(
        !plugin_root.join("mcp.json").exists(),
        "no source mcp.json must yield no standard mcp.json"
    );
}

#[test]
fn codex_hooks_source_failures_do_not_create_a_partial_render() {
    use tempfile::TempDir;

    for source_state in ["missing", "malformed"] {
        let source = TempDir::new().unwrap();
        make_minimal_source(source.path(), None);
        let hooks_path = source.path().join("codex").join("hooks.json");
        if source_state == "missing" {
            fs::remove_file(&hooks_path).unwrap();
        } else {
            fs::write(&hooks_path, "{ invalid json").unwrap();
        }

        let out = TempDir::new().unwrap();
        let plugin_root = out.path().join("gal");
        assert!(
            render_canonical_root_to(source.path(), &plugin_root, false, "0.0.0-test").is_err(),
            "{source_state} Codex hooks source must reject rendering"
        );
        assert!(
            !plugin_root.exists(),
            "{source_state} Codex hooks source must not produce a partial render"
        );
    }
}

#[test]
fn standard_mcp_json_snapshot_render_carries_core_servers_only() {
    use tempfile::TempDir;
    let source = TempDir::new().unwrap();
    make_minimal_source(
        source.path(),
        Some(&serde_json::json!({
            "core-srv": { "type": "http", "url": "https://core.example" }
        })),
    );

    let out = TempDir::new().unwrap();
    let plugin_root = out.path().join("gal");
    // include_machine_local = false: personal layer never consulted, even if
    // present on the machine running this test.
    render_canonical_root_to(source.path(), &plugin_root, false, "0.0.0-test").unwrap();

    let parsed: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(plugin_root.join("mcp.json")).unwrap()).unwrap();
    let mapped = parsed["mcpServers"].as_object().unwrap();
    assert_eq!(
        mapped.len(),
        1,
        "snapshot render must carry core servers only"
    );
    assert_eq!(mapped["core-srv"]["type"], "streamable-http");
}

#[test]
fn version_stamp_reflects_source_mcp_json_content() {
    // Regression for an ordering defect found during audit: the stamp used to
    // be computed before `mcp.json` was written, so the digest walk never saw
    // it and two sources differing only in their MCP config stamped
    // identically. The stamp is now computed after every portable file is
    // written, so core MCP content must reach the digest.
    use tempfile::TempDir;

    let render_stamp = |servers: &serde_json::Value| -> String {
        let source = TempDir::new().unwrap();
        make_minimal_source(source.path(), Some(servers));
        let out = TempDir::new().unwrap();
        let plugin_root = out.path().join("gal");
        render_canonical_root_to(source.path(), &plugin_root, false, "1.2.3").unwrap();
        let raw = fs::read_to_string(plugin_root.join("plugin.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        json["version"].as_str().unwrap().to_string()
    };

    let a = render_stamp(&serde_json::json!({
        "core-srv": { "type": "http", "url": "https://core.example/a" }
    }));
    let b = render_stamp(&serde_json::json!({
        "core-srv": { "type": "http", "url": "https://core.example/b" }
    }));

    assert_ne!(
        a, b,
        "a change confined to the source mcp.json must change the version stamp"
    );
}

#[test]
fn version_stamp_reflects_codex_hooks_json_content() {
    use tempfile::TempDir;

    let render_stamp = |hooks_content: &str| -> String {
        let source = TempDir::new().unwrap();
        make_minimal_source(source.path(), None);
        fs::write(
            source.path().join("codex").join("hooks.json"),
            hooks_content,
        )
        .unwrap();
        let out = TempDir::new().unwrap();
        let plugin_root = out.path().join("gal");
        render_canonical_root_to(source.path(), &plugin_root, false, "1.2.3").unwrap();
        let raw = fs::read_to_string(plugin_root.join("plugin.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();
        json["version"].as_str().unwrap().to_string()
    };

    let a = render_stamp(
        r#"{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"gal hook user-prompt-submit"}]}],"PreToolUse":[{"hooks":[{"type":"command","command":"gal hook pre-tool-use"}]}],"Stop":[{"hooks":[{"type":"command","command":"gal hook stop"}]}]}}"#,
    );
    let b = render_stamp(
        r#"{"hooks":{"UserPromptSubmit":[{"hooks":[{"type":"command","command":"gal hook user-prompt-submit-v2"}]}],"PreToolUse":[{"hooks":[{"type":"command","command":"gal hook pre-tool-use"}]}],"Stop":[{"hooks":[{"type":"command","command":"gal hook stop"}]}]}}"#,
    );

    assert_ne!(
        a, b,
        "a change confined to codex/hooks.json must change the version stamp"
    );
}

#[test]
fn standard_mcp_json_personal_merge_core_wins() {
    use tempfile::TempDir;
    let source = TempDir::new().unwrap();
    make_minimal_source(
        source.path(),
        Some(&serde_json::json!({
            "shared-srv": { "type": "http", "url": "https://core.example/core" }
        })),
    );

    with_mocked_home(|home| {
        let local_mcp = home.join(".gal").join("local").join("mcp.json");
        fs::create_dir_all(local_mcp.parent().unwrap()).unwrap();
        fs::write(
            &local_mcp,
            serde_json::to_string_pretty(&serde_json::json!({
                "servers": {
                    "shared-srv": { "type": "http", "url": "https://personal.example/personal" },
                    "extra-srv": { "type": "stdio", "command": "personal-cmd" },
                }
            }))
            .unwrap(),
        )
        .unwrap();

        let out = TempDir::new().unwrap();
        let plugin_root = out.path().join("gal");
        render_canonical_root_to(source.path(), &plugin_root, true, "0.0.0-test").unwrap();

        let parsed: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(plugin_root.join("mcp.json")).unwrap())
                .unwrap();
        let mapped = parsed["mcpServers"].as_object().unwrap();
        assert_eq!(
            mapped["shared-srv"]["url"], "https://core.example/core",
            "core-wins: colliding personal server must be skipped"
        );
        assert_eq!(mapped["shared-srv"]["type"], "streamable-http");
        assert_eq!(mapped["extra-srv"]["command"], "personal-cmd");
        assert_eq!(mapped.len(), 2);
    });
}

// ── materialize_embedded_source: concurrency-safe staging + atomic swap ────

fn with_mocked_home<R>(f: impl FnOnce(&Path) -> R) -> R {
    let _guard = crate::test_support::HOME_ENV_GUARD
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::TempDir::new().unwrap();
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    let prev = std::env::var_os(key);
    std::env::set_var(key, tmp.path());
    let result = f(tmp.path());
    match prev {
        Some(v) => std::env::set_var(key, v),
        None => std::env::remove_var(key),
    }
    result
}

#[test]
fn materialize_embedded_source_is_idempotent_and_matches_embedded_bytes() {
    if !crate::embedded::is_populated() {
        return; // non-git build embeds an empty tree — nothing to materialize
    }
    with_mocked_home(|_home| {
        let first = materialize_embedded_source()
            .expect("first materialize")
            .expect("embedded payload should be populated");
        assert!(embedded_matches_disk(&first));
        // Second call hits the embedded_matches_disk fast path — no swap needed.
        let second = materialize_embedded_source()
            .expect("second materialize")
            .expect("embedded payload should be populated");
        assert_eq!(first, second);
        assert!(embedded_matches_disk(&second));
    });
}

#[test]
fn materialize_embedded_source_never_leaves_a_shared_tmp_name() {
    // The old shared `embedded-src.tmp` name must never appear — every
    // staging directory is process-unique now.
    if !crate::embedded::is_populated() {
        return;
    }
    with_mocked_home(|home| {
        materialize_embedded_source()
            .expect("materialize")
            .expect("embedded payload should be populated");
        assert!(
            !home.join(".gal").join("embedded-src.tmp").exists(),
            "the old shared staging name must never be used"
        );
    });
}

#[test]
fn concurrent_materialize_calls_leave_one_byte_complete_canonical_tree() {
    // Real concurrency: N threads race to materialize onto the same mocked
    // HOME. Every thread must succeed (own unique staging + atomic_swap, or
    // a losing racer's byte-for-byte recheck accepting the winner's result),
    // and the canonical target must end up byte-complete — never a missing
    // or partial tree, never a direct delete of a concurrent winner's output.
    if !crate::embedded::is_populated() {
        return;
    }
    with_mocked_home(|home| {
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(materialize_embedded_source))
            .collect();
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(
            results.iter().all(|r| matches!(r, Ok(Some(_)))),
            "every concurrent caller must succeed (own staging + recheck-on-conflict): {results:?}"
        );
        let target = home.join(".gal").join("embedded-src");
        assert!(
            embedded_matches_disk(&target),
            "canonical target must be byte-complete after concurrent materialization"
        );
    });
}

#[test]
fn materialize_embedded_source_reports_unusable_home() {
    if !crate::embedded::is_populated() {
        return;
    }
    let _guard = crate::test_support::HOME_ENV_GUARD
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    let tmp = tempfile::TempDir::new().unwrap();
    let unusable_home = tmp.path().join("home-file");
    fs::write(&unusable_home, b"not a directory").unwrap();
    #[cfg(windows)]
    let key = "USERPROFILE";
    #[cfg(not(windows))]
    let key = "HOME";
    let previous = std::env::var_os(key);
    std::env::set_var(key, &unusable_home);

    let result = materialize_embedded_source();

    match previous {
        Some(value) => std::env::set_var(key, value),
        None => std::env::remove_var(key),
    }
    let error = result.expect_err("an unusable home must preserve the I/O failure");
    let message = error.to_string();
    assert!(message.contains("create embedded source staging directory"));
    assert!(message.contains("embedded-src.staging-"));
    assert!(message.contains(&unusable_home.display().to_string()));
}
