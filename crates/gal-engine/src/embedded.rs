//! Build-time embedded gal-core payload for the Cargo install channel.
//!
//! `cargo install --git` produces a bare binary with no source payload beside
//! it. To keep `gal refresh` / `gal init` self-sufficient on that channel, the
//! whole gal-core tree is baked into the binary at build time (staged + filtered
//! by `build.rs`, then embedded here via `include_dir`).
//!
//! Only the Cargo channel needs this — Homebrew/WinGet/curl ship the payload
//! beside the binary (FHS `share/gal` or flat layout), resolved by
//! [`crate::render::resolve_source_from_exe_dir`].

use include_dir::{include_dir, Dir};
use std::path::Path;

/// The embedded gal-core source tree (skills/agents/commands/templates/… +
/// mcp.json), minus baked `SKILL.md` files that have a sibling
/// `SKILL.template.md` (filtered out by `build.rs`).
pub static EMBEDDED_GAL_CORE: Dir<'_> = include_dir!("$OUT_DIR/embedded-gal-core");

/// True when a real payload was embedded at build time (false only on a non-git
/// build that staged an empty tree).
pub fn is_populated() -> bool {
    !EMBEDDED_GAL_CORE.entries().is_empty()
}

/// Materialize the embedded payload into `target` (recursively). Used by the
/// refresh source-resolution chain when neither a repo checkout nor an exe-side
/// payload is present (the Cargo bare-binary case).
pub fn extract_to(target: &Path) -> std::io::Result<()> {
    EMBEDDED_GAL_CORE.extract(target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_tree_has_expected_top_level_entries() {
        // The payload must carry the source-root markers + templates + mcp.json.
        assert!(is_populated(), "embedded gal-core tree is empty");
        for dir in ["commands", "agents", "skills", "templates"] {
            assert!(
                EMBEDDED_GAL_CORE.get_dir(dir).is_some(),
                "embedded tree missing top-level dir: {dir}"
            );
        }
        assert!(
            EMBEDDED_GAL_CORE.get_file("mcp.json").is_some(),
            "embedded tree missing mcp.json"
        );
    }

    #[test]
    fn embedded_tree_excludes_baked_skill_md_with_sibling_template() {
        // A baked commands/<cmd>/SKILL.md with a sibling SKILL.template.md is a
        // gitignored derived product and must never be embedded.
        let commands = EMBEDDED_GAL_CORE
            .get_dir("commands")
            .expect("commands dir present");
        for cmd_dir in commands.dirs() {
            let has_template = cmd_dir
                .files()
                .any(|f| f.path().file_name().is_some_and(|n| n == "SKILL.template.md"));
            if has_template {
                let has_baked = cmd_dir
                    .files()
                    .any(|f| f.path().file_name().is_some_and(|n| n == "SKILL.md"));
                assert!(
                    !has_baked,
                    "embedded tree leaked a baked SKILL.md in {:?}",
                    cmd_dir.path()
                );
            }
        }
    }
}
