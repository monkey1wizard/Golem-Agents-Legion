use super::*;
use gal_foundation::health::HealthCheck;
use gal_foundation::platform::{create_dir_link, is_symlink_or_junction};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

#[cfg(windows)]
#[test]
fn remove_link_removes_directory_junction_not_target() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("target");
    fs::create_dir(&target).unwrap();
    fs::write(target.join("keep.txt"), "keep").unwrap();
    let link = tmp.path().join("junction");
    let status = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &link.to_string_lossy(),
            &target.to_string_lossy(),
        ])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "mklink /J failed to create the test junction"
    );
    assert!(
        is_symlink_or_junction(&link),
        "test setup: link should be a junction"
    );
    // Regression: before the fix `remove_link` gated on `is_dir()` (false for a
    // Windows junction) and fell to `fs::remove_file` → ERROR_ACCESS_DENIED (os error 5).
    remove_link(&link)
        .expect("remove_link must remove a directory junction (os error 5 regression)");
    assert!(!link.exists(), "junction must be removed");
    assert!(
        target.join("keep.txt").exists(),
        "removing a junction must NOT touch the target directory"
    );
}

// Prune path: a GAL-owned real-file residue (retired golem-agent copy without
// a GAL header, or a header-stamped file) that is no longer active gets
// removed even when absent from the lockfile (positive content ID = a2 escape
// hatch); a genuine non-GAL user file is preserved.
#[test]
fn prune_stale_links_removes_gal_owned_real_files_keeps_user_file() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("agents");
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("golem-analyst.agent.md"),
        "---\nname: golem-analyst\n---\nbody\n",
    )
    .unwrap();
    fs::write(
        root.join("golem-dockeeper.agent.md"),
        "---\nname: golem-dockeeper\n---\nbody\n",
    )
    .unwrap();
    fs::write(
        root.join("stale.toml"),
        format!("{GAL_MANAGED_FILE_HEADER}\nx = 1\n"),
    )
    .unwrap();
    fs::write(root.join("my-notes.md"), "personal notes\n").unwrap();

    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&tmp.path().join("nolock.json"), &mut report);
    prune_stale_links(
        &root,
        ["golem-analyst.agent.md"],
        false,
        &mut report,
        &mut registry,
        &[],
    )
    .unwrap();

    assert!(
        root.join("golem-analyst.agent.md").exists(),
        "active agent must be kept"
    );
    assert!(
        !root.join("golem-dockeeper.agent.md").exists(),
        "retired golem-agent real file must be pruned"
    );
    assert!(
        !root.join("stale.toml").exists(),
        "GAL-header-stamped stale real file must be pruned"
    );
    assert!(
        root.join("my-notes.md").exists(),
        "non-GAL user file must be preserved"
    );
}

// Single source: projection's absent-state fallback for selected runtimes is
// gal_foundation::runtime::VALID_RUNTIMES — the same set the install side now defaults
// to, so the two paths cannot disagree when install-state.json is missing.
#[test]
fn load_selected_runtimes_absent_state_falls_back_to_valid_runtimes() {
    let tmp = TempDir::new().unwrap();
    let got = load_selected_runtimes(tmp.path()).unwrap();
    let mut expected: Vec<String> = gal_foundation::runtime::VALID_RUNTIMES
        .iter()
        .map(|r| (*r).to_string())
        .collect();
    expected.sort();
    expected.dedup();
    assert_eq!(got, expected);
}

fn fixture_roots() -> (TempDir, PathBuf, PathBuf, PathBuf) {
    let temp = TempDir::new().unwrap();
    let repo_root = temp.path().join("repo");
    let source_root = repo_root.join("plugins").join("gal-core");
    let home = temp.path().join("home");
    fs::create_dir_all(source_root.join("skills").join("sample-skill")).unwrap();
    fs::create_dir_all(source_root.join("agents")).unwrap();
    fs::create_dir_all(source_root.join("commands")).unwrap();
    (temp, repo_root, source_root, home)
}

fn write_skill_fixture(source_root: &Path, name: &str) {
    let skill_root = source_root.join("skills").join(name);
    fs::create_dir_all(&skill_root).unwrap();
    fs::write(skill_root.join("SKILL.md"), format!("# {name}\n")).unwrap();
}

/// Probe whether the current process may create file symlinks.
///
/// On Windows a file symlink needs Developer Mode /
/// `SeCreateSymbolicLinkPrivilege`. Tests that assert real file-link creation
/// skip gracefully without it instead of reporting a false failure on
/// unprivileged dev machines and CI. Probes the OS symlink primitive directly
/// (no GAL helper needed).
fn file_symlink_supported() -> bool {
    let probe = match TempDir::new() {
        Ok(p) => p,
        Err(_) => return false,
    };
    let target = probe.path().join("probe-target");
    let link = probe.path().join("probe-link");
    if fs::write(&target, "probe").is_err() {
        return false;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(&target, &link).is_ok()
    }
    #[cfg(not(windows))]
    {
        std::os::unix::fs::symlink(&target, &link).is_ok()
    }
}

fn write_command_fixture(source_root: &Path, name: &str) {
    let command_root = source_root.join("commands").join(name);
    fs::create_dir_all(&command_root).unwrap();
    fs::write(
        command_root.join("SKILL.template.md"),
        "---\ndescription: sample command\n---\nUse {{GAL_ROOT}}\n",
    )
    .unwrap();
    fs::write(command_root.join("SKILL.local.md"), "Local note\n").unwrap();
}

#[test]
fn update_skills_projects_links_and_agents() {
    if !file_symlink_supported() {
        eprintln!(
            "skipping update_skills_projects_links_and_agents: file symlink creation \
                 not permitted (needs Windows Developer Mode / SeCreateSymbolicLinkPrivilege)"
        );
        return;
    }
    let (_temp, repo_root, source_root, home) = fixture_roots();
    fs::write(
        source_root
            .join("skills")
            .join("sample-skill")
            .join("SKILL.md"),
        "# Skill",
    )
    .unwrap();
    fs::write(
        source_root.join("agents").join("helper.agent.md"),
        "---\ndescription: helper\ncolor: green\ntools: [read, edit, execute]\n---\nBody\n",
    )
    .unwrap();

    let report = run_update_skills(&SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec![
            "copilot".into(),
            "gemini".into(),
            "antigravity".into(),
            "opencode".into(),
        ],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(!report.created_links.is_empty());
    assert!(home
        .join(".agents")
        .join("skills")
        .join("sample-skill")
        .exists());
    assert!(home
        .join(".copilot")
        .join("agents")
        .join("helper.agent.md")
        .exists());
    assert!(home.join(".copilot").join("gal").exists());
    assert!(!home.join(".gemini").join("gal").exists());
    assert!(home
        .join(".gemini")
        .join("antigravity-cli")
        .join("gal")
        .exists());
    let rendered = fs::read_to_string(
        home.join(".config")
            .join("opencode")
            .join("agents")
            .join("helper.md"),
    )
    .unwrap();
    assert!(rendered.starts_with(GAL_MANAGED_FILE_HEADER));
    assert!(rendered.contains("color: green"));
    assert!(rendered.contains("  read: allow"));
    assert!(rendered.contains("  list: allow"));
    assert!(rendered.contains("  edit: allow"));
    assert!(rendered.contains("  bash: allow"));
}

#[test]
fn update_skills_skips_shared_projection_without_codex_or_opencode() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    fs::write(
        source_root
            .join("skills")
            .join("sample-skill")
            .join("SKILL.md"),
        "# Skill",
    )
    .unwrap();

    run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["copilot".into(), "gemini".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(!home
        .join(".agents")
        .join("skills")
        .join("sample-skill")
        .exists());
}

fn write_agent_fixture(source_root: &Path, name: &str, color: &str) {
    fs::write(
        source_root.join("agents").join(format!("{name}.agent.md")),
        format!(
            "---\ndescription: {name}\ncolor: {color}\ntools: [read, edit, execute]\n---\nBody\n"
        ),
    )
    .unwrap();
}

// Write-side half of the foreign-survival test: the per-file write loop that
// replaced the directory swap must never touch a file it did not itself
// write, and every GAL-written agent must carry the managed-file header.
#[test]
fn update_skills_opencode_write_preserves_foreign_file_and_stamps_gal_header() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_agent_fixture(&source_root, "helper", "green");
    let opencode_agents = home.join(".config").join("opencode").join("agents");
    fs::create_dir_all(&opencode_agents).unwrap();
    let foreign_content = "# Generated by SomeOtherTool. Do not edit manually.\nforeign body\n";
    fs::write(opencode_agents.join("foreign.md"), foreign_content).unwrap();

    run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert_eq!(
        fs::read_to_string(opencode_agents.join("foreign.md")).unwrap(),
        foreign_content,
        "foreign file beside the GAL agents must be byte-for-byte unchanged"
    );
    let rendered = fs::read_to_string(opencode_agents.join("helper.md")).unwrap();
    assert!(rendered.starts_with(GAL_MANAGED_FILE_HEADER));
}

// Write-side half of the no-agents-to-project test: the write loop must not
// disturb an existing foreign file or directory, and must not create a
// directory where none existed (the zero-agent case adds no `ensure_dir`
// call by design).
#[test]
fn update_skills_opencode_no_agents_preserves_foreign_file_and_directory_state() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    let opencode_agents = home.join(".config").join("opencode").join("agents");
    fs::create_dir_all(&opencode_agents).unwrap();
    let foreign_content = "# Generated by SomeOtherTool. Do not edit manually.\nforeign body\n";
    fs::write(opencode_agents.join("foreign.md"), foreign_content).unwrap();

    run_update_skills(&SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert_eq!(
        fs::read_to_string(opencode_agents.join("foreign.md")).unwrap(),
        foreign_content,
        "no-agents run must leave the foreign file untouched"
    );
    assert!(
        opencode_agents.exists(),
        "directory must not be emptied away"
    );

    let (_temp2, repo_root2, source_root2, home2) = fixture_roots();
    let opencode_agents2 = home2.join(".config").join("opencode").join("agents");
    assert!(
        !opencode_agents2.exists(),
        "test setup: directory must start absent"
    );

    run_update_skills(&SkillUpdateOptions {
        repo_root: repo_root2,
        sources: vec![ProjectionSource {
            root: source_root2,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home2,
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(
        !opencode_agents2.exists(),
        "zero-agent case must create no directory"
    );
}

// A dry_run pass over an agent set not yet written to the directory must not
// touch disk, but report.written_files must still list the paths a real run
// would write.
#[test]
fn update_skills_opencode_dry_run_writes_nothing_but_lists_would_write_files() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_agent_fixture(&source_root, "helper", "green");
    let opencode_agents = home.join(".config").join("opencode").join("agents");
    assert!(
        !opencode_agents.exists(),
        "test setup: directory must start absent"
    );

    let report = run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: true,
        replace: true,
    })
    .unwrap();

    assert!(
        !opencode_agents.join("helper.md").exists(),
        "dry_run must not touch disk"
    );
    assert!(report
        .written_files
        .contains(&opencode_agents.join("helper.md")));
}

// report.written_files carries one row per written agent, never a row for
// the OpenCode agents directory itself.
#[test]
fn update_skills_opencode_written_files_lists_per_agent_not_directory() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_agent_fixture(&source_root, "helper", "green");
    write_agent_fixture(&source_root, "second", "blue");
    let opencode_agents = home.join(".config").join("opencode").join("agents");

    let report = run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(report
        .written_files
        .contains(&opencode_agents.join("helper.md")));
    assert!(report
        .written_files
        .contains(&opencode_agents.join("second.md")));
    assert!(
        !report.written_files.contains(&opencode_agents),
        "written_files must not contain a row for the directory itself"
    );
}

// Prune-side half of the foreign-survival test: a GAL-header agent written
// by a since-uninstalled source and no longer projected is removed through
// the ownership ladder, while a foreign file beside it and a still-projected
// GAL agent both survive byte for byte.
#[test]
fn update_skills_opencode_prunes_retired_gal_agent_keeps_foreign_and_active() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_agent_fixture(&source_root, "helper", "green");
    write_agent_fixture(&source_root, "second", "blue");
    let opencode_agents = home.join(".config").join("opencode").join("agents");
    let foreign_content = "# Generated by SomeOtherTool. Do not edit manually.\nforeign body\n";

    let first_opts = SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "removed-source".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&first_opts).unwrap();
    assert!(
        opencode_agents.join("second.md").exists(),
        "test setup: second.md must exist before retirement"
    );
    fs::write(opencode_agents.join("foreign.md"), foreign_content).unwrap();
    fs::remove_file(source_root.join("agents").join("second.agent.md")).unwrap();

    let active_opts = SkillUpdateOptions {
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "active-source".into(),
            persistent: true,
        }],
        ..first_opts
    };
    let report = run_update_skills(&active_opts).unwrap();

    assert!(
        !opencode_agents.join("second.md").exists(),
        "retired GAL agent must be pruned"
    );
    assert!(
        report
            .removed_paths
            .contains(&opencode_agents.join("second.md")),
        "prune must be reported"
    );
    assert!(
        opencode_agents.join("helper.md").exists(),
        "still-projected agent must survive"
    );
    assert_eq!(
        fs::read_to_string(opencode_agents.join("foreign.md")).unwrap(),
        foreign_content,
        "foreign file beside the retired agent must be byte-for-byte unchanged"
    );
}

// Prune-side: a file whose owning source is still present in installed_ids
// is retained on the installed-source branch before the ownership ladder is
// reached, even when it is no longer projected this run.
#[test]
fn update_skills_opencode_retains_file_whose_source_stays_installed() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_agent_fixture(&source_root, "helper", "green");
    write_agent_fixture(&source_root, "second", "blue");
    let opencode_agents = home.join(".config").join("opencode").join("agents");

    let opts = SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "installed-source".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&opts).unwrap();
    fs::remove_file(source_root.join("agents").join("second.agent.md")).unwrap();

    let report = run_update_skills(&opts).unwrap();

    assert!(
        opencode_agents.join("second.md").exists(),
        "installed-owner branch must retain the file even though it is no longer projected"
    );
    assert!(!report
        .removed_paths
        .contains(&opencode_agents.join("second.md")));
}

// OpenCode deselected: the directory, including agents GAL wrote on an
// earlier run, is left completely untouched. Existing early-return
// behavior (`lib.rs:590-593`), not a defect.
#[test]
fn update_skills_opencode_deselected_leaves_directory_untouched() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_agent_fixture(&source_root, "helper", "green");
    let opencode_agents = home.join(".config").join("opencode").join("agents");

    let first_opts = SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&first_opts).unwrap();
    let before = fs::read_to_string(opencode_agents.join("helper.md")).unwrap();

    let deselected_opts = SkillUpdateOptions {
        selected_runtimes: vec!["copilot".into()],
        ..first_opts
    };
    run_update_skills(&deselected_opts).unwrap();

    assert_eq!(
        fs::read_to_string(opencode_agents.join("helper.md")).unwrap(),
        before,
        "deselected opencode run must not touch the directory"
    );
}

