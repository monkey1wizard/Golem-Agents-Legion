//! `gal dispatch-script` — chat-control-plane dispatch block emitter (Rust port of the
//! `scripts/gal.{ps1,sh}` `dispatch` branch). Split by cohesion:
//! `model` = data types + parsing + filesystem layer; `build` = dispatch-block
//! constructors that compose `model`. Public paths re-exported so `dispatch_script::*`
//! stays stable for `crate::dispatch_script` consumers in main.rs/commands.

mod build;
mod model;

pub use build::*;
pub use model::*;

#[cfg(test)]
mod tests;
