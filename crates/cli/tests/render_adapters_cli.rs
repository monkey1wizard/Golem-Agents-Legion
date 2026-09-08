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

#[test]
fn render_adapters_cli_end_to_end() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path();

    // Prepare tempdir with `gal init`
    let (code, _stdout, stderr) = run_gal(repo, &["init"]);
    assert_eq!(code, 0, "gal init must succeed: {stderr}");
    assert!(repo.join(".dev").join("project.md").is_file());
    assert!(repo.join(".dev").join("state.md").is_file());
    assert!(repo.join("AGENTS.md").is_file());
    assert!(repo.join("CLAUDE.md").is_file());

    // First run of render-adapters: exits 0 and stdout contains "Rendered repo-local adapters in:"
    // plus one row for AGENTS.md and one for CLAUDE.md.
    let (code, stdout, stderr) = run_gal(repo, &["render-adapters"]);
    assert!(
        stdout.contains("Rendered repo-local adapters in:"),
        "Rendered repo-local adapters in: expected in stdout, got code {code}, stdout: {stdout}, stderr: {stderr}"
    );
    assert_eq!(code, 0, "render-adapters must succeed: {stderr}");

    assert!(
        stdout.contains("AGENTS.md"),
        "stdout must contain row for AGENTS.md, got: {stdout}"
    );
    assert!(
        stdout.contains("CLAUDE.md"),
        "stdout must contain row for CLAUDE.md, got: {stdout}"
    );

    // Second run: exits 0 with both rows Unchanged
    let (code, stdout, stderr) = run_gal(repo, &["render-adapters"]);
    assert_eq!(code, 0, "second render-adapters must succeed: {stderr}");
    assert!(
        stdout.contains("- Unchanged: AGENTS.md"),
        "stdout must report AGENTS.md Unchanged, got: {stdout}"
    );
    assert!(
        stdout.contains("- Unchanged: CLAUDE.md"),
        "stdout must report CLAUDE.md Unchanged, got: {stdout}"
    );

    // After deleting .dev/state.md, a third run exits 0 and stdout does not contain .dev/state.md
    std::fs::remove_file(repo.join(".dev").join("state.md")).unwrap();
    let (code, stdout, stderr) = run_gal(repo, &["render-adapters"]);
    assert_eq!(code, 0, "third render-adapters must succeed: {stderr}");
    assert!(
        !stdout.contains(".dev/state.md"),
        "stdout must not contain .dev/state.md, got: {stdout}"
    );

    // In a fresh empty TempDir it exits 1 and stderr contains "Run gal init first"
    let empty_temp = TempDir::new().unwrap();
    let (code, stdout, stderr) = run_gal(empty_temp.path(), &["render-adapters"]);
    assert_eq!(
        code, 1,
        "empty repo render-adapters must exit 1, got code {code}, stdout: {stdout}"
    );
    assert!(
        stderr.contains("Run gal init first"),
        "stderr must contain 'Run gal init first', got: {stderr}"
    );

    // gal --help stdout contains render-adapters
    let (code, stdout, stderr) = run_gal(repo, &["--help"]);
    assert_eq!(code, 0, "gal --help must succeed: {stderr}");
    assert!(
        stdout.contains("render-adapters"),
        "gal --help must contain 'render-adapters', got: {stdout}"
    );
}
