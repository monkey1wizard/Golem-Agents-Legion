//! Real-CLI source-resolution checks with a copied executable and isolated home.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

struct Fixture {
    temp: TempDir,
    binary: PathBuf,
    blocked_home: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let binary_dir = temp.path().join("bin");
        fs::create_dir_all(&binary_dir).unwrap();
        let binary = binary_dir.join(format!("gal{}", std::env::consts::EXE_SUFFIX));
        fs::copy(env!("CARGO_BIN_EXE_gal"), &binary).unwrap();
        let blocked_home = temp.path().join("blocked-home");
        fs::write(&blocked_home, b"not a directory").unwrap();
        Self {
            temp,
            binary,
            blocked_home,
        }
    }

    fn directory(&self, name: &str) -> PathBuf {
        let path = self.temp.path().join(name);
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn prompt(&self, repo: &Path) -> PathBuf {
        let path = repo.join("plan.prompt.md");
        let task = format!("T-{:02}", 1);
        fs::write(
            &path,
            format!("# Plan\n\n## Tasks\n\n- [ ] {task} - Test.\n"),
        )
        .unwrap();
        path
    }

    fn run(&self, cwd: &Path, args: &[&str]) -> Output {
        Command::new(&self.binary)
            .current_dir(cwd)
            .env("USERPROFILE", &self.blocked_home)
            .env("HOME", &self.blocked_home)
            .args(args)
            .output()
            .unwrap()
    }
}

