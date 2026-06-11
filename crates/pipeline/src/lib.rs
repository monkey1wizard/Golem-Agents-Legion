//! pipeline: GAL orchestration scaffold.
//!
//! T-002 establishes the dependency-correct skeleton only:
//! - the `Transport` trait lives here (so xmachine can implement it later)
//! - the local transport marker also lives here
//! - pipeline phases reuse `dispatch::stage::Phase` instead of redefining them
//!
//! This crate intentionally does NOT implement task splitting or task-spec yet.

use dispatch::stage::Phase;
use thiserror::Error;

/// One bounded pipeline execution request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineRequest {
    pub task_id: String,
    pub phase: Phase,
}

/// Result of sending one bounded request through a transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportReceipt {
    pub task_id: String,
    pub phase: Phase,
    pub transport: &'static str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
    #[error("task id must not be blank")]
    BlankTaskId,
}

/// Transport abstraction owned by the pipeline layer.
///
/// Later tasks add a real local executor path and the SSH+zellij xmachine impl.
pub trait Transport {
    fn label(&self) -> &'static str;
    fn dispatch(&self, request: &PipelineRequest) -> Result<TransportReceipt, PipelineError>;
}

/// Local transport marker.
///
/// T-002 proves the ownership boundary only: local execution belongs to pipeline,
/// but phase/role semantics come from `dispatch` rather than being rebuilt here.
#[derive(Debug, Default, Clone, Copy)]
pub struct LocalTransport;

impl Transport for LocalTransport {
    fn label(&self) -> &'static str {
        "local"
    }

    fn dispatch(&self, request: &PipelineRequest) -> Result<TransportReceipt, PipelineError> {
        if request.task_id.trim().is_empty() {
            return Err(PipelineError::BlankTaskId);
        }

        Ok(TransportReceipt {
            task_id: request.task_id.clone(),
            phase: request.phase,
            transport: self.label(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_transport_accepts_valid_request() {
        let transport = LocalTransport;
        let request = PipelineRequest {
            task_id: "T-002".to_string(),
            phase: Phase::Implement,
        };

        let receipt = transport.dispatch(&request).unwrap();
        assert_eq!(receipt.task_id, "T-002");
        assert_eq!(receipt.phase, Phase::Implement);
        assert_eq!(receipt.transport, "local");
    }

    #[test]
    fn local_transport_rejects_blank_task_id() {
        let transport = LocalTransport;
        let request = PipelineRequest {
            task_id: "   ".to_string(),
            phase: Phase::Implement,
        };

        let err = transport.dispatch(&request).unwrap_err();
        assert_eq!(err, PipelineError::BlankTaskId);
    }

    #[test]
    fn pipeline_reuses_dispatch_phase_model() {
        let request = PipelineRequest {
            task_id: "T-002".to_string(),
            phase: Phase::Audit,
        };

        assert_eq!(request.phase.role(), "AUDITOR");
        assert_eq!(request.phase.as_str(), "audit");
    }
}
