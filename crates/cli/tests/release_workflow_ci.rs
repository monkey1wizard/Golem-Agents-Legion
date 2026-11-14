//! Reads the production `.github/workflows/release.yml` and asserts
//! its wiring rather than trusting an unrun CI execution as evidence.
//!
//! Confirms the release version still reaches
//! `gal release --output-dir bundle/release-out`, the stable
//! `Create GitHub Release` step uses
//! `body_path: bundle/release-out/release-body.md` with generated notes off,
//! and only the prerelease step retains generated notes and omits the stable
//! body path. It also pins the owner's release order: the rehearsal tests
//! every channel before anything is published, the publish run starts as a
//! pre-release, and it becomes the full release and Latest only after
//! Homebrew, WinGet, and every smoke check passed.

use std::fs;
use std::path::Path;

fn workflow_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/release.yml");
    fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

/// Return one top-level job: its header line through the line before the
/// next two-space-indented key or comment.
fn job(workflow: &str, name: &str) -> String {
    let header = format!("  {name}:");
    let mut lines = workflow.lines().skip_while(|line| *line != header);
    let first = lines
        .next()
        .unwrap_or_else(|| panic!("expected a {name} job"));
    let mut body = vec![first];
    for line in lines {
        let top_level = line.starts_with("  ") && !line.starts_with("   ");
        if top_level {
            break;
        }
        body.push(line);
    }
    body.join("\n") + "\n"
}

#[test]
fn production_invocation_still_calls_gal_release_with_output_dir() {
    let workflow = workflow_source();
    assert!(
        workflow.contains("./target/release/gal release \\"),
        "expected the production gal release invocation to remain in release.yml"
    );
    assert!(
        workflow.contains("--version \"${VERSION}\""),
        "expected the release VERSION to still be passed as --version"
    );
    assert!(
        workflow.contains("--output-dir bundle/release-out"),
        "expected the bundle/release-out output-dir flag to remain wired"
    );
}

#[test]
fn stable_release_step_uses_verified_body_path_without_generated_notes() {
    let workflow = workflow_source();
    let stable_start = workflow
        .find("Create the GitHub Release (stable")
        .expect("expected a stable Create GitHub Release step");
    let prerelease_start = workflow
        .find("Create the GitHub Release (prerelease")
        .expect("expected a prerelease Create GitHub Release step");
    assert!(
        stable_start < prerelease_start,
        "expected the stable step to precede the prerelease step"
    );

    let stable_step = &workflow[stable_start..prerelease_start];
    assert!(
        stable_step.contains("if: ${{ needs.meta.outputs.prerelease == 'false' }}"),
        "expected the stable step to gate on the non-prerelease predicate"
    );
    assert!(
        stable_step.contains("body_path: bundle/release-out/release-body.md"),
        "expected the stable step to source its body from the verified release-body.md"
    );
    assert!(
        stable_step.contains("generate_release_notes: false"),
        "expected the stable step to keep GitHub-inferred notes off"
    );
    assert!(
        !stable_step.contains("generate_release_notes: true"),
        "stable step must never combine body_path with generated notes"
    );
}

#[test]
fn prerelease_step_keeps_generated_notes_and_no_stable_body_reference() {
    let workflow = workflow_source();
    let prerelease_start = workflow
        .find("Create the GitHub Release (prerelease")
        .expect("expected a prerelease Create GitHub Release step");
    let prerelease_end = workflow[prerelease_start..]
        .find("\n  publish-homebrew:")
        .map_or(workflow.len(), |offset| prerelease_start + offset);
    let prerelease_step = &workflow[prerelease_start..prerelease_end];

    assert!(
        prerelease_step.contains("if: ${{ needs.meta.outputs.prerelease == 'true' }}"),
        "expected the prerelease step to gate on the prerelease predicate"
    );
    assert!(
        prerelease_step.contains("generate_release_notes: true"),
        "expected the prerelease step to keep the generated-notes rehearsal path"
    );
    assert!(
        !prerelease_step.contains("body_path:"),
        "prerelease step must not set body_path to the stable release-body.md"
    );
}

#[test]
fn no_second_changelog_parser_is_embedded_in_workflow_shell() {
    let workflow = workflow_source();
    // The only decision surface for stable-vs-prerelease body sourcing must be
    // the two mutually exclusive `if:` conditions on the Create GitHub Release
    // steps, backed by the already-unit-tested `gal release` gate. Comments
    // may explain that gate by name, but no `run:` shell block may read or
    // grep CHANGELOG.md itself — that would be a second, unexecuted parser.
    for (line_no, line) in workflow.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        assert!(
            !line.contains("CHANGELOG.md"),
            "line {} references CHANGELOG.md outside a comment: {line}",
            line_no + 1
        );
    }
}

