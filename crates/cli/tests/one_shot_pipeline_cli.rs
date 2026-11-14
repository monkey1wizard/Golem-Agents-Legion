use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command};
use tempfile::TempDir;

struct AttemptHarness {
    _home: TempDir,
    _bin: TempDir,
    repo: TempDir,
    path: std::ffi::OsString,
}

impl AttemptHarness {
    fn new() -> Self {
        let home = TempDir::new().unwrap();
        let config = home.path().join(".gal/config");
        fs::create_dir_all(&config).unwrap();
        fs::write(
            config.join("config.json"),
            br#"{"executorRouting":{"executors":{"agy":"fixture-model"},"pipeline":{"CODER":{"executor":"agy"}}}}"#,
        )
        .unwrap();

        let bin = TempDir::new().unwrap();
        write_fake_agy(bin.path());
        let mut paths = vec![bin.path().to_path_buf()];
        if let Some(existing) = std::env::var_os("PATH") {
            paths.extend(std::env::split_paths(&existing));
        }
        let path = std::env::join_paths(paths).unwrap();

        let repo = TempDir::new().unwrap();
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo.path())
            .status()
            .unwrap();
        assert!(status.success());

        Self {
            _home: home,
            _bin: bin,
            repo,
            path,
        }
    }

    fn pipeline(&self, args: &[&str]) -> std::process::Output {
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_gal")))
            .current_dir(self.repo.path())
            .args(args)
            .env("HOME", self._home.path())
            .env("USERPROFILE", self._home.path())
            .env("PATH", &self.path)
            .env("GEMINI_API_KEY", "test-key")
            .output()
            .unwrap()
    }

    fn attempt_route_marker(&self, name: &str, route_args: &[&str]) -> String {
        let spec = self.repo.path().join(format!("{name}.md"));
        fs::write(&spec, format!("# {name}\n\nExecute the fixture attempt.\n")).unwrap();
        let spec = spec.to_string_lossy().into_owned();
        let mut args = vec!["pipeline", &spec, "--phase", "implement"];
        args.extend_from_slice(route_args);
        let output = self.pipeline(&args);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stdout.contains("Dispatch: phase=implement"),
            "pipeline did not launch an attempt: stdout={stdout}; stderr={stderr}"
        );
        let log_path = stderr
            .lines()
            .find_map(|line| line.split_once(" log=").map(|(_, path)| path.trim()))
            .expect("pipeline attempt must report its log path");
        assert!(
            fs::read_to_string(log_path).is_ok(),
            "pipeline attempt log must exist"
        );
        stdout
            .lines()
            .find(|line| line.starts_with("Dispatch: phase=implement"))
            .expect("pipeline attempt marker")
            .to_owned()
    }
}

