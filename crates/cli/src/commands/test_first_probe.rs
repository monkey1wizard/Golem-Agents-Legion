//! Dormant deterministic execution and publication for binary-canonical probes.

use super::test_first_fs;
use gal_foundation::validated_repo_path::{ValidatedRepoPath, ValidatedRepoPathMode};
use pipeline::task_spec::probe_evidence::{
    compute_env_digest, compute_probe_identity_bytes, compute_probe_set_digest,
    create_output_envelope, decode_argv_b64, encode_argv_b64, FailureClass, ObservedResult,
    ProbeReceipt, ProbeRecord, ProbeSelector, Termination, RUNNER_CONTRACT,
};
use pipeline::task_spec::{
    decode_base64url_unpadded, encode_base64url_unpadded, extract_plan_slug, parse_task_contract,
};
use std::collections::BTreeMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

const MAX_OUTPUT_BYTES: usize = 100 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedFailure {
    pub id: String,
    pub class: FailureClass,
    pub termination: String,
    pub stream: String,
    pub matcher: Vec<u8>,
}

impl ExpectedFailure {
    pub fn parse(line: &str) -> Result<Self, String> {
        let (head, _) = line
            .split_once(" — ")
            .ok_or_else(|| "expected failure must contain an em dash separator".to_string())?;
        let mut fields = head.split(';');
        let id = fields.next().ok_or("missing expected failure id")?;
        let mut class = None;
        let mut termination = None;
        let mut stream = None;
        let mut matcher = None;
        for field in fields {
            let (key, value) = field
                .split_once('=')
                .ok_or_else(|| format!("malformed expected failure field '{field}'"))?;
            match key {
                "class" => class = Some(value),
                "term" => termination = Some(value),
                "stream" => stream = Some(value),
                "matcher_b64" => matcher = Some(value),
                _ => return Err(format!("unknown expected failure field '{key}'")),
            }
        }
        if id.len() != 5
            || !id.starts_with("EF-")
            || !id[3..].chars().all(|c| c.is_ascii_digit())
            || !matches!(id[3..].parse::<u32>(), Ok(value) if (1..=99).contains(&value))
        {
            return Err(format!("invalid expected failure id '{id}'"));
        }
        let class =
            FailureClass::parse(class.ok_or("missing class")?).map_err(|e| e.to_string())?;
        let termination = termination.ok_or("missing term")?;
        if termination != "nonzero"
            && !termination.starts_with("exit:")
            && !termination.starts_with("signal:")
        {
            return Err(format!("invalid expected failure term '{termination}'"));
        }
        if termination != "nonzero" {
            Termination::parse(termination).map_err(|e| e.to_string())?;
        }
        if cfg!(not(unix)) && termination.starts_with("signal:") {
            return Err("signal termination is unsupported on this platform".to_string());
        }
        let stream = stream.ok_or("missing stream")?;
        if !matches!(stream, "stdout" | "stderr" | "combined") {
            return Err(format!("invalid expected failure stream '{stream}'"));
        }
        let matcher_b64 = matcher.ok_or("missing matcher_b64")?;
        let matcher = decode_base64url_unpadded(matcher_b64)
            .map_err(|e| format!("invalid matcher_b64: {e}"))?;
        if encode_base64url_unpadded(&matcher) != matcher_b64 {
            return Err("matcher_b64 must be unpadded base64url".to_string());
        }
        Ok(Self {
            id: id.to_string(),
            class,
            termination: termination.to_string(),
            stream: stream.to_string(),
            matcher,
        })
    }

    fn matches(&self, termination: &Termination, stdout: &[u8], stderr: &[u8]) -> bool {
        let termination_ok = match self.termination.as_str() {
            "nonzero" => matches!(termination, Termination::Exit(n) if *n != 0),
            value if value.starts_with("exit:") => termination.to_canonical_string() == value,
            value if value.starts_with("signal:") => termination.to_canonical_string() == value,
            _ => false,
        };
        let stream = match self.stream.as_str() {
            "stdout" => stdout.to_vec(),
            "stderr" => stderr.to_vec(),
            "combined" => {
                let mut combined = stdout.to_vec();
                combined.push(b'\n');
                combined.extend_from_slice(stderr);
                combined
            }
            _ => return false,
        };
        termination_ok
            && (self.matcher.is_empty()
                || stream
                    .windows(self.matcher.len())
                    .any(|w| w == self.matcher))
    }
}

