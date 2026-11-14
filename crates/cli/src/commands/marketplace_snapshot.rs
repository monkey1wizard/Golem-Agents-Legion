//! `gal marketplace-snapshot --source <dir> --out <dir>` — render the public
//! marketplace plugin tree to an explicit output directory.
//!
//! Core-only (`include_machine_local = false`): no runtime projection, no
//! `~/.gal` write, no personal layer, and no bundled host binary. Produces:
//!   `<out>/gal/…`                          rendered plugin root (commands / agents /
//!                                          skills + Claude & Codex manifests)
//!   `<out>/.claude-plugin/marketplace.json` catalog, plugin source `./gal`
//!
//! Publisher-internal binary subcommand; NOT on the public `/gal` surface.

use crate::commands::refresh::marketplace_json;
use gal_engine::render::{render_canonical_root_to, RenderError};
use gal_engine::ExitCode;
use std::path::{Path, PathBuf};

/// Errors `run_snapshot` can return.
#[derive(Debug)]
enum SnapshotError {
    Render(RenderError),
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotError::Render(e) => write!(f, "render error: {e}"),
            SnapshotError::Io(e) => write!(f, "I/O error: {e}"),
            SnapshotError::Json(e) => write!(f, "JSON error: {e}"),
        }
    }
}

impl From<RenderError> for SnapshotError {
    fn from(e: RenderError) -> Self {
        SnapshotError::Render(e)
    }
}

impl From<std::io::Error> for SnapshotError {
    fn from(e: std::io::Error) -> Self {
        SnapshotError::Io(e)
    }
}

impl From<serde_json::Error> for SnapshotError {
    fn from(e: serde_json::Error) -> Self {
        SnapshotError::Json(e)
    }
}

fn print_help() {
    println!("gal marketplace-snapshot — render the public marketplace plugin tree to a dir");
    println!();
    println!("Usage: gal marketplace-snapshot --source <dir> --out <dir>");
    println!();
    println!(
        "  --source <dir>   GAL source root (skills/ + agents/ + commands/, e.g. plugins/gal-core)"
    );
    println!(
        "  --out <dir>      Output dir; writes <out>/gal/ + <out>/.claude-plugin/marketplace.json"
    );
    println!();
    println!("Core-only: no projection, no ~/.gal write, no personal layer, no bundled binary.");
}

/// `gal marketplace-snapshot` CLI entry point.
pub(crate) fn cmd_marketplace_snapshot(args: &[String]) -> ExitCode {
    let extra = &args[1..];
    let mut source: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut i = 0;
    while i < extra.len() {
        let a = &extra[i];
        if a == "--help" || a == "-h" {
            print_help();
            return ExitCode::Success;
        } else if let Some(v) = a.strip_prefix("--source=") {
            source = Some(PathBuf::from(v));
        } else if let Some(v) = a.strip_prefix("--out=") {
            out = Some(PathBuf::from(v));
        } else if a == "--source" {
            i += 1;
            match extra.get(i) {
                Some(v) => source = Some(PathBuf::from(v)),
                None => {
                    eprintln!("gal marketplace-snapshot: --source requires a path argument");
                    return ExitCode::Usage;
                }
            }
        } else if a == "--out" {
            i += 1;
            match extra.get(i) {
                Some(v) => out = Some(PathBuf::from(v)),
                None => {
                    eprintln!("gal marketplace-snapshot: --out requires a path argument");
                    return ExitCode::Usage;
                }
            }
        } else {
            eprintln!("gal marketplace-snapshot: unknown option '{a}'");
            return ExitCode::Usage;
        }
        i += 1;
    }

    let source = match source {
        Some(s) => s,
        None => {
            eprintln!("gal marketplace-snapshot: --source <dir> is required");
            return ExitCode::Usage;
        }
    };
    let out = match out {
        Some(o) => o,
        None => {
            eprintln!("gal marketplace-snapshot: --out <dir> is required");
            return ExitCode::Usage;
        }
    };

    if !source.is_dir() {
        eprintln!(
            "gal marketplace-snapshot: --source is not a directory: {}",
            source.display()
        );
        return ExitCode::Usage;
    }

    println!("gal marketplace-snapshot");
    println!("  source: {}", source.display());
    println!("  out:    {}", out.display());

    match run_snapshot(&source, &out) {
        Ok(()) => {
            println!("done.");
            ExitCode::Success
        }
        Err(e) => {
            eprintln!("gal marketplace-snapshot: {e}");
            ExitCode::Error
        }
    }
}

