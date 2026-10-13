//! Lifecycle commands: update.

use gal_engine::ExitCode;

/// Run `gal update` — print current version and upgrade guidance.
///
/// Binary upgrades are handled by the package manager (cargo/winget/Homebrew).
/// This command exists as a stable entry point for scripted version checks and
/// to direct users to the correct upgrade path.
pub(crate) fn cmd_update(args: &[String]) -> ExitCode {
    for a in args.iter().skip(1) {
        if a.starts_with("--") && a != "--version" {
            eprintln!("gal update: unknown option '{a}'");
            eprintln!("usage: gal update");
            return ExitCode::Usage;
        }
    }
    println!("gal {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("To upgrade: use the package manager you installed gal with.");
    for line in upgrade_guidance() {
        println!("{line}");
    }
    ExitCode::Success
}

/// Package-manager upgrade guidance lines. Pure + testable.
///
/// gal is not published to crates.io, so the cargo channel must use `--git`
/// with the real repo URL. The winget package identifier is `Monkey1Wizard.GAL`
/// (see the winget manifest generator in `crates/gal-engine/src/release.rs`).
fn upgrade_guidance() -> [&'static str; 3] {
    [
        "  cargo install --git https://github.com/monkey1wizard/golem-agents-legion gal-cli   # cargo",
        "  winget upgrade Monkey1Wizard.GAL   # Windows",
        "  brew upgrade gal                   # macOS",
    ]
}

#[cfg(test)]
mod tests {
    use super::upgrade_guidance;

    #[test]
    fn upgrade_guidance_uses_git_url_and_winget_identifier() {
        let joined = upgrade_guidance().join("\n");
        assert!(
            joined.contains("--git https://github.com/monkey1wizard/golem-agents-legion"),
            "cargo channel must pin the real --git URL: {joined}"
        );
        assert!(
            joined.contains("Monkey1Wizard.GAL"),
            "winget must use the real package identifier: {joined}"
        );
        assert!(
            !joined.contains("BruceLee.gal"),
            "stale winget identifier must be gone: {joined}"
        );
        assert!(
            !joined.contains("cargo install gal-cli"),
            "bare crates.io form must be gone: {joined}"
        );
    }
}