#[cfg(not(windows))]
fn write_fake_agy(bin: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;

    let path = bin.join("agy");
    fs::write(
        &path,
        "#!/bin/sh\nprintf '%s\\n' fixture > route-evidence-target.txt\nprintf '%s\\n' '{\"conversation_id\":\"176f1141-b606-47aa-a47b-76f2a6147623\",\"status\":\"SUCCESS\",\"usage\":{\"input_tokens\":120,\"output_tokens\":30,\"thinking_tokens\":5,\"cache_read_tokens\":40,\"total_tokens\":150}}'\n",
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(windows)]
fn write_fake_agy(bin: &std::path::Path) {
    fs::write(
        bin.join("agy.cmd"),
        "@echo off\r\necho fixture>route-evidence-target.txt\r\necho {\"conversation_id\":\"176f1141-b606-47aa-a47b-76f2a6147623\",\"status\":\"SUCCESS\",\"usage\":{\"input_tokens\":120,\"output_tokens\":30,\"thinking_tokens\":5,\"cache_read_tokens\":40,\"total_tokens\":150}}\r\n",
    )
    .unwrap();
}

fn run_pipeline(
    config: &[u8],
    phase: &str,
    route_args: &[&str],
) -> (TempDir, std::process::Output) {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let config_dir = home.join(".gal/config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("config.json"), config).unwrap();
    let workdir = temp.path().join("workdir");
    fs::create_dir_all(&workdir).unwrap();
    let mut command = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_gal")));
    command
        .current_dir(&workdir)
        .args(["pipeline", "missing.prompt.md", "--phase", phase])
        .args(route_args)
        .env("HOME", &home)
        .env("USERPROFILE", &home);
    (temp, command.output().unwrap())
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn write_guarded_grant(plugin: &std::path::Path, repo: &std::path::Path, prompt: &std::path::Path) {
    let prompt_hash = sha256(&fs::read(prompt).unwrap());
    let worktree = fs::canonicalize(repo)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let binding = serde_json::json!({
        "session_id": "route-session",
        "turn_id": "route-turn",
        "worktree": worktree,
        "prompt_hash": prompt_hash,
        "generation": 1,
        "nonce": "route-nonce",
        "handler_digest": "fixture-handler"
    });
    fs::create_dir_all(plugin).unwrap();
    fs::write(
        plugin.join("ready-grant.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "state": "ReadyGrant",
            "expires": now + 300,
            "stops": 1,
            "presented_turn_id": "route-turn",
            "binding": binding
        }))
        .unwrap(),
    )
    .unwrap();

    let scope = prompt
        .file_name()
        .unwrap()
        .to_string_lossy()
        .strip_suffix(".prompt.md")
        .unwrap()
        .to_string();
    let control_dir = repo.join(".dev/pipeline").join(scope);
    fs::create_dir_all(&control_dir).unwrap();
    fs::write(
        control_dir.join("codex-hook-grant.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "session_id": "route-session",
            "turn_id": "route-turn",
            "worktree": fs::canonicalize(repo).unwrap().to_string_lossy(),
            "prompt_hash": prompt_hash,
            "generation": 1,
            "expires": now + 300,
            "nonce_digest": sha256(b"route-nonce"),
            "handler_digest": "fixture-handler"
        }))
        .unwrap(),
    )
    .unwrap();
}

fn guarded_fixture() -> (TempDir, TempDir, TempDir, TempDir, PathBuf) {
    let repo = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();
    let plugin = TempDir::new().unwrap();
    let bin = TempDir::new().unwrap();
    write_fake_agy(bin.path());
    let config = home.path().join(".gal/config");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.json"),
        br#"{"executorRouting":{"executors":{"agy":"fixture-model"},"pipeline":{"CODER":{"executor":"agy"}}}}"#,
    )
    .unwrap();
    let convention = repo
        .path()
        .join("plugins/gal-core/conventions/task-quality.md");
    fs::create_dir_all(convention.parent().unwrap()).unwrap();
    fs::write(
        convention,
        "# Task Quality\n\nCanonical questions for checking whether a task is sufficiently specified.\n",
    )
    .unwrap();
    let prompt_dir = repo.path().join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("route-evidence.prompt.md");
    let task = format!("T-{:02}", 1);
    let probe = format!("TP-{:02}", 1);
    fs::write(
        &prompt,
        format!(
            "# Route fixture\n\n## Files to Create or Modify\n\n- `route-evidence-target.txt`\n\n## Tasks\n\n- [ ] {task} — Run routed attempt.\n  - Targets: `route-evidence-target.txt`\n  - Change: Update the fixture target.\n  - Acceptance: The fixture target contains the executor result.\n  - **Verification Impact**: {{\"contract_docs\":false,\"code_logic\":true,\"cli_surface\":true,\"cross_boundary\":false}}\n\n## Verification Map\n\n| Task | Layer | Proof Command | Expected | Producer | Evidence |\n| --- | --- | --- | --- | --- | --- |\n| {task} | unit | cargo test -p gal-cli --test one_shot_pipeline_cli | pass | TESTER | test output |\n| {task} | cli | cargo test -p gal-cli --test one_shot_pipeline_cli | pass | TESTER | test output |\n\n## Test Plan\n\n{probe} route fixture\n\n## Status\n\nWorkflow: IMPLEMENT\n"
        ),
    )
    .unwrap();
    fs::write(repo.path().join("route-evidence-target.txt"), "initial\n").unwrap();
    fs::write(prompt_dir.join("route-evidence.md"), "# Route fixture\n").unwrap();
    fs::write(repo.path().join(".gitignore"), ".dev/pipeline/\n").unwrap();
    fs::write(
        repo.path().join(".dev/state.md"),
        "# State\n\n| plan | .dev/plans/route-evidence.prompt.md |\n",
    )
    .unwrap();
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["init", "-q"])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args(["add", "."])
        .status()
        .unwrap()
        .success());
    assert!(Command::new("git")
        .current_dir(repo.path())
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-q",
            "-m",
            "fixture"
        ])
        .status()
        .unwrap()
        .success());
    write_guarded_grant(plugin.path(), repo.path(), &prompt);
    (repo, home, bin, plugin, prompt)
}

