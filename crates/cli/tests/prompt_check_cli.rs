use std::process::Command;
use tempfile::TempDir;

fn task_id(number: u32) -> String {
    format!("T-{number:02}")
}

fn prompt(sentinel: &str, add_cli_row: bool) -> String {
    let id = task_id(1);
    let mut map = format!("| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {id} | unit | `touch ");
    map.push_str(sentinel);
    map.push_str("` | pass | TESTER | output |\n");
    if add_cli_row {
        map.push_str(&format!("| {id} | cli | `touch "));
        map.push_str(sentinel);
        map.push_str("` | pass | TESTER | output |\n");
    }
    format!(
        "## Status\nCurrent Task: {id}\n\n## Tasks\n- [ ] {id} — verify map\n  - **Verification Impact**: `{{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":true,\"cross_boundary\":false}}`\n\n## Verification Map\n{map}\n## Test Results\nPending\n\n## Review Results\n### Architecture Review\nCLEAR\n### Business Review\nNot requested.\n### Design Review\nNot requested.\n### Engineering Review\nCLEAR\n"
    )
}

#[test]
fn prompt_check_writes_receipt_for_coverage_and_never_runs_proof_command() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    let tools = temp.path().join("tools");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&tools).unwrap();
    let binary = tools.join(
        std::path::Path::new(env!("CARGO_BIN_EXE_gal"))
            .file_name()
            .unwrap(),
    );
    std::fs::copy(env!("CARGO_BIN_EXE_gal"), &binary).unwrap();
    let sentinel = temp.path().join("proof-command-ran");
    let prompt_path = temp.path().join("valid.prompt.md");
    let receipt_path = temp.path().join("valid.receipt.md");
    std::fs::write(&prompt_path, prompt(&sentinel.to_string_lossy(), true)).unwrap();

    let output = Command::new(&binary)
        .current_dir(&repo)
        .args(["prompt-check"])
        .arg(&prompt_path)
        .args(["--receipt"])
        .arg(&receipt_path)
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt = std::fs::read_to_string(receipt_path).unwrap();
    assert!(receipt.contains("overall: pass"));
    assert!(receipt.contains(&format!("| verification-map-{} | pass |", task_id(1))));
    assert!(!sentinel.exists(), "prompt-check executed a Proof Command");
}

#[test]
fn prompt_check_reports_missing_required_layer_and_nonzero_exit() {
    let temp = TempDir::new().unwrap();
    let repo = temp.path().join("repo");
    let tools = temp.path().join("tools");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::create_dir_all(&tools).unwrap();
    let binary = tools.join(
        std::path::Path::new(env!("CARGO_BIN_EXE_gal"))
            .file_name()
            .unwrap(),
    );
    std::fs::copy(env!("CARGO_BIN_EXE_gal"), &binary).unwrap();
    let sentinel = temp.path().join("must-not-run");
    let prompt_path = temp.path().join("invalid.prompt.md");
    let receipt_path = temp.path().join("invalid.receipt.md");
    std::fs::write(&prompt_path, prompt(&sentinel.to_string_lossy(), false)).unwrap();

    let output = Command::new(&binary)
        .current_dir(&repo)
        .args(["prompt-check"])
        .arg(&prompt_path)
        .args(["--receipt"])
        .arg(&receipt_path)
        .output()
        .unwrap();

    assert!(!output.status.success());
    let receipt = std::fs::read_to_string(receipt_path).unwrap();
    assert!(receipt.contains("overall: fail"));
    assert!(receipt.contains(&format!("| verification-map-{} | fail |", task_id(1))));
    assert!(!sentinel.exists(), "prompt-check executed a Proof Command");
}
