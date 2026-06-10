//! gal-engine: shared primitives for the `gal` CLI.
//!
//! Provides configuration parsing (Rust owns config truth), command enum,
//! exit-code classification, and path resolution.

pub mod commit_msg;
pub mod catalog;
pub mod doctor;
pub mod git_filters;
pub mod install;
pub mod mcp;
pub mod release;
pub mod render;
pub mod translation;

// Provider projection now lives in the `providers` crate (R-00/T-007). Re-export
// so `crate::providers` (install) and `gal_engine::providers` (integration tests)
// keep resolving. providers depends only on `base` — no cycle (BUG-01 pre-empted
// in T-003 by relocating MCP types to `base::mcp`).
pub use providers;

// Foundation modules now live in the `base` crate (R-00/T-003). Re-export them
// so existing `crate::config` / `gal_engine::config` paths keep resolving during
// the JIT decomposition. Importers migrate to `base::` directly in later phases.
pub use base::{config, ledger, mode, paths};

/// Known subcommands of the `gal` CLI.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CommandKind {
    Install,
    Update,
    Sync,
    Doctor,
    Uninstall,
    /// `gal commit-msg <msg-file>` — git commit-msg hook (T-013, optional/R5).
    CommitMsg,
    /// `gal release [--dry-run] [--version <tag>] [--output-dir <dir>]`
    /// — produce release artifacts (checksums.txt, artifact-manifest.json) (T-014).
    Release,
    /// `gal mcp [update]` — update per-provider MCP config files (T-011, R-02).
    Mcp,
    /// `gal setup [...]` — machine-setup orchestration (T-018, R-04).
    Setup,
    /// `gal clean` / `gal smudge` — git filter content transforms (T-025, R-06).
    Clean,
    Smudge,
    /// `gal init-repo [targetPath] [projectName] [--blank] [--force]`.
    InitRepo,
    /// `gal resolve-catalog` — deterministic plugin resolution and lockfile output.
    ResolveCatalog,
    /// `gal translation-freshness` — report translation freshness across docs/i18n.
    TranslationFreshness,
    DispatchScript,
}

impl CommandKind {
    /// Every known subcommand, in help/display order.
    pub const ALL: [CommandKind; 15] = [
        CommandKind::Install,
        CommandKind::Update,
        CommandKind::Sync,
        CommandKind::Doctor,
        CommandKind::Uninstall,
        CommandKind::CommitMsg,
        CommandKind::Release,
        CommandKind::Mcp,
        CommandKind::Setup,
        CommandKind::Clean,
        CommandKind::Smudge,
        CommandKind::InitRepo,
        CommandKind::ResolveCatalog,
        CommandKind::TranslationFreshness,
        CommandKind::DispatchScript,
    ];

    /// The canonical CLI spelling of this subcommand.
    pub fn as_str(self) -> &'static str {
        match self {
            CommandKind::Install => "install",
            CommandKind::Update => "update",
            CommandKind::Sync => "sync",
            CommandKind::Doctor => "doctor",
            CommandKind::Uninstall => "uninstall",
            CommandKind::CommitMsg => "commit-msg",
            CommandKind::Release => "release",
            CommandKind::Mcp => "mcp",
            CommandKind::Setup => "setup",
            CommandKind::Clean => "clean",
            CommandKind::Smudge => "smudge",
            CommandKind::InitRepo => "init-repo",
            CommandKind::ResolveCatalog => "resolve-catalog",
            CommandKind::TranslationFreshness => "translation-freshness",
            CommandKind::DispatchScript => "dispatch-script",
        }
    }

    /// Parse a token into a known subcommand, or `None` if unrecognized.
    pub fn parse(input: &str) -> Option<CommandKind> {
        match input.trim().to_ascii_lowercase().as_str() {
            "install" => Some(CommandKind::Install),
            "update" => Some(CommandKind::Update),
            "sync" => Some(CommandKind::Sync),
            "doctor" => Some(CommandKind::Doctor),
            "uninstall" => Some(CommandKind::Uninstall),
            "commit-msg" => Some(CommandKind::CommitMsg),
            "release" => Some(CommandKind::Release),
            "mcp" => Some(CommandKind::Mcp),
            "setup" => Some(CommandKind::Setup),
            "clean" => Some(CommandKind::Clean),
            "smudge" => Some(CommandKind::Smudge),
            "init-repo" => Some(CommandKind::InitRepo),
            "resolve-catalog" => Some(CommandKind::ResolveCatalog),
            "translation-freshness" => Some(CommandKind::TranslationFreshness),
            "dispatch-script" => Some(CommandKind::DispatchScript),
            _ => None,
        }
    }
}

