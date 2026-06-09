//! base: GAL foundation crate.
//!
//! Owns configuration truth, install mode resolution, install-state ledger,
//! path *location* resolution, and shared serde data-model types (MCP manifest
//! types). This crate depends on no other GAL crate — it is the dependency-law
//! foundation: every other GAL crate may depend on `base`, never the reverse.

pub mod config;
pub mod ledger;
pub mod mcp;
pub mod mode;

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
