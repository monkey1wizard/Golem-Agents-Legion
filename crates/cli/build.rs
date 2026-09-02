//! Build-time git stamp for `gal --version`.
//!
//! Bakes the current short commit hash (plus a `-dirty` marker when the working
//! tree has uncommitted changes) into the binary so `gal --version` can tell the
//! dev whether the `PATH` binary was built from the current source. Fail-safe:
//! if git is unavailable (packaged build, no `.git`), the stamp is empty and
//! `gal --version` falls back to the bare `CARGO_PKG_VERSION`.

use std::process::Command;

fn main() {
    // Re-stamp when HEAD or the index moves (best-effort; relative to the crate dir).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");

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

/// Run a git command, returning trimmed stdout on success.
fn git(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if !out.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
}
