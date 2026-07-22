//! `gal` CLI subcommand handlers, split from main.rs by cohesion.
//! `run` (main.rs) dispatches to these `pub(crate)` handlers.

pub(crate) mod boundary_check;
pub(crate) mod converge_check;
pub(crate) mod dispatch;
pub(crate) mod doctor;
pub(crate) mod executor_smoke;
pub(crate) mod finalize_check;
pub(crate) mod finalize_hygiene;
pub(crate) mod lifecycle;
pub(crate) mod marketplace_snapshot;
pub(crate) mod pipeline_preflight;
pub(crate) mod planning_authority;
pub(crate) mod planning_check;
pub(crate) mod prompt_check;
pub(crate) mod refining_check;
pub(crate) mod refresh;
pub(crate) mod release;
pub(crate) mod restore;
pub(crate) mod system;

// Single process-global guard for every test in this binary that mutates
// process-wide env (HOME/USERPROFILE) or the current directory. Per-module
// guards do NOT serialize against each other, so refresh/restore tests could
// race on global HOME and write to the wrong dir. One shared mutex fixes that.
#[cfg(test)]
pub(crate) static ENV_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
