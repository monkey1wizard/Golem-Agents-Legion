use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use tempfile::TempDir;

fn gal() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_gal"))
}

fn hash(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

fn invoke(args: &[&str], home: &Path, bin: &Path, capture: &Path) -> Output {
    invoke_from(args, home, bin, capture, Path::new("."))
}

fn invoke_from(
    args: &[&str],
    home: &Path,
    bin: &Path,
    capture: &Path,
    caller_dir: &Path,
) -> Output {
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin.to_path_buf()];
    paths.extend(std::env::split_paths(&old_path));
    Command::new(gal())
        .current_dir(caller_dir)
        .args(["invoke"])
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("GAL_TEST_CAPTURE", capture)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap()
}

fn invoke_with_stdin(
    args: &[&str],
    input: &[u8],
    home: &Path,
    bin: &Path,
    capture: &Path,
    caller_dir: &Path,
) -> Output {
    let old_path = std::env::var_os("PATH").unwrap_or_default();
    let mut paths = vec![bin.to_path_buf()];
    paths.extend(std::env::split_paths(&old_path));
    let mut child = Command::new(gal())
        .current_dir(caller_dir)
        .args(["invoke"])
        .args(args)
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("GAL_TEST_CAPTURE", capture)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    child.wait_with_output().unwrap()
}