/// Core snapshot logic, separated from the CLI entry so it is unit-testable.
///
/// Renders the plugin tree to `<out>/gal` core-only (no machine-local content)
/// and writes the marketplace catalog to `<out>/.claude-plugin/marketplace.json`.
/// Does not run runtime projection and does not write `~/.gal`.
fn run_snapshot(source: &Path, out: &Path) -> Result<(), SnapshotError> {
    std::fs::create_dir_all(out)?;

    // Core-only render: no personal layer, no bundled binary, no projection.
    let plugin_root = out.join("gal");
    render_canonical_root_to(source, &plugin_root, false, env!("CARGO_PKG_VERSION"))?;

    // Marketplace catalog at the snapshot root; plugin entry source is "./gal".
    let catalog_dir = out.join(".claude-plugin");
    std::fs::create_dir_all(&catalog_dir)?;
    let catalog = catalog_dir.join("marketplace.json");
    let content = serde_json::to_string_pretty(&marketplace_json())?;
    let tmp = catalog.with_extension("json.tmp");
    std::fs::write(&tmp, &content)?;
    std::fs::rename(&tmp, &catalog)?;

    println!("  plugin root: {}", plugin_root.display());
    println!("  marketplace: {}", catalog.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::ENV_GUARD;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn source_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .unwrap()
            .join("plugins")
            .join("gal-core")
    }

    // Snapshot produces the catalog + both plugin manifests under the explicit
    // out dir, and produces no bin/ (no bundled binary). The output is fully
    // contained under out — nothing is written to ~/.gal.
    #[test]
    fn snapshot_produces_tree_without_bin() {
        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );
        let out = TempDir::new().unwrap();
        run_snapshot(&src, out.path()).expect("snapshot must succeed");

        assert!(out
            .path()
            .join(".claude-plugin")
            .join("marketplace.json")
            .exists());
        assert!(out
            .path()
            .join("gal")
            .join(".claude-plugin")
            .join("plugin.json")
            .exists());
        assert!(out
            .path()
            .join("gal")
            .join(".codex-plugin")
            .join("plugin.json")
            .exists());
        assert!(out.path().join("gal").join("commands").is_dir());
        assert!(out.path().join("gal").join("skills").is_dir());
        // No bundled host binary in a public snapshot.
        assert!(!out.path().join("gal").join("bin").exists());
    }

    // Even with a machine that has a personal skill present (the personal layer
    // is presence-based now — there is no config flag to enable), the snapshot
    // output contains no personal skill (and no bin/).
    #[test]
    fn snapshot_excludes_personal_layer_with_personal_skill_present() {
        let _guard = ENV_GUARD.lock().unwrap_or_else(|p| p.into_inner());
        let src = source_root();
        assert!(
            src.exists(),
            "plugins/gal-core must exist in the repo checkout at {}: this test renders from the real source",
            src.display()
        );

        let home = TempDir::new().unwrap();
        #[cfg(windows)]
        std::env::set_var("USERPROFILE", home.path());
        #[cfg(not(windows))]
        std::env::set_var("HOME", home.path());

        // Plant a personal skill under ~/.gal/local/skills (no config.json needed
        // -- presence of the directory is itself the opt-in signal).
        let marker = home
            .path()
            .join(".gal")
            .join("local")
            .join("skills")
            .join("zzz-personal-marker");
        std::fs::create_dir_all(&marker).unwrap();
        std::fs::write(
            marker.join("SKILL.md"),
            "---\nname: zzz-personal-marker\n---\n",
        )
        .unwrap();

        let out = TempDir::new().unwrap();
        run_snapshot(&src, out.path()).expect("snapshot must succeed");

        // Personal skill must NOT leak into the public snapshot.
        assert!(!out
            .path()
            .join("gal")
            .join("skills")
            .join("zzz-personal-marker")
            .exists());
        // And still no bundled binary.
        assert!(!out.path().join("gal").join("bin").exists());
    }
}
