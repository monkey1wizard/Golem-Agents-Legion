//! `gal-dispatch` — headless executor dispatch core (T-004..T-011).
//!
//! This crate owns:
//! - Routing JSON parsing (`~/.gal/config/executor-routing.json`)
//! - Stage → Role mapping (implement→CODER, test→TESTER, etc.)
//! - CLI argument parsing (`--phase`, `--task`, `--workdir`, `--timeout`, `--receipt`)
//! - Subprocess spawn + stdin feed + process-tree timeout recovery (T-005)
//! - Write-back verification + session id capture (T-006)
//! - Default safety gate + text-dispatch degradation (T-007)
//! - Five native Rust executor adapters (T-008)

pub mod adapters;
pub mod cli;
pub mod dispatch;
pub mod routing;
pub mod stage;