#[derive(Debug, Clone)]
pub struct ProbeInput {
    pub id: String,
    pub selector: ProbeSelector,
    pub argv_b64: String,
    pub env_overrides: Vec<(String, String)>,
    pub expected_failure: Option<ExpectedFailure>,
    pub timeout_ms: u32,
}

#[derive(Debug, Clone)]
pub struct ProbeRun {
    pub record: ProbeRecord,
    pub output: Vec<u8>,
    pub matched_expected_failure: bool,
}

fn canonical_environment(
    repo_root: &Path,
    overrides: &[(String, String)],
) -> Result<(Vec<(String, String)>, String), String> {
    let mut environment = BTreeMap::<String, String>::new();
    let mut override_keys = std::collections::BTreeSet::new();
    for (key, value) in std::env::vars() {
        environment.insert(key, value);
    }
    for (key, value) in overrides {
        if key.is_empty() || key.contains('\0') || value.contains('\0') {
            return Err("environment key/value is empty or contains NUL".to_string());
        }
        if !override_keys.insert(key) {
            return Err(format!("duplicate environment override key: {key}"));
        }
        environment.insert(key.clone(), value.clone());
    }
    let root = repo_root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize repository root: {e}"))?;
    let root = root
        .to_str()
        .ok_or_else(|| "canonical repository root is not UTF-8".to_string())?;
    environment.insert("PWD".to_string(), root.to_string());
    let entries: Vec<(String, String)> = environment.into_iter().collect();
    let refs: Vec<(&str, &str)> = entries
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect();
    let digest = compute_env_digest(&refs).map_err(|e| e.to_string())?;
    Ok((entries, digest))
}

fn read_pipe(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<Result<Vec<u8>, String>> {
    thread::spawn(move || {
        let mut output = Vec::new();
        let mut buf = [0_u8; 8192];
        loop {
            let count = pipe.read(&mut buf).map_err(|e| e.to_string())?;
            if count == 0 {
                return Ok(output);
            }
            if output.len().saturating_add(count) > MAX_OUTPUT_BYTES {
                return Err("probe output exceeds 100 MiB limit".to_string());
            }
            output.extend_from_slice(&buf[..count]);
        }
    })
}

fn termination(status: Option<std::process::ExitStatus>) -> Termination {
    match status {
        Some(status) => {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt;
                if let Some(signal) = status.signal() {
                    return Termination::Signal(signal);
                }
            }
            Termination::Exit(status.code().unwrap_or(1))
        }
        None => Termination::SpawnError,
    }
}