// Idempotent second real run: rerunning the write-preserves-foreign-file
// fixture with unchanged content leaves every file byte-for-byte unchanged
// and prunes nothing under the OpenCode agents directory. Pins the
// unconditional keep-set and mark_with_source collection against the
// byte-identical-content trap: gating either on write_text's return would
// empty the keep set on this second pass and delete the just-written agents.
#[test]
fn update_skills_opencode_second_real_run_is_idempotent() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_agent_fixture(&source_root, "helper", "green");
    let opencode_agents = home.join(".config").join("opencode").join("agents");
    fs::create_dir_all(&opencode_agents).unwrap();
    let foreign_content = "# Generated by SomeOtherTool. Do not edit manually.\nforeign body\n";
    fs::write(opencode_agents.join("foreign.md"), foreign_content).unwrap();

    let opts = SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&opts).unwrap();
    let before = fs::read_to_string(opencode_agents.join("helper.md")).unwrap();

    let second_report = run_update_skills(&opts).unwrap();

    assert_eq!(
        fs::read_to_string(opencode_agents.join("helper.md")).unwrap(),
        before,
        "second real run must leave the GAL agent byte-for-byte unchanged"
    );
    assert_eq!(
        fs::read_to_string(opencode_agents.join("foreign.md")).unwrap(),
        foreign_content,
        "foreign file must survive a second real run unchanged"
    );
    assert!(
        !second_report
            .removed_paths
            .iter()
            .any(|p| p.starts_with(&opencode_agents)),
        "second real run must remove nothing under the OpenCode agents directory"
    );
}

#[test]
fn healthcheck_warns_when_projection_missing() {
    let temp = TempDir::new().unwrap();
    let check = SkillsProjectionHealthCheck::with_path(temp.path().join("missing"));
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].severity,
        gal_foundation::health::Severity::Warning
    );
}

#[test]
fn healthcheck_accepts_existing_directory() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("skills");
    fs::create_dir_all(&path).unwrap();
    let check = SkillsProjectionHealthCheck::with_path(path);
    assert!(check.check().is_empty());
}

#[test]
fn render_opencode_agent_matches_legacy_shape() {
    let rendered = render_opencode_agent(
        "helper",
        &Frontmatter {
            name: None,
            description: Some("helper line 1\nhelper line 2".into()),
            color: Some("green".into()),
            tools: Vec::new(),
        },
        "Body",
    );

    assert_eq!(
            rendered,
            "# Generated by GAL Setup-Machine. Do not edit manually.\n---\ndescription: |\n  helper line 1\n  helper line 2\nmode: subagent\ncolor: green\npermission:\n  read: allow\n  list: allow\n---\n\nBody\n"
        );
}

#[test]
fn render_opencode_agent_expands_search_permissions() {
    let rendered = render_opencode_agent(
        "helper",
        &Frontmatter {
            name: None,
            description: Some("helper".into()),
            color: None,
            tools: vec!["search".into()],
        },
        "Body",
    );

    assert!(rendered.contains("  read: allow"));
    assert!(rendered.contains("  list: allow"));
    assert!(rendered.contains("  grep: allow"));
    assert!(rendered.contains("  glob: allow"));
}

#[test]
fn render_opencode_command_git_commit_msg_uses_context_not_print() {
    let rendered = render_opencode_command(
        "git-commit-msg",
        &Frontmatter {
            name: None,
            description: Some("Generate a commit message".into()),
            color: None,
            tools: Vec::new(),
        },
        "unused for this special-cased command",
    );

    assert!(rendered.contains("gal commit-msg --context"));
    assert!(!rendered.contains("gal commit-msg --print"));
}

#[test]
fn update_commands_bakes_and_projects_outputs() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "sample-command");

    let report = run_update_commands(&CommandUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into(), "opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    let baked_path = home
        .join(".gal")
        .join("plugins")
        .join("gal")
        .join("commands")
        .join("sample-command")
        .join("SKILL.md");
    let baked = fs::read_to_string(&baked_path).unwrap();
    assert!(baked.contains(&repo_root.display().to_string()));
    assert!(baked.contains("GAL LOCAL OVERRIDE START"));
    assert!(!source_root
        .join("commands")
        .join("sample-command")
        .join("SKILL.md")
        .exists());

    // Codex command skill now lives at the shared ~/.agents/skills (official
    // path), not the legacy ~/.codex/skills link.
    let codex_skill = home
        .join(".agents")
        .join("skills")
        .join("sample-command")
        .join("SKILL.md");
    assert!(
        codex_skill.exists(),
        "codex command projected as a shared skill"
    );
    assert!(
        !home
            .join(".codex")
            .join("skills")
            .join("sample-command")
            .exists(),
        "legacy ~/.codex/skills command target removed"
    );

    assert!(
        !home.join(".gemini").join("commands").exists(),
        "retired Gemini command directory must not be created"
    );

    let opencode = fs::read_to_string(
        home.join(".config")
            .join("opencode")
            .join("commands")
            .join("sample-command.md"),
    )
    .unwrap();
    assert!(opencode.starts_with(GAL_MANAGED_FILE_HEADER));
    assert!(opencode.contains("User command arguments, if any: $ARGUMENTS"));
    assert!(!report.written_files.is_empty());
}

#[test]
fn update_commands_projects_copilot_command_skill() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "gal-status");

    run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["copilot".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    let skill = home
        .join(".copilot")
        .join("skills")
        .join("gal-status")
        .join("SKILL.md");
    assert!(
        skill.exists(),
        "copilot command must be projected as a skill (~/.copilot/skills/<name>/SKILL.md)"
    );
    let body = fs::read_to_string(&skill).unwrap();
    assert!(
        body.contains("sample command"),
        "projected skill carries the baked command body"
    );
}

#[test]
fn update_commands_cleans_command_skill_when_runtime_deselected() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "gal-status");

    run_update_commands(&CommandUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();
    let shared = home.join(".agents").join("skills").join("gal-status");
    assert!(shared.exists(), "codex command-skill written when selected");

    // Deselect codex: the GAL-written command-skill must be cleaned so it does
    // not linger in the shared dir and double-load for opencode/copilot.
    run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["gemini".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();
    assert!(
        !shared.exists(),
        "command-skill removed when its runtime is deselected"
    );
}

#[test]
fn update_commands_projects_antigravity_command_skill() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "gal-status");

    run_update_commands(&CommandUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["antigravity".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    let skill = home
        .join(".gemini")
        .join("antigravity-cli")
        .join("skills")
        .join("gal-status")
        .join("SKILL.md");
    assert!(
        skill.exists(),
        "antigravity command must be projected as a skill (~/.gemini/antigravity-cli/skills/<name>/SKILL.md)"
    );
    assert!(
        !home.join(".gemini").join("commands").exists(),
        "retired Gemini command directory must not be created"
    );

    // Deselect antigravity: the GAL-written command-skill must be cleaned.
    run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["copilot".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();
    assert!(
        !skill.exists(),
        "antigravity command-skill removed when its runtime is deselected"
    );
}

#[test]
fn update_commands_opencode_only_gets_no_shared_command_skill() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "gal-status");

    run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    // Double-load guard: opencode keeps its native command; the shared
    // ~/.agents/skills must NOT also carry a command-skill (that is gated on
    // codex being selected), so the same command does not appear twice.
    assert!(
        home.join(".config")
            .join("opencode")
            .join("commands")
            .join("gal-status.md")
            .exists(),
        "opencode keeps its native command projection"
    );
    assert!(
        !home
            .join(".agents")
            .join("skills")
            .join("gal-status")
            .exists(),
        "no shared command-skill when codex is not selected (no double-load)"
    );
}

#[test]
fn update_commands_removes_managed_outputs_for_unselected_runtimes() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "sample-command");

    let gemini_target = home
        .join(".gemini")
        .join("commands")
        .join("sample-command.toml");
    let opencode_target = home
        .join(".config")
        .join("opencode")
        .join("commands")
        .join("sample-command.md");
    let claude_target = home
        .join(".claude")
        .join("commands")
        .join("sample-command.md");
    fs::create_dir_all(gemini_target.parent().unwrap()).unwrap();
    fs::create_dir_all(opencode_target.parent().unwrap()).unwrap();
    fs::create_dir_all(claude_target.parent().unwrap()).unwrap();
    fs::write(&gemini_target, format!("{GAL_MANAGED_FILE_HEADER}\nold\n")).unwrap();
    fs::write(
        &opencode_target,
        format!("{GAL_MANAGED_FILE_HEADER}\nold\n"),
    )
    .unwrap();
    fs::write(&claude_target, format!("{GAL_MANAGED_FILE_HEADER}\nold\n")).unwrap();

    run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["copilot".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(!gemini_target.exists());
    assert!(!opencode_target.exists());
    assert!(!claude_target.exists());
}

#[test]
fn update_commands_prunes_obsolete_managed_files() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "sample-command");

    let gemini_commands = home.join(".gemini").join("commands");
    let opencode_commands = home.join(".config").join("opencode").join("commands");
    fs::create_dir_all(&gemini_commands).unwrap();
    fs::create_dir_all(&opencode_commands).unwrap();
    fs::write(
        gemini_commands.join("old-command.toml"),
        format!("{GAL_MANAGED_FILE_HEADER}\nold\n"),
    )
    .unwrap();
    fs::write(
        opencode_commands.join("old-command.md"),
        format!("{GAL_MANAGED_FILE_HEADER}\nold\n"),
    )
    .unwrap();

    run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(!gemini_commands.join("old-command.toml").exists());
    assert!(!opencode_commands.join("old-command.md").exists());
    assert!(!gemini_commands.join("sample-command.toml").exists());
    assert!(opencode_commands.join("sample-command.md").exists());
}

#[test]
fn update_commands_is_idempotent_and_preserves_untracked_tools() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "sample-command");

    let options = CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["gemini".into()],
        dry_run: false,
        replace: true,
    };

    let first_report = run_update_commands(&options).unwrap();
    assert!(
        !first_report.written_files.is_empty(),
        "first sync should materialize managed artifacts"
    );

    let lockfile = home.join(".gal").join("state").join("plugins.lock.json");
    let first_lockfile = fs::read_to_string(&lockfile).unwrap();
    assert!(
        first_lockfile.contains("\"_galProjection\""),
        "projection marker must be persisted as the sync source of truth"
    );
    assert!(
        first_lockfile.contains("sample-command"),
        "managed command paths must be recorded in the projection marker"
    );

    let stale_managed_looking = home
        .join(".gemini")
        .join("commands")
        .join("old-command.toml");
    fs::create_dir_all(stale_managed_looking.parent().unwrap()).unwrap();
    fs::write(
        &stale_managed_looking,
        format!("{GAL_MANAGED_FILE_HEADER}\nuser-owned\n"),
    )
    .unwrap();
    let unrelated_claude_tool = home.join(".claude").join("plugins").join("my-tool");
    fs::create_dir_all(&unrelated_claude_tool).unwrap();

    let second_report = run_update_commands(&options).unwrap();

    assert!(
        second_report.written_files.is_empty(),
        "second sync should be idempotent when inputs do not change"
    );
    assert!(
        second_report.removed_paths.is_empty(),
        "second sync must not delete untracked surfaces"
    );
    assert!(
        second_report.warnings.is_empty(),
        "stable rerun should not emit cleanup warnings"
    );
    assert!(
        stale_managed_looking.exists(),
        "files not tracked in plugins.lock.json must be preserved even if they look GAL-managed"
    );
    assert!(
        unrelated_claude_tool.exists(),
        "sync must not delete unrelated non-GAL Claude tools"
    );
    assert_eq!(
        fs::read_to_string(&lockfile).unwrap(),
        first_lockfile,
        "idempotent sync must leave the projection marker unchanged"
    );
}

#[test]
fn update_commands_preserves_user_owned_shared_skill_link() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "sample-command");

    let external_target = home.join("external-command");
    fs::create_dir_all(&external_target).unwrap();
    let shared_target = home.join(".agents").join("skills").join("sample-command");
    fs::create_dir_all(shared_target.parent().unwrap()).unwrap();
    create_dir_link(&external_target, &shared_target).unwrap();

    run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["gemini".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(shared_target.exists());
    assert_eq!(
        fs::canonicalize(&shared_target).unwrap(),
        fs::canonicalize(&external_target).unwrap()
    );
}

#[test]
fn update_commands_preserves_user_owned_shared_command_skill_dir() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "sample-command");

    // A genuine user directory at the shared skills target (same name as a
    // command) must be preserved, not clobbered by the command-skill write.
    let shared_target = home.join(".agents").join("skills").join("sample-command");
    fs::create_dir_all(&shared_target).unwrap();
    fs::write(shared_target.join("SKILL.md"), "user's own skill\n").unwrap();
    fs::write(shared_target.join("user.txt"), "keep").unwrap();

    let report = run_update_commands(&CommandUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert_eq!(
        fs::read_to_string(shared_target.join("SKILL.md")).unwrap(),
        "user's own skill\n",
        "user's SKILL.md must not be overwritten"
    );
    assert!(shared_target.join("user.txt").exists());
    assert!(report
        .warnings
        .iter()
        .any(|warning| warning.message.contains("preserved user-owned path")));
}

#[test]
fn registry_mark_with_source_records_and_persists_attribution() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let path_a = tmp.path().join("skill-a");
    let path_b = tmp.path().join("skill-b");

    // Write an initial lockfile with no attribution so the first load is a no-attribution baseline.
    let mut report = ProjectionReport::default();
    let mut reg = ManagedArtifactRegistry::load(&lockfile, &mut report);

    reg.mark(MANAGED_SKILL_PATHS, &path_a);
    reg.mark_with_source(MANAGED_SKILL_PATHS, &path_b, "my-bundle");
    reg.persist(false, &mut report).unwrap();

    // Reload and verify attribution is present for path_b, absent for path_a.
    let reg2 = ManagedArtifactRegistry::load(&lockfile, &mut report);
    assert_eq!(
        reg2.owning_source_of(&path_b),
        Some("my-bundle"),
        "reload must preserve source attribution for mark_with_source path"
    );
    assert_eq!(
        reg2.owning_source_of(&path_a),
        None,
        "paths marked without source must have no attribution"
    );
}

#[test]
fn registry_category_rebuild_drops_stale_attribution() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let stale_path = tmp.path().join("stale-skill");
    let current_path = tmp.path().join("current-skill");
    let stale_path = stale_path.to_string_lossy().replace('\\', "/");
    let current_path = current_path.to_string_lossy().replace('\\', "/");
    fs::write(
        &lockfile,
        serde_json::json!({
            "_galProjection": {
                "schemaVersion": 1,
                "skillProjectionPaths": [stale_path],
                "sourceAttribution": { stale_path.clone(): "old-source" }
            }
        })
        .to_string(),
    )
    .unwrap();

    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.replace_category(MANAGED_SKILL_PATHS);
    registry.mark_with_source(
        MANAGED_SKILL_PATHS,
        Path::new(&current_path),
        "current-source",
    );
    registry.persist(false, &mut report).unwrap();

    let reloaded = ManagedArtifactRegistry::load(&lockfile, &mut report);
    assert_eq!(reloaded.owning_source_of(Path::new(&stale_path)), None);
    assert_eq!(
        reloaded.owning_source_of(Path::new(&current_path)),
        Some("current-source")
    );
}

