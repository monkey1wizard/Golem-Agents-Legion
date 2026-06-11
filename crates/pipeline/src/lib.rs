//! pipeline: GAL local orchestration layer.
//!
//! Responsibilities (DAG `base ← dispatch ← pipeline ← xmachine`):
//! - `orchestration` (R-01): task-split + multi-provider dispatch + multi-stage
//!   orchestration that *composes* `dispatch` (routing → adapter → `spawn_executor`),
//!   never rebuilding spawn/write-back/routing/stage. Owns the `Transport` trait;
//!   `LocalTransport` runs through `dispatch::spawn_executor`. xmachine implements
//!   the remote `Transport` (direction pipeline ← xmachine).
//! - `task_spec` (R-02): the `New-TaskSpec.ps1` port — assembles a compact, single
//!   task spec (multi-line task block + per-task affected files) from an execution
//!   prompt for a secondary headless CLI.
//!
//! Transport-agnostic by design: remote SSH/zellij path-planning and session
//! records live in `crates/xmachine` (relocated there at T-005, F-1).

pub mod orchestration;
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