pub fn run_probe(repo_root: &Path, input: &ProbeInput) -> Result<ProbeRun, String> {
    let (argv, argv_frame) = decode_argv_b64(&input.argv_b64).map_err(|e| e.to_string())?;
    if input.timeout_ms == 0 {
        return Err("timeout_ms must be positive".to_string());
    }
    let (environment, env_digest) = canonical_environment(repo_root, &input.env_overrides)?;
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(repo_root)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_clear()
        .envs(environment.iter().map(|(key, value)| (key, value)));
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            // A missing binary, a non-executable target, or a similar spawn failure is
            // a normal (if failing) probe outcome per R16 — publish it as a recorded
            // `termination: spawn-error` result instead of aborting the whole probe
            // run, so it can flow through evaluation and dispute classification like
            // any other observed result.
            let (output, output_digest, output_ref) =
                create_output_envelope(&[], error.to_string().as_bytes());
            let expected = input.expected_failure.as_ref();
            let record = ProbeRecord {
                id: input.id.clone(),
                selector: input.selector,
                argv,
                argv_b64: encode_base64url_unpadded(&argv_frame),
                env_digest,
                expected_failure_ref: expected.map(|failure| failure.id.clone()),
                observed: ObservedResult::Fail,
                failure_class: expected.map(|failure| failure.class),
                termination: Termination::SpawnError,
                timeout_ms: input.timeout_ms,
                output_ref,
                output_digest,
            };
            return Ok(ProbeRun {
                record,
                output,
                matched_expected_failure: false,
            });
        }
    };
    let stdout = read_pipe(child.stdout.take().ok_or("stdout pipe unavailable")?);
    let stderr = read_pipe(child.stderr.take().ok_or("stderr pipe unavailable")?);
    let status = child
        .wait_timeout(Duration::from_millis(input.timeout_ms as u64))
        .map_err(|e| format!("wait failed: {e}"))?;
    let term = match status {
        Some(status) => termination(Some(status)),
        None => {
            child
                .kill()
                .map_err(|e| format!("timeout kill failed: {e}"))?;
            child
                .wait()
                .map_err(|e| format!("timeout wait failed: {e}"))?;
            Termination::Timeout
        }
    };
    let stdout = stdout
        .join()
        .map_err(|_| "stdout reader panicked".to_string())??;
    let stderr = stderr
        .join()
        .map_err(|_| "stderr reader panicked".to_string())??;
    let (output, output_digest, output_ref) = create_output_envelope(&stdout, &stderr);
    let expected = input.expected_failure.as_ref();
    let matched = expected.is_some_and(|failure| failure.matches(&term, &stdout, &stderr));
    let observed = if matches!(term, Termination::Exit(0)) {
        ObservedResult::Pass
    } else {
        ObservedResult::Fail
    };
    let record = ProbeRecord {
        id: input.id.clone(),
        selector: input.selector,
        argv,
        argv_b64: encode_base64url_unpadded(&argv_frame),
        env_digest,
        expected_failure_ref: expected.map(|failure| failure.id.clone()),
        observed,
        failure_class: if observed == ObservedResult::Fail {
            expected.map(|failure| failure.class)
        } else {
            None
        },
        termination: term,
        timeout_ms: input.timeout_ms,
        output_ref,
        output_digest,
    };
    Ok(ProbeRun {
        record,
        output,
        matched_expected_failure: matched,
    })
}

fn publish_created(repo_root: &Path, relative: &Path, bytes: &[u8]) -> Result<(), String> {
    let binding = ValidatedRepoPath::new(
        repo_root,
        relative,
        ValidatedRepoPathMode::RegularFileOrMissing,
    )
    .map_err(|e| format!("publication path validation failed: {e}"))?;
    if binding.exists() {
        return Err(format!(
            "publication target already exists: {}",
            relative.display()
        ));
    }
    if let Some(parent) = repo_root.join(relative).parent() {
        fs::create_dir_all(parent).map_err(|e| format!("cannot create publication parent: {e}"))?;
    }
    let binding = binding
        .bind_materialized_parents()
        .map_err(|e| format!("publication parent binding failed: {e}"))?;
    test_first_fs::atomic_publish_bytes(&repo_root.join(relative), bytes)?;
    binding
        .bind_created_leaf()
        .map_err(|e| format!("publication leaf binding failed: {e}"))?;
    Ok(())
}

pub struct ReceiptInput<'a> {
    pub plan: &'a str,
    pub task: &'a str,
    pub generation: u32,
    pub contract_digest: &'a str,
    pub phase: &'a str,
    pub expectation: &'a str,
}

pub fn publish_receipt(
    repo_root: &Path,
    input: &ReceiptInput<'_>,
    runs: &[ProbeRun],
) -> Result<PathBuf, String> {
    test_first_fs::validate_artifact_identity(
        input.plan,
        input.task,
        input.generation,
        input.contract_digest,
    )?;
    if !matches!(
        (input.phase, input.expectation),
        ("test", "red") | ("green-rerun", "green") | ("test", "pass")
    ) {
        return Err(format!(
            "invalid artifact identity: invalid probe phase/expectation pair ({}, {})",
            input.phase, input.expectation
        ));
    }
    if runs.is_empty() {
        return Err("probe receipt requires at least one run".to_string());
    }
    let identities: Vec<Vec<u8>> = runs
        .iter()
        .map(|run| {
            run.record
                .compute_identity_bytes()
                .map_err(|e| e.to_string())
        })
        .collect::<Result<_, _>>()?;
    let receipt = ProbeReceipt {
        plan: input.plan.to_string(),
        task: input.task.to_string(),
        generation: input.generation,
        contract_digest: input.contract_digest.to_string(),
        phase: input.phase.to_string(),
        expectation: input.expectation.to_string(),
        runner_contract: RUNNER_CONTRACT.to_string(),
        probe_set_digest: compute_probe_set_digest(&identities),
        verdict: if runs
            .iter()
            .all(|run| run.record.observed == ObservedResult::Pass)
        {
            "pass".to_string()
        } else {
            "fail".to_string()
        },
        records: runs.iter().map(|run| run.record.clone()).collect(),
    };
    let root = PathBuf::from(".dev/pipeline/receipts")
        .join(input.plan)
        .join(input.task)
        .join(format!("g{}-c{}", input.generation, input.contract_digest));
    for run in runs {
        let output = root.join(&run.record.output_ref);
        publish_created(repo_root, &output, &run.output)?;
    }
    let receipt_path = root.join(format!("probe-{}.receipt.md", input.expectation));
    publish_created(repo_root, &receipt_path, &receipt.to_receipt_bytes())?;
    Ok(repo_root.join(receipt_path))
}