#[test]
fn registry_category_rebuild_preserves_unrelated_state() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_AGENT_PATHS, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let stale_path = tmp.path().join("stale-skill");
    let unrelated_path = tmp.path().join("unrelated-agent");
    let stale_path = stale_path.to_string_lossy().replace('\\', "/");
    let unrelated_path = unrelated_path.to_string_lossy().replace('\\', "/");
    fs::write(
        &lockfile,
        serde_json::json!({
            "customKey": "preserve-me",
            "_galProjection": {
                "schemaVersion": 1,
                "skillProjectionPaths": [stale_path],
                "agentProjectionPaths": [unrelated_path],
                "sourceAttribution": {
                    stale_path.clone(): "old-source",
                    unrelated_path.clone(): "other-source"
                }
            }
        })
        .to_string(),
    )
    .unwrap();

    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.replace_category(MANAGED_SKILL_PATHS);
    registry.persist(false, &mut report).unwrap();

    let raw: Value = serde_json::from_str(&fs::read_to_string(&lockfile).unwrap()).unwrap();
    let projection = raw.get("_galProjection").unwrap();
    assert_eq!(
        raw.get("customKey"),
        Some(&Value::String("preserve-me".into()))
    );
    assert!(projection[MANAGED_SKILL_PATHS]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        projection[MANAGED_AGENT_PATHS].as_array().unwrap(),
        &[Value::String(unrelated_path.clone())]
    );
    assert_eq!(
        projection["sourceAttribution"][unrelated_path],
        Value::String("other-source".into())
    );
    assert!(projection["sourceAttribution"].get(&stale_path).is_none());
}

// ── ManagedArtifactRegistry — personal plugin projection safety ──

/// Non-GAL paths are NOT in the registry → can_mutate() must return false
/// once a lockfile has been written (i.e. prior is non-empty).
/// This guarantees that personal plugin projection never deletes a user-owned
/// file at an unrelated path.
#[test]
fn registry_can_mutate_false_for_non_managed_path_with_populated_prior() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let gal_managed_path = tmp.path().join("gal-managed-skill");
    let user_path = tmp.path().join("user-owned-file");

    // Write a lockfile recording gal_managed_path.
    let mut report = ProjectionReport::default();
    let mut reg = ManagedArtifactRegistry::load(&lockfile, &mut report);
    reg.mark(MANAGED_SKILL_PATHS, &gal_managed_path);
    reg.persist(false, &mut report).unwrap();

    // Reload — prior is now non-empty.
    let reg2 = ManagedArtifactRegistry::load(&lockfile, &mut report);

    // GAL-managed path → can_mutate() = true.
    assert!(
        reg2.can_mutate(&gal_managed_path),
        "GAL-managed path must be mutable"
    );
    // User-owned path (never in the registry) → can_mutate() = false.
    assert!(
        !reg2.can_mutate(&user_path),
        "non-GAL path must NOT be mutable (prune safety)"
    );
}

/// Personal plugin artifacts are marked via mark() during projection.
/// After persist + reload, those paths satisfy can_mutate() = true
/// (they are GAL-managed), while a different user path remains false.
#[test]
fn registry_personal_plugin_artifacts_are_managed_after_mark() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let personal_skill = tmp.path().join("personal-skill-projected");
    let unrelated_user_file = tmp.path().join("claude-plugins-external-never-touched");

    let mut report = ProjectionReport::default();
    let mut reg = ManagedArtifactRegistry::load(&lockfile, &mut report);
    // Simulates the projection of a personal plugin skill artifact.
    reg.mark(MANAGED_SKILL_PATHS, &personal_skill);
    reg.persist(false, &mut report).unwrap();

    let reg2 = ManagedArtifactRegistry::load(&lockfile, &mut report);
    assert!(
        reg2.can_mutate(&personal_skill),
        "personal plugin projected artifact must be mutable (GAL owns it)"
    );
    assert!(
        !reg2.can_mutate(&unrelated_user_file),
        "unrelated user file must NOT be mutable — prune must never delete it"
    );
}

#[test]
fn update_skills_projects_two_sources_to_shared_skills() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    // gal-core: skill-a
    write_skill_fixture(&source_root, "skill-a");
    // bundle source: skill-b (different name → no collision)
    let bundle_root = _temp.path().join("bundle");
    fs::create_dir_all(bundle_root.join("skills").join("skill-b")).unwrap();
    fs::write(
        bundle_root.join("skills").join("skill-b").join("SKILL.md"),
        "# skill-b\n",
    )
    .unwrap();
    fs::create_dir_all(bundle_root.join("agents")).unwrap();

    run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![
            ProjectionSource {
                root: source_root.clone(),
                id: "gal-core".into(),
                persistent: true,
            },
            ProjectionSource {
                root: bundle_root,
                id: "my-bundle".into(),
                persistent: false,
            },
        ],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    let shared = home.join(".agents").join("skills");
    assert!(
        shared.join("skill-a").exists(),
        "gal-core skill must be projected"
    );
    assert!(
        shared.join("skill-b").exists(),
        "bundle skill must be projected"
    );
}

#[test]
fn update_skills_single_source_output_unchanged() {
    // Single-source (1-element Vec) must be byte-identical to pre-change behavior.
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "sample-skill");

    let report = run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert!(home
        .join(".agents")
        .join("skills")
        .join("sample-skill")
        .exists());
    assert!(
        report.warnings.is_empty(),
        "single-source must emit no warnings"
    );
}

#[test]
fn registry_attribution_round_trips_multiple_sources() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_AGENT_PATHS, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");

    let paths = [
        (tmp.path().join("skill-core"), "gal-core"),
        (tmp.path().join("skill-bundle"), "my-bundle"),
        (tmp.path().join("agent-bundle"), "my-bundle"),
    ];

    let mut report = ProjectionReport::default();
    let mut reg = ManagedArtifactRegistry::load(&lockfile, &mut report);

    reg.mark_with_source(MANAGED_SKILL_PATHS, &paths[0].0, paths[0].1);
    reg.mark_with_source(MANAGED_SKILL_PATHS, &paths[1].0, paths[1].1);
    reg.mark_with_source(MANAGED_AGENT_PATHS, &paths[2].0, paths[2].1);
    reg.persist(false, &mut report).unwrap();

    let reg2 = ManagedArtifactRegistry::load(&lockfile, &mut report);
    for (path, expected_source) in &paths {
        assert_eq!(
            reg2.owning_source_of(path),
            Some(*expected_source),
            "source attribution for {} must survive round-trip",
            path.display()
        );
    }
}

// Collision policy: core-wins + additive-only + warn
#[test]
fn update_skills_collision_core_wins_emits_warning() {
    // gal-core and bundle both declare "shared-skill" → core item preserved, bundle skipped + warning.
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "shared-skill");
    write_skill_fixture(&source_root, "core-only");

    let bundle_root = _temp.path().join("bundle-collision");
    fs::create_dir_all(bundle_root.join("skills").join("shared-skill")).unwrap();
    fs::write(
        bundle_root
            .join("skills")
            .join("shared-skill")
            .join("SKILL.md"),
        "# bundle version\n",
    )
    .unwrap();
    fs::create_dir_all(bundle_root.join("agents")).unwrap();

    let report = run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![
            ProjectionSource {
                root: source_root.clone(),
                id: "gal-core".into(),
                persistent: true,
            },
            ProjectionSource {
                root: bundle_root,
                id: "my-bundle".into(),
                persistent: false,
            },
        ],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    // Core item must be projected.
    let shared_dir = home.join(".agents").join("skills").join("shared-skill");
    assert!(
        shared_dir.exists(),
        "core skill must be projected even when bundle collides"
    );

    // Exactly one warning must mention the collision.
    assert_eq!(
        report.warnings.len(),
        1,
        "exactly one collision warning expected"
    );
    let msg = &report.warnings[0].message;
    assert!(
        msg.contains("shared-skill") && msg.contains("my-bundle") && msg.contains("gal-core"),
        "warning must name the skill and both sources; got: {msg}"
    );
}

#[test]
fn update_skills_collision_non_core_first_wins_emits_warning() {
    // Two non-core sources collide → first-loaded wins, second skipped + warning.
    let (_temp, repo_root, source_root, home) = fixture_roots();
    // gal-core has no "conflict-skill"
    fs::create_dir_all(source_root.join("skills")).unwrap();
    fs::create_dir_all(source_root.join("agents")).unwrap();

    let bundle_a = _temp.path().join("bundle-a");
    fs::create_dir_all(bundle_a.join("skills").join("conflict-skill")).unwrap();
    fs::write(
        bundle_a
            .join("skills")
            .join("conflict-skill")
            .join("SKILL.md"),
        "# A\n",
    )
    .unwrap();
    fs::create_dir_all(bundle_a.join("agents")).unwrap();

    let bundle_b = _temp.path().join("bundle-b");
    fs::create_dir_all(bundle_b.join("skills").join("conflict-skill")).unwrap();
    fs::write(
        bundle_b
            .join("skills")
            .join("conflict-skill")
            .join("SKILL.md"),
        "# B\n",
    )
    .unwrap();
    fs::create_dir_all(bundle_b.join("agents")).unwrap();

    let report = run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![
            ProjectionSource {
                root: source_root.clone(),
                id: "gal-core".into(),
                persistent: true,
            },
            ProjectionSource {
                root: bundle_a,
                id: "bundle-a".into(),
                persistent: false,
            },
            ProjectionSource {
                root: bundle_b,
                id: "bundle-b".into(),
                persistent: false,
            },
        ],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    // First non-core source wins → skill exists.
    assert!(
        home.join(".agents")
            .join("skills")
            .join("conflict-skill")
            .exists(),
        "first non-core source skill must be projected"
    );

    // Exactly one warning for the second source collision.
    assert_eq!(
        report.warnings.len(),
        1,
        "exactly one collision warning expected"
    );
    let msg = &report.warnings[0].message;
    assert!(
        msg.contains("conflict-skill") && msg.contains("bundle-b") && msg.contains("bundle-a"),
        "warning must name skill and both non-core sources; got: {msg}"
    );
}

// Per-source prune safety: source still installed → skip even if absent from invocation
#[test]
fn prune_stale_links_skips_items_from_installed_source() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    // Simulate two previously-projected skills: one from gal-core, one from my-bundle.
    let core_skill = root.join("core-skill");
    let bundle_skill = root.join("bundle-skill");
    fs::create_dir_all(&core_skill).unwrap();
    fs::create_dir_all(&bundle_skill).unwrap();

    let lockfile = tmp.path().join("lock.json");
    let mut report = ProjectionReport::default();
    let mut reg = ManagedArtifactRegistry::load(&lockfile, &mut report);
    reg.mark_with_source(MANAGED_SKILL_PATHS, &core_skill, "gal-core");
    reg.mark_with_source(MANAGED_SKILL_PATHS, &bundle_skill, "my-bundle");
    reg.persist(false, &mut report).unwrap();

    // Reload (simulate a fresh invocation).
    let mut reg2 = ManagedArtifactRegistry::load(&lockfile, &mut report);

    // Only gal-core in keep_names (bundle-skill not projected this invocation).
    // installed_source_ids includes my-bundle → bundle-skill must NOT be pruned.
    prune_stale_links(
        &root,
        ["core-skill"],
        false,
        &mut report,
        &mut reg2,
        &["gal-core", "my-bundle"],
    )
    .unwrap();

    assert!(core_skill.exists(), "core skill in keep list must be kept");
    assert!(
        bundle_skill.exists(),
        "bundle skill from installed source must NOT be pruned even when absent from keep list"
    );
}

#[test]
fn prune_stale_links_removes_items_from_removed_source() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    let core_skill = root.join("core-skill");
    let bundle_skill = root.join("bundle-skill");
    fs::create_dir_all(&core_skill).unwrap();
    fs::create_dir_all(&bundle_skill).unwrap();

    // Write a GAL-managed file inside the dir so is_gal_owned_real_path authorizes removal.
    // Reference the constant (not a hardcoded string) so renaming the marker breaks
    // this fixture at compile time instead of silently desyncing.
    fs::write(
        bundle_skill.join("SKILL.md"),
        format!("{GAL_MANAGED_FILE_HEADER}\n# bundle-skill\n"),
    )
    .unwrap();

    let lockfile = tmp.path().join("lock.json");
    let mut report = ProjectionReport::default();
    let mut reg = ManagedArtifactRegistry::load(&lockfile, &mut report);
    reg.mark_with_source(MANAGED_SKILL_PATHS, &core_skill, "gal-core");
    reg.mark_with_source(MANAGED_SKILL_PATHS, &bundle_skill, "my-bundle");
    reg.persist(false, &mut report).unwrap();

    let mut reg2 = ManagedArtifactRegistry::load(&lockfile, &mut report);

    // my-bundle removed from installed_source_ids → bundle-skill IS prunable.
    prune_stale_links(
        &root,
        ["core-skill"],
        false,
        &mut report,
        &mut reg2,
        &["gal-core"],
    )
    .unwrap();

    assert!(core_skill.exists(), "core skill must survive");
    assert!(
        !bundle_skill.exists(),
        "bundle skill from removed source must be pruned"
    );
}

// Source assembly: collect_projection_sources returns gal-core 1-element list by default
#[test]
fn collect_projection_sources_returns_gal_core_single_source() {
    use crate::collect_projection_sources;

    let tmp = TempDir::new().unwrap();
    let source_root = tmp.path().to_path_buf();

    let sources = collect_projection_sources(&source_root);
    assert_eq!(
        sources.len(),
        1,
        "default assembly must return exactly 1 source"
    );
    assert_eq!(sources[0].id, "gal-core");
    assert_eq!(sources[0].root, source_root);
    assert!(sources[0].persistent, "gal-core must be persistent");
}

#[test]
fn orchestrated_only_golem_names_constant_contains_expected_roles() {
    // Verify the exclusion constant covers exactly the four orchestrated-only roles.
    let expected = [
        "golem-implementer",
        "golem-tester",
        "golem-auditor",
        "golem-researcher",
    ];
    for role in &expected {
        assert!(
            ORCHESTRATED_ONLY_GOLEM_NAMES.contains(role),
            "orchestrated-only set must include {role}"
        );
    }
    assert_eq!(
        ORCHESTRATED_ONLY_GOLEM_NAMES.len(),
        expected.len(),
        "orchestrated-only set size must match (no phantom entries)"
    );
}

#[test]
fn orchestrated_only_golem_names_excludes_consult_and_utility_roles() {
    // Consult roles and utility roles must NOT be in the exclusion set.
    for role in &[
        "golem-architect",
        "golem-analyst",
        "golem-designer",
        "golem-releaser",
        "golem-debugger",
        "golem-steward",
    ] {
        assert!(
            !ORCHESTRATED_ONLY_GOLEM_NAMES.contains(role),
            "consult/utility role must not be in orchestrated-only set: {role}"
        );
    }
}

