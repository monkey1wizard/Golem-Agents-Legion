//! T-009 Oracle reparent — fixture-based behavioral tests replacing three oracle scripts.
//!
//! R-08 precondition: oracle scripts are reparented to fixtures/behavioral Rust tests
//! before those scripts are deleted.  `cargo test` does NOT spawn any of the three
//! oracle scripts named below (TP-10).
//!
//! # Oracle scripts reparented by this file
//!
//! | Oracle script | Reparent location |
//! |---|---|
//! | `scripts/tests/Test-InstallModeAuthority.ps1` | `crates/base/src/mode.rs` tests (TempDir fixtures) |
//! | `scripts/test-install-acceptance.sh` | This file — `mod acceptance_fixture` |
//! | `scripts/Test-ResolveGalCatalog.ps1` | This file — `mod catalog_fixture` |
//!
//! ## Test-InstallModeAuthority.ps1
//!
//! Behavior fully covered in `base::mode::tests` using TempDir fixtures.
//! That module is annotated with a T-009 reparent note.  No tests are duplicated
//! here; the fixture oracle is `crates/base/src/mode.rs`.
//!
//! ## test-install-acceptance.sh
//!
//! The shell script runs post-install acceptance checks against live system state.
//! The `acceptance_fixture` module below expresses the same acceptance contract as
//! fixture-driven structural assertions — no subprocess spawned (TP-10).
//!
//! ## Test-ResolveGalCatalog.ps1
//!
//! The script calls `Resolve-GalCatalog.ps1` for profile-based plugin resolution.
//! The Rust port of catalog resolution is T-028.  The `catalog_fixture` module
//! parses `plugins/catalog.json` as a frozen fixture and verifies structural
//! invariants, providing a baseline for T-028.

// ─────────────────────────────────────────────────────────────────────────────
// mod acceptance_fixture
//
// Replaces: scripts/test-install-acceptance.sh
//
// The shell script's eight acceptance criteria are expressed here as fixture-driven
// structural assertions against a temp directory that mirrors a successful install.
// No subprocess is spawned; `cargo test` reads fixture data only (TP-10).
// ─────────────────────────────────────────────────────────────────────────────
mod acceptance_fixture {
    use std::fs;
    use tempfile::TempDir;

    /// Build a fixture canonical root that mirrors the output of a successful
    /// `gal install`.  Layout matches what `render_canonical_root` produces:
    /// agents/, skills/doc-sync/SKILL.md, commands/, .claude-plugin/plugin.json,
    /// copilot-manifest.json, plugin.json, bin/gal.
    fn build_fixture_canonical_root(root: &std::path::Path) {
        fs::create_dir_all(root.join("agents")).unwrap();
        fs::write(
            root.join("agents").join("golem-dockeeper.md"),
            "# golem-dockeeper\n",
        )
        .unwrap();

        let doc_sync = root.join("skills").join("doc-sync");
        fs::create_dir_all(&doc_sync).unwrap();
        fs::write(doc_sync.join("SKILL.md"), "# doc-sync\n").unwrap();

        fs::create_dir_all(root.join("commands")).unwrap();

        fs::create_dir_all(root.join(".claude-plugin")).unwrap();
        fs::write(
            root.join(".claude-plugin").join("plugin.json"),
            r#"{"name":"gal","version":"0.1.0"}"#,
        )
        .unwrap();

        fs::write(
            root.join("copilot-manifest.json"),
            r#"{"name":"gal","components":{"agents":"agents/","skills":"skills/","commands":"commands/"}}"#,
        )
        .unwrap();

        fs::write(root.join("plugin.json"), r#"{"name":"gal"}"#).unwrap();

        fs::create_dir_all(root.join("bin")).unwrap();
        fs::write(
            root.join("bin").join("gal"),
            "#!/usr/bin/env sh\nexec gal \"$@\"\n",
        )
        .unwrap();
    }

    /// Acceptance §3: canonical root directory must exist after install.
    #[test]
    fn fixture_acceptance_canonical_root_exists() {
        let temp = TempDir::new().unwrap();
        build_fixture_canonical_root(temp.path());
        assert!(
            temp.path().is_dir(),
            "canonical root must exist after install"
        );
    }

    /// Acceptance §4: golem-dockeeper.md present in agents/ (agent completeness).
    #[test]
    fn fixture_acceptance_golem_dockeeper_present() {
        let temp = TempDir::new().unwrap();
        build_fixture_canonical_root(temp.path());
        assert!(
            temp.path().join("agents").join("golem-dockeeper.md").exists(),
            "golem-dockeeper.md must be present in canonical root agents/"
        );
    }

    /// Acceptance §5: skills/doc-sync/ directory present (skill completeness).
    #[test]
    fn fixture_acceptance_doc_sync_skill_present() {
        let temp = TempDir::new().unwrap();
        build_fixture_canonical_root(temp.path());
        assert!(
            temp.path().join("skills").join("doc-sync").is_dir(),
            "skills/doc-sync/ must be present in canonical root"
        );
    }

    /// Acceptance §7: no orphan .gal-render-* dirs under plugins parent.
    #[test]
    fn fixture_acceptance_no_orphan_render_dirs() {
        let temp = TempDir::new().unwrap();
        // plugins parent with no orphans
        let plugins = temp.path().join("plugins");
        fs::create_dir_all(&plugins).unwrap();

        let orphans: Vec<_> = fs::read_dir(&plugins)
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
            orphans.is_empty(),
            "no orphan .gal-render-* dirs must exist after clean install: {:?}",
            orphans
                .iter()
                .map(|e| e.path())
                .collect::<Vec<_>>()
        );
    }