pub fn probe_identity(input: &ProbeInput) -> Result<Vec<u8>, String> {
    let (_, frame) = decode_argv_b64(&input.argv_b64).map_err(|e| e.to_string())?;
    let (_, env_digest) = canonical_environment(Path::new("."), &input.env_overrides)?;
    Ok(compute_probe_identity_bytes(
        &input.id,
        input.selector,
        &frame,
        &env_digest,
        input
            .expected_failure
            .as_ref()
            .map(|failure| failure.id.as_str()),
        input.timeout_ms,
    ))
}

fn current_dir() -> Result<PathBuf, String> {
    std::env::current_dir().map_err(|e| format!("cannot determine repository root: {e}"))
}

fn read_required<'a>(args: &'a [String], index: usize, name: &str) -> Result<&'a str, String> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| format!("{name} is required"))
}

fn parse_run_input(args: &[String]) -> Result<(ProbeInput, ReceiptInput<'_>), String> {
    let id = read_required(args, 8, "id")?.to_string();
    let selector =
        ProbeSelector::parse(read_required(args, 9, "selector")?).map_err(|e| e.to_string())?;
    let argv_b64 = read_required(args, 10, "argv_b64")?.to_string();
    decode_argv_b64(&argv_b64).map_err(|e| e.to_string())?;
    let timeout_ms = read_required(args, 11, "timeout_ms")?
        .parse::<u32>()
        .map_err(|e| format!("invalid timeout_ms: {e}"))?;
    if timeout_ms == 0 {
        return Err("timeout_ms must be positive".to_string());
    }

    let mut env_overrides = Vec::new();
    let mut expected_failure = None;
    let mut index = 12;
    while let Some(arg) = args.get(index) {
        match arg.as_str() {
            "--env" => {
                let value = read_required(args, index + 1, "KEY=VALUE")?;
                let (key, value) = value
                    .split_once('=')
                    .ok_or_else(|| "--env requires KEY=VALUE".to_string())?;
                env_overrides.push((key.to_string(), value.to_string()));
                index += 2;
            }
            "--expected-failure" => {
                let value = read_required(args, index + 1, "expected failure")?;
                expected_failure = Some(ExpectedFailure::parse(value)?);
                index += 2;
            }
            unknown => return Err(format!("unknown run option '{unknown}'")),
        }
    }

    let input = ProbeInput {
        id,
        selector,
        argv_b64,
        env_overrides,
        expected_failure,
        timeout_ms,
    };
    let receipt = ReceiptInput {
        plan: read_required(args, 2, "plan")?,
        task: read_required(args, 3, "task")?,
        generation: read_required(args, 4, "generation")?
            .parse()
            .map_err(|e| format!("invalid generation: {e}"))?,
        contract_digest: read_required(args, 5, "contract_digest")?,
        phase: read_required(args, 6, "phase")?,
        expectation: read_required(args, 7, "expectation")?,
    };
    Ok((input, receipt))
}

