use gal_engine::render::render_canonical_root_to;
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn core_render_delivers_writing_quality_to_rules_and_copilot_projection() {
    let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root")
        .join("plugins")
        .join("gal-core");
    let output = TempDir::new().expect("temporary render root");

    render_canonical_root_to(&source, &output.path().join("gal"), false, "0.0.0-test")
        .expect("core-only render must succeed");

    let root = output.path().join("gal");
    let rules = fs::read_to_string(root.join("rules").join("gal.md"))
        .expect("rendered GAL rules must exist");
    let copilot = fs::read_to_string(root.join("com.github.copilot").join("rules").join("gal.md"))
        .expect("rendered Copilot rules must exist");

    let convention = fs::read_to_string(source.join("conventions").join("writing-quality.md"))
        .expect("writing-quality.md must exist");
    let convention_len = convention.len();
    assert!(
        convention_len <= 4096,
        "writing-quality.md must stay <= 4096 bytes, found {convention_len}"
    );

    for obligation in [
        "actual recipient",
        "tool observations actually made in this task",
        "inferences, assumptions, suggestions, placeholders, and future commitments",
        "Before every message",
        "actual unsent draft",
        "Compare each material factual assertion",
        "with specific support",
        "Remove or qualify unsupported claims",
        "Missing completion evidence does not establish that work never started",
        "Honor artifact-only requests unless a higher-priority host instruction requires otherwise",
        "in-process",
        "progress messages",
        "run it on that draft",
        "at most two careful repairs",
        "contextual review prompts, never evidence of AI authorship",
        "automatic fixes only under an explicit safe-fix configuration",
        "Undo any repair",
        "hard findings or operational failure",
        "Advisory findings alone do not block",
        "disclose the limitation once",
        "recursively check checker-status output",
        "When absent or off",
        "do not prove execution or host interception",
    ] {
        assert!(
            convention.contains(obligation),
            "missing obligation: {obligation}"
        );
    }

    for rendered in [&rules, &copilot] {
        assert!(rendered.contains("### writing-quality.md"));
        assert!(
            rendered.contains(&convention),
            "rendered writing-quality.md must contain the full canonical contract"
        );
    }
    assert!(!output.path().join("gal").join("bin").exists());
}