// Verifies that persist() does not write gal-self.json when the registry is empty
// (byte-identical guarantee for the no-writer stage). An empty registry fires the
// equality guard (prior == next, both empty BTreeMaps) and returns early without
// calling write_text — the file stays absent.
#[test]
fn registry_empty_gal_self_no_write_on_persist() {
    use crate::support::ManagedArtifactRegistry;

    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("gal-self.json");

    let mut report = ProjectionReport::default();
    let reg = ManagedArtifactRegistry::load(&path, &mut report);
    reg.persist(false, &mut report).unwrap();

    assert!(
        !path.exists(),
        "empty gal-self registry must not write gal-self.json (persist() non-empty guard)"
    );
}

// Interleaved coexistence: two registries at distinct lockfile paths (one
// plugins.lock.json-like for "manager", one gal-self.json for "gal-self") share a
// projection output directory. Each registry owns a disjoint subset of artifacts
// via mark_with_source with a distinct source-id. Prune from each registry's side
// must not delete the other registry's artifacts, and a non-GAL external path must
// be untouched by both prune operations.
#[test]
fn registry_interleaved_coexistence_isolation() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let shared_dir = tmp.path().join("skills");
    fs::create_dir_all(&shared_dir).unwrap();

    // Three entries in the shared output directory — directories (no GAL header)
    // so is_gal_owned_real_path cannot authorize accidental removal.
    let manager_artifact = shared_dir.join("manager-skill");
    let gal_self_artifact = shared_dir.join("gal-self-skill");
    let external_path = shared_dir.join("user-external");
    fs::create_dir_all(&manager_artifact).unwrap();
    fs::create_dir_all(&gal_self_artifact).unwrap();
    fs::create_dir_all(&external_path).unwrap();

    let mut report = ProjectionReport::default();

    // Registry A (plugins.lock.json-like): records manager-artifact as owned by "manager".
    let lock_a = tmp.path().join("plugins.lock.json");
    let mut reg_a = ManagedArtifactRegistry::load(&lock_a, &mut report);
    reg_a.mark_with_source(MANAGED_SKILL_PATHS, &manager_artifact, "manager");
    reg_a.persist(false, &mut report).unwrap();

    // Registry B (gal-self.json): records gal-self-artifact as owned by "gal-self".
    let lock_b = tmp.path().join("gal-self.json");
    let mut reg_b = ManagedArtifactRegistry::load(&lock_b, &mut report);
    reg_b.mark_with_source(MANAGED_SKILL_PATHS, &gal_self_artifact, "gal-self");
    reg_b.persist(false, &mut report).unwrap();

    // Reload both registries to simulate a fresh invocation on each side.
    let mut reg_a2 = ManagedArtifactRegistry::load(&lock_a, &mut report);
    let mut reg_b2 = ManagedArtifactRegistry::load(&lock_b, &mut report);

    // --- Prune from registry A's perspective (source "manager" installed) ---
    // manager-skill is in keep_names → always kept.
    // gal-self-skill is NOT in reg_a's prior → can_mutate = false → not authorized.
    // external_path is not in any registry → can_mutate = false → not authorized.
    prune_stale_links(
        &shared_dir,
        ["manager-skill"],
        false,
        &mut report,
        &mut reg_a2,
        &["manager"],
    )
    .unwrap();

    assert!(
        manager_artifact.exists(),
        "manager artifact must survive its own registry prune"
    );
    assert!(
        gal_self_artifact.exists(),
        "gal-self artifact must NOT be deleted by manager-side prune (absent from manager registry)"
    );
    assert!(
        external_path.exists(),
        "external non-GAL path must be untouched by manager-side prune"
    );

    // --- Prune from registry B's perspective (source "gal-self" installed) ---
    // gal-self-skill is in keep_names → always kept.
    // manager-skill is NOT in reg_b's prior → can_mutate = false → not authorized.
    // external_path is not in any registry → can_mutate = false → not authorized.
    prune_stale_links(
        &shared_dir,
        ["gal-self-skill"],
        false,
        &mut report,
        &mut reg_b2,
        &["gal-self"],
    )
    .unwrap();

    assert!(
        gal_self_artifact.exists(),
        "gal-self artifact must survive its own registry prune"
    );
    assert!(
        manager_artifact.exists(),
        "manager artifact must NOT be deleted by gal-self-side prune (absent from gal-self registry)"
    );
    assert!(
        external_path.exists(),
        "external non-GAL path must be untouched by gal-self-side prune"
    );
}

// machine_skill_options builds a SkillUpdateOptions with the right defaults.
// Uses a minimal fixture source root (empty dirs) so it doesn't need the real repo.
#[test]
fn machine_skill_options_returns_correct_defaults() {
    let tmp = TempDir::new().unwrap();
    // Minimal source root: just the dirs projection expects to exist.
    let source_root = tmp.path().join("plugins").join("gal-core");
    let home = tmp.path().join("home");
    fs::create_dir_all(&source_root).unwrap();
    fs::create_dir_all(&home).unwrap();

    let opts =
        machine_skill_options(&source_root, &home).expect("machine_skill_options should succeed");

    assert_eq!(
        opts.repo_root, source_root,
        "repo_root must equal source_root"
    );
    assert_eq!(opts.user_home, home, "user_home must match");
    assert!(!opts.dry_run, "dry_run must be false");
    assert!(opts.replace, "replace must be true");

    // sources[0] must be gal-core.
    assert!(!opts.sources.is_empty(), "must have at least one source");
    assert_eq!(
        opts.sources[0].id, "gal-core",
        "primary source must be gal-core"
    );

    // With no install-state.json, falls back to all 5 runtimes.
    let mut expected: Vec<String> = gal_foundation::runtime::VALID_RUNTIMES
        .iter()
        .map(|r| (*r).to_string())
        .collect();
    expected.sort();
    expected.dedup();
    let mut got = opts.selected_runtimes.clone();
    got.sort();
    got.dedup();
    assert_eq!(
        got, expected,
        "selected_runtimes must fall back to all 5 runtimes (F2)"
    );
}

// materialize_skill_dir — recursive copy, idempotent, junction-replace, dry_run.

#[test]
fn materialize_skill_dir_copies_content_matching_source() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nbody\n").unwrap();
    fs::write(source.join("nested").join("asset.txt"), "asset content").unwrap();

    let target = tmp.path().join("target");
    let mut report = ProjectionReport::default();
    let changed = materialize_skill_dir(&source, &target, false, &mut report)
        .expect("materialize_skill_dir should succeed");

    assert!(
        changed,
        "first copy into an empty target must report a change"
    );
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "---\nname: demo\n---\nbody\n"
    );
    assert_eq!(
        fs::read_to_string(target.join("nested").join("asset.txt")).unwrap(),
        "asset content"
    );
    assert!(
        !is_symlink_or_junction(&target),
        "materialized target must be a real directory, not a link"
    );
}

#[test]
fn materialize_skill_dir_idempotent_on_identical_content() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nbody\n").unwrap();

    let target = tmp.path().join("target");
    let mut report = ProjectionReport::default();
    materialize_skill_dir(&source, &target, false, &mut report).unwrap();

    let mut second_report = ProjectionReport::default();
    let changed = materialize_skill_dir(&source, &target, false, &mut second_report)
        .expect("second call over identical content should succeed");

    assert!(
        !changed,
        "second call with byte-identical content must return false (idempotent)"
    );
    assert!(
        second_report.written_files.is_empty(),
        "no files should be rewritten when content is already identical"
    );
}

#[cfg(windows)]
#[test]
fn materialize_skill_dir_replaces_existing_junction() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nbody\n").unwrap();

    let elsewhere = tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let target = tmp.path().join("target");
    let status = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &target.to_string_lossy(),
            &elsewhere.to_string_lossy(),
        ])
        .status()
        .unwrap();
    assert!(status.success(), "mklink /J failed to create test junction");
    assert!(
        is_symlink_or_junction(&target),
        "test setup: target should be a junction"
    );

    let mut report = ProjectionReport::default();
    let changed = materialize_skill_dir(&source, &target, false, &mut report)
        .expect("materialize_skill_dir should replace the junction");

    assert!(
        changed,
        "replacing a junction with real content must report a change"
    );
    assert!(
        !is_symlink_or_junction(&target),
        "junction must be replaced by a real directory"
    );
    assert_eq!(
        fs::read_to_string(target.join("SKILL.md")).unwrap(),
        "---\nname: demo\n---\nbody\n"
    );
}

#[test]
fn materialize_skill_dir_dry_run_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    let source = tmp.path().join("source");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("SKILL.md"), "---\nname: demo\n---\nbody\n").unwrap();

    let target = tmp.path().join("target");
    let mut report = ProjectionReport::default();
    let changed = materialize_skill_dir(&source, &target, true, &mut report)
        .expect("dry_run materialize_skill_dir should succeed");

    assert!(changed, "dry_run still reports the change that would occur");
    assert!(
        !target.exists(),
        "dry_run must not create the target directory or any file"
    );
}

// update_skills shared-root strategy flip: materialized copy, not a junction.

#[test]
fn update_skills_shared_root_projects_as_real_directory_not_junction() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "demo-skill");

    run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    let projected = home.join(".agents").join("skills").join("demo-skill");
    assert!(projected.join("SKILL.md").is_file());
    assert!(
        !is_symlink_or_junction(&projected),
        "shared-root skill projection must be a real directory, not a ReparsePoint"
    );
    assert_eq!(
        fs::read_to_string(projected.join("SKILL.md")).unwrap(),
        "# demo-skill\n"
    );
}

#[test]
fn update_skills_preserves_user_owned_shared_skill_collision() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "demo-skill");

    // A genuine, never-GAL-managed user directory occupies the same name up front.
    let user_dir = home.join(".agents").join("skills").join("demo-skill");
    fs::create_dir_all(&user_dir).unwrap();
    fs::write(user_dir.join("SKILL.md"), "not GAL content\n").unwrap();

    let report = run_update_skills(&SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    assert_eq!(
        fs::read_to_string(user_dir.join("SKILL.md")).unwrap(),
        "not GAL content\n",
        "user-owned collision must never be overwritten"
    );
    assert!(
        report
            .warnings
            .iter()
            .any(|w| w.message.contains("preserved user-owned path")),
        "collision must be reported as a warning"
    );
}

#[test]
fn update_skills_shared_root_recovers_after_single_skill_deletion_and_is_idempotent() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "demo-skill");

    let opts = SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    };

    run_update_skills(&opts).unwrap();
    let projected = home.join(".agents").join("skills").join("demo-skill");
    assert!(projected.is_dir());

    // Simulate the external-deletion pattern this task defends against.
    fs::remove_dir_all(&projected).unwrap();
    assert!(!projected.exists());

    run_update_skills(&opts).unwrap();
    assert!(
        projected.join("SKILL.md").is_file(),
        "re-projection must restore a single deleted materialized skill"
    );

    // Re-projection over already-current content must be byte-identical (no writes).
    let report = run_update_skills(&opts).unwrap();
    assert!(
        report.written_files.is_empty(),
        "re-projection over unchanged content must write nothing (idempotent)"
    );
}

// Direct unit test of the prune_stale_links directory-deletion extension: a real
// (non-symlink) directory the prior lockfile recorded as GAL-managed is now
// prunable via recursive delete, mirroring the pre-existing plain-file branch.
// (Full update_skills-level pruning of a retired-from-source skill is covered by
// the dedicated zombie-cleanup path, which does not go through this per-source guard.)
#[test]
fn prune_stale_links_removes_lockfile_attributed_materialized_directory_keeps_unmanaged() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    let managed_dir = root.join("old-skill");
    fs::create_dir_all(&managed_dir).unwrap();
    fs::write(
        managed_dir.join("SKILL.md"),
        "---\nname: old-skill\n---\nbody\n",
    )
    .unwrap();

    let user_dir = root.join("user-skill");
    fs::create_dir_all(&user_dir).unwrap();
    fs::write(user_dir.join("SKILL.md"), "not GAL content\n").unwrap();

    let lockfile_path = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile_path, &mut report);
    registry.mark(MANAGED_SKILL_PATHS, &managed_dir);
    registry.persist(false, &mut report).unwrap();

    // Reload so `prior` reflects the persisted lockfile (mirrors a fresh invocation).
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile_path, &mut report);

    prune_stale_links(
        &root,
        std::iter::empty(),
        false,
        &mut report,
        &mut registry,
        &[],
    )
    .unwrap();

    assert!(
        !managed_dir.exists(),
        "a lockfile-attributed materialized directory must be pruned"
    );
    assert!(
        user_dir.exists(),
        "a directory the lockfile never recorded must never be pruned"
    );
}

fn projection_meta(lockfile: &Path) -> Value {
    let root: Value = serde_json::from_str(&fs::read_to_string(lockfile).unwrap()).unwrap();
    root.get("_galProjection").cloned().unwrap()
}

fn ownership_path(path: &Path) -> String {
    let rendered = path.to_string_lossy().replace('/', "\\");
    #[cfg(windows)]
    {
        rendered.to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        rendered
    }
}

#[test]
fn prune_stale_links_ownership_retains_directory_keyed_record() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let path = root.join("directory-owned");
    fs::create_dir_all(&path).unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_SKILL_PATHS, &path, "installed");
    registry.persist(false, &mut report).unwrap();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    prune_stale_links(
        &root,
        std::iter::empty(),
        false,
        &mut report,
        &mut registry,
        &["installed"],
    )
    .unwrap();
    registry.persist(false, &mut report).unwrap();
    let meta = projection_meta(&lockfile);
    assert!(path.exists());
    assert!(meta["skillProjectionPaths"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == &Value::String(ownership_path(&path))));
    assert_eq!(
        meta["sourceAttribution"][ownership_path(&path)],
        "installed"
    );
}

#[test]
fn prune_stale_links_ownership_retains_skill_md_keyed_record() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_COMMAND_PATHS};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let path = root.join("skill-md-owned");
    let skill_file = path.join("SKILL.md");
    fs::create_dir_all(&path).unwrap();
    fs::write(&skill_file, "---\nname: skill-md-owned\n---\n").unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_COMMAND_PATHS, &skill_file, "installed");
    registry.persist(false, &mut report).unwrap();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    prune_stale_links(
        &root,
        std::iter::empty(),
        false,
        &mut report,
        &mut registry,
        &["installed"],
    )
    .unwrap();
    registry.persist(false, &mut report).unwrap();
    let meta = projection_meta(&lockfile);
    assert!(path.exists());
    assert!(meta["commandProjectionPaths"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == &Value::String(ownership_path(&skill_file))));
    assert_eq!(
        meta["sourceAttribution"][ownership_path(&skill_file)],
        "installed"
    );
}

