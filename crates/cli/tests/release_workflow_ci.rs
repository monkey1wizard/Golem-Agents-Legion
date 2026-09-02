//! Reads the production `.github/workflows/release.yml` and asserts
//! its wiring rather than trusting an unrun CI execution as evidence.
//!
//! Confirms the tag still reaches `gal release --output-dir release-out`,
//! the stable `Create GitHub Release` step uses
//! `body_path: release-out/release-body.md` with generated notes off, and
//! only the prerelease step retains generated notes / omits the stable body
//! path — never both `body_path` and `generate_release_notes: true` on the
//! same step.

use std::fs;
use std::path::Path;

fn workflow_source() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/release.yml");
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()))
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
        "expected the tag-derived VERSION to still be passed as --version"
    );
    assert!(
        workflow.contains("--output-dir release-out"),
        "expected the release-out output-dir flag to remain wired"
    );
}

#[test]
fn stable_release_step_uses_verified_body_path_without_generated_notes() {
    let workflow = workflow_source();
    let stable_start = workflow
        .find("Create GitHub Release (stable")
        .expect("expected a stable Create GitHub Release step");
    let prerelease_start = workflow
        .find("Create GitHub Release (prerelease")
        .expect("expected a prerelease Create GitHub Release step");
    assert!(
        stable_start < prerelease_start,
        "expected the stable step to precede the prerelease step"
    );

    let stable_step = &workflow[stable_start..prerelease_start];
    assert!(
        stable_step.contains("if: ${{ !contains(github.ref_name, '-') }}"),
        "expected the stable step to gate on the non-prerelease predicate"
    );
    assert!(
        stable_step.contains("body_path: release-out/release-body.md"),
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
        .find("Create GitHub Release (prerelease")
        .expect("expected a prerelease Create GitHub Release step");
    let prerelease_step = &workflow[prerelease_start..];

    assert!(
        prerelease_step.contains("if: ${{ contains(github.ref_name, '-') }}"),
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
