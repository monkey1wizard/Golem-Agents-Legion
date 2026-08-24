//! base: GAL foundation crate.
//!
//! Owns configuration truth, install mode resolution,
//! path location resolution, shared JSON/env utilities, runtime selection
//! helpers, and shared serde data-model types (MCP manifest types).
//! This crate depends on no other GAL crate — it is the dependency-law
//! foundation: every other GAL crate may depend on `base`, never the reverse.

pub mod config;
pub mod env_config;
pub mod health;
pub mod json_util;
pub mod mcp;
pub mod paths;
pub mod platform;
pub mod render;
pub mod runtime;
pub mod secret;
pub mod time;
