#[path = "../src/commands/pipeline_execution.rs"]
mod pipeline_execution;

use sha2::{Digest, Sha256};
use std::{fs, path::Path, thread, time::Instant};
use tempfile::TempDir;

#[test]
fn task_commit_drift_is_accepted_only_at_test_when_task_files_are_unchanged() {
    use std::process::Command;
    for scenario in [
        "unrelated",
        "modified",
        "rename_from",
        "rename_to",
        "non_ancestor",
        "empty",
        "audit",
    ] {
        let repo = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let root = repo.path();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .current_dir(root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "git {args:?}: {}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "test"]);
        fs::write(root.join("task.txt"), "task contents\n").unwrap();
        fs::write(root.join("other.txt"), "other contents\n").unwrap();
        let agents = root.join("plugins/gal-core/agents");
        fs::create_dir_all(&agents).unwrap();
        for role in ["tester", "auditor"] {
            fs::write(
                agents.join(format!("golem-{role}.agent.md")),
                format!("---\nname: golem-{role}\n---\n"),
            )
            .unwrap();
        }
        git(&["add", "."]);
        git(&["-c", "commit.gpgsign=false", "commit", "-qm", "base"]);
        let base = git(&["rev-parse", "HEAD"]);
        match scenario {
            "modified" => fs::write(root.join("task.txt"), "changed\n").unwrap(),
            "rename_from" => {
                git(&["mv", "task.txt", "renamed.txt"]);
            }
            "rename_to" => {
                git(&["mv", "other.txt", "renamed.txt"]);
            }
            _ => fs::write(root.join("other.txt"), "changed\n").unwrap(),
        }
        git(&["add", "."]);
        git(&["-c", "commit.gpgsign=false", "commit", "-qm", "next"]);
        let next = git(&["rev-parse", "HEAD"]);
        let recorded = if scenario == "non_ancestor" {
            git(&["reset", "--hard", "-q", &base]);
            &next
        } else {
            &base
        };
        let task = format!("T-{:02}", 1);
        let files = match scenario {
            "empty" => "",
            "rename_to" => "`renamed.txt`",
            _ => "`task.txt`",
        };
        let prompt = root.join(".dev/plans/example.prompt.md");
        fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        let original = format!("# Plan\n\n## Tasks\n\n- [ ] {task} — Fixture\n  - Files: {files}\n  - Change: Fixture.\n  - Acceptance: Pass.\n\n## Status\n\nTask Base Commit: {base}\nTask Final Commit: {recorded}\n\n### Deviations\n\n| ID | Status | Description | Evidence |\n| --- | --- | --- | --- |\n\n## Test Results\n\n## Review Results\n");
        fs::write(&prompt, &original).unwrap();
        let phase = if scenario == "audit" { "audit" } else { "test" };
        let run = || {
            Command::new(env!("CARGO_BIN_EXE_gal"))
                .current_dir(root)
                .args([
                    "pipeline",
                    ".dev/plans/example.prompt.md",
                    "--task",
                    &task,
                    "--phase",
                    phase,
                    "--dispatch",
                    "host-subagent",
                ])
                .env("HOME", home.path())
                .env("USERPROFILE", home.path())
                .output()
                .unwrap()
        };
        let output = run();
        let actual = fs::read_to_string(&prompt).unwrap();
        if scenario == "unrelated" {
            assert!(
                actual.contains(&format!("Task Final Commit: {next}")),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(actual.contains(&format!("Task Base Commit: {base}")));
            assert_eq!(
                actual.matches("Accepted Task Final Commit drift").count(),
                1
            );
        } else {
            assert!(!output.status.success(), "{scenario}");
            assert_eq!(actual, original, "{scenario}");
            let error = String::from_utf8_lossy(&output.stderr);
            assert!(
                error.contains("Task Final Commit drift"),
                "{scenario}: {error}"
            );
            if scenario.starts_with("rename") || scenario == "modified" {
                assert!(
                    error.contains(if scenario == "rename_to" {
                        "renamed.txt"
                    } else {
                        "task.txt"
                    }),
                    "{error}"
                );
            }
            if scenario == "audit" {
                assert!(error.contains("--phase test"), "{error}");
                let again = run();
                assert!(!again.status.success());
                assert!(String::from_utf8_lossy(&again.stderr).contains("--phase test"));
                assert_eq!(fs::read_to_string(&prompt).unwrap(), original);
                assert!(!root
                    .join(".dev/pipeline/example")
                    .join(&task)
                    .join("audit.log")
                    .exists());
            }
        }
    }
}

#[test]
fn legacy_routed_test_host_open_is_single_use_and_requires_matching_evidence() {
    use std::process::Command;
    for (terminal, same_head, host, guarded, dirty, handoff, opens) in [
        ("timeout-midrun", true, "codex", false, false, false, true),
        ("workdir-changed", true, "codex", false, false, false, true),
        ("failed", true, "codex", false, false, false, false),
        ("timeout-midrun", false, "codex", false, false, false, false),
        ("timeout-midrun", true, "none", false, false, false, false),
        ("timeout-midrun", true, "codex", true, false, false, false),
        ("workdir-changed", true, "codex", false, true, false, false),
        ("timeout-midrun", true, "codex", false, false, true, false),
    ] {
        let repo = TempDir::new().unwrap();
        let home = TempDir::new().unwrap();
        let root = repo.path();
        let git = |args: &[&str]| {
            let out = Command::new("git")
                .current_dir(root)
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
            String::from_utf8_lossy(&out.stdout).trim().to_owned()
        };
        git(&["init", "-q"]);
        git(&["config", "user.email", "test@example.invalid"]);
        git(&["config", "user.name", "test"]);
        fs::write(root.join(".gitignore"), ".dev/pipeline/\n").unwrap();
        let agents = root.join("plugins/gal-core/agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(
            agents.join("golem-tester.agent.md"),
            "---\nname: golem-tester\n---\n",
        )
        .unwrap();
        git(&["add", "."]);
        git(&["-c", "commit.gpgsign=false", "commit", "-qm", "fixture"]);
        let head = git(&["rev-parse", "HEAD"]);
        let short = git(&["rev-parse", "--short", "HEAD"]);
        let task_id = format!("T-{:02}", 1);
        let prompt = root.join(".dev/plans/example.prompt.md");
        fs::create_dir_all(prompt.parent().unwrap()).unwrap();
        fs::write(&prompt, format!("# Plan\n\n## Tasks\n\n- [ ] {task_id} — Test fixture.\n  - Files: `example.txt`\n  - Change: Verify behavior.\n  - Acceptance: Pass.\n\n## Status\n\nTask Final Commit: {head}\n\n## Test Results\n\n## Review Results\n")).unwrap();
        if handoff {
            let body = fs::read_to_string(&prompt).unwrap().replace("## Test Results", &format!("### Handoff Notes\n\n#### Retry Handoff — {task_id} / IMPLEMENT\n- Status: OPEN\n- Problem: repair\n- Next human step: repair\n\n## Test Results"));
            fs::write(&prompt, body).unwrap();
        }
        if dirty {
            fs::write(root.join("unrestored.txt"), "changed").unwrap();
        }
        let scope = root.join(".dev/pipeline/example");
        let task = scope.join(&task_id);
        fs::create_dir_all(&task).unwrap();
        let pin = format!("run_id: run-1\nhost: {host}\ndispatch_mode: routed\n");
        fs::write(scope.join("preflight.receipt.md"), &pin).unwrap();
        fs::write(task.join("implement.log"), format!("GAL-DISPATCH-LOG v1\ntimestamp_start: 2026-01-01T00:00:00.000Z\nexecutor: agy\nphase: implement\ntask_id: {task_id}\nterminal_state: completed\ntier: standard\n---STDOUT---\n")).unwrap();
        fs::write(task.join("test.log"), format!("GAL-DISPATCH-LOG v1\ntimestamp_start: 2026-01-02T00:00:00.000Z\nexecutor: agy\nphase: test\ntask_id: {task_id}\ngit_head: {}\nterminal_state: {terminal}\n---STDOUT---\n", if same_head { &short } else { "badhead" })).unwrap();
        if guarded {
            fs::write(scope.join("coordinator.json"), "{}\n").unwrap();
        }
        let run = || {
            Command::new(env!("CARGO_BIN_EXE_gal"))
                .current_dir(root)
                .args([
                    "pipeline",
                    ".dev/plans/example.prompt.md",
                    "--task",
                    &task_id,
                    "--phase",
                    "test",
                    "--dispatch",
                    "host-subagent",
                ])
                .env("HOME", home.path())
                .env("USERPROFILE", home.path())
                .output()
                .unwrap()
        };
        let first = run();
        assert_eq!(
            first.status.success(),
            opens,
            "terminal={terminal}, same_head={same_head}, host={host}, guarded={guarded}, dirty={dirty}, handoff={handoff}: {}",
            String::from_utf8_lossy(&first.stderr)
        );
        assert_eq!(
            fs::read_to_string(scope.join("preflight.receipt.md")).unwrap(),
            pin
        );
        if opens {
            let second = run();
            assert!(!second.status.success(), "second host OPEN succeeded");
        }
    }
}

fn write_project(root: &Path, source: &str) -> std::path::PathBuf {
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("Cargo.toml"), "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[[bin]]\nname = \"gal\"\npath = \"src/main.rs\"\n").unwrap();
    fs::write(root.join("src/main.rs"), source).unwrap();
    root.join("Cargo.toml")
}

fn hash(path: &Path) -> String {
    format!("{:x}", Sha256::digest(fs::read(path).unwrap()))
}

#[test]
fn private_builds_serialize_publish_immutable_hashes_and_keep_binding_on_failure() {
    let fixture = TempDir::new().unwrap();
    let root = fixture.path().join("source");
    fs::create_dir_all(&root).unwrap();
    let manifest = write_project(&root, "fn main() { println!(\"generation-one\"); }\n");

    let started = Instant::now();
    let root_a = root.clone();
    let manifest_a = manifest.clone();
    let first = thread::spawn(move || {
        pipeline_execution::build_private_generation(&root_a, &manifest_a).unwrap()
    });
    let root_b = root.clone();
    let manifest_b = manifest.clone();
    let second = thread::spawn(move || {
        pipeline_execution::build_private_generation(&root_b, &manifest_b).unwrap()
    });
    let generation_a = first.join().unwrap();
    let generation_b = second.join().unwrap();
    let elapsed = started.elapsed();

    assert_eq!(
        generation_a, generation_b,
        "serialized process results must reuse one immutable generation"
    );
    let digest = hash(&generation_a);
    assert_eq!(
        generation_a
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy(),
        digest
    );
    assert!(
        root.join("target/gal-pipeline/cargo-target/debug").is_dir(),
        "Cargo must use the worktree-private target directory"
    );
    let original_bytes = fs::read(&generation_a).unwrap();

    let binding = root.join("binding.json");
    fs::write(&binding, b"previous-binding\n").unwrap();
    let failed_manifest = write_project(&root, "fn main( { this is invalid rust\n");
    let failure =
        pipeline_execution::build_private_generation(&root, &failed_manifest).unwrap_err();
    assert!(
        failure.to_string().contains("cargo build failed"),
        "failed Cargo process result: {failure}"
    );
    assert_eq!(
        fs::read(&binding).unwrap(),
        b"previous-binding\n",
        "a failed build must preserve the previous binding"
    );
    assert_eq!(
        fs::read(&generation_a).unwrap(),
        original_bytes,
        "a failed build must preserve published generation bytes"
    );
    assert_eq!(
        hash(&generation_a),
        digest,
        "published executable hash must remain immutable"
    );

    eprintln!(
        "Process results: concurrent builds returned the same generation; invalid source returned a Cargo failure"
    );
    eprintln!(
        "Overlap and serialization timing: two concurrent calls completed in {elapsed:?} under one fs2 worktree lock"
    );
    eprintln!("Immutable hash: {digest}; prior binding bytes unchanged after failure");
}

#[test]
fn changed_entry_source_publishes_a_new_private_generation() {
    let fixture = TempDir::new().unwrap();
    let root = fixture.path().join("source");
    fs::create_dir_all(&root).unwrap();
    let manifest = write_project(&root, "fn main() { println!(\"before-recovery\"); }\n");
    let before = pipeline_execution::build_private_generation(&root, &manifest).unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"after-recovery\"); }\n",
    )
    .unwrap();

    let after = pipeline_execution::build_private_generation(&root, &manifest).unwrap();
    assert_ne!(
        before, after,
        "changed HEAD source must publish a new generation"
    );
    assert_eq!(
        hash(&after),
        after
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_string_lossy()
    );
}