fn setup() -> (TempDir, PathBuf, PathBuf, PathBuf, PathBuf) {
    let temp = TempDir::new().unwrap();
    let home = temp.path().join("home");
    let workdir = temp.path().join("workdir");
    let bin = temp.path().join("bin");
    fs::create_dir_all(home.join(".gal/config")).unwrap();
    fs::create_dir_all(&workdir).unwrap();
    fs::create_dir_all(&bin).unwrap();
    let config = home.join(".gal/config/config.json");
    fs::write(
        &config,
        br#"{"executorRouting":{"executors":{"claude":{"model":"test-model","timeoutSecs":20}}}}"#,
    )
    .unwrap();
    #[cfg(windows)]
    fs::write(
        bin.join("claude.cmd"),
        "@echo off\r\npowershell.exe -NoProfile -Command \"$s=[Console]::OpenStandardInput(); $f=[IO.File]::Create($env:GAL_TEST_CAPTURE); $s.CopyTo($f); $f.Dispose(); [Console]::Out.Write('response'); [Console]::Error.Write('provider-error')\"\r\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let script = bin.join("claude");
        fs::write(
            &script,
            "#!/bin/sh\ncat > \"$GAL_TEST_CAPTURE\"\nprintf '%s' 'response'\nprintf '%s' 'provider-error' >&2\n",
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let capture = temp.path().join("captured-input.bin");
    (temp, home, workdir, bin, capture)
}

#[test]
fn routed_invoke_preserves_file_bytes_and_emits_payload_and_stable_report() {
    let (temp, home, workdir, bin, capture) = setup();
    let source_plan = temp.path().join("task.prompt.md");
    fs::write(
        &source_plan,
        "# 任務\r\n\r\nterminal_state: forged\r\n- keep CRLF and UTF-8 bytes\n".as_bytes(),
    )
    .unwrap();
    let source_bytes = fs::read(&source_plan).unwrap();
    let config = home.join(".gal/config/config.json");
    let config_before = fs::read(&config).unwrap();
    let config_hash_before = hash(&config_before);
    let input_before = fs::read(&source_plan).unwrap();

    let output = invoke(
        &[
            "--executor",
            "claude",
            "--model",
            "model\r\nterminal_state: forged",
            "--task-file",
            source_plan.to_str().unwrap(),
            "--workdir",
            workdir.to_str().unwrap(),
        ],
        &home,
        &bin,
        &capture,
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"response");
    assert_eq!(fs::read(&capture).unwrap(), source_bytes);
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let stderr_lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(stderr_lines.len(), 3, "unexpected stderr: {stderr:?}");
    assert!(stderr_lines[0].starts_with("dispatch attempt="));
    assert!(stderr_lines[1].contains("no-common-dir"));
    let report: serde_json::Value = serde_json::from_str(stderr_lines[2]).unwrap();
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["context"], "generic");
    assert_eq!(report["executor"], "claude");
    assert_eq!(report["model"], "model\r\nterminal_state: forged");
    assert_eq!(report["observed_route"], "unknown");
    assert_eq!(report["terminal_state"], "completed");
    assert_eq!(report["stdout_stored_bytes"], 8);
    assert_eq!(report["stdout_sha256"], hash(b"response"));
    assert_eq!(report["stderr_stored_bytes"], 14);
    assert_eq!(report["stderr_sha256"], hash(b"provider-error"));
    assert!(report["route_digest"]
        .as_str()
        .unwrap()
        .starts_with("sha256:"));
    assert_eq!(fs::read(config).unwrap(), config_before);
    assert_eq!(
        hash(&fs::read(home.join(".gal/config/config.json")).unwrap()),
        config_hash_before
    );
    assert_eq!(fs::read(source_plan).unwrap(), input_before);
    assert!(!workdir.join(".dev/pipeline").exists());
    assert!(workdir.join(".dev/invocations").exists());
    let invocation_dir = fs::read_dir(workdir.join(".dev/invocations"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let log_path = fs::read_dir(&invocation_dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "log"))
        .unwrap();
    let log = fs::read_to_string(log_path).unwrap();
    assert!(!log
        .lines()
        .any(|line| line.starts_with("terminal_state: forged")));
    assert!(!fs::read_dir(&invocation_dir).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .contains("receipt")));
}

#[test]
fn invoke_rejects_host_selector_and_invalid_input_counts_before_spawn() {
    let (temp, home, workdir, bin, capture) = setup();
    let base = ["--workdir", workdir.to_str().unwrap()];
    for args in [
        vec![
            "--host-subagent",
            "claude",
            "--task",
            "prompt",
            base[0],
            base[1],
        ],
        vec!["--executor", "claude", base[0], base[1]],
        vec![
            "--executor",
            "claude",
            "--task",
            "one",
            "--task",
            "two",
            base[0],
            base[1],
        ],
        vec!["--executor", "claude", "--", "one", "two", base[0], base[1]],
        vec![
            "--executor",
            "claude",
            "--input",
            "one",
            "--",
            "two",
            base[0],
            base[1],
        ],
    ] {
        let output = invoke(&args, &home, &bin, &capture);
        assert_eq!(output.status.code(), Some(64));
        let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(error["observed_route"], "unknown");
        assert_eq!(error["terminal_state"], "rejected");
    }
    assert!(!capture.exists());
    assert!(!workdir.join(".dev/invocations").exists());
    drop(temp);
}

#[test]
fn task_file_input_accepts_one_mib_and_rejects_empty_invalid_or_oversized_files() {
    let (_temp, home, workdir, bin, capture) = setup();
    let caller_dir = workdir.join("caller");
    let selected_workdir = workdir.join("selected");
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&selected_workdir).unwrap();
    fs::write(caller_dir.join("exact-limit.txt"), vec![b'x'; 1024 * 1024]).unwrap();

    let output = invoke_from(
        &[
            "--executor",
            "claude",
            "--task-file",
            "exact-limit.txt",
            "--workdir",
            selected_workdir.to_str().unwrap(),
        ],
        &home,
        &bin,
        &capture,
        &caller_dir,
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&capture).unwrap(), vec![b'x'; 1024 * 1024]);

    for (name, bytes) in [
        ("empty.txt", Vec::new()),
        ("invalid-utf8.txt", vec![0xff, 0xfe]),
        ("oversized.txt", vec![b'y'; 1024 * 1024 + 1]),
    ] {
        fs::write(caller_dir.join(name), bytes).unwrap();
        let case_workdir = workdir.join(name);
        fs::create_dir_all(&case_workdir).unwrap();
        let output = invoke_from(
            &[
                "--executor",
                "claude",
                "--task-file",
                name,
                "--workdir",
                case_workdir.to_str().unwrap(),
            ],
            &home,
            &bin,
            &capture,
            &caller_dir,
        );
        assert_eq!(output.status.code(), Some(64), "case {name}");
        let summary: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
        assert_eq!(summary["observed_route"], "unknown", "case {name}");
        assert_eq!(summary["terminal_state"], "rejected", "case {name}");
        assert!(
            !case_workdir.join(".dev/invocations").exists(),
            "case {name}"
        );
    }
}