fn run_guarded_entry(
    repo: &std::path::Path,
    home: &std::path::Path,
    bin: &std::path::Path,
    plugin: &std::path::Path,
    prompt: &std::path::Path,
) -> std::process::Output {
    run_guarded_command(repo, home, bin, plugin, prompt, &["--phase", "implement"])
}

fn run_guarded_command(
    repo: &std::path::Path,
    home: &std::path::Path,
    bin: &std::path::Path,
    plugin: &std::path::Path,
    prompt: &std::path::Path,
    command_args: &[&str],
) -> std::process::Output {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_gal"));
    let mut command = Command::new(&executable);
    command
        .current_dir(repo)
        .args(["pipeline", "--require-codex-stop-v1"])
        .arg(prompt)
        .args(command_args)
        .env("PLUGIN_DATA", plugin)
        .env(
            "GAL_PINNED_EXECUTABLE_SHA256",
            sha256(&fs::read(&executable).unwrap()),
        )
        .env("GAL_INSTALLED_EXECUTABLE", &executable)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env(
            "PATH",
            std::env::join_paths(
                std::iter::once(bin.to_path_buf()).chain(std::env::split_paths(
                    &std::env::var_os("PATH").unwrap_or_default(),
                )),
            )
            .unwrap(),
        );
    command.output().unwrap()
}

fn submit_guarded_task_quality_receipt(
    repo: &std::path::Path,
    home: &std::path::Path,
    prompt: &std::path::Path,
) {
    let scope = repo.join(".dev/pipeline/route-evidence");
    let coordinator_path = scope.join("coordinator.json");
    let coordinator: serde_json::Value =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    let head = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(head.status.success());
    let receipt = serde_json::json!({
        "receipt_id": "route-entry-quality",
        "checkpoint_id": coordinator["checkpoint"]["checkpoint_id"],
        "revision": coordinator["checkpoint"]["revision"],
        "prompt_sha256": coordinator["checkpoint"]["prompt_sha256"],
        "execution_binding_sha256": coordinator["checkpoint"]["execution_binding_sha256"],
        "commit": String::from_utf8_lossy(&head.stdout).trim(),
        "accepted": true
    });
    fs::write(
        scope.join("orchestrator-receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    let output = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_gal")))
        .current_dir(repo)
        .args([
            "pipeline",
            "--require-codex-stop-v1",
            prompt.to_str().unwrap(),
            "--submit-checkpoint-receipt",
        ])
        .env("HOME", home)
        .env("USERPROFILE", home)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "checkpoint receipt submission failed: stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn route_override_does_not_leak_into_restart_retry_or_next_task() {
    let harness = AttemptHarness::new();
    let routed = harness.attempt_route_marker(
        "retry-task",
        &["--executor", "agy", "--model", "one-shot-model"],
    );
    let restarted = harness.attempt_route_marker("retry-task", &[]);
    let next_task = harness.attempt_route_marker("next-task", &[]);

    assert!(routed.contains("model=one-shot-model"), "{routed}");
    assert!(restarted.contains("model=fixture-model"), "{restarted}");
    assert!(next_task.contains("model=fixture-model"), "{next_task}");
}