#[test]
fn prune_stale_links_ownership_removes_absent_source_record() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let path = root.join("removed-source");
    fs::create_dir_all(&path).unwrap();
    fs::write(
        path.join("SKILL.md"),
        format!("{GAL_MANAGED_FILE_HEADER}\n"),
    )
    .unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_SKILL_PATHS, &path, "removed");
    registry.persist(false, &mut report).unwrap();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    prune_stale_links(
        &root,
        std::iter::empty(),
        false,
        &mut report,
        &mut registry,
        &[],
    )
    .unwrap();
    assert!(!path.exists());
    assert!(report.removed_paths.contains(&path));
}

#[test]
fn prune_stale_links_ownership_preserves_user_and_dangling_paths() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let user = root.join("user-owned");
    let dangling = root.join("dangling-attribution");
    fs::create_dir_all(&user).unwrap();
    fs::create_dir_all(&dangling).unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let lock = serde_json::json!({
        "_galProjection": {
            "schemaVersion": 1,
            "agentProjectionPaths": [], "codexAgentProjectionPaths": [],
            "commandProjectionPaths": [], "discussSkillProjectionPaths": [],
            "legacyProjectionPaths": [], "skillProjectionPaths": [],
            "sourceAttribution": { ownership_path(&dangling): "old-source" }
        }
    });
    fs::write(&lockfile, serde_json::to_string(&lock).unwrap()).unwrap();
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    prune_stale_links(
        &root,
        std::iter::empty(),
        false,
        &mut report,
        &mut registry,
        &[],
    )
    .unwrap();
    registry.persist(false, &mut report).unwrap();
    let meta = projection_meta(&lockfile);
    assert!(user.exists() && dangling.exists());
    assert_eq!(
        meta["sourceAttribution"][ownership_path(&dangling)],
        "old-source"
    );
    assert!(meta["sourceAttribution"]
        .get(ownership_path(&user))
        .is_none());
}

#[test]
fn prune_stale_links_ownership_defers_discuss_dir_for_installed_source() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_DISCUSS_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let path = root.join("discuss-architect");
    let skill_file = path.join("SKILL.md");
    fs::create_dir_all(&path).unwrap();
    fs::write(&skill_file, "---\nname: discuss-golem-architect\n---\n").unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_DISCUSS_SKILL_PATHS, &skill_file, "installed");
    registry.persist(false, &mut report).unwrap();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    prune_stale_links(
        &root,
        std::iter::empty(),
        false,
        &mut report,
        &mut registry,
        &["installed"],
    )
    .unwrap();
    assert!(path.exists());
}

#[test]
fn prune_stale_links_ownership_retains_discuss_dir_when_deletion_unauthorized() {
    use crate::support::{prune_stale_links, ManagedArtifactRegistry, MANAGED_DISCUSS_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let path = root.join("discuss-architect");
    let skill_file = path.join("SKILL.md");
    fs::create_dir_all(&path).unwrap();
    fs::write(&skill_file, "---\nname: discuss-golem-architect\n---\n").unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_DISCUSS_SKILL_PATHS, &skill_file, "removed");
    registry.persist(false, &mut report).unwrap();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    prune_stale_links(
        &root,
        std::iter::empty(),
        false,
        &mut report,
        &mut registry,
        &[],
    )
    .unwrap();
    registry.persist(false, &mut report).unwrap();
    let meta = projection_meta(&lockfile);
    assert!(path.exists());
    // MANAGED_DISCUSS_SKILL_PATHS is not a persisted category yet — only
    // the attribution side of retention is observable at this point.
    assert_eq!(
        meta["sourceAttribution"][ownership_path(&skill_file)],
        "removed"
    );
}

#[test]
fn discuss_skill_category_round_trips_through_persist() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_DISCUSS_SKILL_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let initial = serde_json::json!({"unrelated": {"keep": true}});
    fs::write(&lockfile, serde_json::to_string(&initial).unwrap()).unwrap();
    let skill_file = tmp
        .path()
        .join("skills")
        .join("discuss-architect")
        .join("SKILL.md");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_DISCUSS_SKILL_PATHS, &skill_file, "gal-core");
    registry.persist(false, &mut report).unwrap();

    let meta = projection_meta(&lockfile);
    assert_eq!(meta["schemaVersion"], 1);
    assert!(meta["discussSkillProjectionPaths"]
        .as_array()
        .unwrap()
        .contains(&Value::String(ownership_path(&skill_file))));
    let root: Value = serde_json::from_str(&fs::read_to_string(&lockfile).unwrap()).unwrap();
    assert_eq!(root["unrelated"]["keep"], true);

    let loaded = ManagedArtifactRegistry::load(&lockfile, &mut report);
    assert!(loaded.is_managed(&skill_file));
}

#[test]
fn opencode_agent_category_round_trips_through_persist() {
    use crate::support::{ManagedArtifactRegistry, MANAGED_OPENCODE_AGENT_PATHS};

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let initial = serde_json::json!({"unrelated": {"keep": true}});
    fs::write(&lockfile, serde_json::to_string(&initial).unwrap()).unwrap();
    let agent_file = tmp.path().join("agents").join("golem-implementer.md");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_OPENCODE_AGENT_PATHS, &agent_file, "gal-core");
    registry.persist(false, &mut report).unwrap();

    let meta = projection_meta(&lockfile);
    assert_eq!(meta["schemaVersion"], 1);
    assert!(meta["opencodeAgentProjectionPaths"]
        .as_array()
        .unwrap()
        .contains(&Value::String(ownership_path(&agent_file))));
    let root: Value = serde_json::from_str(&fs::read_to_string(&lockfile).unwrap()).unwrap();
    assert_eq!(root["unrelated"]["keep"], true);

    let loaded = ManagedArtifactRegistry::load(&lockfile, &mut report);
    assert!(loaded.is_managed(&agent_file));
}

#[test]
fn discuss_skill_category_attribution_has_category_backing() {
    use crate::support::{
        ManagedArtifactRegistry, MANAGED_COMMAND_PATHS, MANAGED_DISCUSS_SKILL_PATHS,
    };

    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("plugins.lock.json");
    let discuss_file = tmp.path().join("discuss-architect").join("SKILL.md");
    let command_file = tmp.path().join("gal-status").join("SKILL.md");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_DISCUSS_SKILL_PATHS, &discuss_file, "gal-core");
    registry.mark_with_source(MANAGED_COMMAND_PATHS, &command_file, "gal-core");
    registry.persist(false, &mut report).unwrap();

    let meta = projection_meta(&lockfile);
    let categories = [
        "agentProjectionPaths",
        "codexAgentProjectionPaths",
        "commandProjectionPaths",
        "discussSkillProjectionPaths",
        "legacyProjectionPaths",
        "skillProjectionPaths",
    ];
    for path in meta["sourceAttribution"].as_object().unwrap().keys() {
        assert!(
            categories.iter().any(|category| {
                meta[*category]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value.as_str() == Some(path))
            }),
            "sourceAttribution key {path} must have category backing"
        );
    }
}

#[test]
fn discuss_skill_category_does_not_widen_command_skill_deletion() {
    use crate::support::{
        list_command_dirs, remove_gal_command_skill, ManagedArtifactRegistry,
        MANAGED_DISCUSS_SKILL_PATHS,
    };

    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_command_fixture(&source_root, "sample-command");
    let skills_root = home.join(".agents").join("skills");
    let discuss_dir = skills_root.join("discuss-architect");
    let discuss_file = discuss_dir.join("SKILL.md");
    fs::create_dir_all(&discuss_dir).unwrap();
    fs::write(&discuss_file, "---\nname: discuss-golem-architect\n---\n").unwrap();
    let lockfile = home.join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    registry.mark_with_source(MANAGED_DISCUSS_SKILL_PATHS, &discuss_file, "gal-core");
    registry.persist(false, &mut report).unwrap();
    let registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    for command_dir in list_command_dirs(&source_root.join("commands")).unwrap() {
        let name = command_dir.file_name().unwrap().to_str().unwrap();
        remove_gal_command_skill(
            &skills_root,
            name,
            &repo_root,
            &registry,
            false,
            &mut report,
        )
        .unwrap();
    }

    assert!(discuss_dir.exists());
    assert!(report.removed_paths.is_empty());
}

#[cfg(windows)]
#[test]
fn update_skills_copilot_gal_junction_survives_shared_skill_materialization() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "demo-skill");

    let opts = SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into(), "copilot".into()],
        dry_run: false,
        replace: true,
    };

    run_update_skills(&opts).unwrap();
    run_update_skills(&opts).unwrap();

    let copilot_gal = home.join(".copilot").join("gal");
    assert!(
        is_symlink_or_junction(&copilot_gal),
        "the GAL-private ~/.copilot/gal namespace must stay a junction, unaffected by the \
         shared ~/.agents/skills materialization strategy"
    );
}

// SkillsProjectionHealthCheck required-skill inventory — one finding kind per fixture.

#[test]
fn inventory_healthcheck_flags_missing_dir_as_error_for_critical_skill() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    let check =
        SkillsProjectionHealthCheck::with_inventory(root, vec!["adversarial-review".into()], None);
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].severity,
        gal_foundation::health::Severity::Error
    );
    assert!(findings[0].message.contains("adversarial-review"));
}

#[test]
fn inventory_healthcheck_flags_missing_dir_as_warning_for_non_critical_skill() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    let check = SkillsProjectionHealthCheck::with_inventory(root, vec!["some-skill".into()], None);
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].severity,
        gal_foundation::health::Severity::Warning
    );
}

#[test]
fn inventory_healthcheck_flags_missing_skill_md() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(root.join("demo-skill")).unwrap();

    let check = SkillsProjectionHealthCheck::with_inventory(root, vec!["demo-skill".into()], None);
    let findings = check.check();
    assert!(findings
        .iter()
        .any(|f| f.severity == gal_foundation::health::Severity::Error
            && f.message.contains("missing or unreadable")));
}

#[test]
fn inventory_healthcheck_flags_frontmatter_name_mismatch() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let dir = root.join("demo-skill");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: something-else\ndescription: d\n---\nbody\n",
    )
    .unwrap();

    let check = SkillsProjectionHealthCheck::with_inventory(root, vec!["demo-skill".into()], None);
    let findings = check.check();
    assert!(findings
        .iter()
        .any(|f| f.message.contains("does not match its directory name")));
}

#[test]
fn skills_projection_health_check_reports_stale_projected_skill() {
    let tmp = TempDir::new().unwrap();
    let shared = tmp.path().join("skills");
    let canonical = tmp.path().join("canonical");

    let write_skill = |root: &Path, body: &str| {
        let dir = root.join("gal-pipeline");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("SKILL.md"),
            format!("---\nname: gal-pipeline\ndescription: d\n---\n{body}\n"),
        )
        .unwrap();
    };

    // Projected copy carries the OLD body; canonical source has the NEW body.
    write_skill(&shared, "old contract body");
    write_skill(&canonical, "new contract body");

    let stale_finding = |c: &SkillsProjectionHealthCheck| {
        c.check()
            .into_iter()
            .find(|f| f.message.contains("differs from current canonical source"))
    };

    let check = SkillsProjectionHealthCheck::with_inventory(
        shared.clone(),
        vec!["gal-pipeline".into()],
        None,
    )
    .with_canonical_source(canonical.clone());
    let finding = stale_finding(&check).expect("drifted projected skill must be reported stale");
    assert!(finding.message.contains("gal-pipeline"));
    assert!(finding.message.contains("gal refresh"));

    // Fresh: projected == canonical → no stale finding (line-ending difference
    // must also be tolerated, not reported as drift).
    let fresh_shared = tmp.path().join("fresh");
    let dir = fresh_shared.join("gal-pipeline");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        "---\r\nname: gal-pipeline\r\ndescription: d\r\n---\r\nnew contract body\r\n",
    )
    .unwrap();
    let fresh = SkillsProjectionHealthCheck::with_inventory(
        fresh_shared,
        vec!["gal-pipeline".into()],
        None,
    )
    .with_canonical_source(canonical.clone());
    assert!(
        stale_finding(&fresh).is_none(),
        "identical content (modulo line endings) must not be reported stale"
    );

    // No canonical file for the item → freshness disabled, never a false stale.
    let no_canon =
        SkillsProjectionHealthCheck::with_inventory(shared, vec!["gal-pipeline".into()], None)
            .with_canonical_source(tmp.path().join("empty-canonical"));
    assert!(
        stale_finding(&no_canon).is_none(),
        "a missing canonical file must never produce a false stale report"
    );
}

#[test]
fn stale_finding_resolves_canonical_from_commands_subdir() {
    // A command projected as a shared skill (e.g. gal-pipeline) has its canonical
    // source under `<plugin-root>/commands/<name>/SKILL.md`, not the flat layout.
    let tmp = TempDir::new().unwrap();
    let shared = tmp.path().join("skills");
    let plugin_root = tmp.path().join("plugin");

    let dir = shared.join("gal-pipeline");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: gal-pipeline\ndescription: d\n---\nold command body\n",
    )
    .unwrap();

    let canon_dir = plugin_root.join("commands").join("gal-pipeline");
    fs::create_dir_all(&canon_dir).unwrap();
    fs::write(
        canon_dir.join("SKILL.md"),
        "---\nname: gal-pipeline\ndescription: d\n---\nnew command body\n",
    )
    .unwrap();

    let check =
        SkillsProjectionHealthCheck::with_inventory(shared, vec!["gal-pipeline".into()], None)
            .with_canonical_source(plugin_root);
    assert!(
        check
            .check()
            .iter()
            .any(|f| f.message.contains("differs from current canonical source")),
        "a command-projected skill under commands/<name> must be freshness-checked"
    );
}

#[cfg(windows)]
#[test]
fn inventory_healthcheck_flags_residual_junction_as_legacy() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();
    let elsewhere = tmp.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    let dir = root.join("demo-skill");
    let status = Command::new("cmd")
        .args([
            "/C",
            "mklink",
            "/J",
            &dir.to_string_lossy(),
            &elsewhere.to_string_lossy(),
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let check = SkillsProjectionHealthCheck::with_inventory(root, vec!["demo-skill".into()], None);
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("legacy junction"));
}

#[test]
fn inventory_healthcheck_flags_non_gal_owned_collision() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let dir = root.join("demo-skill");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: d\n---\nbody\n",
    )
    .unwrap();

    // No lockfile path -> ownership cannot be proven -> collision warning.
    let check = SkillsProjectionHealthCheck::with_inventory(root, vec!["demo-skill".into()], None);
    let findings = check.check();
    assert!(findings
        .iter()
        .any(|f| f.message.contains("not GAL-managed")));
}

#[test]
fn inventory_healthcheck_clean_when_lockfile_attributes_ownership() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    let dir = root.join("demo-skill");
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join("SKILL.md"),
        "---\nname: demo-skill\ndescription: d\n---\nbody\n",
    )
    .unwrap();

    let lockfile_path = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile_path, &mut report);
    registry.mark(MANAGED_SKILL_PATHS, &dir);
    registry.persist(false, &mut report).unwrap();

    let check = SkillsProjectionHealthCheck::with_inventory(
        root,
        vec!["demo-skill".into()],
        Some(lockfile_path),
    );
    assert!(
        check.check().is_empty(),
        "a lockfile-attributed skill with matching content must produce no finding"
    );
}

