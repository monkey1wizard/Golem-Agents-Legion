//! gal-core: shared primitives for the `gal` CLI.
//!
//! Phase 1 holds exactly three concerns: the command enum, exit-code
//! classification, and path *location* resolution.
//!
//! It MUST NOT parse machine config. Scripts own config/path truth; the Rust
//! entry only passes user intent and resolves where files live, never what they
//! contain (see plan BUG-01 / R-004 / T-009).

/// Known subcommands of the `gal` CLI.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum CommandKind {
    Install,
    Update,
    Doctor,
    Uninstall,
    DispatchScript,
}

impl CommandKind {
    /// Every known subcommand, in help/display order.
    pub const ALL: [CommandKind; 5] = [
        CommandKind::Install,
        CommandKind::Update,
        CommandKind::Doctor,
        CommandKind::Uninstall,
        CommandKind::DispatchScript,
    ];

    /// The canonical CLI spelling of this subcommand.
    pub fn as_str(self) -> &'static str {
        match self {
            CommandKind::Install => "install",
            CommandKind::Update => "update",
            CommandKind::Doctor => "doctor",
            CommandKind::Uninstall => "uninstall",
            CommandKind::DispatchScript => "dispatch-script",
        }
    }

    /// Parse a token into a known subcommand, or `None` if unrecognized.
    pub fn parse(input: &str) -> Option<CommandKind> {
        match input.trim().to_ascii_lowercase().as_str() {
            "install" => Some(CommandKind::Install),
            "update" => Some(CommandKind::Update),
            "doctor" => Some(CommandKind::Doctor),
            "uninstall" => Some(CommandKind::Uninstall),
            "dispatch-script" => Some(CommandKind::DispatchScript),
            _ => None,
        }
    }
}

/// Process exit-code classification for the CLI.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum ExitCode {
    /// Completed successfully.
    Success = 0,
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

pub mod paths {
    //! Path *location* resolution only — never reads or parses config content.

    use std::path::PathBuf;

    /// The GAL home directory (`~/.gal`). Resolves location only.
    pub fn gal_home() -> Option<PathBuf> {
        home_dir().map(|home| home.join(".gal"))
    }

    /// The machine config file location (`~/.gal/config/config.json`).
    ///
    /// Returns the path only; reading/parsing it is the scripts' responsibility.
    pub fn machine_config_path() -> Option<PathBuf> {
        gal_home().map(|home| home.join("config").join("config.json"))
    }

    fn home_dir() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            std::env::var_os("USERPROFILE").map(PathBuf::from)
        }
        #[cfg(not(windows))]
        {
            std::env::var_os("HOME").map(PathBuf::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_every_known_command() {
        assert_eq!(CommandKind::parse("install"), Some(CommandKind::Install));
        assert_eq!(CommandKind::parse("update"), Some(CommandKind::Update));
        assert_eq!(CommandKind::parse("doctor"), Some(CommandKind::Doctor));
        assert_eq!(CommandKind::parse("uninstall"), Some(CommandKind::Uninstall));
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
