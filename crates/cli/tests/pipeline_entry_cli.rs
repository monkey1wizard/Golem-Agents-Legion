use sha2::{Digest, Sha256};
use std::{fs, path::Path, process::Command};
use tempfile::TempDir;

fn tid() -> String {
    format!("T-{:02}", 1)
}

fn tpid() -> String {
    format!("TP-{:02}", 1)
}

fn hash_file(path: &Path) -> String {
    let bytes = fs::read(path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    format!("{:x}", hasher.finalize())
}

#[test]
fn installed_parent_hands_off_every_trust_bearing_command() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let installed = TempDir::new().unwrap();
    let parent_source = Path::new(env!("CARGO_BIN_EXE_gal"));
    let parent = installed
        .path()
        .join(if cfg!(windows) { "gal.exe" } else { "gal" });
    fs::copy(parent_source, &parent).unwrap();
    let parent_hash = hash_file(&parent);
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(installed.path().to_path_buf()).chain(std::env::split_paths(&old_path)),
    )
    .unwrap();

    let commands = [
        "pipeline",
        "pipeline-preflight",
        "boundary-check",
        "pipeline-converge-check",
        "pipeline-handback-check",
        "planning-check",
        "refining-check",
        "prompt-check",
        "finalize-check",
        "naming-gate",
    ];
    for command in commands {
        let output = Command::new(if cfg!(windows) { "gal.exe" } else { "gal" })
            .current_dir(source)
            .env("PATH", &path)
            .arg(command)
            .arg("--handoff-probe")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let handoff = stderr
            .lines()
            .find(|line| line.contains("gal handoff: action="))
            .unwrap_or_else(|| panic!("{command}: expected handoff log; stderr={stderr}"));
        let action = format!("action={command}");
        assert!(handoff.contains(&action), "{handoff}");
        let child_path = handoff
            .split("child_path=")
            .nth(1)
            .and_then(|rest| rest.split(" child_sha256=").next())
            .expect("handoff log must include absolute child path");
        let child = Path::new(child_path);
        assert!(child.is_absolute(), "{handoff}");
        let child_hash = hash_file(child);
        assert!(
            handoff.contains(&format!("child_sha256={child_hash}")),
            "{handoff}"
        );
        let exit_status = output.status.code().unwrap_or(1);
        assert!(
            handoff.contains(&format!("exit_status={exit_status}")),
            "{handoff}"
        );
        println!(
            "handoff evidence: action={command} parent_sha256={parent_hash} child_path={} child_sha256={child_hash} exit_status={exit_status}",
            child.display()
        );
    }
}

#[test]
fn installed_parent_hands_off_from_source_checkout_subdirectory() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let private_root = fs::canonicalize(source)
        .unwrap()
        .join("target/gal-pipeline/bin");
    let installed = TempDir::new().unwrap();
    let parent = installed
        .path()
        .join(if cfg!(windows) { "gal.exe" } else { "gal" });
    fs::copy(env!("CARGO_BIN_EXE_gal"), &parent).unwrap();

    for (subdirectory, command) in [
        ("crates", "naming-gate"),
        ("docs", "pipeline-handback-check"),
        ("crates/cli/src", "finalize-check"),
    ] {
        let cwd = source.join(subdirectory);
        assert!(cwd.is_dir(), "missing fixture directory {}", cwd.display());
        let output = Command::new(&parent)
            .current_dir(&cwd)
            .arg(command)
            .arg("--handoff-probe")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        let handoff = stderr
            .lines()
            .find(|line| line.contains("gal handoff: action="))
            .unwrap_or_else(|| {
                panic!("{command} from {subdirectory}: expected handoff log; stderr={stderr}")
            });
        assert!(handoff.contains(&format!("action={command}")), "{handoff}");
        let child_path = handoff
            .split("child_path=")
            .nth(1)
            .and_then(|rest| rest.split(" child_sha256=").next())
            .expect("handoff log must include absolute child path");
        let child = fs::canonicalize(child_path).unwrap();
        assert!(
            child.starts_with(&private_root),
            "{command} from {subdirectory}: child {} is outside {}",
            child.display(),
            private_root.display()
        );
        println!(
            "subdirectory handoff evidence: cwd={subdirectory} action={command} child_path={}",
            child.display()
        );
    }
}

