use sha2::Digest;
use std::process::Command;
use tempfile::TempDir;

fn receipt_field<'a>(text: &'a str, name: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.strip_prefix(&format!("{name}: ")))
        .unwrap_or_else(|| panic!("missing {name} field in receipt:\n{text}"))
}

fn assert_standalone_identity(text: &str) {
    assert_eq!(receipt_field(text, "binding_scope"), "standalone");

    let binding = receipt_field(text, "execution_binding_sha256");
    assert_eq!(
        binding.len(),
        64,
        "invalid execution binding digest: {binding}"
    );
    assert!(binding.bytes().all(|byte| byte.is_ascii_hexdigit()));

    let executable_path = receipt_field(text, "executable_path");
    let executable_path = std::path::Path::new(executable_path);
    assert!(
        executable_path.is_absolute(),
        "executable path is not absolute: {}",
        executable_path.display()
    );
    let executable = std::fs::read(executable_path).unwrap_or_else(|error| {
        panic!(
            "cannot read bound executable {}: {error}",
            executable_path.display()
        )
    });
    let actual_hash = format!("{:x}", sha2::Sha256::digest(&executable));
    assert_eq!(receipt_field(text, "executable_sha256"), actual_hash);
}

#[test]
fn planning_receipt_records_execution_identity_and_standalone_scope() {
    let temp = TempDir::new().unwrap();
    let plan = temp.path().join("plan.md");
    let receipt = temp.path().join("receipt.md");
    std::fs::write(&plan, "# Plan\n").unwrap();
    let _ = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(temp.path())
        .args([
            "planning-check",
            plan.to_str().unwrap(),
            "--receipt",
            receipt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let text = std::fs::read_to_string(receipt).unwrap();
    assert_standalone_identity(&text);
}

#[test]
fn pipeline_preflight_receipt_round_trips_through_doctor_verifier() {
    let binary = env!("CARGO_BIN_EXE_gal");
    let temp = TempDir::new().unwrap();
    let plans = temp.path().join(".dev/plans");
    std::fs::create_dir_all(&plans).unwrap();
    let prompt = plans.join("hotfix.prompt.md");
    let receipt = temp.path().join("preflight.receipt.md");
    let task_id = format!("T-{:02}", 1);
    let test_id = format!("TP-{:02}", 1);
    std::fs::write(
        &prompt,
        format!(
            "## Status\nWorkflow: DRAFT\nCurrent Task: —\n\n## Tasks\n- [ ] {task_id} — hotfix\n\n## Test Plan\n| ID | Type | Description | Covers |\n| --- | --- | --- | --- |\n| {test_id} | unit | verify receipt | {task_id} |\n"
        ),
    )
    .unwrap();
    std::fs::write(
        temp.path().join(".dev/state.md"),
        "## Active Plans\n\n| Plan | Prompt | Status | Updated |\n| --- | --- | --- | --- |\n| hotfix | `.dev/plans/hotfix.prompt.md` | ready | 2026-10-07 |\n",
    )
    .unwrap();

    let preflight = Command::new(binary)
        .current_dir(temp.path())
        .args([
            "pipeline-preflight",
            prompt.to_str().unwrap(),
            "--receipt",
            receipt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        preflight.status.success(),
        "pipeline-preflight failed: {}",
        String::from_utf8_lossy(&preflight.stderr)
    );

    let verify = Command::new(binary)
        .current_dir(temp.path())
        .args([
            "doctor",
            "--verify-receipt",
            receipt.to_str().unwrap(),
            "--expect-scope",
            "standalone",
        ])
        .output()
        .unwrap();
    assert!(
        verify.status.success(),
        "receipt verification failed: {}",
        String::from_utf8_lossy(&verify.stderr)
    );

    let boundary_receipt = temp.path().join("boundary.receipt.md");
    let _boundary = Command::new(binary)
        .current_dir(temp.path())
        .args([
            "boundary-check",
            prompt.to_str().unwrap(),
            "--task",
            &task_id,
            "--receipt",
            boundary_receipt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let verify_boundary = Command::new(binary)
        .current_dir(temp.path())
        .args([
            "doctor",
            "--verify-receipt",
            boundary_receipt.to_str().unwrap(),
            "--expect-scope",
            "standalone",
        ])
        .output()
        .unwrap();
    assert!(
        verify_boundary.status.success(),
        "boundary receipt verification failed: {}",
        String::from_utf8_lossy(&verify_boundary.stderr)
    );
}

#[test]
fn refining_prompt_and_finalize_receipts_include_standalone_envelopes() {
    let binary = env!("CARGO_BIN_EXE_gal");
    let temp = TempDir::new().unwrap();
    let plan = temp.path().join("plan.md");
    let prompt = temp.path().join("plan.prompt.md");
    std::fs::write(&plan, "# Plan\n").unwrap();
    let task_label = format!("T-{:02}", 1);
    let prompt_text = format!("## Status\nCurrent Task: -\n\n## Tasks\n- [ ] {task_label} - do work\n\n## Test Results\nPending\n\n## Review Results\n### Architecture Review\nCLEAR\n### Business Review\nNot requested.\n### Design Review\nNot requested.\n### Engineering Review\nCLEAR\n");
    std::fs::write(&prompt, prompt_text).unwrap();
    for (command, target, extra) in [
        ("refining-check", plan.as_path(), Vec::<&str>::new()),
        ("prompt-check", prompt.as_path(), Vec::<&str>::new()),
        ("finalize-check", plan.as_path(), vec!["--hygiene-only"]),
    ] {
        let receipt = temp.path().join(format!("{command}.receipt.md"));
        let output = Command::new(binary)
            .current_dir(temp.path())
            .args([command, target.to_str().unwrap()])
            .args(extra)
            .args(["--receipt", receipt.to_str().unwrap()])
            .output()
            .unwrap();
        let text = std::fs::read_to_string(&receipt).unwrap_or_else(|_| {
            panic!(
                "{command} did not write receipt: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        assert_standalone_identity(&text);
    }
}

#[test]
fn finalize_receipt_records_coordinator_binding() {
    use pipeline::coordinator::{CoordinatorState, ExecutionBinding};
    let binary = env!("CARGO_BIN_EXE_gal");
    let temp = TempDir::new().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let executable = std::path::Path::new(binary).canonicalize().unwrap();
    let normalized = |path: &std::path::Path| {
        let text = path.to_string_lossy();
        text.strip_prefix(r"\\?\")
            .unwrap_or(&text)
            .replace('\\', "/")
    };
    let executable_path = normalized(&executable);
    let executable_sha256 = format!(
        "{:x}",
        sha2::Sha256::digest(std::fs::read(&executable).unwrap())
    );
    let scope = "coordinator:binding-test";
    let binding = ExecutionBinding {
        version: ExecutionBinding::VERSION,
        binding_scope: scope.to_string(),
        worktree_root: normalized(&root),
        executable_path,
        executable_sha256,
    };
    let mut state = CoordinatorState::new_legacy("binding-test", "prompt", "T-EV", "implement");
    state.execution_binding = Some(binding.clone());
    let coordinator = root.join(".dev/pipeline/binding-test/coordinator.json");
    std::fs::create_dir_all(coordinator.parent().unwrap()).unwrap();
    std::fs::write(&coordinator, serde_json::to_vec(&state).unwrap()).unwrap();
    let plan = root.join("plan.md");
    let receipt = root.join("finalize.receipt.md");
    std::fs::write(&plan, "# Plan\n").unwrap();
    let output = Command::new(binary)
        .current_dir(&root)
        .args([
            "finalize-check",
            "plan.md",
            "--hygiene-only",
            "--scope",
            scope,
            "--receipt",
            receipt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let text = std::fs::read_to_string(&receipt).unwrap_or_else(|_| {
        panic!(
            "coordinator finalize check did not write receipt: {}",
            String::from_utf8_lossy(&output.stderr)
        )
    });
    assert_eq!(receipt_field(&text, "binding_scope"), scope);
    assert_eq!(receipt_field(&text, "plan_scope"), "binding-test");
    assert_eq!(
        receipt_field(&text, "coordinator_path"),
        coordinator
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .replace('\\', "/")
    );
    assert_eq!(
        receipt_field(&text, "execution_binding_sha256"),
        binding.digest().unwrap()
    );
}

#[test]
fn installed_parent_in_source_worktree_cannot_write_pass_receipt() {
    let temp = TempDir::new().unwrap();
    let plan = temp.path().join("plan.md");
    let receipt = temp.path().join("receipt.md");
    std::fs::write(&plan, "# Plan\n").unwrap();
    let source_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(source_root)
        .args([
            "planning-check",
            plan.to_str().unwrap(),
            "--receipt",
            receipt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("gal handoff:"),
        "source-worktree invocation did not hand off: {stderr}"
    );
    assert!(
        stderr.contains("child_path="),
        "handoff did not report child path: {stderr}"
    );
    let wrote_pass = std::fs::read_to_string(&receipt)
        .map(|text| text.contains("overall: pass"))
        .unwrap_or(false);
    println!(
        "installed parent direct-PATH pass receipt: {}",
        if wrote_pass { "written" } else { "blocked" }
    );
    assert!(
        !wrote_pass,
        "installed parent wrote pass evidence in a source worktree"
    );
    assert!(!output.status.success() || !wrote_pass);
}
