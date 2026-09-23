use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn run_gal(repo: &Path, args: &[&str]) -> (i32, String, String) {
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(args)
        .output()
        .expect("failed to spawn gal binary");
    (
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout).to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn sentinel_project_md() -> String {
    "# My Existing Project\n\n\
     ## What This Is\n\nSentinel body that must survive.\n\n\
     ## Tech Stack\n\n| Layer | Technology |\n| --- | --- |\n| Language | Rust |\n\n\
     ## Architecture\n\nSentinel architecture.\n\n\
     ## Constraints\n\n- none\n\n\
     ## Response Style\n\n- concise\n\n\
     ## Freshness\n\n- none\n\n\
     ## Project Language\n\n- `PROJECT_LANGUAGE`: `en`\n\n\
     ## Protected Paths\n\n- none\n"
        .to_string()
}

#[test]
fn init_refuses_already_initialized_repo() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    // First bootstrap creates the initialized repository
    let (code, _stdout, stderr) = run_gal(repo, &["init"]);
    assert_eq!(code, 0, "first gal init must succeed: {stderr}");
    let project_before = fs::read_to_string(repo.join(".dev").join("project.md")).unwrap();
    let state_before = fs::read_to_string(repo.join(".dev").join("state.md")).unwrap();
    let agents_before = fs::read_to_string(repo.join("AGENTS.md")).unwrap();
    let claude_before = fs::read_to_string(repo.join("CLAUDE.md")).unwrap();

    // Second run: must refuse already initialized repo
    let (code, stdout, stderr) = run_gal(repo, &["init"]);
    assert!(
        stderr.contains("already initialized"),
        "stderr must contain 'already initialized', got code {code}, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(
        code, 1,
        "gal init on initialized repo must exit 1, got code {code}, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stderr.contains("gal render-adapters"),
        "stderr must contain 'gal render-adapters', got: {stderr}"
    );

    // Both .dev files and both adapter roots must be byte-identical
    assert_eq!(
        fs::read_to_string(repo.join(".dev").join("project.md")).unwrap(),
        project_before
    );
    assert_eq!(
        fs::read_to_string(repo.join(".dev").join("state.md")).unwrap(),
        state_before
    );
    assert_eq!(
        fs::read_to_string(repo.join("AGENTS.md")).unwrap(),
        agents_before
    );
    assert_eq!(
        fs::read_to_string(repo.join("CLAUDE.md")).unwrap(),
        claude_before
    );
}

#[test]
fn init_refuses_half_initialized_repo_with_project_only() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    let dev_dir = repo.join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    let project_content = sentinel_project_md();
    fs::write(dev_dir.join("project.md"), &project_content).unwrap();

    let (code, stdout, stderr) = run_gal(repo, &["init"]);
    assert_eq!(
        code, 1,
        "gal init must exit 1 when only .dev/project.md is present, got code {code}, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stderr.contains(".dev/state.md is missing"),
        "stderr must contain '.dev/state.md is missing', got: {stderr}"
    );
    assert!(
        !repo.join(".dev").join("state.md").exists(),
        ".dev/state.md must still be absent"
    );
    assert!(
        !stdout.contains("Preserved"),
        "stdout must not contain 'Preserved', got: {stdout}"
    );
}

#[test]
fn init_refuses_half_initialized_repo_with_state_only() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();
    let dev_dir = repo.join(".dev");
    fs::create_dir_all(&dev_dir).unwrap();
    let state_content = "# State Sentinel\n\nsentinel state content\n";
    fs::write(dev_dir.join("state.md"), state_content).unwrap();

    let (code, stdout, stderr) = run_gal(repo, &["init"]);
    assert_eq!(
        code, 1,
        "gal init must exit 1 when only .dev/state.md is present, got code {code}, stdout: {stdout}, stderr: {stderr}"
    );
    assert!(
        stderr.contains(".dev/project.md is missing"),
        "stderr must contain '.dev/project.md is missing', got: {stderr}"
    );
    assert!(
        !repo.join(".dev").join("project.md").exists(),
        ".dev/project.md must still be absent"
    );
    assert!(
        !stdout.contains("Preserved"),
        "stdout must not contain 'Preserved', got: {stdout}"
    );
}

#[test]
fn init_fresh_bootstrap_succeeds_on_empty_repo() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    let (code, _stdout, stderr) = run_gal(repo, &["init"]);
    assert_eq!(code, 0, "gal init on empty repo must succeed: {stderr}");
    assert!(repo.join(".dev").join("project.md").is_file());
    assert!(repo.join(".dev").join("state.md").is_file());
    assert!(repo.join("AGENTS.md").is_file());
    assert!(repo.join("CLAUDE.md").is_file());
}
