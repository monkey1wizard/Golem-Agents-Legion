use gal_foundation::runtime::VALID_RUNTIMES;
use projection::{run_update_commands, CommandUpdateOptions, ProjectionSource};
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

fn canonical_command_path(home: &Path) -> PathBuf {
    home.join(".gal")
        .join("plugins")
        .join("gal")
        .join("commands")
        .join("gal-pipeline")
        .join("SKILL.md")
}

fn carrier_content(runtime: &str, home: &Path) -> PathBuf {
    match runtime {
        "copilot" => home
            .join(".copilot")
            .join("skills")
            .join("gal-pipeline")
            .join("SKILL.md"),
        "antigravity" => home
            .join(".gemini")
            .join("antigravity-cli")
            .join("skills")
            .join("gal-pipeline")
            .join("SKILL.md"),
        "codex" => home
            .join(".agents")
            .join("skills")
            .join("gal-pipeline")
            .join("SKILL.md"),
        "opencode" => home
            .join(".config")
            .join("opencode")
            .join("commands")
            .join("gal-pipeline.md"),
        "claude" => home
            .join(".claude")
            .join("commands")
            .join("gal-pipeline.md"),
        other => panic!("unsupported runtime carrier: {other}"),
    }
}

fn pipeline_body(content: &str) -> String {
    let content = if content.starts_with("---") {
        content
            .find("\n---")
            .and_then(|end| {
                content[end + 1..]
                    .find('\n')
                    .map(|offset| end + 1 + offset + 1)
            })
            .map(|start| &content[start..])
            .unwrap_or(content)
    } else {
        content
    };
    content
        .split_once("User command arguments, if any: $ARGUMENTS\n\n")
        .map(|(_, body)| body.trim())
        .unwrap_or_else(|| content.trim())
        .replace("\r\n", "\n")
}

#[test]
fn pipeline_contract_is_byte_identical_across_every_runtime_carrier() {
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repository root");
    let source_root = repository_root.join("plugins").join("gal-core");
    let temporary_home = tempdir().expect("temporary home");
    let mut carrier_count = 0;

    for runtime in VALID_RUNTIMES {
        let report = run_update_commands(&CommandUpdateOptions {
            repo_root: repository_root.clone(),
            sources: vec![ProjectionSource {
                root: source_root.clone(),
                id: "gal-core".into(),
                persistent: true,
            }],
            user_home: temporary_home.path().to_path_buf(),
            selected_runtimes: vec![(*runtime).to_string()],
            dry_run: false,
            replace: true,
        })
        .unwrap_or_else(|error| panic!("projection failed for {runtime}: {error}"));

        let canonical = canonical_command_path(temporary_home.path());
        let carrier = carrier_content(runtime, temporary_home.path());
        assert!(carrier.is_file(), "missing carrier for {runtime}");
        let canonical_bytes = fs::read(&canonical).expect("baked canonical command");
        let carrier_bytes = fs::read(&carrier).expect("runtime carrier command");
        let carrier_contract =
            pipeline_body(std::str::from_utf8(&carrier_bytes).expect("UTF-8 carrier"));
        let canonical_contract =
            pipeline_body(std::str::from_utf8(&canonical_bytes).expect("UTF-8 canonical"));
        assert_eq!(
            carrier_contract,
            canonical_contract,
            "pipeline contract drifted for {runtime}: carrier={} canonical={}",
            carrier_contract.len(),
            canonical_contract.len()
        );
        assert!(
            report.written_files.contains(&carrier),
            "carrier was not reported for {runtime}"
        );
        carrier_count += 1;
    }

    assert_eq!(carrier_count, VALID_RUNTIMES.len());
}