#[test]
fn guarded_entry_binding_is_accepted_by_finalize_and_doctor_receipt_verifiers() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("plan.prompt.md");
    fs::write(&prompt, "# Test Plan\n\n## Tasks\n\n- [ ] @T@ initial task\n\n## Test Plan\n\n@TP@ spec test\n\n## Status\n\nWorkflow: IMPLEMENT\n".replace("@T@", &tid()).replace("@TP@", &tpid())).unwrap();
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "plan"])
        .status()
        .unwrap()
        .success());
    let prompt_hash = hash_file(&prompt);
    let worktree = fs::canonicalize(repo.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        300,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );

    // The production guarded entry persists the coordinator execution binding.
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .env_remove("GAL_PINNED_EXECUTABLE_SHA256")
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let coordinator: pipeline::coordinator::CoordinatorState = serde_json::from_slice(
        &fs::read(repo.path().join(".dev/pipeline/plan/coordinator.json")).unwrap(),
    )
    .unwrap();
    let binding = coordinator
        .execution_binding
        .expect("guarded entry must persist an execution binding");
    assert!(!binding.worktree_root.starts_with("//?/"), "{binding:?}");
    assert!(!binding.executable_path.starts_with("//?/"), "{binding:?}");

    let plan = repo.path().join(".dev/plans/plan.md");
    fs::write(&plan, "# Plan\n").unwrap();
    for cwd in [repo.path().to_path_buf(), prompt_dir.clone()] {
        let receipt = repo.path().join("finalize.receipt.md");
        let _ = fs::remove_file(&receipt);
        let output = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(&cwd)
            .args([
                "finalize-check",
                plan.to_str().unwrap(),
                "--hygiene-only",
                "--scope",
                "coordinator:plan",
                "--receipt",
                receipt.to_str().unwrap(),
            ])
            .output()
            .unwrap();
        let text = fs::read_to_string(&receipt).unwrap_or_else(|_| {
            panic!(
                "finalize-check from {} rejected the guarded entry binding: {}",
                cwd.display(),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        let field = |name: &str| {
            text.lines()
                .find_map(|line| line.strip_prefix(&format!("{name}: ")))
                .unwrap_or_else(|| panic!("missing {name} in receipt:\n{text}"))
                .to_string()
        };
        assert_eq!(field("binding_scope"), "coordinator:plan");
        assert_eq!(field("executable_path"), binding.executable_path);
        assert_eq!(field("execution_binding_sha256"), binding.digest().unwrap());

        let output = Command::new(env!("CARGO_BIN_EXE_gal"))
            .current_dir(&cwd)
            .args([
                "doctor",
                "--verify-receipt",
                receipt.to_str().unwrap(),
                "--expect-scope",
                "coordinator:plan",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "doctor rejected the guarded entry receipt from {}: {}",
            cwd.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn checkout_with_unrelated_cli_package_does_not_start_a_build() {
    let fixture = TempDir::new().unwrap();
    let cli = fixture.path().join("crates/cli");
    let engine = fixture.path().join("crates/gal-engine");
    fs::create_dir_all(&cli).unwrap();
    fs::create_dir_all(&engine).unwrap();
    fs::write(
        fixture.path().join("Cargo.toml"),
        "[workspace]\nmembers = []\n",
    )
    .unwrap();
    fs::write(
        cli.join("Cargo.toml"),
        "[package]\nname = \"unrelated-cli\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    fs::write(
        engine.join("Cargo.toml"),
        "[package]\nname = \"gal-engine\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();

    let shim = fixture.path().join("shim");
    fs::create_dir_all(&shim).unwrap();
    let marker = fixture.path().join("cargo-started");
    #[cfg(windows)]
    {
        let script = shim.join("cargo.cmd");
        fs::write(
            &script,
            format!("@echo started > \"{}\"\r\n", marker.display()),
        )
        .unwrap();
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let script = shim.join("cargo");
        fs::write(
            &script,
            format!("#!/bin/sh\nprintf started > '{}'\n", marker.display()),
        )
        .unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(script, permissions).unwrap();
    }
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(std::iter::once(shim).chain(std::env::split_paths(&old_path)))
        .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(fixture.path())
        .env("PATH", path)
        .args(["pipeline-preflight", "--help"])
        .output()
        .unwrap();

    assert!(
        !marker.exists(),
        "cargo build started for an unrelated checkout"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("private executable unavailable"),
        "{stderr}"
    );
}

#[test]
fn private_generation_hash_mismatch_rejects_checks_before_handler() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    let installed = TempDir::new().unwrap();
    let parent = installed
        .path()
        .join(if cfg!(windows) { "gal.exe" } else { "gal" });
    fs::copy(env!("CARGO_BIN_EXE_gal"), &parent).unwrap();
    let private_dir = source.join("target/gal-pipeline/bin/not-a-valid-binding");
    fs::create_dir_all(&private_dir).unwrap();
    let private_exe = private_dir.join(if cfg!(windows) { "gal.exe" } else { "gal" });
    fs::copy(&parent, &private_exe).unwrap();

    for command in ["pipeline-converge-check", "pipeline-handback-check"] {
        let output = Command::new(&private_exe)
            .current_dir(source)
            .arg(command)
            .arg("--handler-must-not-run")
            .output()
            .unwrap();
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "{command} unexpectedly succeeded");
        assert!(
            stderr.contains("bound private executable hash mismatch"),
            "{stderr}"
        );
        assert!(
            !stderr.contains("unknown option"),
            "handler ran for {command}: {stderr}"
        );
        println!(
            "handoff rejection evidence: action={command} child_path={} child_sha256={} rejection=bound-private-executable-hash-mismatch exit_status={}",
            fs::canonicalize(&private_exe).unwrap().display(),
            hash_file(&private_exe),
            output.status.code().unwrap_or(1)
        );
    }
    fs::remove_dir_all(source.join("target/gal-pipeline/bin/not-a-valid-binding")).unwrap();
}

fn init_repo(path: &std::path::Path) {
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "test"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        assert!(Command::new("git")
            .current_dir(path)
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    fs::write(path.join("README.md"), "fixture\n").unwrap();
    assert!(Command::new("git")
        .current_dir(path)
        .args(["add", "README.md"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(path)
        .args(["commit", "-q", "-m", "fixture"])
        .status()
        .unwrap()
        .success());
}

fn write_grant_stores(
    plugin: &Path,
    repo: &Path,
    scope: &str,
    prompt_hash: &str,
    expires_offset_secs: i64,
    state: &str,
    session: &str,
    turn: &str,
    worktree: &str,
) {
    let expires = if expires_offset_secs < 0 {
        100u64
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + expires_offset_secs as u64
    };

    let binding = serde_json::json!({
        "session_id": session,
        "turn_id": turn,
        "worktree": worktree,
        "prompt_hash": prompt_hash,
        "generation": 1,
        "nonce": "n1",
        "handler_digest": "h1"
    });

    let grant = serde_json::json!({
        "state": state,
        "expires": expires,
        "stops": 1,
        "presented_turn_id": turn,
        "binding": binding
    });

    fs::create_dir_all(plugin).unwrap();
    fs::write(
        plugin.join("ready-grant.json"),
        serde_json::to_vec_pretty(&grant).unwrap(),
    )
    .unwrap();

    let control_dir = repo.join(".dev").join("pipeline").join(scope);
    fs::create_dir_all(&control_dir).unwrap();

    let mut hasher = Sha256::new();
    hasher.update(b"n1");
    let nonce_digest = format!("{:x}", hasher.finalize());

    let control = serde_json::json!({
        "session_id": session,
        "turn_id": turn,
        "worktree": worktree,
        "prompt_hash": prompt_hash,
        "generation": 1,
        "expires": expires,
        "nonce_digest": nonce_digest,
        "handler_digest": "h1"
    });

    fs::write(
        control_dir.join("codex-hook-grant.json"),
        serde_json::to_vec_pretty(&control).unwrap(),
    )
    .unwrap();
}

#[test]
fn missing_grant_fails_before_creating_authority_or_starting_provider() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("plan.prompt.md");
    let task = format!("T-{}", 34);
    fs::write(
        &prompt,
        format!("## Tasks\n\n- [ ] {task} guarded entry\n\n## Test Plan\n"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("host-continuation-not-ready"), "{stderr}");
    assert!(!repo.path().join(".dev/pipeline").exists());
    assert_eq!(fs::read_dir(plugin.path()).unwrap().count(), 0);
}

#[test]
fn expired_or_stale_grant_fails_closed_without_starting_provider() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("plan.prompt.md");
    fs::write(
        &prompt,
        "## Tasks\n\n- [ ] @T@ task\n\n## Test Plan\n\n## Status\nWorkflow: IMPLEMENT\n"
            .replace("@T@", &tid())
            .replace("@TP@", &tpid()),
    )
    .unwrap();
    let prompt_hash = hash_file(&prompt);
    let worktree = fs::canonicalize(repo.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();

    // Expired grant
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        -100,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("host-continuation-not-ready"), "{stderr}");
    assert!(!repo
        .path()
        .join(".dev/pipeline/plan/coordinator.json")
        .exists());

    // Already consumed grant
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        300,
        "Consumed",
        "s1",
        "t1",
        &worktree,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("host-continuation-not-ready"), "{stderr}");
    assert!(!repo
        .path()
        .join(".dev/pipeline/plan/coordinator.json")
        .exists());
}

#[test]
fn binding_mismatch_fails_closed() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("plan.prompt.md");
    fs::write(
        &prompt,
        "## Tasks\n\n- [ ] @T@ task\n\n## Test Plan\n\n## Status\nWorkflow: IMPLEMENT\n"
            .replace("@T@", &tid())
            .replace("@TP@", &tpid()),
    )
    .unwrap();
    let prompt_hash = hash_file(&prompt);
    let worktree = fs::canonicalize(repo.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();

    // Write grant with wrong prompt hash in binding
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        "wronghash123",
        300,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("host-continuation-not-ready"), "{stderr}");
    assert!(!repo
        .path()
        .join(".dev/pipeline/plan/coordinator.json")
        .exists());

    // Write grant with mismatched session between control and ready-grant
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        300,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );
    let control_path = repo.path().join(".dev/pipeline/plan/codex-hook-grant.json");
    let mut control: serde_json::Value =
        serde_json::from_slice(&fs::read(&control_path).unwrap()).unwrap();
    control["session_id"] = serde_json::json!("mismatched-session");
    fs::write(&control_path, serde_json::to_vec_pretty(&control).unwrap()).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("host-continuation-not-ready"), "{stderr}");
    assert!(!repo
        .path()
        .join(".dev/pipeline/plan/coordinator.json")
        .exists());
}

#[test]
fn missing_or_stale_self_bootstrap_proof_yields_root_checkpoint() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("plan.prompt.md");
    fs::write(&prompt, "# Test Plan\n\n## Tasks\n\n- [ ] @T@ initial task\n\n## Test Plan\n\n@TP@ spec test\n\n## Status\n\nWorkflow: IMPLEMENT\n".replace("@T@", &tid()).replace("@TP@", &tpid())).unwrap();
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "plan"])
        .status()
        .unwrap()
        .success());

    let prompt_hash = hash_file(&prompt);
    let worktree = fs::canonicalize(repo.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        300,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );

    // Run without GAL_PINNED_EXECUTABLE_SHA256 (missing proof)
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .env_remove("GAL_PINNED_EXECUTABLE_SHA256")
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("checkpoint=self-bootstrap revision=2"),
        "{stdout}"
    );

    let coord_path = repo.path().join(".dev/pipeline/plan/coordinator.json");
    assert!(coord_path.exists());
    let coord: serde_json::Value = serde_json::from_slice(&fs::read(&coord_path).unwrap()).unwrap();
    assert_eq!(coord["revision"], 2);
    assert_eq!(coord["checkpoint"]["checkpoint_id"], "self-bootstrap");
    assert_eq!(coord["checkpoint"]["kind"], "task_quality");
    assert_eq!(coord["profile"], "codex_stop_v1");

    // Grant was consumed
    let ready_grant: serde_json::Value =
        serde_json::from_slice(&fs::read(plugin.path().join("ready-grant.json")).unwrap()).unwrap();
    assert_eq!(ready_grant["state"], "Consumed");

    // Preflight was NOT run yet (no preflight receipt or entry binding)
    assert!(!repo
        .path()
        .join(".dev/pipeline/plan/entry-binding.json")
        .exists());
}