#[test]
fn forced_rebind_from_existing_private_executable_runs_the_new_child_once() {
    let fixture = TempDir::new().unwrap();
    let root = fixture.path().join("source");
    fs::create_dir_all(&root).unwrap();
    let manifest = write_project(&root, "fn main() { println!(\"old-child\"); }\n");
    let prior = pipeline_execution::build_private_generation(&root, &manifest).unwrap();
    fs::write(
        root.join("src/main.rs"),
        "fn main() { println!(\"rebound-child\"); }\n",
    )
    .unwrap();

    let rebound = pipeline_execution::build_private_generation(&root, &manifest).unwrap();
    assert_ne!(prior, rebound, "recovery must not reuse the old executable");
    let output = std::process::Command::new(&rebound)
        .output()
        .expect("rebound child must start");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "rebound-child"
    );
}

#[test]
fn real_engine_build_accepts_private_target_from_canonical_worktree() {
    let worktree = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("workspace root must canonicalize");
    let manifest = worktree.join("Cargo.toml");
    let private_target = worktree.join("target/gal-pipeline/cargo-target");
    let cargo_worktree = pipeline_execution::strip_windows_verbatim_prefix(&worktree);
    let cargo_manifest = pipeline_execution::strip_windows_verbatim_prefix(&manifest);
    let cargo_target = pipeline_execution::strip_windows_verbatim_prefix(&private_target);
    fs::create_dir_all(&private_target).unwrap();

    let output = std::process::Command::new("cargo")
        .current_dir(&cargo_worktree)
        .args(["build", "--manifest-path"])
        .arg(&cargo_manifest)
        .args(["-p", "gal-engine"])
        .env("CARGO_TARGET_DIR", &cargo_target)
        .output()
        .expect("cargo must start");
    assert!(
        output.status.success(),
        "cargo build -p gal-engine failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "builds the real gal binary into the worktree-private target; run with --ignored"]
fn real_worktree_build_reuses_the_published_generation_when_nothing_changed() {
    let worktree = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
        .expect("workspace root must canonicalize");
    let manifest = worktree.join("Cargo.toml");
    let generations = worktree.join("target/gal-pipeline/bin");

    let first = pipeline_execution::build_private_generation(&worktree, &manifest)
        .expect("first private build must succeed");
    let count_after_first = fs::read_dir(&generations).unwrap().count();
    let second = pipeline_execution::build_private_generation(&worktree, &manifest)
        .expect("second private build must succeed");

    assert_eq!(
        first, second,
        "an unchanged tree must reuse the published generation"
    );
    assert_eq!(hash(&first), hash(&second));
    assert_eq!(
        fs::read_dir(&generations).unwrap().count(),
        count_after_first,
        "a repeated build must not publish another generation"
    );
}