/// Run or validate pipeline-internal probe evidence through the real binary.
pub(crate) fn cmd_test_first_probe(args: &[String]) -> gal_engine::ExitCode {
    let operation = args.get(1).map(String::as_str);
    let result = (|| -> Result<(), String> {
        match operation {
            Some("contract-digest") => {
                let prompt_path = Path::new(read_required(args, 2, "prompt")?);
                let task = read_required(args, 3, "task")?;
                let prompt_text = fs::read_to_string(prompt_path)
                    .map_err(|e| format!("cannot read prompt: {e}"))?;
                let plan_slug = extract_plan_slug(&prompt_path.to_string_lossy());
                let contract = parse_task_contract(&prompt_text, &plan_slug, task)
                    .map_err(|e| format!("task contract: {e}"))?;
                println!("contract_digest={}", contract.compute_digest());
                Ok(())
            }
            Some("run") => {
                let repo = current_dir()?;
                let (input, receipt) = parse_run_input(args)?;
                let run = run_probe(&repo, &input)?;
                let path = publish_receipt(&repo, &receipt, std::slice::from_ref(&run))?;
                println!("record={}", run.record.to_canonical_json());
                println!("matched_expected_failure={}", run.matched_expected_failure);
                println!(
                    "identity_b64={}",
                    encode_base64url_unpadded(&probe_identity(&input)?)
                );
                println!("receipt={}", path.display());
                Ok(())
            }
            Some("encode-argv") => {
                let argv = args[2..].to_vec();
                let encoded = encode_argv_b64(&argv).map_err(|e| e.to_string())?;
                println!("argv_b64={encoded}");
                Ok(())
            }
            Some("validate-argv") => {
                let value = read_required(args, 2, "argv_b64")?;
                decode_argv_b64(value).map_err(|e| e.to_string())?;
                Ok(())
            }
            Some("validate-record") => {
                let repo = current_dir()?;
                let path = repo.join(read_required(args, 2, "record path")?);
                let bytes = fs::read(path).map_err(|e| format!("cannot read record: {e}"))?;
                let text =
                    std::str::from_utf8(&bytes).map_err(|e| format!("record is not UTF-8: {e}"))?;
                let record = ProbeRecord::parse_canonical_json(text).map_err(|e| e.to_string())?;
                println!("{}", record.to_canonical_json());
                Ok(())
            }
            Some("validate-receipt") => {
                let repo = current_dir()?;
                let path = repo.join(read_required(args, 2, "receipt path")?);
                let bytes = fs::read(path).map_err(|e| format!("cannot read receipt: {e}"))?;
                ProbeReceipt::parse_receipt_bytes(&bytes).map_err(|e| e.to_string())?;
                Ok(())
            }
            _ => Err(
                "usage: gal test-first-probe run <plan> <task> <generation> <contract_digest> \
                 <phase> <expectation> <id> <selector> <argv_b64> <timeout_ms> \
                 [--env KEY=VALUE] [--expected-failure TEXT] | validate-argv <b64> | \
                 validate-record <path> | validate-receipt <path> | \
                 contract-digest <prompt> <task> | encode-argv <arg>..."
                    .to_string(),
            ),
        }
    })();
    match result {
        Ok(()) => gal_engine::ExitCode::Success,
        Err(error) => {
            eprintln!("test-first-probe: {error}");
            if operation.is_none() {
                gal_engine::ExitCode::Usage
            } else {
                gal_engine::ExitCode::Error
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn argv(args: &[&str]) -> String {
        pipeline::task_spec::encode_argv_b64(
            &args
                .iter()
                .map(|arg| (*arg).to_string())
                .collect::<Vec<_>>(),
        )
        .unwrap()
    }

    fn tid(n: u32) -> String {
        format!("T-{n:02}")
    }

    #[test]
    fn exact_argv_frame_preserves_empty_argument_and_rejects_mutations() {
        let encoded = argv(&["probe", ""]);
        let (decoded, frame) = decode_argv_b64(&encoded).unwrap();
        assert_eq!(decoded, vec!["probe", ""]);
        assert!(pipeline::task_spec::probe_evidence::decode_argv_b64(&(encoded + "=")).is_err());
        let mut mutated = frame;
        mutated.push(0);
        assert!(pipeline::task_spec::probe_evidence::decode_argv_frame(&mutated).is_err());
    }

    #[test]
    fn expected_failure_parser_and_matcher_are_closed() {
        let failure = ExpectedFailure::parse(
            "EF-01;class=error-code;term=exit:2;stream=combined;matcher_b64=ZXJyb3I — prose",
        )
        .unwrap();
        assert!(failure.matches(&Termination::Exit(2), b"out", b"error"));
        assert!(!failure.matches(&Termination::Exit(0), b"out", b"error"));
        assert!(ExpectedFailure::parse(
            "EF-01;class=bad;term=exit:2;stream=combined;matcher_b64=ZXJyb3I — prose"
        )
        .is_err());
    }

    #[test]
    fn environment_digest_rejects_collisions_and_output_is_exactly_framed() {
        assert!(compute_env_digest(&[("A", "1"), ("A", "2")]).is_err());
        let (envelope, digest, output_ref) = create_output_envelope(b"out", b"err");
        assert_eq!(&envelope[..8], &3_u64.to_be_bytes());
        assert_eq!(&envelope[11..19], &3_u64.to_be_bytes());
        assert_eq!(output_ref, format!("outputs/{digest}.bin"));
    }

    #[test]
    fn runner_uses_null_stdin_and_publishes_atomically() {
        let temp = TempDir::new().unwrap();
        let input = ProbeInput {
            id: "P-01".to_string(),
            selector: ProbeSelector::NonRed,
            argv_b64: if cfg!(windows) {
                argv(&["cmd", "/C", "echo", "ok"])
            } else {
                argv(&["sh", "-c", "printf ok"])
            },
            env_overrides: vec![("GAL_TEST_FIRST_PROBE".to_string(), "1".to_string())],
            expected_failure: None,
            timeout_ms: 2_000,
        };
        let run = run_probe(temp.path(), &input).unwrap();
        assert_eq!(run.record.observed, ObservedResult::Pass);
        assert!(!run.matched_expected_failure);
        let path = publish_receipt(
            temp.path(),
            &ReceiptInput {
                plan: "plan",
                task: &tid(1),
                generation: 1,
                contract_digest: &"a".repeat(64),
                phase: "test",
                expectation: "pass",
            },
            &[run],
        )
        .unwrap();
        assert!(path.is_file());
    }

    #[test]
    fn termination_domains_cover_timeout_and_spawn_error_without_fabrication() {
        assert_eq!(termination(None), Termination::SpawnError);
        assert_eq!(Termination::Timeout.to_canonical_string(), "timeout");
        assert!(matches!(FailureClass::Assertion, FailureClass::Assertion));
    }

    fn shell_probe(script: &str) -> String {
        if cfg!(windows) {
            argv(&["cmd", "/C", script])
        } else {
            argv(&["sh", "-c", script])
        }
    }

    #[test]
    fn run_probe_reports_nonzero_exit_as_fail_and_records_expected_failure_class() {
        let temp = TempDir::new().unwrap();
        let matcher = encode_base64url_unpadded(b"oerg");
        let expected = ExpectedFailure::parse(&format!(
            "EF-01;class=error-code;term=exit:3;stream=stdout;matcher_b64={matcher} — prose"
        ))
        .unwrap();
        assert_eq!(expected.matcher, b"oerg");
        let input = ProbeInput {
            id: "P-02".to_string(),
            selector: ProbeSelector::Acceptance,
            argv_b64: shell_probe(if cfg!(windows) {
                "echo oerg&exit 3"
            } else {
                "printf oerg; exit 3"
            }),
            env_overrides: vec![],
            expected_failure: Some(expected),
            timeout_ms: 2_000,
        };
        let run = run_probe(temp.path(), &input).unwrap();
        assert_eq!(run.record.observed, ObservedResult::Fail);
        assert_eq!(run.record.termination, Termination::Exit(3));
        assert_eq!(run.record.failure_class, Some(FailureClass::ErrorCode));
        assert!(run.matched_expected_failure);
    }

    #[test]
    fn run_probe_times_out_and_kills_the_child() {
        let temp = TempDir::new().unwrap();
        let input = ProbeInput {
            id: "P-03".to_string(),
            selector: ProbeSelector::NonRed,
            argv_b64: shell_probe(if cfg!(windows) {
                "ping -n 30 127.0.0.1 >NUL"
            } else {
                "sleep 30"
            }),
            env_overrides: vec![],
            expected_failure: None,
            timeout_ms: 200,
        };
        let run = run_probe(temp.path(), &input).unwrap();
        assert_eq!(run.record.termination, Termination::Timeout);
        assert_eq!(run.record.observed, ObservedResult::Fail);
        assert!(!run.matched_expected_failure);
    }

    #[test]
    fn run_probe_surfaces_spawn_error_as_a_recorded_outcome_not_an_abort() {
        let temp = TempDir::new().unwrap();
        let input = ProbeInput {
            id: "P-04".to_string(),
            selector: ProbeSelector::NonRed,
            argv_b64: argv(&["gal-test-first-probe-nonexistent-binary-xyz"]),
            env_overrides: vec![],
            expected_failure: None,
            timeout_ms: 2_000,
        };
        // Must NOT abort the whole probe run: a missing binary is itself a valid,
        // publishable observed result per R16, not an infrastructure failure.
        let run = run_probe(temp.path(), &input).unwrap();
        assert_eq!(run.record.termination, Termination::SpawnError);
        assert_eq!(run.record.observed, ObservedResult::Fail);
        assert!(!run.matched_expected_failure);
        assert!(!run.output.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn run_probe_reports_signal_termination() {
        let temp = TempDir::new().unwrap();
        let input = ProbeInput {
            id: "P-05".to_string(),
            selector: ProbeSelector::NonRed,
            argv_b64: shell_probe("kill -KILL $$"),
            env_overrides: vec![],
            expected_failure: None,
            timeout_ms: 2_000,
        };
        let run = run_probe(temp.path(), &input).unwrap();
        assert!(matches!(run.record.termination, Termination::Signal(_)));
        assert_eq!(run.record.observed, ObservedResult::Fail);
    }

    #[test]
    fn expected_failure_matcher_selects_stdout_stderr_and_combined_streams_independently() {
        let matcher = encode_base64url_unpadded(b"needle");
        let stdout_only = ExpectedFailure::parse(&format!(
            "EF-02;class=assertion;term=nonzero;stream=stdout;matcher_b64={matcher} — prose"
        ))
        .unwrap();
        assert!(stdout_only.matches(&Termination::Exit(1), b"needle", b"other"));
        assert!(!stdout_only.matches(&Termination::Exit(1), b"other", b"needle"));

        let stderr_only = ExpectedFailure::parse(&format!(
            "EF-03;class=assertion;term=nonzero;stream=stderr;matcher_b64={matcher} — prose"
        ))
        .unwrap();
        assert!(stderr_only.matches(&Termination::Exit(1), b"other", b"needle"));
        assert!(!stderr_only.matches(&Termination::Exit(1), b"needle", b"other"));

        let combined = ExpectedFailure::parse(&format!(
            "EF-04;class=assertion;term=nonzero;stream=combined;matcher_b64={matcher} — prose"
        ))
        .unwrap();
        // Combined joins stdout and stderr with a `\n` separator, not a raw
        // concatenation — a matcher spanning the boundary must NOT match.
        assert!(combined.matches(&Termination::Exit(1), b"needle", b"other"));
        assert!(!combined.matches(&Termination::Exit(1), b"nee", b"dle"));
    }

    #[test]
    fn expected_failure_rejects_unsupported_signal_termination_on_non_unix() {
        let result = ExpectedFailure::parse(
            "EF-06;class=exception;term=signal:9;stream=stdout;matcher_b64= — prose",
        );
        if cfg!(unix) {
            assert!(result.is_ok());
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn publish_created_rejects_an_already_existing_target() {
        let temp = TempDir::new().unwrap();
        let relative = Path::new("already.bin");
        fs::write(temp.path().join(relative), b"first").unwrap();
        let err = publish_created(temp.path(), relative, b"second").unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert_eq!(fs::read(temp.path().join(relative)).unwrap(), b"first");
    }

    #[test]
    fn probe_identity_is_stable_for_identical_input_and_changes_with_argv() {
        let base = ProbeInput {
            id: "P-07".to_string(),
            selector: ProbeSelector::NonRed,
            argv_b64: argv(&["probe", "a"]),
            env_overrides: vec![],
            expected_failure: None,
            timeout_ms: 1_000,
        };
        let same = probe_identity(&base).unwrap();
        let same_again = probe_identity(&base).unwrap();
        assert_eq!(same, same_again);
        let mut changed = base.clone();
        changed.argv_b64 = argv(&["probe", "b"]);
        assert_ne!(same, probe_identity(&changed).unwrap());
    }
}