#[test]
fn hosted_test_job_gates_signing() {
    let workflow = workflow_source();
    let test_job = job(&workflow, "test");
    assert!(
        test_job.contains("if: ${{ needs.meta.outputs.mode == 'rehearsal' }}"),
        "expected the hosted test job to run in the rehearsal"
    );
    assert!(
        test_job.contains("permissions:\n      contents: read"),
        "expected the hosted test job to have read-only contents permission"
    );
    assert!(
        test_job.contains("uses: ./.github/workflows/ci.yml"),
        "expected the hosted test job to call the repository CI workflow"
    );
    assert!(
        job(&workflow, "assemble").contains("needs: [meta, build, test, writing, ignored-tests]"),
        "expected signing to wait for build, hosted tests, the writing check, and ignored tests"
    );
    assert!(
        job(&workflow, "writing").contains("node tools/writing/check.mjs --required"),
        "expected the rehearsal to run the repository writing check"
    );
    let ignored = job(&workflow, "ignored-tests");
    assert!(ignored.contains("--ignored --exact real_worktree_build_reuses_the_published_generation_when_nothing_changed"));
    assert!(ignored.contains("--ignored render_produces_expected_structure"));
}

#[test]
fn rehearsal_never_publishes_and_tests_every_channel() {
    let workflow = workflow_source();
    assert!(
        workflow.contains("  push:\n    branches: [release-candidate]\n"),
        "expected the rehearsal to run on release-candidate pushes"
    );
    assert!(
        !workflow.contains("    tags:\n"),
        "a tag push must not start a release run"
    );
    for name in [
        "build",
        "test",
        "writing",
        "ignored-tests",
        "assemble",
        "rehearse-artifacts",
        "rehearse-installer",
        "rehearse-installer-windows",
        "rehearse-homebrew",
        "rehearse-winget",
        "rehearse-cargo",
    ] {
        let body = job(&workflow, name);
        assert!(
            !body.contains("contents: write"),
            "rehearsal job {name} must not get write access"
        );
        assert!(
            !body.contains("secrets."),
            "rehearsal job {name} must not read publishing secrets"
        );
    }
    let winget = job(&workflow, "rehearse-winget");
    assert!(winget.contains("winget validate"));
    assert!(winget.contains("winget install --manifest"));
    assert!(winget.contains("ahead_by"));
    assert!(
        !winget.contains("merge-upstream"),
        "CI must never sync the WinGet fork"
    );
    let homebrew = job(&workflow, "rehearse-homebrew");
    assert!(homebrew.contains("brew audit --strict"));
    assert!(homebrew.contains("brew install"));
    assert!(homebrew.contains("gal --version"));
    assert!(job(&workflow, "rehearse-installer").contains("GAL_RELEASE_BASE_URL"));
    assert!(job(&workflow, "rehearse-installer-windows").contains("GAL_RELEASE_BASE_URL"));
}

#[test]
fn release_starts_as_prerelease_and_is_promoted_only_after_every_channel() {
    let workflow = workflow_source();
    let publish = job(&workflow, "publish-release");
    assert!(
        publish.contains("if: ${{ needs.meta.outputs.mode == 'publish' }}"),
        "expected the GitHub Release to be created only in the publish run"
    );
    assert_eq!(
        publish.matches("make_latest: 'false'").count(),
        2,
        "both release steps must leave the release not Latest"
    );
    assert_eq!(
        publish.matches("prerelease: true").count(),
        2,
        "both release steps must create a pre-release"
    );
    assert!(
        !publish.contains("prerelease: false"),
        "the publish run must never create a full release directly"
    );
    assert!(
        !workflow.contains("mark-latest"),
        "promote-release replaced mark-latest"
    );
    assert_eq!(
        workflow.matches("--latest").count(),
        1,
        "only promote-release may mark the release Latest"
    );
    let mark = job(&workflow, "promote-release");
    assert!(mark.contains("--prerelease=false --latest"));
    assert!(mark.contains("isPrerelease"));
    assert!(mark.contains("releases/latest"));
    for dependency in [
        "publish-homebrew",
        "smoke-homebrew",
        "submit-winget",
        "smoke-artifacts",
        "smoke-installer",
        "smoke-installer-windows",
        "smoke-cargo",
    ] {
        assert!(
            mark.contains(&format!("      - {dependency}\n")),
            "promote-release must wait for {dependency}"
        );
    }
    for (name, token) in [
        ("publish-homebrew", "HOMEBREW_TAP_TOKEN"),
        ("submit-winget", "WINGET_CREATE_GITHUB_TOKEN"),
    ] {
        let body = job(&workflow, name);
        assert!(
            body.contains("needs: [meta, publish-release]"),
            "{name} must run after the release files exist"
        );
        assert!(
            body.contains(&format!("{token} secret is not set")),
            "{name} must fail, not skip, without {token}"
        );
    }
}