#[test]
fn no_stale_machine_only_hint_in_support_source() {
    let source = include_str!("support.rs");
    assert!(
        !source.contains("machine-only"),
        "the retired `gal update --machine-only` hint must not reappear"
    );
}

// Deletion discriminator — locked-missing / unlocked-missing / no-lockfile.

#[test]
fn inventory_healthcheck_missing_but_locked_reports_external_deletion() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    let lockfile_path = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile_path, &mut report);
    registry.mark(MANAGED_SKILL_PATHS, &root.join("demo-skill"));
    registry.persist(false, &mut report).unwrap();

    let check = SkillsProjectionHealthCheck::with_inventory(
        root,
        vec!["demo-skill".into()],
        Some(lockfile_path),
    );
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].message.contains("external deletion"),
        "missing-but-locked must be discriminated as external deletion: {}",
        findings[0].message
    );
    assert!(
        findings[0].message.contains("last GAL projection:"),
        "external deletion finding must cite the lockfile mtime: {}",
        findings[0].message
    );
}

#[test]
fn inventory_healthcheck_missing_and_unlocked_reports_not_projected() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    // A lockfile exists but never recorded this skill (it was simply never projected).
    let lockfile_path = tmp.path().join("plugins.lock.json");
    let mut report = ProjectionReport::default();
    let mut registry = ManagedArtifactRegistry::load(&lockfile_path, &mut report);
    registry.mark(MANAGED_SKILL_PATHS, &root.join("some-other-skill"));
    registry.persist(false, &mut report).unwrap();

    let check = SkillsProjectionHealthCheck::with_inventory(
        root,
        vec!["demo-skill".into()],
        Some(lockfile_path),
    );
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].message.contains("not projected / pruned"),
        "missing-and-unlocked must be discriminated as not-projected: {}",
        findings[0].message
    );
}

#[test]
fn inventory_healthcheck_missing_no_lockfile_reports_unknown_not_false_report() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();

    // No lockfile at all (e.g. this machine never ran a projection).
    let check = SkillsProjectionHealthCheck::with_inventory(root, vec!["demo-skill".into()], None);
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].message.contains("unknown"),
        "no lockfile must discriminate as unknown, not a guessed cause: {}",
        findings[0].message
    );
    assert!(
        !findings[0].message.contains("external deletion"),
        "must never claim external deletion without lockfile evidence"
    );
}

// Budget footprint — total description-char accounting against the warn threshold.

fn write_skill_with_description(root: &Path, name: &str, description_len: usize) {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    let description = "x".repeat(description_len);
    fs::write(
        dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\nbody\n"),
    )
    .unwrap();
}

#[test]
fn inventory_healthcheck_budget_warns_at_or_above_threshold() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();
    write_skill_with_description(&root, "big-skill-a", 4_000);
    write_skill_with_description(&root, "big-skill-b", 3_500);

    let check = SkillsProjectionHealthCheck::with_inventory(
        root,
        vec!["big-skill-a".into(), "big-skill-b".into()],
        None,
    );
    let findings = check.check();
    assert!(
        findings
            .iter()
            .any(|f| f.message.contains("description footprint is 7500 chars")),
        "expected a budget warning citing the exact total: {findings:?}"
    );
    assert!(
        findings.iter().any(|f| f.message.contains("$skill-name")),
        "budget warning must note the omitted-skill still-callable escape hatch"
    );
}

#[test]
fn inventory_healthcheck_budget_silent_below_threshold() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();
    write_skill_with_description(&root, "small-skill", 100);

    let check = SkillsProjectionHealthCheck::with_inventory(root, vec!["small-skill".into()], None);
    let findings = check.check();
    assert!(
        !findings.iter().any(|f| f.message.contains("footprint")),
        "must not warn below the threshold: {findings:?}"
    );
}

#[test]
fn inventory_healthcheck_budget_footprint_sums_across_three_skills() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();
    write_skill_with_description(&root, "skill-a", 2_000);
    write_skill_with_description(&root, "skill-b", 2_500);
    write_skill_with_description(&root, "skill-c", 2_600);

    let check = SkillsProjectionHealthCheck::with_inventory(
        root,
        vec!["skill-a".into(), "skill-b".into(), "skill-c".into()],
        None,
    );
    // 2000 + 2500 + 2600 = 7100 >= 7000: proves the sum is accumulated across
    // every item, not just the largest one or a single-item comparison.
    assert!(
        check
            .check()
            .iter()
            .any(|f| f.message.contains("description footprint is 7100 chars")),
        "three-skill sum must be exact"
    );
}

#[test]
fn skills_projection_budget_finding_lists_top_contributors() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().join("skills");
    fs::create_dir_all(&root).unwrap();
    // Cross the 7000-char threshold; the biggest contributor must be named first.
    write_skill_with_description(&root, "huge-skill", 4_000);
    write_skill_with_description(&root, "mid-skill", 2_500);
    write_skill_with_description(&root, "tiny-skill", 800);

    let check = SkillsProjectionHealthCheck::with_inventory(
        root,
        vec!["huge-skill".into(), "mid-skill".into(), "tiny-skill".into()],
        None,
    );
    let msg = check
        .check()
        .into_iter()
        .find(|f| f.message.contains("footprint"))
        .expect("budget warning must fire above the threshold")
        .message;

    assert!(
        msg.contains("Largest contributors:"),
        "must list contributors: {msg}"
    );
    assert!(
        msg.contains("`huge-skill` (4000 chars)"),
        "must name the biggest: {msg}"
    );
    assert!(
        msg.contains("`mid-skill` (2500 chars)"),
        "must name the second: {msg}"
    );
    // Largest-first ordering: huge-skill appears before mid-skill.
    let contributors = msg.split("Largest contributors:").nth(1).unwrap();
    assert!(
        contributors.find("huge-skill").unwrap() < contributors.find("mid-skill").unwrap(),
        "contributors must be ordered largest-first: {msg}"
    );
}

#[test]
fn refresh_prune_ownership_survives_intermediate_persist() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "retained-skill");
    write_command_fixture(&source_root, "sample-command");
    let installed = ProjectionSource {
        root: source_root.clone(),
        id: "installed-source".into(),
        persistent: true,
    };
    let opts = SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![installed.clone()],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    };

    run_update_skills(&opts).unwrap();
    let retained = home.join(".agents").join("skills").join("retained-skill");
    fs::remove_dir_all(&retained).unwrap();
    let skills_report = run_update_skills(&opts).unwrap();
    let commands_report = run_update_commands(&opts).unwrap();
    assert!(
        retained.exists(),
        "installed-owner branch must survive refresh: skills={skills_report:?} commands={commands_report:?}"
    );
    assert!(!skills_report.removed_paths.contains(&retained));
    assert!(!commands_report.removed_paths.contains(&retained));

    let (_temp, repo_root, source_root, home) = fixture_roots();
    fs::write(
        source_root.join("agents").join("golem-architect.agent.md"),
        "---\nname: golem-architect\ndescription: architect\ncolor: green\ntools: [read]\n---\nBody\n",
    )
    .unwrap();
    write_command_fixture(&source_root, "sample-command");
    let first_opts = SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "removed-source".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&first_opts).unwrap();
    fs::remove_file(source_root.join("agents").join("golem-architect.agent.md")).unwrap();

    let active_opts = SkillUpdateOptions {
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "active-source".into(),
            persistent: true,
        }],
        selected_runtimes: vec!["opencode".into()],
        ..first_opts
    };
    run_update_skills(&active_opts).unwrap();
    let retained_discuss = home
        .join(".agents")
        .join("skills")
        .join("discuss-golem-architect");
    let lockfile = home.join(".gal").join("state").join("plugins.lock.json");
    let after_skills = projection_meta(&lockfile);
    assert!(
        retained_discuss.exists(),
        "unauthorized branch must retain the directory"
    );
    assert_eq!(
        after_skills["sourceAttribution"][ownership_path(&retained_discuss.join("SKILL.md"))],
        "removed-source"
    );

    let commands_report = run_update_commands(&active_opts).unwrap();
    assert!(
        retained_discuss.exists(),
        "intermediate persist must preserve attribution"
    );
    assert!(!commands_report.removed_paths.contains(&retained_discuss));
    let after_commands = projection_meta(&lockfile);
    assert_eq!(
        after_commands["sourceAttribution"][ownership_path(&retained_discuss.join("SKILL.md"))],
        "removed-source"
    );
}

#[test]
fn refresh_prune_ownership_prunes_removed_source_once() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "removed-skill");
    write_command_fixture(&source_root, "sample-command");
    let initial = SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "removed-source".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&initial).unwrap();
    let target = home.join(".agents").join("skills").join("removed-skill");
    fs::remove_dir_all(source_root.join("skills").join("removed-skill")).unwrap();

    let active = SkillUpdateOptions {
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "active-source".into(),
            persistent: true,
        }],
        ..initial
    };
    let skills_report = run_update_skills(&active).unwrap();
    let commands_report = run_update_commands(&active).unwrap();
    assert_eq!(
        skills_report
            .removed_paths
            .iter()
            .filter(|path| *path == &target)
            .count(),
        1
    );
    assert!(!commands_report.removed_paths.contains(&target));
    assert!(!target.exists());
}

#[test]
fn refresh_prune_ownership_dry_run_writes_nothing() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "dry-run-skill");
    write_command_fixture(&source_root, "sample-command");
    let mut opts = SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&opts).unwrap();
    let lockfile = home.join(".gal").join("state").join("plugins.lock.json");
    let before_lockfile = fs::read(&lockfile).unwrap();
    let skill_target = home.join(".agents").join("skills").join("dry-run-skill");
    fs::remove_dir_all(source_root.join("skills").join("dry-run-skill")).unwrap();
    let before_tree = fs::read_dir(home.join(".agents").join("skills"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    opts.dry_run = true;

    run_update_skills(&opts).unwrap();
    run_update_commands(&opts).unwrap();
    let after_tree = fs::read_dir(home.join(".agents").join("skills"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(before_tree, after_tree);
    assert!(skill_target.exists());
    assert_eq!(before_lockfile, fs::read(&lockfile).unwrap());
}

#[test]
fn refresh_prune_ownership_second_run_is_idempotent() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "stable-skill");
    write_command_fixture(&source_root, "sample-command");
    let opts = SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root,
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home,
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    };
    run_update_skills(&opts).unwrap();
    run_update_commands(&opts).unwrap();
    let skills_report = run_update_skills(&opts).unwrap();
    let commands_report = run_update_commands(&opts).unwrap();
    assert!(skills_report.written_files.is_empty());
    assert!(skills_report.removed_paths.is_empty());
    assert!(commands_report.written_files.is_empty());
    assert!(commands_report.removed_paths.is_empty());
}

// update_commands zombie cleanup — GAL-owned zombie removed, user-owned +
// active untouched (the create/delete-counterpart gap materialize_skill_dir's
// "gal-init" incident exposed).

#[test]
fn update_commands_zombie_cleanup_removes_gal_owned_keeps_user_owned_and_active() {
    let (_temp, repo_root, source_root, home) = fixture_roots();
    write_skill_fixture(&source_root, "old-skill");
    write_command_fixture(&source_root, "sample-command");

    let opts = SkillUpdateOptions {
        repo_root,
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    };

    // First projection materializes "old-skill" and lockfile-attributes it.
    run_update_skills(&opts).unwrap();
    let shared = home.join(".agents").join("skills");
    assert!(shared.join("old-skill").is_dir());

    // A genuine user directory occupies a different name in the same shared root.
    let user_dir = shared.join("user-owned");
    fs::create_dir_all(&user_dir).unwrap();
    fs::write(user_dir.join("SKILL.md"), "not GAL content\n").unwrap();

    // "old-skill" retired from source between runs — nothing re-runs
    // update_skills's own prune, so only update_commands's zombie sweep can
    // catch it.
    fs::remove_dir_all(source_root.join("skills").join("old-skill")).unwrap();

    run_update_commands(&opts).unwrap();

    assert!(
        !shared.join("old-skill").exists(),
        "a GAL-owned zombie (retired from source, still lockfile-attributed) must be swept"
    );
    assert!(
        user_dir.exists(),
        "a user-owned directory must never be touched"
    );
    assert!(
        shared.join("sample-command").join("SKILL.md").is_file(),
        "the active command-skill this same run projected must remain"
    );
}

// CodexSkillsConfigCheck — [[skills.config]] enabled=false disable-scan.

#[test]
fn codex_skills_config_check_flags_folder_shape_disable() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        "[[skills.config]]\npath = \"/home/user/.agents/skills/adversarial-review\"\nenabled = false\n",
    )
    .unwrap();

    let check = CodexSkillsConfigCheck::new(config_path, vec!["adversarial-review".into()]);
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("adversarial-review"));
    assert!(findings[0].message.contains("enabled = false"));
}

#[test]
fn codex_skills_config_check_flags_skill_file_shape_disable() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        "[[skills.config]]\npath = \"/home/user/.agents/skills/doc-sync/SKILL.md\"\nenabled = false\n",
    )
    .unwrap();

    let check = CodexSkillsConfigCheck::new(config_path, vec!["doc-sync".into()]);
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("doc-sync"));
}

#[test]
fn codex_skills_config_check_invalid_toml_warns() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(&config_path, "this is [ not valid toml").unwrap();

    let check = CodexSkillsConfigCheck::new(config_path, vec!["adversarial-review".into()]);
    let findings = check.check();
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].severity,
        gal_foundation::health::Severity::Warning
    );
}

#[test]
fn codex_skills_config_check_no_file_no_finding() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml"); // never written

    let check = CodexSkillsConfigCheck::new(config_path, vec!["adversarial-review".into()]);
    assert!(check.check().is_empty());
}

#[test]
fn codex_skills_config_check_enabled_entry_is_silent() {
    let tmp = TempDir::new().unwrap();
    let config_path = tmp.path().join("config.toml");
    fs::write(
        &config_path,
        "[[skills.config]]\npath = \"/home/user/.agents/skills/adversarial-review\"\nenabled = true\n",
    )
    .unwrap();

    let check = CodexSkillsConfigCheck::new(config_path, vec!["adversarial-review".into()]);
    assert!(check.check().is_empty());
}