fn source_root(path: &Path, contract: Option<&str>) {
    let agents = path.join("agents");
    fs::create_dir_all(&agents).unwrap();
    fs::create_dir_all(path.join("skills")).unwrap();
    fs::create_dir_all(path.join("commands")).unwrap();
    if let Some(body) = contract {
        fs::write(agents.join("golem-implementer.agent.md"), body).unwrap();
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    format!("{:x}", sha2::Sha256::digest(bytes))
}

fn canonical_text(path: &Path) -> String {
    path.canonicalize()
        .unwrap()
        .to_string_lossy()
        .trim_start_matches("\\\\?\\")
        .replace('\\', "/")
}

fn canonical_raw_text(path: &Path) -> String {
    path.canonicalize()
        .unwrap()
        .to_string_lossy()
        .replace('\\', "/")
}

fn receipt_envelope(repo: &Path, executable: &Path, scope: &str) -> String {
    let worktree = canonical_text(repo);
    let executable = canonical_text(executable);
    let executable_sha256 = sha256(&fs::read(&executable).unwrap());
    let mut digest = sha2::Sha256::new();
    use sha2::Digest;
    digest.update(b"GAL-EXECUTION-BINDING\0");
    digest.update(1u32.to_be_bytes());
    for value in [
        scope,
        worktree.as_str(),
        executable.as_str(),
        executable_sha256.as_str(),
    ] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    let binding_digest = format!("{:x}", digest.finalize());
    let coordinator_fields = if let Some(plan_scope) = scope.strip_prefix("coordinator:") {
        let coordinator = repo
            .join(".dev/pipeline")
            .join(plan_scope)
            .join("coordinator.json");
        let coordinator = canonical_raw_text(&coordinator);
        format!("plan_scope: {plan_scope}\ncoordinator_path: {coordinator}\n")
    } else {
        String::new()
    };
    format!(
        "binding_scope: {scope}\nexecution_binding_sha256: {binding_digest}\nexecutable_path: {executable}\nexecutable_sha256: {executable_sha256}\n{coordinator_fields}"
    )
}

fn coordinator_json(repo: &Path, executable: &Path, scope: &str) -> String {
    let worktree = canonical_text(repo);
    let executable = canonical_text(executable);
    let executable_sha256 = sha256(&fs::read(&executable).unwrap());
    format!(
        r#"{{"schema_version":1,"revision":0,"profile":"legacy_interactive","plan_scope":"{scope}","prompt_sha256":"{}","task_id":"task-one","phase":"IMPLEMENT","active_attempt":null,"retry":{{"attempt":0,"max_attempts":0,"last_failure":null}},"last_verified_evidence":null,"pending_projection_id":null,"checkpoint":null,"activation_digest":null,"execution_binding":{{"version":1,"binding_scope":"coordinator:{scope}","worktree_root":"{worktree}","executable_path":"{executable}","executable_sha256":"{executable_sha256}"}},"next_action":{{"kind":"complete"}},"consumed_checkpoint_receipts":[]}}"#,
        "0".repeat(64)
    )
}

fn run_receipt(fixture: &Fixture, repo: &Path, receipt: &Path, scope: &str) -> Output {
    fixture.run(
        repo,
        &[
            "doctor",
            "--verify-receipt",
            receipt.to_str().unwrap(),
            "--expect-scope",
            scope,
        ],
    )
}

#[test]
fn pipeline_reports_embedded_materialization_io_failure() {
    let fixture = Fixture::new();
    let repo = fixture.directory("isolated-repo");
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(
        error.contains("embedded agent-contract source materialization failed"),
        "{error}"
    );
    assert!(
        error.contains("create embedded source staging directory"),
        "{error}"
    );
    assert!(
        error.contains(&fixture.blocked_home.display().to_string()),
        "{error}"
    );
    assert!(
        !error.contains("no GAL agent-contract source root is available"),
        "{error}"
    );
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn init_refresh_and_render_adapters_preserve_materialization_reason() {
    let fixture = Fixture::new();
    for (name, args, marker) in [
        (
            "init-repo",
            vec!["init"],
            "Could not materialize the embedded GAL source",
        ),
        (
            "refresh-repo",
            vec!["refresh"],
            "embedded source materialization failed",
        ),
        (
            "render-repo",
            vec!["render-adapters"],
            "could not materialize embedded GAL source",
        ),
    ] {
        let repo = fixture.directory(name);
        if name == "render-repo" {
            fs::create_dir_all(repo.join(".dev")).unwrap();
            fs::write(repo.join(".dev/project.md"), "# Fixture\n").unwrap();
        }
        let output = fixture.run(&repo, &args);
        let error = stderr(&output);
        assert!(!output.status.success(), "{name} unexpectedly succeeded");
        assert!(error.contains(marker), "{name}: {error}");
        assert!(
            error.contains("create embedded source staging directory"),
            "{name}: {error}"
        );
        assert!(
            error.contains(&fixture.blocked_home.display().to_string()),
            "{name}: {error}"
        );
    }
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn pipeline_corrupt_workdir_source_never_falls_back() {
    let fixture = Fixture::new();
    let repo = fixture.directory("workdir-repo");
    source_root(&repo.join("plugins/gal-core"), None);
    source_root(fixture.binary.parent().unwrap(), Some("exe-side contract"));
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(error.contains("tier `workdir`"), "{error}");
    assert!(error.contains("golem-implementer.agent.md"), "{error}");
    assert!(!error.contains("materialization failed"), "{error}");
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn pipeline_corrupt_ancestor_source_wins_over_exe_side() {
    let fixture = Fixture::new();
    let ancestor = fixture.directory("ancestor-repo");
    source_root(&ancestor.join("plugins/gal-core"), None);
    source_root(fixture.binary.parent().unwrap(), Some("exe-side contract"));
    let repo = ancestor.join("nested");
    fs::create_dir_all(&repo).unwrap();
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(error.contains("tier `ancestor`"), "{error}");
    assert!(!error.contains("materialization failed"), "{error}");
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn pipeline_exe_side_source_precedes_embedded_materialization() {
    let fixture = Fixture::new();
    let repo = fixture.directory("exe-side-repo");
    source_root(fixture.binary.parent().unwrap(), None);
    let prompt = fixture.prompt(&repo);
    let output = fixture.run(&repo, &["pipeline", prompt.to_str().unwrap()]);
    let error = stderr(&output);

    assert_eq!(output.status.code(), Some(1), "{error}");
    assert!(error.contains("tier `exe-side`"), "{error}");
    assert!(!error.contains("materialization failed"), "{error}");
    assert!(!fixture.blocked_home.join(".gal").exists());
}

#[test]
fn doctor_receipt_verifier_rejects_missing_arguments_and_malformed_receipts() {
    let fixture = Fixture::new();
    let repo = fixture.directory("receipt-repo");
    let missing = fixture.run(&repo, &["doctor", "--verify-receipt"]);
    assert_eq!(missing.status.code(), Some(1), "{}", stderr(&missing));
    assert!(stderr(&missing).contains("requires <path>"));

    let receipt = repo.join("receipt.md");
    fs::write(&receipt, "binding_scope: standalone\n").unwrap();
    let output = fixture.run(
        &repo,
        &[
            "doctor",
            "--verify-receipt",
            receipt.to_str().unwrap(),
            "--expect-scope",
            "standalone",
        ],
    );
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("receipt verification failed"));
}

#[test]
fn doctor_receipt_verifier_fails_closed_when_coordinator_is_missing_or_malformed() {
    let fixture = Fixture::new();
    let repo = fixture.directory("coordinator-receipt-repo");
    let receipt = repo.join("receipt.md");
    fs::write(
        &receipt,
        "binding_scope: coordinator:example\nplan_scope: example\ncoordinator_path: placeholder\n",
    )
    .unwrap();
    let args = [
        "doctor",
        "--verify-receipt",
        receipt.to_str().unwrap(),
        "--expect-scope",
        "coordinator:example",
    ];
    let missing = fixture.run(&repo, &args);
    assert_eq!(missing.status.code(), Some(1), "{}", stderr(&missing));
    assert!(stderr(&missing).contains("coordinator unavailable"));

    let coordinator = repo.join(".dev/pipeline/example/coordinator.json");
    fs::create_dir_all(coordinator.parent().unwrap()).unwrap();
    fs::write(&coordinator, "{").unwrap();
    let malformed = fixture.run(&repo, &args);
    assert_eq!(malformed.status.code(), Some(1), "{}", stderr(&malformed));
    assert!(stderr(&malformed).contains("coordinator malformed"));
}

#[test]
fn doctor_receipt_verifier_accepts_matching_standalone_and_coordinator_receipts() {
    let fixture = Fixture::new();
    let repo = fixture.directory("valid-receipt-repo");
    let receipt = repo.join("standalone.receipt.md");
    fs::write(
        &receipt,
        receipt_envelope(&repo, &fixture.binary, "standalone"),
    )
    .unwrap();
    let standalone = run_receipt(&fixture, &repo, &receipt, "standalone");
    assert_eq!(standalone.status.code(), Some(0), "{}", stderr(&standalone));

    let scope = "example";
    let coordinator = repo.join(".dev/pipeline/example/coordinator.json");
    fs::create_dir_all(coordinator.parent().unwrap()).unwrap();
    fs::write(
        &coordinator,
        coordinator_json(&repo, &fixture.binary, scope),
    )
    .unwrap();
    let receipt = repo.join("coordinator.receipt.md");
    fs::write(
        &receipt,
        receipt_envelope(&repo, &fixture.binary, "coordinator:example"),
    )
    .unwrap();
    let output = run_receipt(&fixture, &repo, &receipt, "coordinator:example");
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn doctor_receipt_verifier_rejects_scope_and_envelope_mismatches() {
    let fixture = Fixture::new();
    let repo = fixture.directory("mismatched-receipt-repo");
    let receipt = repo.join("receipt.md");
    let valid = receipt_envelope(&repo, &fixture.binary, "standalone");

    fs::write(&receipt, &valid).unwrap();
    let wrong_scope = run_receipt(&fixture, &repo, &receipt, "coordinator:example");
    assert_eq!(
        wrong_scope.status.code(),
        Some(1),
        "{}",
        stderr(&wrong_scope)
    );

    for (name, tampered) in [
        (
            "digest",
            valid.replace("execution_binding_sha256: ", "execution_binding_sha256: f"),
        ),
        (
            "path",
            valid.replace("executable_path: ", "executable_path: /wrong/"),
        ),
        (
            "hash",
            valid.replace("executable_sha256: ", "executable_sha256: f"),
        ),
    ] {
        fs::write(&receipt, tampered).unwrap();
        let output = run_receipt(&fixture, &repo, &receipt, "standalone");
        assert_eq!(output.status.code(), Some(1), "{name}: {}", stderr(&output));
    }

    let coordinator = repo.join(".dev/pipeline/example/coordinator.json");
    fs::create_dir_all(coordinator.parent().unwrap()).unwrap();
    fs::write(
        &coordinator,
        coordinator_json(&repo, &fixture.binary, "example"),
    )
    .unwrap();
    let coordinator_receipt = receipt_envelope(&repo, &fixture.binary, "coordinator:example");
    fs::write(&receipt, &coordinator_receipt).unwrap();
    let downgraded = run_receipt(&fixture, &repo, &receipt, "standalone");
    assert_eq!(downgraded.status.code(), Some(1), "{}", stderr(&downgraded));
    let upgraded = run_receipt(&fixture, &repo, &receipt, "coordinator:other");
    assert_eq!(upgraded.status.code(), Some(1), "{}", stderr(&upgraded));
}

#[test]
fn doctor_receipt_verifier_rejects_safe_rebind_to_new_generation() {
    let fixture = Fixture::new();
    let repo = fixture.directory("rebound-receipt-repo");
    let scope = "example";
    let coordinator = repo.join(".dev/pipeline/example/coordinator.json");
    fs::create_dir_all(coordinator.parent().unwrap()).unwrap();
    let new_generation = repo.join("new-generation");
    fs::write(
        &coordinator,
        coordinator_json(&repo, &fixture.binary, scope),
    )
    .unwrap();
    let receipt = repo.join("receipt.md");
    fs::write(
        &receipt,
        receipt_envelope(&repo, &fixture.binary, "coordinator:example"),
    )
    .unwrap();

    fs::copy(&fixture.binary, &new_generation).unwrap();
    fs::write(
        &coordinator,
        coordinator_json(&repo, &new_generation, scope),
    )
    .unwrap();
    let rebound = run_receipt(&fixture, &repo, &receipt, "coordinator:example");
    assert_eq!(rebound.status.code(), Some(1), "{}", stderr(&rebound));
}
