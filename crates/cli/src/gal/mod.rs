//! gal-exclusive workflow code (gal tree).
//!
//! Modules here belong to the **gal** product (workflow), not ccync (management).
//! They were de-hybridized out of the management crates (gal-engine / projection)
//! in S2 and live under `crates/cli/src/gal/` so the eventual S3 cli split can move
//! the whole subtree to `gal-cli` with zero further untangling. None of these
//! modules may `use` a ccync-tree crate (projection / mcp / gal_engine / setup /
//! gal_foundation).

pub mod commit_msg;
pub mod git_filters;
pub mod naming_gate;
pub mod render;
pub mod skill_discovery;
pub mod translation;