#[test]
fn codex_config_check_reports_memory_and_mcp_context() {
    let tmp = TempDir::new().unwrap();

    // Enabled memory + configured MCP servers → two advisory findings.
    let enabled_path = tmp.path().join("enabled.toml");
    fs::write(
        &enabled_path,
        "[memory]\nenabled = true\n\n[mcp_servers.codebase-memory]\ncommand = \"x\"\n\n[mcp_servers.playwright]\ncommand = \"y\"\n",
    )
    .unwrap();
    let findings =
        CodexSkillsConfigCheck::new(enabled_path, vec!["adversarial-review".into()]).check();
    let memory = findings
        .iter()
        .find(|f| f.message.contains("Codex Memories are enabled"))
        .expect("enabled memory must produce an advisory");
    assert!(
        memory.message.contains("NOT GAL file memory") && memory.message.contains("advisory only"),
        "memory advisory must distinguish Codex Memories from GAL authority: {}",
        memory.message
    );
    let mcp = findings
        .iter()
        .find(|f| f.message.contains("[mcp_servers]"))
        .expect("configured MCP servers must be reported");
    assert!(mcp.message.contains("codebase-memory") && mcp.message.contains("playwright"));
    assert!(
        mcp.message
            .contains("not the same as its graph/tools being exposed"),
        "mcp advisory must capability-gate: {}",
        mcp.message
    );

    // Disabled memory → NOT blamed (no memory finding at all).
    let disabled_path = tmp.path().join("disabled.toml");
    fs::write(&disabled_path, "[memory]\nenabled = false\n").unwrap();
    let disabled = CodexSkillsConfigCheck::new(disabled_path, vec![]).check();
    assert!(
        !disabled
            .iter()
            .any(|f| f.message.contains("Codex Memories")),
        "a disabled memory must never be reported as a workflow-authority problem: {disabled:?}"
    );
}

// ── resolve_codex_routing reads the planning group + retired-flat diagnostic ──

#[test]
fn resolve_codex_routing_reads_planning_group() {
    let cfg = Some(serde_json::json!({
        "executorRouting": {
            "executors": { "codex": "gpt-5.4-mini" },
            "planning": {
                "ARCHITECT": { "executor": "codex", "model": "gpt-5.4", "effort": "high" }
            }
        }
    }));
    let (model, effort) = resolve_codex_routing(&cfg, "ARCHITECT");
    assert_eq!(model, "gpt-5.4");
    assert_eq!(effort.as_deref(), Some("high"));
}

#[test]
fn resolve_codex_routing_planning_model_from_executor_default() {
    let cfg = Some(serde_json::json!({
        "executorRouting": {
            "executors": { "codex": "gpt-5.4-mini" },
            "planning": { "ANALYST": { "executor": "codex" } }
        }
    }));
    let (model, effort) = resolve_codex_routing(&cfg, "ANALYST");
    assert_eq!(model, "gpt-5.4-mini");
    assert!(effort.is_none());
}

#[test]
fn resolve_codex_routing_absent_role_falls_back() {
    let cfg = Some(serde_json::json!({
        "executorRouting": { "planning": { "ARCHITECT": { "executor": "codex", "model": "gpt-5.4" } } }
    }));
    // DESIGNER is not configured → default fallback, no effort.
    let (model, effort) = resolve_codex_routing(&cfg, "DESIGNER");
    assert_eq!(model, "gpt-5.4-mini");
    assert!(effort.is_none());
}

#[test]
fn resolve_codex_routing_ignores_flat_top_level_role() {
    // A flat top-level ARCHITECT (retired shape, no planning group) is NOT read — the
    // resolver falls back to the default. The retired-flat diagnostic (below) is what
    // tells the user why.
    let cfg = Some(serde_json::json!({
        "executorRouting": { "ARCHITECT": { "executor": "codex", "model": "gpt-5.4" } }
    }));
    let (model, _effort) = resolve_codex_routing(&cfg, "ARCHITECT");
    assert_eq!(
        model, "gpt-5.4-mini",
        "flat top-level role must not be read"
    );
}

#[test]
fn retired_flat_warning_fires_on_flat_consult_role() {
    let cfg = Some(serde_json::json!({
        "executorRouting": {
            "ARCHITECT": { "executor": "codex", "model": "gpt-5.4" },
            "ANALYST": { "executor": "codex" }
        }
    }));
    let w = retired_flat_planning_warning(&cfg).expect("flat consult roles must warn");
    assert!(
        w.contains("ARCHITECT"),
        "warning should name the stray role(s): {w}"
    );
    assert!(
        w.contains("planning"),
        "warning should point at the planning group: {w}"
    );
}

#[test]
fn retired_flat_warning_none_when_planning_present() {
    let cfg = Some(serde_json::json!({
        "executorRouting": {
            "planning": { "ARCHITECT": { "executor": "codex" } }
        }
    }));
    assert!(retired_flat_planning_warning(&cfg).is_none());
}

#[test]
fn retired_flat_warning_none_when_no_consult_key() {
    // Only pipeline roles at the top level (also retired, but not a consult concern) → no
    // consult-specific diagnostic here.
    let cfg = Some(serde_json::json!({
        "executorRouting": { "pipeline": { "CODER": { "executor": "claude" } } }
    }));
    assert!(retired_flat_planning_warning(&cfg).is_none());
}

#[test]
fn retired_flat_warning_none_when_no_routing() {
    assert!(retired_flat_planning_warning(&None).is_none());
    assert!(retired_flat_planning_warning(&Some(serde_json::json!({}))).is_none());
}

// ─── OpenCodeProjectionHealthCheck tests ───

/// Helper: create a minimal lockfile with OpenCode agent and command paths.
fn write_opencode_lockfile(lockfile_path: &Path, agent_paths: &[&str], command_paths: &[&str]) {
    let mut projection = serde_json::Map::new();
    projection.insert(
        MANAGED_OPENCODE_AGENT_PATHS.to_string(),
        Value::Array(
            agent_paths
                .iter()
                .map(|p| Value::String(p.to_string()))
                .collect(),
        ),
    );
    projection.insert(
        MANAGED_COMMAND_PATHS.to_string(),
        Value::Array(
            command_paths
                .iter()
                .map(|p| Value::String(p.to_string()))
                .collect(),
        ),
    );
    let root = serde_json::json!({
        "_galProjection": projection,
        "schemaVersion": 1
    });
    fs::write(lockfile_path, serde_json::to_string_pretty(&root).unwrap()).unwrap();
}

/// Helper: create a canonical root with a command and an agent.
fn write_canonical_root(
    canonical: &Path,
    command_name: &str,
    agent_name: &str,
    agent_color: Option<&str>,
) {
    // Command: canonical root's commands/<name>/SKILL.md
    let cmd_dir = canonical.join("commands").join(command_name);
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(
        cmd_dir.join("SKILL.md"),
        format!(
            "---\nname: {command_name}\ndescription: Test command\n---\nCommand body for {command_name}\n"
        ),
    )
    .unwrap();

    // Agent: canonical root's agy-agents/<name>.agent.md (verbatim copy)
    let agents_dir = canonical.join("agy-agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(canonical.join("commands")).unwrap();
    let color_line = agent_color
        .map(|c| format!("color: {c}\n"))
        .unwrap_or_default();
    fs::write(
        agents_dir.join(format!("{agent_name}.agent.md")),
        format!(
            "---\nname: {agent_name}\ndescription: Test agent\n{color_line}tools: [\"read\", \"edit\"]\n---\nAgent body for {agent_name}\n"
        ),
    )
    .unwrap();
}

/// Stale warning when on-disk command content differs from re-rendered.
#[test]
fn test_opencode_health_check_stale() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("lock.json");

    let cmd_name = "gal-pipeline";
    let agent_name = "golem-analyst";
    let cmd_path = opencode_root
        .join("commands")
        .join(format!("{cmd_name}.md"));
    let agent_path = opencode_root
        .join("agents")
        .join(format!("{agent_name}.md"));

    // Set up canonical root
    write_canonical_root(&canonical, cmd_name, agent_name, Some("blue"));

    // Write STALE on-disk projection (different body)
    fs::create_dir_all(opencode_root.join("commands")).unwrap();
    fs::create_dir_all(opencode_root.join("agents")).unwrap();
    fs::write(
        &cmd_path,
        "stale command body that differs from canonical\n",
    )
    .unwrap();

    // Write FRESH agent projection (matches canonical)
    let agent_frontmatter = Frontmatter {
        name: Some(agent_name.to_string()),
        description: Some("Test agent".to_string()),
        color: Some("blue".to_string()),
        tools: vec!["read".to_string(), "edit".to_string()],
    };
    let agent_body = format!("Agent body for {agent_name}");
    let expected_agent = render_opencode_agent(agent_name, &agent_frontmatter, &agent_body);
    fs::write(&agent_path, &expected_agent).unwrap();

    // Write lockfile
    write_opencode_lockfile(
        &lockfile,
        &[agent_path.to_str().unwrap()],
        &[cmd_path.to_str().unwrap()],
    );

    let check = OpenCodeProjectionHealthCheck::new(
        opencode_root.clone(),
        Some(lockfile.clone()),
        canonical.clone(),
    );
    let findings = check.check();

    // Should have exactly one stale warning for the command
    let stale_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.message.contains("differs from canonical source"))
        .collect();
    assert_eq!(
        stale_findings.len(),
        1,
        "expected exactly one stale finding"
    );
    assert!(stale_findings[0].message.contains(cmd_name));
    assert_eq!(
        stale_findings[0].severity,
        gal_foundation::health::Severity::Warning
    );
}

/// Missing error when projected agent file doesn't exist.
#[test]
fn test_opencode_health_check_missing() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("lock.json");

    let agent_name = "golem-analyst";
    let agent_path = opencode_root
        .join("agents")
        .join(format!("{agent_name}.md"));

    // Set up canonical root
    write_canonical_root(&canonical, "unused", agent_name, Some("blue"));

    // Do NOT write the agent file — it's missing

    // Write lockfile pointing to the missing agent
    write_opencode_lockfile(&lockfile, &[agent_path.to_str().unwrap()], &[]);

    let check = OpenCodeProjectionHealthCheck::new(
        opencode_root.clone(),
        Some(lockfile.clone()),
        canonical.clone(),
    );
    let findings = check.check();

    // Should have exactly one missing error for the agent
    let missing_findings: Vec<_> = findings
        .iter()
        .filter(|f| f.message.contains("agent 'golem-analyst' is missing at"))
        .collect();
    assert_eq!(
        missing_findings.len(),
        1,
        "expected exactly one missing finding"
    );
    assert!(missing_findings[0].message.contains(agent_name));
    assert_eq!(
        missing_findings[0].severity,
        gal_foundation::health::Severity::Error
    );
}

/// Clean projection returns no findings, with agent that has `color:` in frontmatter.
#[test]
fn test_opencode_health_check_clean() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("lock.json");

    let cmd_name = "gal-status";
    let agent_name = "golem-architect";
    let cmd_path = opencode_root
        .join("commands")
        .join(format!("{cmd_name}.md"));
    let agent_path = opencode_root
        .join("agents")
        .join(format!("{agent_name}.md"));

    // Set up canonical root with color
    write_canonical_root(&canonical, cmd_name, agent_name, Some("purple"));

    // Write FRESH on-disk projections
    fs::create_dir_all(opencode_root.join("commands")).unwrap();
    fs::create_dir_all(opencode_root.join("agents")).unwrap();

    // Command
    let cmd_frontmatter = Frontmatter {
        name: Some(cmd_name.to_string()),
        description: Some("Test command".to_string()),
        color: None,
        tools: Vec::new(),
    };
    let cmd_body = format!("Command body for {cmd_name}");
    let expected_cmd = render_opencode_command(cmd_name, &cmd_frontmatter, &cmd_body);
    fs::write(&cmd_path, &expected_cmd).unwrap();

    // Agent with color
    let agent_frontmatter = Frontmatter {
        name: Some(agent_name.to_string()),
        description: Some("Test agent".to_string()),
        color: Some("purple".to_string()),
        tools: vec!["read".to_string(), "edit".to_string()],
    };
    let agent_body = format!("Agent body for {agent_name}");
    let expected_agent = render_opencode_agent(agent_name, &agent_frontmatter, &agent_body);
    fs::write(&agent_path, &expected_agent).unwrap();

    // Write lockfile
    write_opencode_lockfile(
        &lockfile,
        &[agent_path.to_str().unwrap()],
        &[cmd_path.to_str().unwrap()],
    );

    let check = OpenCodeProjectionHealthCheck::new(
        opencode_root.clone(),
        Some(lockfile.clone()),
        canonical.clone(),
    );
    let findings = check.check();

    // Should have no findings
    assert!(
        findings.is_empty(),
        "clean projection should have no findings, got: {:?}",
        findings
    );
}

/// Graceful handling of missing lockfile.
#[test]
fn test_opencode_health_check_no_lockfile() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("nonexistent-lock.json");

    // Set up canonical root
    write_canonical_root(&canonical, "unused", "unused", None);

    // Do NOT write the lockfile

    let check = OpenCodeProjectionHealthCheck::new(
        opencode_root.clone(),
        Some(lockfile.clone()),
        canonical.clone(),
    );
    let findings = check.check();

    // Should have no findings (graceful skip)
    assert!(
        findings.is_empty(),
        "missing lockfile should result in no findings, got: {:?}",
        findings
    );
}

/// Lowercased Windows-style lockfile command path still matches prefix filter.
#[test]
fn test_opencode_health_check_windows_lowercase_path() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("lock.json");

    let cmd_name = "gal-pipeline";
    let cmd_path = opencode_root
        .join("commands")
        .join(format!("{cmd_name}.md"));

    // Set up canonical root
    write_canonical_root(&canonical, cmd_name, "unused", None);

    // Write FRESH on-disk projection
    fs::create_dir_all(opencode_root.join("commands")).unwrap();
    let cmd_frontmatter = Frontmatter {
        name: Some(cmd_name.to_string()),
        description: Some("Test command".to_string()),
        color: None,
        tools: Vec::new(),
    };
    let cmd_body = format!("Command body for {cmd_name}");
    let expected_cmd = render_opencode_command(cmd_name, &cmd_frontmatter, &cmd_body);
    fs::write(&cmd_path, &expected_cmd).unwrap();

    // Write lockfile with LOWERCASED path (simulating Windows)
    let lowercased_path = cmd_path.to_str().unwrap().to_ascii_lowercase();
    write_opencode_lockfile(&lockfile, &[], &[&lowercased_path]);

    let check = OpenCodeProjectionHealthCheck::new(
        opencode_root.clone(),
        Some(lockfile.clone()),
        canonical.clone(),
    );
    let findings = check.check();

    // The command itself must match despite case differences. The helper also
    // creates an intentionally unprojected agent, which canonical inventory
    // correctly reports independently.
    assert!(
        !findings.iter().any(|finding| finding
            .message
            .contains("command 'gal-pipeline' is absent from projection lockfile inventory")),
        "lowercased Windows command path should match, got: {:?}",
        findings
    );
}

