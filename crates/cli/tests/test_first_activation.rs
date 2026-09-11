//! Contract tests for the final activation boundary.
//!
//! These tests intentionally inspect the tracked command contracts. Activation is
//! owned by the two planning emitters; templates and runtime consumers must stay
//! dormant or preserve the marker through the atomic transition seam.

use std::fs;
use std::path::{Path, PathBuf};

const MARKER: &str = "Pipeline Contract: test-first-v1";
const REFINING: &str = "plugins/gal-core/commands/refining-plan/SKILL.template.md";
const PLAN_TO_PROMPT: &str = "plugins/gal-core/commands/plan-to-prompt/SKILL.template.md";
const TEMPLATE_DIR: &str = "plugins/gal-core/templates";

fn repo_file(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has a workspace parent")
        .parent()
        .expect("workspace has a repository root")
        .join(relative)
}

fn read(relative: &str) -> String {
    fs::read_to_string(repo_file(relative)).unwrap_or_else(|error| {
        panic!("failed to read {relative}: {error}");
    })
}

/// True when `bytes` presents the exact marker line as its own line inside a
/// fenced code block (an authored example), as opposed to embedding it
/// mid-sentence in prose with inline backticks.
fn marker_in_fenced_example(bytes: &str) -> bool {
    let mut in_fence = false;
    for line in bytes.lines() {
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence && line.trim() == MARKER {
            return true;
        }
    }
    false
}

fn anchored_marker(bytes: &str) -> bool {
    let mut before_heading = true;
    let mut found = false;
    for line in bytes.lines() {
        if line.starts_with("##") {
            before_heading = false;
        }
        if before_heading && line == MARKER {
            assert!(!found, "the contract marker must occur at most once");
            found = true;
        }
    }
    found
}

fn marker_is_valid(bytes: &str) -> bool {
    let mut lines = bytes.lines();
    let Some(first) = lines.next() else {
        return false;
    };
    if !first.starts_with('#') || first.starts_with("##") {
        return false;
    }

    let mut marker_count = 0;
    for line in lines {
        if line.starts_with("##") {
            break;
        }
        if line == MARKER {
            marker_count += 1;
        } else if line.contains("Pipeline Contract:") {
            return false;
        }
    }
    marker_count == 1
}

#[test]
fn only_the_two_planning_emitters_can_activate_the_contract() {
    let refining = read(REFINING);
    let plan_to_prompt = read(PLAN_TO_PROMPT);

    assert!(
        refining.contains(MARKER),
        "refining-plan must emit the exact source marker"
    );
    assert!(
        plan_to_prompt.contains(MARKER),
        "plan-to-prompt must preserve the exact marker"
    );
    assert!(
        refining.contains("source marker"),
        "refining-plan must identify the marker as source-owned"
    );
    assert!(
        plan_to_prompt.contains("preserve") && plan_to_prompt.contains("validat"),
        "plan-to-prompt must preserve and validate marker state"
    );

    // A file "emits" the marker only when it presents the exact marker line as
    // its own line inside a fenced code block — a genuine authored example of
    // the literal text, not merely a conditional runtime reference embedded in
    // prose (e.g. "Under `Pipeline Contract: test-first-v1`, when a task..."
    // in gal-pipeline's own contract, which legitimately branches on the
    // marker's presence without ever originating it).
    let mut emitters = 0;
    for entry in fs::read_dir(repo_file("plugins/gal-core/commands")).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path().join("SKILL.template.md");
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path).unwrap();
        if marker_in_fenced_example(&text) {
            emitters += 1;
            assert!(
                path.ends_with(Path::new("refining-plan").join("SKILL.template.md"))
                    || path.ends_with(Path::new("plan-to-prompt").join("SKILL.template.md")),
                "unexpected activation authority: {}",
                path.display()
            );
        }
    }
    assert_eq!(
        emitters, 2,
        "exactly two command authorities may present the marker as an authored example"
    );
}

#[test]
fn marker_is_anchored_and_partial_forms_fail_closed() {
    let valid = format!("# Plan\n\n{MARKER}\n\n## Tasks\n");
    assert!(anchored_marker(&valid));
    assert!(marker_is_valid(&valid));

    for partial in [
        "# Plan\n\nPipeline Contract: test-first\n\n## Tasks\n",
        "# Plan\n\n`Pipeline Contract: test-first-v1`\n\n## Tasks\n",
        "# Plan\n\nPipeline Contract: test-first-v1 extra\n\n## Tasks\n",
        "# Plan\n\n## Tasks\n\nPipeline Contract: test-first-v1\n",
    ] {
        assert!(!anchored_marker(partial));
        assert!(!marker_is_valid(partial));
    }
}

#[test]
fn templates_remain_dormant_and_markerless() {
    for entry in fs::read_dir(repo_file(TEMPLATE_DIR)).unwrap() {
        let entry = entry.unwrap();
        if entry.path().is_file() {
            let text = fs::read_to_string(entry.path()).unwrap();
            assert!(
                !anchored_marker(&text),
                "template {} must not emit an activation marker",
                entry.path().display()
            );
        }
    }
}

#[test]
fn prompt_installation_is_delegated_to_the_atomic_cas_transition() {
    let contract = read(PLAN_TO_PROMPT);
    assert!(contract.contains("test-first-transition"));
    assert!(contract.contains("CAS") || contract.contains("compare-and-swap"));
    assert!(contract.contains("atomic"));
    assert!(
        contract.contains("candidate") && contract.contains("journal"),
        "installation must name the crash-safe transition artifacts"
    );
}

#[test]
fn markerless_prompts_keep_the_legacy_path() {
    let refining = read(REFINING);
    let plan_to_prompt = read(PLAN_TO_PROMPT);
    assert!(refining.contains("markerless") && refining.contains("legacy"));
    assert!(plan_to_prompt.contains("Markerless") && plan_to_prompt.contains("Legacy"));
}
