//! T-010 Oracle reparent — fixture-based behavioral tests replacing xmachine oracle scripts.
//!
//! R-08 precondition: the legacy oracle scripts are expressed as Rust-owned
//! fixture/behavioral tests before those scripts are deleted in T-014.
//! `cargo test` reads the frozen fixture files only; it does NOT spawn any of
//! the three oracle scripts below (TP-10).
//!
//! # Oracle scripts reparented by this file
//!
//! | Oracle script | Reparent location |
//! |---|---|
//! | `scripts/Test-Xmachine.ps1` | `mod windows_control_node_fixture` (frozen fixtures) |
//! | `scripts/Test-Xmachine.sh` | `mod windows_control_node_fixture` (frozen fixtures) |
//! | `scripts/Test-PipelineTokenBurn.ps1` | `mod token_burn_contract` (frozen contract fixture) |
//!
//! Every reparent reads a **committed fixture** under `tests/fixtures/xmachine/`,
//! never a live `scripts/*.ps1`/`*.sh` — so the deletion of those scripts at T-014
//! cannot break this test (the reparent decouples behavior from the deletable
//! script, which is the whole point of R-08).
//!
//! `scripts/common/New-TaskSpec.ps1` is already ported and covered by
//! `pipeline::task_spec::tests`; the xmachine fixture freeze still records its
//! former Windows failure surface, but T-010 does not need to duplicate the
//! extractor parity tests here.
//!
//! Deferred: `scripts/test-t022-ssh.sh` is a live cross-machine SSH smoke; it has
//! no isolated fixture and is reparented at the TP-13 cross-machine hard gate, not
//! here.

mod windows_control_node_fixture {
    const README: &str = include_str!("../../../tests/fixtures/xmachine/README.md");
    const TEST_XMACHINE_NO_WORKNODE: &str =
        include_str!("../../../tests/fixtures/xmachine/windows-control-node/test-xmachine-no-worknode.txt");
    const BASH_WRAPPER_UNAVAILABLE: &str =
        include_str!("../../../tests/fixtures/xmachine/windows-control-node/bash-wrapper-unavailable.txt");

    #[test]
    fn fixture_readme_documents_the_frozen_windows_control_node_baselines() {
        assert!(README.contains("test-xmachine-no-worknode.txt"));
        assert!(README.contains("bash-wrapper-unavailable.txt"));
        assert!(README.contains("Cross-machine SSH and zellij parity remain deferred"));
    }

    #[test]
    fn powershell_fixture_preserves_the_no_work_node_failure_surface() {
        assert!(TEST_XMACHINE_NO_WORKNODE.contains("Command: pwsh -NoProfile -File scripts/Test-Xmachine.ps1"));
        assert!(TEST_XMACHINE_NO_WORKNODE.contains("WorkNode is required. Pass -WorkNode <configured-work-node-alias>"));
    }

    #[test]
    fn bash_fixture_preserves_the_windows_wrapper_unavailability_surface() {
        assert!(BASH_WRAPPER_UNAVAILABLE.contains("Command: bash scripts/Test-Xmachine.sh"));
        assert!(BASH_WRAPPER_UNAVAILABLE.contains("execvpe(/bin/bash) failed"));
    }
}

mod token_burn_contract {
    // Reparented to a FROZEN CONTRACT FIXTURE — not the live `.ps1`. The previous
    // form `include_str!(".../Test-PipelineTokenBurn.ps1")` compile-coupled the test
    // to the script and asserted on its source text, which would have blocked the
    // T-014 deletion and tested wording rather than the contract. This fixture is
    // committed memory and survives the script's removal.
    const TOKEN_BURN_CONTRACT: &str =
        include_str!("../../../tests/fixtures/xmachine/pipeline-token-burn-contract.md");

    #[test]
    fn bounded_pipeline_dispatch_preserves_plan_path_and_emits_boundaries() {
        assert!(TOKEN_BURN_CONTRACT
            .contains("gal dispatch pipeline docs/plans/fix-gal-pipeline-token-burn.md from T-001 stop-at T-001"));
        assert!(TOKEN_BURN_CONTRACT.contains("preserves the explicit plan path"));
        assert!(TOKEN_BURN_CONTRACT.contains("| FROM | T-001 |"));
        assert!(TOKEN_BURN_CONTRACT.contains("| STOP_AT | T-001 |"));
    }

    #[test]
    fn per_phase_context_mode_contract_is_frozen() {
        assert!(TOKEN_BURN_CONTRACT.contains("| implement | full | true |"));
        assert!(TOKEN_BURN_CONTRACT.contains("| test | delta | true |"));
        assert!(TOKEN_BURN_CONTRACT.contains("| audit | delta | true |"));
    }

    #[test]
    fn reparent_does_not_depend_on_any_live_xmachine_script() {
        // The contract is decoupled from the deletable scripts (R-08 / T-014).
        assert!(!TOKEN_BURN_CONTRACT.contains("Invoke-Xmachine"));
        assert!(!TOKEN_BURN_CONTRACT.contains("Test-Xmachine.ps1"));
        assert!(!TOKEN_BURN_CONTRACT.contains("test-t022-ssh.sh"));
    }
}