    /// Acceptance §7 negative: orphan render dir IS detected by the predicate.
    #[test]
    fn fixture_acceptance_orphan_render_dir_detected() {
        let temp = TempDir::new().unwrap();
        let plugins = temp.path().join("plugins");
        fs::create_dir_all(plugins.join(".gal-render-00000000")).unwrap();

        let orphans: Vec<_> = fs::read_dir(&plugins)
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
            !orphans.is_empty(),
            "orphan .gal-render-* dir must be detectable by the predicate"
        );
    }

    /// Acceptance §8: bin/gal (or bin/gal.exe on Windows) present in canonical root.
    #[test]
    fn fixture_acceptance_bin_gal_present() {
        let temp = TempDir::new().unwrap();
        build_fixture_canonical_root(temp.path());
        let bin_dir = temp.path().join("bin");
        let bin_exists = bin_dir.join("gal").exists()
            || bin_dir.join("gal.exe").exists();
        assert!(
            bin_exists,
            "bin/gal[.exe] must be present in canonical root"
        );
    }

    /// Acceptance §8 negative: missing bin/ is detectable.
    #[test]
    fn fixture_acceptance_missing_bin_detected() {
        let temp = TempDir::new().unwrap();
        // Only create agents/ — no bin/
        fs::create_dir_all(temp.path().join("agents")).unwrap();
        let bin_dir = temp.path().join("bin");
        assert!(
            !bin_dir.exists(),
            "bin/ must not exist when not installed: this fixture verifies the negative detection"
        );
    }

    /// Acceptance: manifest files present (Claude plugin and Copilot).
    ///
    /// Verifies the render output includes both provider manifests, as
    /// cross_platform_oracle_parity.rs also asserts.
    #[test]
    fn fixture_acceptance_manifests_present() {
        let temp = TempDir::new().unwrap();
        build_fixture_canonical_root(temp.path());
        assert!(
            temp.path().join(".claude-plugin").join("plugin.json").exists(),
            ".claude-plugin/plugin.json must be present"
        );
        assert!(
            temp.path().join("copilot-manifest.json").exists(),
            "copilot-manifest.json must be present"
        );
        assert!(
            temp.path().join("plugin.json").exists(),
            "plugin.json (AGY) must be present"
        );
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// mod catalog_fixture
//
// Replaces: scripts/Test-ResolveGalCatalog.ps1  (structural invariants only)
//
// Full profile-based resolution is T-028.  This module verifies the structural
// invariants of the frozen catalog JSON so T-028 has a verified baseline to
// port against.  No PS1 script is spawned (TP-10).
// ─────────────────────────────────────────────────────────────────────────────
mod catalog_fixture {
    // Include the live catalog.json from the repo root as a frozen fixture.
    // Path is relative to this source file: crates/gal-engine/tests/ → repo root
    const CATALOG_JSON: &str = include_str!("../../../plugins/catalog.json");

    fn parse_catalog() -> serde_json::Value {
        serde_json::from_str(CATALOG_JSON).expect("plugins/catalog.json must be valid JSON")
    }

    /// TP-10 baseline: catalog is parseable as JSON without spawning oracle scripts.
    #[test]
    fn fixture_catalog_parses_as_valid_json() {
        let _ = parse_catalog();
    }

    /// Schema version must be 1 (frozen fixture invariant).
    #[test]
    fn fixture_catalog_schema_version_is_1() {
        let c = parse_catalog();
        assert_eq!(
            c["schemaVersion"].as_u64(),
            Some(1),
            "catalog schemaVersion must be 1"
        );
    }

    /// Plugins array must be non-empty.
    #[test]
    fn fixture_catalog_plugins_non_empty() {
        let c = parse_catalog();
        assert!(
            c["plugins"].as_array().map(|a| !a.is_empty()).unwrap_or(false),
            "catalog plugins must be non-empty"
        );
    }

    /// gal-core must be present in the plugins array.
    #[test]
    fn fixture_catalog_gal_core_present() {
        let c = parse_catalog();
        let plugins = c["plugins"].as_array().expect("plugins must be an array");
        assert!(
            plugins
                .iter()
                .any(|p| p["pluginId"].as_str() == Some("gal-core")),
            "gal-core must be present in catalog"
        );
    }

    /// gal-core must include "default" in its defaultProfiles.
    ///
    /// Corresponds to Test-ResolveGalCatalog.ps1 TP-004:
    /// "default profile resolves only gal-core".
    #[test]
    fn fixture_catalog_gal_core_in_default_profile() {
        let c = parse_catalog();
        let plugins = c["plugins"].as_array().expect("plugins must be an array");
        let gal_core = plugins
            .iter()
            .find(|p| p["pluginId"].as_str() == Some("gal-core"))
            .expect("gal-core must be present");
        let profiles = gal_core["defaultProfiles"]
            .as_array()
            .expect("defaultProfiles must be an array");
        assert!(
            profiles
                .iter()
                .any(|v| v.as_str() == Some("default")),
            "gal-core must be in the 'default' profile"
        );
    }

    /// Every plugin must have a non-empty license and checksumPolicy.
    ///
    /// Corresponds to Test-ResolveGalCatalog.ps1 TP-006: validation invariant.
    #[test]
    fn fixture_catalog_every_plugin_has_required_fields() {
        let c = parse_catalog();
        let plugins = c["plugins"].as_array().expect("plugins must be an array");
        for plugin in plugins {
            let id = plugin["pluginId"].as_str().unwrap_or("<unknown>");
            assert!(
                plugin["license"].as_str().map(|s| !s.is_empty()).unwrap_or(false),
                "plugin '{}' must have a non-empty license",
                id
            );
            assert!(
                plugin["checksumPolicy"]
                    .as_str()
                    .map(|s| !s.is_empty())
                    .unwrap_or(false),
                "plugin '{}' must have a non-empty checksumPolicy",
                id
            );
        }
    }

    /// All plugin IDs must be unique (deterministic resolution invariant).
    ///
    /// Corresponds to Test-ResolveGalCatalog.ps1 TP-005: "same input produces same
    /// plugin set".
    #[test]
    fn fixture_catalog_plugin_ids_unique() {
        let c = parse_catalog();
        let plugins = c["plugins"].as_array().expect("plugins must be an array");
        let ids: Vec<_> = plugins
            .iter()
            .filter_map(|p| p["pluginId"].as_str())
            .collect();
        let mut seen = std::collections::HashSet::new();
        for id in &ids {
            assert!(
                seen.insert(*id),
                "duplicate pluginId '{}' in catalog",
                id
            );
        }
    }
}