#[test]
fn executable_hash_mismatch_yields_root_checkpoint_and_never_authorizes_gate() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("plan.prompt.md");
    fs::write(&prompt, "# Test Plan\n\n## Tasks\n\n- [ ] @T@ initial task\n\n## Test Plan\n\n@TP@ spec test\n\n## Status\n\nWorkflow: IMPLEMENT\n".replace("@T@", &tid()).replace("@TP@", &tpid())).unwrap();
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "plan"])
        .status()
        .unwrap()
        .success());

    let prompt_hash = hash_file(&prompt);
    let worktree = fs::canonicalize(repo.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        300,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );

    // Run with mismatched GAL_PINNED_EXECUTABLE_SHA256
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .env(
            "GAL_PINNED_EXECUTABLE_SHA256",
            "0000000000000000000000000000000000000000000000000000000000000000",
        )
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("checkpoint=self-bootstrap revision=2"),
        "{stdout}"
    );

    // Gate receipt was NOT authorized
    assert!(!repo
        .path()
        .join(".dev/pipeline/plan/entry-binding.json")
        .exists());
}

#[test]
fn preflight_failure_stops_with_zero_provider_starts_and_no_mutation() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();

    // Prompt missing required ## Test Plan section -> preflight will fail
    let prompt = prompt_dir.join("plan.prompt.md");
    fs::write(
        &prompt,
        "# Test Plan\n\n## Tasks\n\n- [ ] @T@ initial task\n\n## Status\n\nWorkflow: IMPLEMENT\n"
            .replace("@T@", &tid())
            .replace("@TP@", &tpid()),
    )
    .unwrap();
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "plan"])
        .status()
        .unwrap()
        .success());

    let prompt_hash = hash_file(&prompt);
    let worktree = fs::canonicalize(repo.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        300,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );

    let gal_hash = hash_file(Path::new(env!("CARGO_BIN_EXE_gal")));

    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .env("GAL_PINNED_EXECUTABLE_SHA256", &gal_hash)
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("entry-preflight-failed")
            || stderr.contains("entry-preflight-receipt-stale-or-not-pass"),
        "{stderr}"
    );

    // No entry-binding written, no task cursor or progress writes
    assert!(!repo
        .path()
        .join(".dev/pipeline/plan/entry-binding.json")
        .exists());
    let coord: serde_json::Value = serde_json::from_slice(
        &fs::read(repo.path().join(".dev/pipeline/plan/coordinator.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(coord["revision"], 1);
}

#[test]
fn valid_entry_runs_preflight_and_reaches_task_quality_checkpoint() {
    let repo = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    init_repo(repo.path());
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();

    let prompt = prompt_dir.join("plan.prompt.md");
    fs::write(&prompt, "# Test Plan\n\n## Tasks\n\n- [ ] @T@ initial task\n\n## Test Plan\n\n@TP@ spec test\n\n## Status\n\nWorkflow: IMPLEMENT\n".replace("@T@", &tid()).replace("@TP@", &tpid())).unwrap();

    // Create .dev/state.md so preflight wrong-plan-guard passes
    let state_md = repo.path().join(".dev/state.md");
    fs::write(
        &state_md,
        "# State\n\n| plan | .dev/plans/plan.prompt.md |\n",
    )
    .unwrap();

    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["commit", "-q", "-m", "plan"])
        .status()
        .unwrap()
        .success());

    let prompt_hash = hash_file(&prompt);
    let worktree = fs::canonicalize(repo.path())
        .unwrap()
        .to_string_lossy()
        .into_owned();
    write_grant_stores(
        plugin.path(),
        repo.path(),
        "plan",
        &prompt_hash,
        300,
        "ReadyGrant",
        "s1",
        "t1",
        &worktree,
    );

    let gal_hash = hash_file(Path::new(env!("CARGO_BIN_EXE_gal")));

    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .env("PLUGIN_DATA", plugin.path())
        .env("GAL_PINNED_EXECUTABLE_SHA256", &gal_hash)
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("checkpoint=task-quality-entry revision=2"),
        "{stdout}"
    );

    let coord_path = repo.path().join(".dev/pipeline/plan/coordinator.json");
    let coord: serde_json::Value = serde_json::from_slice(&fs::read(&coord_path).unwrap()).unwrap();
    assert_eq!(coord["revision"], 2);
    assert_eq!(coord["checkpoint"]["checkpoint_id"], "task-quality-entry");
    assert_eq!(coord["profile"], "codex_stop_v1");
    assert_eq!(coord["task_id"], tid());
    assert_eq!(coord["phase"], "implement");

    let binding_path = repo.path().join(".dev/pipeline/plan/entry-binding.json");
    assert!(binding_path.exists());
    let binding: serde_json::Value =
        serde_json::from_slice(&fs::read(&binding_path).unwrap()).unwrap();
    assert_eq!(binding["prompt_sha256"], prompt_hash);
    assert_eq!(binding["executable_sha256"], gal_hash);
    assert!(binding.get("receipt_sha256").is_some());
    assert!(binding.get("head").is_some());
}

#[test]
fn ordinary_pipeline_omission_defaults_remain_on_legacy_route() {
    let repo = TempDir::new().unwrap();
    init_repo(repo.path());
    let output = Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo.path())
        .args(["pipeline"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing plan / prompt / task-spec path")
    );
}
