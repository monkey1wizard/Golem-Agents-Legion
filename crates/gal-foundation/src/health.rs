//! Health-check vocabulary and the `HealthCheck` trait.
//!
//! Lives in `base` to invert the doctor dependency (architect decision): each
//! capability/orchestrator domain implements [`HealthCheck`] for its own
//! surfaces, and the `cli` edge aggregates them into `gal doctor`
//! without `base` depending on any domain. Severity → exit grading is owned here
//! so the grading is identical regardless of which domain produced a finding.

/// Severity of a doctor finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// Non-blocking, informational.
    Warning,
    /// Must be fixed; causes non-zero exit code.
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "WARNING"),
            Severity::Error => write!(f, "ERROR"),
        }
    }
}

/// A single doctor finding.
#[derive(Debug, Clone)]
pub struct DoctorFinding {
    pub severity: Severity,
    /// Human-readable description of the issue.
    pub message: String,
    /// Optional hint pointing to the fix surface.
    pub fix_hint: Option<String>,
}

impl DoctorFinding {
    pub fn error(message: impl Into<String>, fix_hint: impl Into<String>) -> Self {
        Self {
            severity: Severity::Error,
            message: message.into(),
            fix_hint: Some(fix_hint.into()),
        }
    }

    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            message: message.into(),
            fix_hint: None,
        }
    }
}

impl std::fmt::Display for DoctorFinding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.severity, self.message)?;
        if let Some(ref hint) = self.fix_hint {
            write!(f, " → {hint}")?;
        }
        Ok(())
    }
}

/// Aggregated doctor report.
#[derive(Debug)]
pub struct DoctorReport {
    pub findings: Vec<DoctorFinding>,
}

impl DoctorReport {
    pub fn new() -> Self {
        Self {
            findings: Vec::new(),
        }
    }

    /// Returns `true` if any finding has `Severity::Error`.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Returns the recommended process exit code.
    ///
    /// - 0 if no errors (warnings only or all clear)
    /// - 1 if any error
    pub fn exit_code(&self) -> i32 {
        if self.has_errors() {
            1
        } else {
            0
        }
    }

    /// Append a single finding.
    pub fn push(&mut self, f: DoctorFinding) {
        self.findings.push(f);
    }
}

impl Default for DoctorReport {
    fn default() -> Self {
        Self::new()
    }
}

/// A read-only health check over one GAL surface.
///
/// Domains implement this for their own surfaces; the `cli` edge collects the
/// trait objects and aggregates their findings into `gal doctor`. Implementations
/// must not modify any files.
pub trait HealthCheck {
    /// Stable short name of the surface this check covers (for aggregation/UX).
    fn name(&self) -> &str;

    /// Run the check and return any findings (read-only).
    fn check(&self) -> Vec<DoctorFinding>;
}
