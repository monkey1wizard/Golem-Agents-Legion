//! Runtime / provider selection utilities.
//!
//! Ports `Get-DefaultPrimaryRuntime` and `Get-PipelinePhaseRole` from
//! `scripts/common/Common.{ps1,sh}`.

/// All recognized runtime keys in canonical preference order.
///
/// Matches the `RuntimeCatalog` keys in `New-SetupContext` from Common.ps1.
pub const VALID_RUNTIMES: &[&str] =
    &["copilot", "antigravity", "gemini", "codex", "opencode", "claude"];

/// Return the default primary runtime from a slice of selected runtime keys.
///
/// Prefers runtimes in this order: `copilot`, `antigravity`, `codex`,
/// `claude`, `opencode`, `gemini` — matching `Get-DefaultPrimaryRuntime`.
/// Returns `None` when `selected` is empty or contains no recognized runtimes.
pub fn default_primary_runtime<'a>(selected: &[&'a str]) -> Option<&'a str> {
    const PREFERRED: &[&str] =
        &["copilot", "antigravity", "codex", "claude", "opencode", "gemini"];
    for &candidate in PREFERRED {
        if selected.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Map a pipeline phase name to its role constant.
///
/// Ports `Get-PipelinePhaseRole` from Common.ps1.
/// Comparison is case-insensitive. Returns `None` for unrecognized phases.
pub fn pipeline_phase_role(phase: &str) -> Option<&'static str> {
    match phase.to_lowercase().as_str() {
        "implement" => Some("CODER"),
        "test" => Some("TESTER"),
        "review" => Some("REVIEWER"),
        "verify" => Some("VERIFIER"),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── VALID_RUNTIMES ────────────────────────────────────────────────────────

    #[test]
    fn valid_runtimes_contains_all_six() {
        assert_eq!(VALID_RUNTIMES.len(), 6);
        for key in &["copilot", "antigravity", "gemini", "codex", "opencode", "claude"] {
            assert!(VALID_RUNTIMES.contains(key), "VALID_RUNTIMES missing: {key}");
        }
    }

    // ── default_primary_runtime ───────────────────────────────────────────────

    #[test]
    fn prefers_copilot_when_present() {
        let selected = &["copilot", "claude", "codex"];
        assert_eq!(default_primary_runtime(selected), Some("copilot"));
    }

    #[test]
    fn falls_to_antigravity_when_no_copilot() {
        let selected = &["antigravity", "codex", "claude"];
        assert_eq!(default_primary_runtime(selected), Some("antigravity"));
    }

    #[test]
    fn falls_to_codex_when_no_copilot_or_antigravity() {
        let selected = &["codex", "opencode"];
        assert_eq!(default_primary_runtime(selected), Some("codex"));
    }

    #[test]
    fn falls_to_claude_when_earlier_absent() {
        let selected = &["claude", "opencode", "gemini"];
        assert_eq!(default_primary_runtime(selected), Some("claude"));
    }

    #[test]
    fn falls_to_opencode_when_only_opencode_and_gemini() {
        let selected = &["opencode", "gemini"];
        assert_eq!(default_primary_runtime(selected), Some("opencode"));
    }

    #[test]
    fn falls_to_gemini_when_only_gemini() {
        let selected = &["gemini"];
        assert_eq!(default_primary_runtime(selected), Some("gemini"));
    }

    #[test]
    fn returns_none_for_empty_selection() {
        let selected: &[&str] = &[];
        assert_eq!(default_primary_runtime(selected), None);
    }

    #[test]
    fn returns_none_for_unknown_runtimes_only() {
        let selected = &["xmachine", "unknown-tool"];
        assert_eq!(default_primary_runtime(selected), None);
    }

    // ── pipeline_phase_role ───────────────────────────────────────────────────

    #[test]
    fn implement_maps_to_coder() {
        assert_eq!(pipeline_phase_role("implement"), Some("CODER"));
    }

    #[test]
    fn implement_is_case_insensitive() {
        assert_eq!(pipeline_phase_role("IMPLEMENT"), Some("CODER"));
        assert_eq!(pipeline_phase_role("Implement"), Some("CODER"));
    }

    #[test]
    fn test_maps_to_tester() {
        assert_eq!(pipeline_phase_role("test"), Some("TESTER"));
    }

    #[test]
    fn review_maps_to_reviewer() {
        assert_eq!(pipeline_phase_role("review"), Some("REVIEWER"));
    }

    #[test]
    fn verify_maps_to_verifier() {
        assert_eq!(pipeline_phase_role("verify"), Some("VERIFIER"));
    }

    #[test]
    fn unknown_phase_returns_none() {
        assert_eq!(pipeline_phase_role("deploy"), None);
        assert_eq!(pipeline_phase_role(""), None);
        assert_eq!(pipeline_phase_role("security"), None);
    }
}
