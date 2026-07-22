//! pipeline: GAL local orchestration layer.
//!
//! Responsibilities (DAG `base ← dispatch ← pipeline`):
//! - `task_spec`: the `New-TaskSpec.ps1` port — assembles a compact, single
//!   task spec (multi-line task block + per-task affected files) from an execution
//!   prompt for a secondary headless CLI.
//!
//! Remote execution is a spawn-lane composed inside `dispatch::run` (the SSH
//! dispatch lane) — it is not a separate crate or `Transport` abstraction; the
//! prior `orchestration` module's `Transport`/`LocalTransport` scaffold had zero
//! production callers once xmachine (its only consumer) was retired, and was
//! removed rather than adopted.

pub mod loop_log;
pub mod task_spec;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
    #[error("task id must not be blank")]
    BlankTaskId,
    #[error("no executor-routing configured for role '{0}'")]
    UnroutedRole(String),
    #[error("unknown executor '{0}' (no adapter)")]
    UnknownExecutor(String),
    #[error("dispatch failed: {0}")]
    Dispatch(String),
    #[error("task '{0}' not found in ## Tasks section")]
    TaskNotFound(String),
}
