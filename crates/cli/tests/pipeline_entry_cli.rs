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
        stdout.contains("checkpoint=self-bootstrap revision=1"),
        "{stdout}"
    );

    let coord_path = repo.path().join(".dev/pipeline/plan/coordinator.json");
    assert!(coord_path.exists());
    let coord: serde_json::Value = serde_json::from_slice(&fs::read(&coord_path).unwrap()).unwrap();
    assert_eq!(coord["revision"], 1);
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
        stdout.contains("checkpoint=self-bootstrap revision=1"),
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
    assert_eq!(coord["revision"], 0);
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
        stdout.contains("checkpoint=task-quality-entry revision=1"),
        "{stdout}"
    );

    let coord_path = repo.path().join(".dev/pipeline/plan/coordinator.json");
    let coord: serde_json::Value = serde_json::from_slice(&fs::read(&coord_path).unwrap()).unwrap();
    assert_eq!(coord["revision"], 1);
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
