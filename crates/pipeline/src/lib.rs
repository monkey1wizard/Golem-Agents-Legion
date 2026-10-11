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

pub mod coordinator;
pub mod loop_log;
pub mod projection_journal;
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
    #[error("prompt is not valid UTF-8: {0}")]
    PromptUtf8(String),
    #[error("receipt is not valid UTF-8: {0}")]
    ReceiptUtf8(String),
    #[error("receipt payload must start with exactly one task heading for '{0}'")]
    InvalidHeading(String),
    #[error("receipt payload contains an unfenced heading")]
    UnfencedHeading,
    #[error("receipt payload task heading does not match '{0}'")]
    TaskMismatch(String),
    #[error("receipt payload is missing the required {0} marker")]
    MissingMarker(&'static str),
    #[error("prompt has no '{0}' section")]
    MissingSection(&'static str),
    #[error("prompt has duplicate '{0}' sections")]
    DuplicateSection(&'static str),
    #[error("prompt has a malformed table boundary")]
    MalformedTableBoundary,
}