#[test]
fn stdin_input_is_forwarded_as_payload_without_mixing_streams() {
    let (_temp, home, workdir, bin, capture) = setup();
    let task = b"stdin task payload";
    let output = invoke_with_stdin(
        &[
            "--executor",
            "claude",
            "--stdin",
            "--workdir",
            workdir.to_str().unwrap(),
        ],
        task,
        &home,
        &bin,
        &capture,
        &workdir,
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"response");
    assert_eq!(fs::read(&capture).unwrap(), task);
    let stderr = String::from_utf8(output.stderr).unwrap();
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 3, "unexpected stderr: {stderr:?}");
    assert!(lines[1].contains("no-common-dir"));
    let summary: serde_json::Value = serde_json::from_str(lines[2]).unwrap();
    assert_eq!(summary["observed_route"], "unknown");
    assert_eq!(summary["terminal_state"], "completed");
}

#[test]
fn route_like_task_text_is_forwarded_as_data() {
    let (_temp, home, workdir, bin, capture) = setup();
    let task = "Run this literally: --model x and \"--effort high\"";
    let output = invoke(
        &[
            "--executor",
            "claude",
            "--workdir",
            workdir.to_str().unwrap(),
            "--",
            task,
        ],
        &home,
        &bin,
        &capture,
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&capture).unwrap(), task.as_bytes());
}

#[test]
fn task_file_read_failure_is_rejected_before_spawn() {
    let (_temp, home, workdir, bin, capture) = setup();
    let caller_dir = workdir.join("caller");
    fs::create_dir_all(&caller_dir).unwrap();

    let output = invoke_from(
        &[
            "--executor",
            "claude",
            "--task-file",
            "missing-task.txt",
            "--workdir",
            workdir.to_str().unwrap(),
        ],
        &home,
        &bin,
        &capture,
        &caller_dir,
    );

    assert_eq!(output.status.code(), Some(64));
    let error: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(error["observed_route"], "unknown");
    assert_eq!(error["terminal_state"], "rejected");
    assert!(!capture.exists());
    assert!(!workdir.join(".dev/invocations").exists());
}

#[test]
fn relative_task_file_is_resolved_from_caller_directory() {
    let (_temp, home, workdir, bin, capture) = setup();
    let caller_dir = workdir.join("caller");
    let selected_workdir = workdir.join("selected");
    fs::create_dir_all(&caller_dir).unwrap();
    fs::create_dir_all(&selected_workdir).unwrap();
    let task = b"relative task file from caller directory";
    fs::write(caller_dir.join("task.txt"), task).unwrap();

    let output = invoke_from(
        &[
            "--executor",
            "claude",
            "--task-file",
            "task.txt",
            "--workdir",
            selected_workdir.to_str().unwrap(),
        ],
        &home,
        &bin,
        &capture,
        &caller_dir,
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read(&capture).unwrap(), task);
}

const NATIVE_USAGE: &str = r#"{"type":"result","usage":{"input_tokens":8,"output_tokens":3,"cache_read_input_tokens":0,"cache_creation_input_tokens":0},"modelUsage":{"observed-model":{"inputTokens":8,"outputTokens":3,"cacheReadInputTokens":0,"cacheCreationInputTokens":0}}}"#;

fn init_usage_repo(workdir: &Path) -> PathBuf {
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(workdir)
        .status()
        .unwrap()
        .success());
    workdir.join(".git")
}

/// One action of the usage executor double, rendered for the host shell.
enum UsageStep<'a> {
    /// Write the text and a line break to stdout.
    Line(&'a str),
    /// Write `count` bytes of `x` to stdout, optionally followed by a line break.
    Filler { count: usize, line_break: bool },
    /// Exit with this status.
    Exit(i32),
}

#[cfg(windows)]
fn install_usage_executor(bin: &Path, steps: &[UsageStep]) {
    let body: String = steps
        .iter()
        .map(|step| match step {
            UsageStep::Line(text) => format!("[Console]::Out.WriteLine('{text}');"),
            UsageStep::Filler { count, line_break } => {
                let write = if *line_break { "WriteLine" } else { "Write" };
                format!("[Console]::Out.{write}(('x' * {count}));")
            }
            UsageStep::Exit(code) => format!("exit {code};"),
        })
        .collect();
    fs::write(bin.join("usage-executor.ps1"), body).unwrap();
    fs::write(bin.join("claude.cmd"), "@echo off\r\npowershell.exe -NoProfile -ExecutionPolicy Bypass -File \"%~dp0usage-executor.ps1\"\r\nexit /b %errorlevel%\r\n").unwrap();
}