#[test]
fn guarded_entry_consumes_a_worktree_and_prompt_bound_grant() {
    let (repo, home, bin, plugin, prompt) = guarded_fixture();
    let output = run_guarded_entry(repo.path(), home.path(), bin.path(), plugin.path(), &prompt);
    assert!(
        output.status.success(),
        "stdout={}; stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let grant: serde_json::Value =
        serde_json::from_slice(&fs::read(plugin.path().join("ready-grant.json")).unwrap()).unwrap();
    assert_eq!(grant["state"], "Consumed");
    let coordinator: serde_json::Value = serde_json::from_slice(
        &fs::read(
            repo.path()
                .join(".dev/pipeline/route-evidence/coordinator.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(coordinator["profile"], "codex_stop_v1");
    assert_eq!(
        coordinator["checkpoint"]["checkpoint_id"],
        "task-quality-entry"
    );
}

#[test]
fn guarded_restart_preserves_the_persisted_route_evidence() {
    let (repo, home, bin, plugin, prompt) = guarded_fixture();
    let entry = run_guarded_entry(repo.path(), home.path(), bin.path(), plugin.path(), &prompt);
    assert!(
        entry.status.success(),
        "guarded entry failed: stdout={}; stderr={}",
        String::from_utf8_lossy(&entry.stdout),
        String::from_utf8_lossy(&entry.stderr)
    );
    submit_guarded_task_quality_receipt(repo.path(), home.path(), &prompt);

    let task = format!("T-{:02}", 1);
    let first = run_guarded_command(
        repo.path(),
        home.path(),
        bin.path(),
        plugin.path(),
        &prompt,
        &[
            "--resume",
            "--task",
            &task,
            "--phase",
            "implement",
            "--executor",
            "agy",
            "--model",
            "one-shot-model",
        ],
    );
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    let expected_dispatch = format!(
        "Dispatch: phase=implement task={task} role=CODER executor=agy model=one-shot-model"
    );
    let boundary_details = first_stdout
        .lines()
        .filter(|line| line.starts_with("gal boundary-check:"))
        .find_map(|line| line.split_once(" → ").map(|(_, path)| path.trim()))
        .and_then(|path| fs::read_to_string(path).ok())
        .unwrap_or_default();
    assert!(
        first_stdout.contains(&expected_dispatch),
        "routed attempt did not launch: stdout={first_stdout}; stderr={}; boundary={boundary_details}",
        String::from_utf8_lossy(&first.stderr)
    );
    assert!(
        boundary_details.contains("overall: pass"),
        "{boundary_details}"
    );
    let coordinator_path = repo
        .path()
        .join(".dev/pipeline/route-evidence/coordinator.json");
    let first_state: serde_json::Value =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    let persisted_evidence = first_state["last_verified_evidence"]
        .as_str()
        .map(str::to_owned)
        .expect("coordinator route evidence");
    assert!(
        persisted_evidence.contains("route_digest=sha256:"),
        "coordinator did not persist the routed attempt: {first_state}"
    );

    let restarted = run_guarded_command(
        repo.path(),
        home.path(),
        bin.path(),
        plugin.path(),
        &prompt,
        &["--status"],
    );
    assert!(
        restarted.status.success(),
        "guarded restart failed: stdout={}; stderr={}",
        String::from_utf8_lossy(&restarted.stdout),
        String::from_utf8_lossy(&restarted.stderr)
    );
    let restarted_state: serde_json::Value =
        serde_json::from_slice(&fs::read(&coordinator_path).unwrap()).unwrap();
    assert_eq!(
        restarted_state["last_verified_evidence"],
        persisted_evidence
    );
}

#[test]
fn host_pinned_one_shot_override_uses_the_routed_executor() {
    let (repo, home, bin, _plugin, prompt) = guarded_fixture();
    fs::write(
        repo.path().join(".dev/pipeline/route-evidence/preflight.receipt.md"),
        "# pipeline-preflight receipt\nrun_id: fixture-run\nhost: codex\ndispatch_mode: host-subagent\n",
    )
    .unwrap();
    let output = Command::new(PathBuf::from(env!("CARGO_BIN_EXE_gal")))
        .current_dir(repo.path())
        .args([
            "pipeline",
            prompt.to_str().unwrap(),
            "--phase",
            "implement",
            "--dispatch",
            "host-subagent",
            "--executor",
            "agy",
            "--model",
            "one-shot-model",
        ])
        .env("HOME", home.path())
        .env("USERPROFILE", home.path())
        .env("GEMINI_API_KEY", "test-key")
        .env(
            "PATH",
            std::env::join_paths(std::iter::once(bin.path().to_path_buf()).chain(
                std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
            ))
            .unwrap(),
        )
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("executor=agy model=one-shot-model"),
        "stdout={stdout}; stderr={stderr}"
    );

    let task_dir = repo
        .path()
        .join(".dev/pipeline/route-evidence")
        .join(format!("T-{:02}", 1));
    let log = fs::read_dir(&task_dir)
        .unwrap_or_else(|error| {
            panic!("dispatch log directory unavailable: {error}; stdout={stdout}; stderr={stderr}")
        })
        .filter_map(Result::ok)
        .map(|entry| fs::read_to_string(entry.path()).unwrap_or_default())
        .find(|content| content.contains("route_digest:"))
        .unwrap_or_else(|| {
            panic!("dispatch log with route digest missing; stdout={stdout}; stderr={stderr}")
        });
    let digest = log
        .lines()
        .find_map(|line| {
            line.split_once(':')
                .filter(|(key, _)| key.trim() == "route_digest")
                .map(|(_, value)| value.trim())
        })
        .expect("dispatch log route digest");
    assert!(digest.starts_with("sha256:"), "{log}");
    assert!(log.contains("executor:        agy"), "{log}");
    assert!(log.contains("actual_model:    one-shot-model"), "{log}");
}

#[test]
fn pipeline_accepts_one_shot_route_flags_before_prompt_validation() {
    let (_temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"},"pipeline":{"CODER":{"executor":"claude"}}}}"#,
        "implement",
        &["--executor", "claude", "--model", "one-shot-model"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        !stderr.contains("unknown argument '--executor'"),
        "{stderr}"
    );
    assert!(
        stderr.contains("pipeline prompt") || stderr.contains("prompt not found"),
        "expected prompt validation after route parsing, got: {stderr}"
    );
}

#[test]
fn test_phase_accepts_one_shot_route_flags_before_prompt_validation() {
    let (_temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"},"pipeline":{"TESTER":{"executor":"claude"}}}}"#,
        "test",
        &["--executor", "claude", "--model", "one-shot-model"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        !stderr.contains("unknown argument '--executor'"),
        "{stderr}"
    );
    assert!(
        stderr.contains("pipeline prompt") || stderr.contains("prompt not found"),
        "expected prompt validation after route parsing, got: {stderr}"
    );
}

#[test]
fn audit_phase_accepts_one_shot_route_flags_before_prompt_validation() {
    let (_temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"},"pipeline":{"AUDITOR":{"executor":"claude"}}}}"#,
        "audit",
        &["--executor", "claude", "--model", "one-shot-model"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        !stderr.contains("unknown argument '--executor'"),
        "{stderr}"
    );
    assert!(
        stderr.contains("pipeline prompt") || stderr.contains("prompt not found"),
        "expected prompt validation after route parsing, got: {stderr}"
    );
}

#[test]
fn guarded_fix_phase_accepts_one_shot_route_flags_before_prompt_validation() {
    let (_temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"},"pipeline":{"CODER":{"executor":"claude"}}}}"#,
        "fix",
        &[
            "--require-codex-stop-v1",
            "--executor",
            "claude",
            "--model",
            "one-shot-model",
        ],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        !stderr.contains("unknown argument '--executor'"),
        "{stderr}"
    );
    assert!(
        stderr.contains("prompt") || stderr.contains("plan"),
        "expected prompt validation after route parsing, got: {stderr}"
    );
}

#[test]
fn finalize_review_accepts_routed_overrides_before_prompt_validation() {
    let (temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"},"pipeline":{"AUDITOR":{"executor":"claude"}}}}"#,
        "finalize-review",
        &["--executor", "claude", "--model", "one-shot-model"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        !stderr.contains("finalize-review does not accept one-shot route options"),
        "{stderr}"
    );
    assert!(
        stderr.contains("prompt") || stderr.contains("plan"),
        "route parsing should reach prompt validation: {stderr}"
    );
    assert!(!temp.path().join("workdir/.dev/pipeline").exists());
}

#[test]
fn finalize_review_rejects_host_selector_before_claim() {
    let (temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"}}}"#,
        "finalize-review",
        &[
            "--host-subagent",
            "codex",
            "--dispatch",
            "host-subagent",
            "--host",
            "codex",
        ],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("finalize-review accepts routed one-shot overrides only"),
        "{stderr}"
    );
    assert!(!temp.path().join("workdir/.dev/pipeline").exists());
}

#[test]
fn model_override_requires_an_effective_role_route() {
    let (_temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"}}}"#,
        "implement",
        &["--model", "one-shot-model"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("no effective executor route for role CODER"),
        "{stderr}"
    );
    assert!(!stderr.contains("prompt not found"), "{stderr}");
}

#[test]
fn explicit_override_resolves_even_when_the_role_route_is_invalid() {
    let (_temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"},"pipeline":{"CODER":{"executor":"missing-executor"}}}}"#,
        "implement",
        &["--executor", "claude", "--model", "one-shot-model"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(stderr.contains("prompt not found"), "{stderr}");
    assert!(!stderr.contains("no effective executor route"), "{stderr}");
}

#[test]
fn converge_rejects_route_flags_before_claim() {
    let (temp, output) = run_pipeline(
        br#"{"executorRouting":{"executors":{"claude":"default-model"},"pipeline":{"CODER":{"executor":"claude"}}}}"#,
        "converge",
        &["--executor", "claude", "--model", "one-shot-model"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success());
    assert!(
        stderr.contains("converge does not accept one-shot route options"),
        "{stderr}"
    );
    assert!(!temp.path().join("workdir/.dev/pipeline").exists());
}

#[test]
fn handback_requires_the_coordinator_route_digest_to_match_the_log() {
    let temp = TempDir::new().unwrap();
    let root = temp.path();
    let home = root.join("home");
    let config = home.join(".gal/config");
    fs::create_dir_all(&config).unwrap();
    fs::write(
        config.join("config.json"),
        br#"{"executorRouting":{"executors":{"claude":"fixture-model"},"pipeline":{"CODER":{"executor":"claude"}}}}"#,
    )
    .unwrap();
    let task = format!("T-{:02}", 1);
    let prompt_dir = root.join(".dev/plans");
    fs::create_dir_all(&prompt_dir).unwrap();
    let prompt = prompt_dir.join("route-evidence.prompt.md");
    fs::write(
        &prompt,
        format!("# Fixture\n\n## Tasks\n- [ ] {task} — Implement fixture.\n\n## Status\nCurrent Task: {task}\n"),
    )
    .unwrap();

    let scope = root.join(".dev/pipeline/route-evidence");
    let task_dir = scope.join(&task);
    fs::create_dir_all(&task_dir).unwrap();
    fs::write(
        scope.join("preflight.receipt.md"),
        "host: none\ndispatch_mode: routed\nrun_id: fixture-run\n",
    )
    .unwrap();
    let digest = "a".repeat(64);
    let coordinator = format!(
        r#"{{"schema_version":2,"revision":0,"profile":"legacy_interactive","plan_scope":"route-evidence","prompt_sha256":"","task_id":"{task}","phase":"implement","active_attempt":null,"retry":{{"attempt":0,"max_attempts":0,"last_failure":null}},"last_verified_evidence":"implement-dispatch:Success;route_digest=sha256:{digest};","pending_projection_id":null,"checkpoint":null,"activation_digest":null,"execution_binding":null,"next_action":{{"kind":"complete"}},"consumed_checkpoint_receipts":[]}}"#
    );
    fs::write(scope.join("coordinator.json"), coordinator).unwrap();

    let log_path = task_dir.join(format!("000001-000001-000001-{task}-implement.log"));
    let log = |route_digest: &str| {
        format!(
            "GAL-DISPATCH-LOG v1\nevidence_contract: v1\nexecutor: claude\nroute_digest: {route_digest}\ntask_id: {task}\nphase: implement\ntimestamp_start: 2026-10-15T00:00:01Z\nterminal_state: completed\nattempt_id: fixture-attempt\n---STDOUT---\n"
        )
    };
    fs::write(&log_path, log(&format!("sha256:{digest}"))).unwrap();

    let receipt = root.join("handback.receipt.md");
    let run_handback = || {
        Command::new(PathBuf::from(env!("CARGO_BIN_EXE_gal")))
            .current_dir(root)
            .args([
                "pipeline-handback-check",
                prompt.to_str().unwrap(),
                "--receipt",
                receipt.to_str().unwrap(),
            ])
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .output()
            .unwrap()
    };

    let _matching = run_handback();
    let matching_receipt = fs::read_to_string(&receipt).unwrap();
    assert!(
        !matching_receipt.contains("host_dispatch_evidence: fail"),
        "{matching_receipt}"
    );
    assert!(
        matching_receipt.contains("decision: continue"),
        "{matching_receipt}"
    );

    fs::write(&log_path, log(&format!("sha256:{}", "b".repeat(64)))).unwrap();
    let mismatched = run_handback();
    let mismatched_receipt = fs::read_to_string(&receipt).unwrap();
    assert!(!mismatched.status.success());
    assert!(
        mismatched_receipt.contains("host_dispatch_evidence: fail"),
        "{mismatched_receipt}"
    );
    assert!(
        mismatched_receipt.contains("reason=mode-mismatch"),
        "{mismatched_receipt}"
    );
}
