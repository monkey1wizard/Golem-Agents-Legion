//! FU-02 render smoke / inspection run (the Claude/Copilot parity surface).
//!
//! Performs the canonical-root render of `gal install` into a caller-specified
//! ISOLATED home directory, so the result can be inspected by a human before
//! deletion. It deliberately does NOT run the ledger write or the AGY junction
//! projection, because those resolve the home via `dirs::home_dir()` (Win32
//! known-folder API, ignores the env override) and would pollute the real
//! `~/.gal` / `~/.gemini`. The Claude/Copilot read surfaces that TP-014 cares
//! about are produced entirely by the render step.
//!
//! (The example is named "render" rather than "install" on purpose: Windows
//! Installer Detection auto-requests UAC elevation for executables whose name
//! contains "install"/"setup"/"update", which breaks a non-interactive run.)
//!
//! Usage:
//!   cargo run -p gal-engine --example fu02_render_smoke -- <isolated-home-dir>
//!
//! Effect:
//!   Renders to <isolated-home-dir>/.gal/plugins/gal/ (normal mode, source
//!   resolved from the binary location via the FU-01 resolver).

use gal_engine::config::GalConfig;
use gal_engine::mode::{resolve_mode, GalMode};
use gal_engine::render::render_canonical_root;
use std::path::Path;

fn main() {
    let home = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: fu02_render_smoke <isolated-home-dir>");
        std::process::exit(64);
    });

    // Redirect the canonical-root home for this process only.
    // render::get_canonical_plugin_root() reads USERPROFILE then HOME.
    std::env::set_var("USERPROFILE", &home);
    std::env::set_var("HOME", &home);

    println!("== FU-02 render smoke ==");
    println!("Isolated home : {home}");

    // Normal mode (no devMode) — source resolved from the binary via FU-01.
    let config = GalConfig::default();
    let mode = match resolve_mode(&config) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("FAIL: mode resolution: {e}");
            std::process::exit(1);
        }
    };
    println!(
        "Mode          : {}",
        match mode {
            GalMode::Normal => "normal",
            GalMode::Dev => "dev",
        }
    );

    let canonical_root = match render_canonical_root(&config, mode) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("FAIL: render: {e}");
            std::process::exit(1);
        }
    };
    println!("Canonical root: {}", canonical_root.display());
    println!();

    println!("-- produced tree (depth 2) --");
    print_tree(&canonical_root, 0, 2);
    println!();

    // Intent + structure assertions (the Claude/Copilot parity surface).
    let checks: Vec<(&str, bool)> = vec![
        (
            "skills/doc-sync/SKILL.md exists",
            canonical_root.join("skills/doc-sync/SKILL.md").exists(),
        ),
        (
            "agents/golem-dockeeper.md exists",
            canonical_root.join("agents/golem-dockeeper.md").exists(),
        ),
        (
            ".claude-plugin/plugin.json exists",
            canonical_root.join(".claude-plugin/plugin.json").exists(),
        ),
        (
            "copilot-manifest.json exists",
            canonical_root.join("copilot-manifest.json").exists(),
        ),
        (
            "commands/ has entries",
            dir_has_entries(&canonical_root.join("commands")),
        ),
        (
            "skills/ has entries",
            dir_has_entries(&canonical_root.join("skills")),
        ),
        (
            "agents/ has entries",
            dir_has_entries(&canonical_root.join("agents")),
        ),
    ];

    println!("-- assertions --");
    let mut all_pass = true;
    for (label, ok) in &checks {
        println!("  [{}] {label}", if *ok { "PASS" } else { "FAIL" });
        if !ok {
            all_pass = false;
        }
    }
    println!();

    if all_pass {
        println!("RESULT: PASS — canonical root rendered into the isolated home.");
        println!("Inspect: {}", canonical_root.display());
    } else {
        eprintln!("RESULT: FAIL — see failing assertions above.");
        std::process::exit(1);
    }
}

/// Print a bounded-depth directory tree.
fn print_tree(path: &Path, depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }
    let mut entries: Vec<_> = match std::fs::read_dir(path) {
        Ok(rd) => rd.filter_map(Result::ok).collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        println!(
            "{}{}{}",
            "  ".repeat(depth + 1),
            name,
            if is_dir { "/" } else { "" }
        );
        if is_dir {
            print_tree(&entry.path(), depth + 1, max_depth);
        }
    }
}

/// True if the directory exists and contains at least one entry.
fn dir_has_entries(path: &Path) -> bool {
    std::fs::read_dir(path)
        .map(|mut rd| rd.next().is_some())
        .unwrap_or(false)
}