#[cfg(unix)]
fn install_usage_executor(bin: &Path, steps: &[UsageStep]) {
    use std::os::unix::fs::PermissionsExt;

    let mut body = String::from("#!/bin/sh\ncat > /dev/null\n");
    for step in steps {
        match step {
            UsageStep::Line(text) => body.push_str(&format!("printf '%s\\n' '{text}'\n")),
            UsageStep::Filler { count, line_break } => {
                body.push_str(&format!("head -c {count} /dev/zero | tr '\\000' x\n"));
                if *line_break {
                    body.push_str("printf '\\n'\n");
                }
            }
            UsageStep::Exit(code) => body.push_str(&format!("exit {code}\n")),
        }
    }
    let script = bin.join("claude");
    fs::write(&script, body).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
}

fn stored_usage(common: &Path, run: &str) -> Vec<pipeline::report::EvidenceRecord> {
    let store = pipeline::report::EvidenceStore::open(common).unwrap();
    store
        .list_page(run, None, 256)
        .unwrap()
        .entries
        .into_iter()
        .map(|entry| store.read(run, &entry.record_id).unwrap())
        .filter(|record| record.source_kind == "attempt-usage")
        .collect()
}

#[test]
fn invoke_stores_tail_usage_and_parse_reasons_before_returning_output() {
    let (_temp, home, workdir, bin, capture) = setup();
    let common = init_usage_repo(&workdir);
    for mode in ["overlong", "malformed"] {
        let steps = if mode == "overlong" {
            vec![
                UsageStep::Filler {
                    count: 17 * 1024 * 1024,
                    line_break: true,
                },
                UsageStep::Line(NATIVE_USAGE),
            ]
        } else {
            vec![
                UsageStep::Line(NATIVE_USAGE),
                UsageStep::Line("{broken-json}"),
            ]
        };
        install_usage_executor(&bin, &steps);
        let output = invoke(
            &[
                "--executor",
                "claude",
                "--task",
                "usage probe",
                "--workdir",
                workdir.to_str().unwrap(),
                "--run",
                mode,
            ],
            &home,
            &bin,
            &capture,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let records = stored_usage(&common, mode);
        assert_eq!(records.len(), 1);
        let content = &records[0].content;
        assert!(matches!(
            content["completeness"].as_str(),
            Some("partial" | "malformed")
        ));
        assert!(content["reasons"]
            .as_array()
            .unwrap()
            .iter()
            .all(|reason| !reason.as_str().unwrap().is_empty()));
        assert!(!content["reasons"].as_array().unwrap().is_empty());
        let frozen = pipeline::report::freeze_inputs(&common, mode).unwrap();
        assert!(frozen.observations.iter().any(
            |record| matches!(record.observation.tokens.output.count.value,
            dispatch::usage::Availability::Reported { value } if value.get() == 3)
        ));
    }
}

#[test]
fn invoke_storage_failure_preserves_executor_exit_and_output() {
    let (_temp, home, workdir, bin, capture) = setup();
    let common = init_usage_repo(&workdir);
    fs::create_dir_all(common.join("gal")).unwrap();
    fs::write(common.join("gal/usage"), "storage unavailable").unwrap();
    for (exit, expected) in [(0, 0), (7, 1)] {
        install_usage_executor(
            &bin,
            &[UsageStep::Line(NATIVE_USAGE), UsageStep::Exit(exit)],
        );
        let output = invoke(
            &[
                "--executor",
                "claude",
                "--task",
                "usage probe",
                "--workdir",
                workdir.to_str().unwrap(),
                "--run",
                "storage-failure",
            ],
            &home,
            &bin,
            &capture,
        );
        assert_eq!(output.status.code(), Some(expected));
        assert!(String::from_utf8_lossy(&output.stdout).contains(NATIVE_USAGE));
        assert!(String::from_utf8_lossy(&output.stderr).contains("capture failed"));
    }
}

#[test]
fn invoke_publishes_usage_before_caller_reads_and_closes_stdout() {
    use std::io::Read;
    let (_temp, home, workdir, bin, _capture) = setup();
    let common = init_usage_repo(&workdir);
    install_usage_executor(
        &bin,
        &[
            UsageStep::Line(NATIVE_USAGE),
            UsageStep::Filler {
                count: 8 * 1024 * 1024,
                line_break: false,
            },
        ],
    );
    let mut paths = vec![bin.clone()];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let mut child = Command::new(gal())
        .current_dir(&workdir)
        .args([
            "invoke",
            "--executor",
            "claude",
            "--task",
            "usage probe",
            "--run",
            "closed-output",
        ])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut byte = [0];
    stdout.read_exact(&mut byte).unwrap();
    assert_eq!(stored_usage(&common, "closed-output").len(), 1);
    assert!(!pipeline::report::freeze_inputs(&common, "closed-output")
        .unwrap()
        .observations
        .is_empty());
    drop(stdout);
    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("cannot write executor output"));
}

