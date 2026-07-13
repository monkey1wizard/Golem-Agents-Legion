//! `gal-dispatch` — headless executor dispatch core.
//!
//! This crate owns:
//! - Routing JSON parsing (`~/.gal/config/config.json#executorRouting`)
//! - Stage → Role mapping (implement→CODER, test→TESTER, etc.)
//! - CLI argument parsing (`--phase`, `--task`, `--workdir`, `--timeout`, `--receipt`)
//! - Subprocess spawn + stdin feed + process-tree timeout recovery
//! - Write-back verification + session id capture
//! - Default safety gate + text-dispatch degradation
//! - Five native Rust executor adapters

pub mod cli;
pub mod dispatch;
pub mod executors;
pub mod routing;
pub mod run;
pub mod stage;
