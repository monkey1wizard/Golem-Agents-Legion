//! Build-time staging of the gal-core payload for the embedded (Cargo) channel.
//!
//! `cargo install --git` produces a bare binary with no source payload beside
//! it, so the whole `plugins/gal-core` tree is baked into the binary at build
//! time (embedded via `include_dir` in `src/embedded.rs`). include_dir has no
//! filtering, and a dev machine's baked `commands/*/SKILL.md` are gitignored
//! derived products that exist on disk — embedding them directly would make a
//! local build and a CI build differ non-reproducibly. So we stage a *filtered*
//! copy into OUT_DIR first (dropping any `SKILL.md` that has a sibling
//! `SKILL.template.md`), then embed the staging dir.

use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR set by cargo"),
    );
    // crates/gal-engine → repo root → plugins/gal-core
    let source = manifest_dir
        .join("..")
        .join("..")
        .join("plugins")
        .join("gal-core");
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR set by cargo"));
    let staging = out_dir.join("embedded-gal-core");

    // Always start from a clean staging dir so removed source files do not linger.
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).expect("create embedded staging dir");

    if source.is_dir() {
        copy_filtered(&source, &staging);
        // Recursively re-run when the payload changes.
        println!("cargo:rerun-if-changed={}", source.display());
    } else {
        // Non-git build (isolated crate build without the repo tree): embed an
        // empty tree rather than fail. Runtime resolution then falls through to
        // its normal "source root not found" path instead of a build break.
        println!(
            "cargo:warning=gal-core payload not found at {} — embedding an empty tree",
            source.display()
        );
    }
}

/// Recursively copy `src` into `dst`, excluding baked `SKILL.md` files that have
/// a sibling `SKILL.template.md` (gitignored derived products regenerated per
/// machine).
fn copy_filtered(src: &Path, dst: &Path) {
    for entry in fs::read_dir(src).expect("read_dir source") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        let name = entry.file_name();
        let target = dst.join(&name);
        if path.is_dir() {
            fs::create_dir_all(&target).expect("create staging subdir");
            copy_filtered(&path, &target);
        } else {
            if name == "SKILL.md" && path.with_file_name("SKILL.template.md").exists() {
                continue; // skip baked derived SKILL.md
            }
            fs::copy(&path, &target).expect("copy payload file");
        }
    }
}