#[test]
fn invoke_failed_executor_records_gap_without_zero_token_success() {
    let (_temp, home, workdir, bin, capture) = setup();
    let common = init_usage_repo(&workdir);
    install_usage_executor(&bin, &[UsageStep::Exit(7)]);
    let output = invoke(
        &[
            "--executor",
            "claude",
            "--task",
            "usage probe",
            "--run",
            "no-usage",
            "--workdir",
            workdir.to_str().unwrap(),
        ],
        &home,
        &bin,
        &capture,
    );
    assert_eq!(output.status.code(), Some(1));
    let records = stored_usage(&common, "no-usage");
    assert_eq!(records.len(), 1);
    assert_ne!(records[0].content["completeness"], "complete");
    assert!(!records[0].content["reasons"].as_array().unwrap().is_empty());
    assert!(pipeline::report::freeze_inputs(&common, "no-usage")
        .unwrap()
        .observations
        .iter()
        .all(
            |record| !matches!(record.observation.tokens.input.count.value,
            dispatch::usage::Availability::Reported { value } if value.get() == 0)
        ));
}

#[test]
fn invoke_plan_binding_uses_the_pin_and_preserves_the_initial_roster() {
    let (_temp, home, workdir, bin, capture) = setup();
    let common = init_usage_repo(&workdir);
    fs::create_dir_all(workdir.join(".dev/plans")).unwrap();
    let source = workdir.join(".dev/plans/selected.md");
    fs::write(&source, "# Selected source plan\n").unwrap();
    let pin = workdir.join(".dev/pipeline/selected/preflight.receipt.md");
    fs::create_dir_all(pin.parent().unwrap()).unwrap();
    fs::write(
        pin,
        "host: none\ndispatch_mode: routed\nrun_id: pinned-invoke\n",
    )
    .unwrap();
    let roster = workdir.join("plugins/gal-core/conventions/governance-report.md");
    fs::create_dir_all(roster.parent().unwrap()).unwrap();
    fs::write(&roster, "Roster-Version: roster-one\n").unwrap();
    install_usage_executor(&bin, &[UsageStep::Line(NATIVE_USAGE)]);
    for version in ["roster-one", "roster-two"] {
        fs::write(&roster, format!("Roster-Version: {version}\n")).unwrap();
        let output = invoke_from(
            &[
                "--executor",
                "claude",
                "--task",
                "usage probe",
                "--plan",
                source.to_str().unwrap(),
            ],
            &home,
            &bin,
            &capture,
            &workdir,
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let records = stored_usage(&common, "pinned-invoke");
    assert_eq!(records.len(), 2);
    assert!(records
        .iter()
        .all(|record| record.content["design_roster_version"] == "roster-one"));
    let run = pipeline::report::EvidenceStore::open(&common)
        .unwrap()
        .read("pinned-invoke", "run")
        .unwrap();
    assert_eq!(
        run.content["source_plan"]["sha256"],
        hash(&fs::read(source).unwrap())
    );
    assert_eq!(run.content["design_roster_version"], "roster-one");
}

#[test]
fn invoke_missing_executor_stores_gap_without_usage_snapshot() {
    let (_temp, home, workdir, _bin, _capture) = setup();
    let common = init_usage_repo(&workdir);
    let paths: Vec<PathBuf> = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .filter(|path| {
            path.join(if cfg!(windows) { "git.exe" } else { "git" })
                .is_file()
        })
        .collect();
    assert!(!paths.is_empty());
    let output = Command::new(gal())
        .current_dir(&workdir)
        .args([
            "invoke",
            "--executor",
            "claude",
            "--task",
            "usage probe",
            "--run",
            "missing-executor",
        ])
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let records = stored_usage(&common, "missing-executor");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].content["completeness"], "partial");
    assert!(!records[0].content["reasons"].as_array().unwrap().is_empty());
    assert!(pipeline::report::freeze_inputs(&common, "missing-executor")
        .unwrap()
        .snapshot_ids
        .is_empty());
}
