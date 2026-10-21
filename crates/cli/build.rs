//! Build-time git stamp for `gal --version`.
//!
//! Bakes the current short commit hash (plus a `-dirty` marker when the working
//! tree has uncommitted changes) into the binary so `gal --version` can tell the
//! dev whether the `PATH` binary was built from the current source. Fail-safe:
//! if git is unavailable (packaged build, no `.git`), the stamp is empty and
//! `gal --version` falls back to the bare `CARGO_PKG_VERSION`.
//!
//! Rerun rules matter beyond the stamp. A private runtime generation is named by
//! the SHA-256 of the built executable, so a build script that reruns on every
//! build relinks the binary and gives it a new hash even when nothing changed.
//! That defeats reuse of an existing generation. Cargo reruns a script on every
//! build when one of its `rerun-if-changed` paths does not exist, and in a linked
//! worktree `.git` is a file, so a fixed `../../.git/HEAD` path never exists.

use std::path::Path;
use std::process::Command;

fn main() {
    // Re-stamp when the checked-out commit moves. Ask git for the real paths so a
    // linked worktree works, and watch only paths that exist.
    watch_git_path("HEAD");
    if let Some(reference) = git(&["symbolic-ref", "-q", "HEAD"]) {
        watch_git_path(&reference);
    }
    watch_git_path("packed-refs");

    // Re-stamp when the content that goes into the binary changes, which keeps the
    // `-dirty` marker accurate. The git index is not watched: `git status` below
    // refreshes it, so watching it would rerun this script on every build.
    for path in [
        "../../crates",
        "../../plugins/gal-core",
        "../../Cargo.toml",
        "../../Cargo.lock",
    ] {
        if Path::new(path).exists() {
            println!("cargo:rerun-if-changed={path}");
        }
    }

    let sha = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_default();
    let dirty = git(&["status", "--porcelain"])
        .map(|out| !out.is_empty())
        .unwrap_or(false);

    let stamp = if sha.is_empty() {
        String::new()
    } else if dirty {
        format!("{sha}-dirty")
    } else {
        sha
    };

    // Always emit the key so `env!("GAL_GIT_STAMP")` resolves; empty = no stamp.
    println!("cargo:rustc-env=GAL_GIT_STAMP={stamp}");
}

/// Ask git where a path inside the git directory really lives and watch it when
/// it exists. A missing path is skipped, never watched.
fn watch_git_path(name: &str) {
    let Some(path) = git(&["rev-parse", "--path-format=absolute", "--git-path", name]) else {
        return;
    };
    if Path::new(&path).exists() {
        println!("cargo:rerun-if-changed={path}");
    }
}

/// Run a git command, returning trimmed stdout on success.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
