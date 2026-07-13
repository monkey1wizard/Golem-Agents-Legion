//! Stage → Role mapping.
//!
//! Pipeline phase names map to abstract role identifiers, which are then
//! looked up in the routing table to find the executor + model.
//!
//! | Phase      | Role     |
//! |------------|----------|
//! | implement  | CODER    |
//! | test       | TESTER   |
//! | audit      | AUDITOR  |
//!
//! Note: there is no `verify` dispatch phase or `VERIFY` role. End-of-run
//! goal-backward verification is owned by the ORCHESTRATOR and always runs
//! in-process (never dispatched); independent cross-verification is AUDITOR's
//! job. See the checking-role triangle in `workflows/coding.md` and the
//! gal-pipeline SKILL Step 3.

use thiserror::Error;

/// A pipeline phase, as named on the CLI (`--phase <value>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Implement,
    Test,
    Audit,
}

impl Phase {
    /// Canonical CLI name (lower-case, matches `--phase` values).
    pub fn as_str(self) -> &'static str {
        match self {
            Phase::Implement => "implement",
            Phase::Test => "test",
            Phase::Audit => "audit",
        }
    }

    /// The role key used in `config.json#executorRouting`.
    pub fn role(self) -> &'static str {
        match self {
            Phase::Implement => "CODER",
            Phase::Test => "TESTER",
            Phase::Audit => "AUDITOR",
        }
    }

    /// Parse from a CLI string; case-insensitive.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self, PhaseParseError> {
        match s.to_ascii_lowercase().as_str() {
            "implement" | "impl" => Ok(Phase::Implement),
            "test" => Ok(Phase::Test),
            "audit" => Ok(Phase::Audit),
            _ => Err(PhaseParseError::Unknown(s.to_string())),
        }
    }
}

#[derive(Debug, Error)]
pub enum PhaseParseError {
    #[error("unknown phase '{0}'; expected one of: implement, test, audit")]
    Unknown(String),
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn implement_maps_to_coder() {
        assert_eq!(Phase::Implement.role(), "CODER");
    }

    #[test]
    fn test_maps_to_tester() {
        assert_eq!(Phase::Test.role(), "TESTER");
    }

    #[test]
    fn audit_maps_to_auditor() {
        assert_eq!(Phase::Audit.role(), "AUDITOR");
    }

    #[test]
    fn phase_from_str_case_insensitive() {
        assert_eq!(Phase::from_str("IMPLEMENT").unwrap(), Phase::Implement);
        assert_eq!(Phase::from_str("Test").unwrap(), Phase::Test);
        assert_eq!(Phase::from_str("AUDIT").unwrap(), Phase::Audit);
    }

    #[test]
    fn impl_alias_works() {
        assert_eq!(Phase::from_str("impl").unwrap(), Phase::Implement);
    }

    #[test]
    fn unknown_phase_is_error() {
        assert!(Phase::from_str("deploy").is_err());
        assert!(Phase::from_str("review").is_err());
        assert!(Phase::from_str("verify").is_err());
        assert!(Phase::from_str("").is_err());
    }

    #[test]
    fn all_phases_have_unique_roles() {
        let roles = [
            Phase::Implement.role(),
            Phase::Test.role(),
            Phase::Audit.role(),
        ];
        let unique: std::collections::HashSet<_> = roles.iter().collect();
        assert_eq!(
            unique.len(),
            roles.len(),
            "each phase must map to a distinct role"
        );
    }
}