/// Check reads agents from `agy-agents` verbatim copy, not filtered `agents` copy.
/// Given a canonical root where the filtered `agents` copy has `color:` stripped,
/// while `agy-agents` copy retains it, a clean projection still yields no finding.
#[test]
fn test_opencode_health_check_uses_agy_agents_not_filtered() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("lock.json");

    let agent_name = "golem-designer";
    let agent_path = opencode_root
        .join("agents")
        .join(format!("{agent_name}.md"));

    // Set up canonical root with agy-agents (verbatim, has color)
    let agents_dir = canonical.join("agy-agents");
    fs::create_dir_all(&agents_dir).unwrap();
    fs::create_dir_all(canonical.join("commands")).unwrap();
    fs::write(
        agents_dir.join(format!("{agent_name}.agent.md")),
        format!(
            "---\nname: {agent_name}\ndescription: Test agent\ncolor: orange\ntools: [\"read\"]\n---\nAgent body\n"
        ),
    )
    .unwrap();

    // Also create a filtered `agents` copy WITHOUT color (simulating filter_agent_for_claude)
    let filtered_agents_dir = canonical.join("agents");
    fs::create_dir_all(&filtered_agents_dir).unwrap();
    fs::write(
        filtered_agents_dir.join(format!("{agent_name}.agent.md")),
        format!(
            "---\nname: {agent_name}\ndescription: Test agent\ntools: [\"read\"]\n---\nAgent body\n"
        ),
    )
    .unwrap();

    // Write FRESH on-disk projection (with color, matching agy-agents)
    fs::create_dir_all(opencode_root.join("agents")).unwrap();
    let agent_frontmatter = Frontmatter {
        name: Some(agent_name.to_string()),
        description: Some("Test agent".to_string()),
        color: Some("orange".to_string()),
        tools: vec!["read".to_string()],
    };
    let expected_agent = render_opencode_agent(agent_name, &agent_frontmatter, "Agent body");
    fs::write(&agent_path, &expected_agent).unwrap();

    // Write lockfile
    write_opencode_lockfile(&lockfile, &[agent_path.to_str().unwrap()], &[]);

    let check = OpenCodeProjectionHealthCheck::new(
        opencode_root.clone(),
        Some(lockfile.clone()),
        canonical.clone(),
    );
    let findings = check.check();

    // Should have no findings — the check uses agy-agents (with color), not filtered agents
    assert!(
        findings.is_empty(),
        "check should use agy-agents verbatim copy (with color), not filtered agents copy, got: {:?}",
        findings
    );
}

/// Full refresh with opencode selected writes commands and agents files
/// and records each in the lockfile under the correct categories with sourceAttribution.
#[test]
fn opencode_projection_full_refresh_records_lockfile() {
    let (_temp, repo_root, source_root, home) = fixture_roots();

    // Write agent fixture
    write_agent_fixture(&source_root, "helper", "green");

    // Write command fixture
    let cmd_dir = source_root.join("commands").join("test-cmd");
    fs::create_dir_all(&cmd_dir).unwrap();
    fs::write(
        cmd_dir.join("SKILL.md"),
        "---\nname: test-cmd\ndescription: Test command\n---\nTest body\n",
    )
    .unwrap();

    // Run full refresh with opencode selected (both skills and commands)
    let _skill_result = run_update_skills(&SkillUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    let _cmd_result = run_update_commands(&CommandUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["opencode".into()],
        dry_run: false,
        replace: true,
    })
    .unwrap();

    // Verify agent file was written
    let opencode_agents = home.join(".config").join("opencode").join("agents");
    let agent_file = opencode_agents.join("helper.md");
    assert!(agent_file.exists(), "agent file must be written");
    let agent_content = fs::read_to_string(&agent_file).unwrap();
    assert!(agent_content.starts_with(GAL_MANAGED_FILE_HEADER));

    // Verify command file was written
    let opencode_commands = home.join(".config").join("opencode").join("commands");
    let cmd_file = opencode_commands.join("test-cmd.md");
    assert!(cmd_file.exists(), "command file must be written");
    let cmd_content = fs::read_to_string(&cmd_file).unwrap();
    assert!(cmd_content.starts_with(GAL_MANAGED_FILE_HEADER));

    // Verify lockfile records the paths
    let lockfile = home.join(".gal").join("state").join("plugins.lock.json");
    assert!(lockfile.exists(), "lockfile must be written");

    let meta = projection_meta(&lockfile);

    // Check opencodeAgentProjectionPaths
    let agent_paths = meta["opencodeAgentProjectionPaths"].as_array().unwrap();
    assert!(
        agent_paths
            .iter()
            .any(|p| p.as_str() == Some(ownership_path(&agent_file).as_str())),
        "lockfile must record agent path in opencodeAgentProjectionPaths"
    );

    // Check commandProjectionPaths includes the opencode command
    let cmd_paths = meta["commandProjectionPaths"].as_array().unwrap();
    assert!(
        cmd_paths
            .iter()
            .any(|p| p.as_str() == Some(ownership_path(&cmd_file).as_str())),
        "lockfile must record command path in commandProjectionPaths"
    );

    // Check sourceAttribution
    let attribution = meta["sourceAttribution"].as_object().unwrap();
    assert!(
        attribution.contains_key(ownership_path(&agent_file).as_str()),
        "lockfile must have sourceAttribution for agent"
    );
    assert_eq!(
        attribution
            .get(ownership_path(&cmd_file).as_str())
            .and_then(Value::as_str),
        Some("gal-core"),
        "OpenCode command attribution must identify its projection source"
    );
}

/// Prune coverage: lockfile-attributed OpenCode skills root is removed whole.
#[test]
fn opencode_skills_root_pruned_when_lockfile_attributed() {
    use crate::support::{remove_if_gal_owned_dir, ManagedArtifactRegistry};

    let tmp = TempDir::new().unwrap();
    let opencode_skills = tmp.path().join(".config").join("opencode").join("skills");
    let lockfile = tmp.path().join("plugins.lock.json");

    // Create the orphan directory
    fs::create_dir_all(&opencode_skills).unwrap();
    fs::write(opencode_skills.join("test.txt"), "orphan content").unwrap();

    // Write lockfile with the path recorded
    let initial = serde_json::json!({
        "_galProjection": {
            "legacyProjectionPaths": [ownership_path(&opencode_skills)]
        }
    });
    fs::write(&lockfile, serde_json::to_string(&initial).unwrap()).unwrap();

    let mut report = ProjectionReport::default();
    let registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    // Verify the directory exists before prune
    assert!(
        opencode_skills.exists(),
        "directory must exist before prune"
    );

    // Run prune
    remove_if_gal_owned_dir(&opencode_skills, false, &mut report, &registry).unwrap();

    // Verify the directory was removed
    assert!(
        !opencode_skills.exists(),
        "lockfile-attributed directory must be removed"
    );
    assert!(
        report.removed_paths.contains(&opencode_skills),
        "removed_paths must record the pruned directory"
    );
}

/// Prune coverage: unattributed OpenCode skills root survives untouched (can_mutate fail-safe).
#[test]
fn opencode_skills_root_survives_when_unattributed() {
    use crate::support::{remove_if_gal_owned_dir, ManagedArtifactRegistry};

    let tmp = TempDir::new().unwrap();
    let opencode_skills = tmp.path().join(".config").join("opencode").join("skills");
    let lockfile = tmp.path().join("plugins.lock.json");

    // Create the orphan directory
    fs::create_dir_all(&opencode_skills).unwrap();
    fs::write(opencode_skills.join("test.txt"), "orphan content").unwrap();

    // Write lockfile WITHOUT the path recorded (simulating pre-attribution state)
    let initial = serde_json::json!({
        "_galProjection": {
            "legacyProjectionPaths": []
        }
    });
    fs::write(&lockfile, serde_json::to_string(&initial).unwrap()).unwrap();

    let mut report = ProjectionReport::default();
    let registry = ManagedArtifactRegistry::load(&lockfile, &mut report);

    // Verify the directory exists before prune
    assert!(
        opencode_skills.exists(),
        "directory must exist before prune"
    );

    // Run prune
    remove_if_gal_owned_dir(&opencode_skills, false, &mut report, &registry).unwrap();

    // Verify the directory survived (can_mutate fail-safe)
    assert!(
        opencode_skills.exists(),
        "unattributed directory must survive prune"
    );
    assert!(
        !report.removed_paths.contains(&opencode_skills),
        "removed_paths must not record the surviving directory"
    );
    // Verify the content is still there
    assert!(
        opencode_skills.join("test.txt").exists(),
        "orphan content must be preserved"
    );
}

#[test]
fn opencode_skills_root_survives_without_lockfile() {
    use crate::support::{remove_if_gal_owned_dir, ManagedArtifactRegistry};

    let tmp = TempDir::new().unwrap();
    let skills = tmp.path().join(".config").join("opencode").join("skills");
    let lockfile = tmp.path().join("missing-lock.json");
    fs::create_dir_all(&skills).unwrap();
    fs::write(skills.join("user-skill.txt"), "owned by user").unwrap();

    let mut report = ProjectionReport::default();
    let registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    remove_if_gal_owned_dir(&skills, false, &mut report, &registry).unwrap();

    assert!(skills.join("user-skill.txt").is_file());
    assert!(!report.removed_paths.contains(&skills));
}

#[test]
fn opencode_skills_root_survives_malformed_lockfile() {
    use crate::support::{remove_if_gal_owned_dir, ManagedArtifactRegistry};

    let tmp = TempDir::new().unwrap();
    let skills = tmp.path().join(".config").join("opencode").join("skills");
    let lockfile = tmp.path().join("plugins.lock.json");
    fs::create_dir_all(&skills).unwrap();
    fs::write(skills.join("user-skill.txt"), "owned by user").unwrap();
    fs::write(&lockfile, "{not json").unwrap();

    let mut report = ProjectionReport::default();
    let registry = ManagedArtifactRegistry::load(&lockfile, &mut report);
    remove_if_gal_owned_dir(&skills, false, &mut report, &registry).unwrap();

    assert!(skills.join("user-skill.txt").is_file());
    assert!(!report.removed_paths.contains(&skills));
}

#[test]
fn opencode_health_check_uses_canonical_inventory_not_incomplete_lockfile() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("lock.json");
    write_canonical_root(&canonical, "gal-status", "golem-architect", Some("purple"));

    let attributed_agent = opencode_root.join("agents").join("retired.md");
    write_opencode_lockfile(&lockfile, &[attributed_agent.to_str().unwrap()], &[]);

    let findings =
        OpenCodeProjectionHealthCheck::new(opencode_root, Some(lockfile), canonical).check();
    assert!(findings.iter().any(|finding| finding.message.contains(
        "OpenCode agent 'golem-architect' is absent from projection lockfile inventory"
    )));
    assert!(findings.iter().any(|finding| finding
        .message
        .contains("OpenCode command 'gal-status' is absent from projection lockfile inventory")));
    assert!(findings.iter().any(|finding| finding
        .message
        .contains("OpenCode agent 'retired' has lockfile attribution but no canonical source")));
}

#[test]
fn opencode_health_check_handles_windows_paths_without_using_them_as_host_paths() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let canonical = tmp.path().join("canonical");
    let lockfile = tmp.path().join("lock.json");
    write_canonical_root(&canonical, "gal-status", "golem-architect", Some("purple"));

    let command_path = opencode_root.join("commands").join("gal-status.md");
    fs::create_dir_all(command_path.parent().unwrap()).unwrap();
    let source = fs::read_to_string(canonical.join("commands/gal-status/SKILL.md")).unwrap();
    let frontmatter = parse_frontmatter(&source);
    fs::write(
        &command_path,
        render_opencode_command("gal-status", &frontmatter, &strip_frontmatter(&source)),
    )
    .unwrap();
    let windows_path = command_path.to_string_lossy().replace('/', "\\");
    write_opencode_lockfile(&lockfile, &[], &[&windows_path]);

    let findings =
        OpenCodeProjectionHealthCheck::new(opencode_root, Some(lockfile), canonical).check();
    assert!(!findings.iter().any(|finding| finding
        .message
        .contains("command 'gal-status' is absent from projection lockfile inventory")));
}

#[test]
fn opencode_health_check_reports_malformed_lockfile() {
    let tmp = TempDir::new().unwrap();
    let lockfile = tmp.path().join("lock.json");
    fs::write(&lockfile, "{not json").unwrap();

    let findings = OpenCodeProjectionHealthCheck::new(
        tmp.path().join("opencode"),
        Some(lockfile),
        tmp.path().join("canonical"),
    )
    .check();
    assert!(findings
        .iter()
        .any(|finding| finding.message.contains("lockfile is malformed")));
}

#[test]
fn opencode_health_check_reports_missing_canonical_inventory() {
    let tmp = TempDir::new().unwrap();
    let opencode_root = tmp.path().join("opencode");
    let lockfile = tmp.path().join("lock.json");
    let attributed_command = opencode_root.join("commands").join("gal-status.md");
    write_opencode_lockfile(
        &lockfile,
        &[],
        &[attributed_command.to_string_lossy().as_ref()],
    );

    let findings = OpenCodeProjectionHealthCheck::new(
        opencode_root,
        Some(lockfile),
        tmp.path().join("missing-canonical"),
    )
    .check();

    assert!(findings.iter().any(|finding| finding
        .message
        .contains("canonical OpenCode agent directory is missing")));
    assert!(findings.iter().any(|finding| finding
        .message
        .contains("canonical OpenCode command directory is missing")));
}

#[test]
fn codex_pipeline_handback_contract_is_projected_byte_identically() {
    let temp = TempDir::new().unwrap();
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let source_root = repo_root.join("plugins").join("gal-core");
    let home = temp.path().join("home");
    let options = CommandUpdateOptions {
        repo_root: repo_root.clone(),
        sources: vec![ProjectionSource {
            root: source_root.clone(),
            id: "gal-core".into(),
            persistent: true,
        }],
        user_home: home.clone(),
        selected_runtimes: vec!["codex".into()],
        dry_run: false,
        replace: true,
    };

    let first = run_update_commands(&options).unwrap();
    let canonical = home.join(".gal/plugins/gal/commands/gal-pipeline/SKILL.md");
    let projected = home.join(".agents/skills/gal-pipeline/SKILL.md");
    assert!(first.written_files.contains(&canonical));
    assert!(first.written_files.contains(&projected));
    assert_eq!(fs::read(&canonical).unwrap(), fs::read(&projected).unwrap());

    let content = fs::read_to_string(&projected).unwrap();
    for required in [
        "gal.exe pipeline-handback-check",
        "(decision, reason, final_authorized, next_action)",
        "run-goal-backward-verification",
        "pending valid future `stop-at`",
        "checker validates freshness and state consistency only",
        "continuous non-empty `must_have_1` through `must_have_N`",
        "`Workflow` is advisory prompt metadata",
        "terminal state `completed`",
        "Pipeline execution is **exempt from working-hours",
        "Runtime cutoff is recovery-only",
        "never authorize final output",
    ] {
        assert!(
            content.contains(required),
            "missing projected clause: {required}"
        );
    }
    for forbidden in [
        "ordinary task progress ... final_authorized: true",
        "Codex has self-wake authority",
        "interruption authorizes final output",
        "model-specific routing is final authority",
    ] {
        assert!(
            !content.contains(forbidden),
            "unexpected authorization clause projected: {forbidden}"
        );
    }

    let second = run_update_commands(&options).unwrap();
    assert!(second.written_files.is_empty());
    assert_eq!(fs::read(&canonical).unwrap(), fs::read(&projected).unwrap());
}