/// Process exit-code classification for the CLI.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExitCode {
    /// Completed successfully (and doctor: no errors, warnings only).
    Success = 0,
    /// Operation failed (install error, render error, uninstall error, doctor errors).
    Error = 1,
    /// Subcommand recognized but not yet wired to the legacy engine (Phase 1).
    NotWired = 2,
    /// Bad usage: unknown command or missing command.
    Usage = 64,
}

impl From<ExitCode> for std::process::ExitCode {
    fn from(code: ExitCode) -> Self {
        std::process::ExitCode::from(code as u8)
    }
}

/// The action the CLI should take for a given argv (binary name already stripped).
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Version,
    Help,
    /// A known subcommand that is not yet wired to the legacy scripts.
    NotWired(CommandKind),
    UnknownCommand(String),
    MissingCommand,
}

/// Classify raw CLI arguments (binary name already stripped).
///
/// Pure: never touches the filesystem or machine config.
pub fn classify_args(args: &[String]) -> Action {
    let first = match args.first() {
        Some(arg) => arg.as_str(),
        None => return Action::MissingCommand,
    };

    match first {
        "--version" | "-V" | "version" => Action::Version,
        "--help" | "-h" | "help" => Action::Help,
        other => match CommandKind::parse(other) {
            Some(cmd) => Action::NotWired(cmd),
            None => Action::UnknownCommand(other.to_string()),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_known_command() {
        assert_eq!(CommandKind::parse("install"), Some(CommandKind::Install));
        assert_eq!(CommandKind::parse("update"), Some(CommandKind::Update));
        assert_eq!(CommandKind::parse("sync"), Some(CommandKind::Sync));
        assert_eq!(CommandKind::parse("doctor"), Some(CommandKind::Doctor));
        assert_eq!(CommandKind::parse("uninstall"), Some(CommandKind::Uninstall));
        assert_eq!(CommandKind::parse("mcp"), Some(CommandKind::Mcp));
        assert_eq!(CommandKind::parse("setup"), Some(CommandKind::Setup));
        assert_eq!(CommandKind::parse("clean"), Some(CommandKind::Clean));
        assert_eq!(CommandKind::parse("smudge"), Some(CommandKind::Smudge));
        assert_eq!(CommandKind::parse("init-repo"), Some(CommandKind::InitRepo));
        assert_eq!(CommandKind::parse("resolve-catalog"), Some(CommandKind::ResolveCatalog));
        assert_eq!(
            CommandKind::parse("translation-freshness"),
            Some(CommandKind::TranslationFreshness)
        );
        assert_eq!(
            CommandKind::parse("dispatch-script"),
            Some(CommandKind::DispatchScript)
        );
        assert_eq!(CommandKind::parse(" DOCTOR "), Some(CommandKind::Doctor));
        assert_eq!(CommandKind::parse("frobnicate"), None);
    }

    #[test]
    fn classifies_version_and_help() {
        assert_eq!(classify_args(&["--version".to_string()]), Action::Version);
        assert_eq!(classify_args(&["-V".to_string()]), Action::Version);
        assert_eq!(classify_args(&["--help".to_string()]), Action::Help);
        assert_eq!(classify_args(&[]), Action::MissingCommand);
    }

    #[test]
    fn classifies_known_and_unknown_subcommands() {
        assert_eq!(
            classify_args(&["doctor".to_string()]),
            Action::NotWired(CommandKind::Doctor)
        );
        assert_eq!(
            classify_args(&["frobnicate".to_string()]),
            Action::UnknownCommand("frobnicate".to_string())
        );
    }

    #[test]
    fn exit_codes_map_to_process_codes() {
        assert_eq!(ExitCode::Success as u8, 0);
        assert_eq!(ExitCode::NotWired as u8, 2);
        assert_eq!(ExitCode::Usage as u8, 64);
    }
}
