//! Real-binary coverage for the deterministic probe runner and evidence ABI.

use pipeline::task_spec::probe_evidence::ProbeRecord;
use pipeline::task_spec::{
    decode_base64url_unpadded, encode_argv_b64, encode_base64url_unpadded, parse_task_contract,
    TaskContractError,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn run(repo: &Path, args: &[String]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gal"))
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

fn argv(args: &[&str]) -> String {
    encode_argv_b64(
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

fn shell_probe(script: &str) -> String {
    if cfg!(windows) {
        argv(&["cmd", "/C", script])
    } else {
        argv(&["sh", "-c", script])
    }
}

fn run_probe(repo: &Path, id: &str, argv_b64: &str, timeout: u32) -> std::process::Output {
    run(
        repo,
        &[
            "test-first-probe".into(),
            "run".into(),
            "probe-plan".into(),
            tid(26),
            "1".into(),
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
            "test".into(),
            "pass".into(),
            id.into(),
            "non-red".into(),
            argv_b64.into(),
            timeout.to_string(),
            "--env".into(),
            "GAL_TEST_FIRST_PROBE=visible".into(),
        ],
    )
}

fn receipt_path(repo: &Path) -> PathBuf {
    repo.join(format!(
        ".dev/pipeline/receipts/probe-plan/{}/g1-c{}/probe-pass.receipt.md",
        tid(26),
        "a".repeat(64)
    ))
}

fn record_json(repo: &Path) -> String {
    let bytes = fs::read_to_string(receipt_path(repo)).unwrap();
    let line = bytes
        .lines()
        .find(|line| line.starts_with("probe_record_b64="))
        .unwrap();
    let encoded = line.strip_prefix("probe_record_b64=").unwrap();
    String::from_utf8(decode_base64url_unpadded(encoded).unwrap()).unwrap()
}

fn validate(repo: &Path, operation: &str, path: &Path) -> std::process::Output {
    run(
        repo,
        &[
            "test-first-probe".into(),
            operation.into(),
            path.strip_prefix(repo)
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        ],
    )
}

fn replace_bytes(bytes: &[u8], from: &[u8], to: &[u8]) -> Vec<u8> {
    let start = bytes
        .windows(from.len())
        .position(|window| window == from)
        .unwrap();
    let mut result = bytes[..start].to_vec();
    result.extend_from_slice(to);
    result.extend_from_slice(&bytes[start + from.len()..]);
    result
}

#[test]
fn real_binary_runs_probe_and_publishes_canonical_record_and_output() {
    let temp = TempDir::new().unwrap();
    let probe_argv = shell_probe(if cfg!(windows) {
        "echo visible&echo error 1>&2"
    } else {
        "printf visible; printf error >&2"
    });
    let output = run_probe(temp.path(), "P-01", &probe_argv, 2_000);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let record = record_json(temp.path());
    assert!(record.starts_with("{\"id\":\"P-01\",\"selector\":\"non-red\",\"argv_b64\":"));
    assert!(record.contains("\"observed\":\"pass\""));
    assert!(record.contains("\"output_ref\":\"outputs/"));
    let parsed = ProbeRecord::parse_canonical_json(&record).unwrap();
    let identity_line = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.starts_with("identity_b64="))
        .unwrap()
        .strip_prefix("identity_b64=")
        .unwrap()
        .to_string();
    assert_eq!(
        decode_base64url_unpadded(&identity_line).unwrap(),
        parsed.compute_identity_bytes().unwrap()
    );

    let receipt = receipt_path(temp.path());
    let receipt_check = validate(temp.path(), "validate-receipt", &receipt);
    assert!(
        receipt_check.status.success(),
        "{}",
        String::from_utf8_lossy(&receipt_check.stderr)
    );
    let output_digest = record
        .split("\"output_ref\":\"outputs/")
        .nth(1)
        .unwrap()
        .split(".bin\"")
        .next()
        .unwrap();
    let envelope = fs::read(temp.path().join(format!(
        ".dev/pipeline/receipts/probe-plan/{}/g1-c{}/outputs/{}.bin",
        tid(26),
        "a".repeat(64),
        output_digest
    )))
    .unwrap();
    let stdout_len = u64::from_be_bytes(envelope[..8].try_into().unwrap()) as usize;
    let stderr_offset = 8 + stdout_len;
    let stderr_len = u64::from_be_bytes(
        envelope[stderr_offset..stderr_offset + 8]
            .try_into()
            .unwrap(),
    ) as usize;
    let stdout = &envelope[8..stderr_offset];
    let stderr = &envelope[stderr_offset + 8..stderr_offset + 8 + stderr_len];
    assert!(stdout == b"visible" || stdout == b"visible\n" || stdout == b"visible\r\n");
    assert!(stderr.starts_with(b"error"), "stderr={stderr:?}");

    let duplicate = run_probe(temp.path(), "P-01", &probe_argv, 2_000);
    assert!(!duplicate.status.success());
}

#[test]
fn real_binary_rejects_noncanonical_argv_count_length_base64_and_trailing_bytes() {
    let temp = TempDir::new().unwrap();
    let canonical = decode_base64url_unpadded(&argv(&["probe", "arg"])).unwrap();

    let mut count = canonical.clone();
    let count_offset = 8 + b"test-first-argv-v1".len();
    count[count_offset..count_offset + 8].copy_from_slice(&0_u64.to_be_bytes());
    let count_result = run(
        temp.path(),
        &[
            "test-first-probe".into(),
            "validate-argv".into(),
            encode_base64url_unpadded(&count),
        ],
    );
    assert!(!count_result.status.success());

    let mut length = canonical.clone();
    let length_offset = count_offset + 8;
    length[length_offset..length_offset + 8].copy_from_slice(&999_u64.to_be_bytes());
    let length_result = run(
        temp.path(),
        &[
            "test-first-probe".into(),
            "validate-argv".into(),
            encode_base64url_unpadded(&length),
        ],
    );
    assert!(!length_result.status.success());

    let mut trailing = canonical;
    trailing.push(0);
    let trailing_result = run(
        temp.path(),
        &[
            "test-first-probe".into(),
            "validate-argv".into(),
            encode_base64url_unpadded(&trailing),
        ],
    );
    assert!(!trailing_result.status.success());

    let padded = run(
        temp.path(),
        &[
            "test-first-probe".into(),
            "validate-argv".into(),
            format!("{}=", argv(&["probe"])),
        ],
    );
    assert!(!padded.status.success());
}

#[test]
fn real_binary_rejects_noncanonical_record_json_and_receipt_enum_or_trailing_bytes() {
    let temp = TempDir::new().unwrap();
    let output = run_probe(temp.path(), "P-02", &shell_probe("exit 0"), 2_000);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut record = record_json(temp.path());
    record = record.replace("\"selector\":\"non-red\"", "\"selector\":\"bogus\"");
    let record_path = temp.path().join("record.json");
    fs::write(&record_path, record).unwrap();
    let record_check = validate(temp.path(), "validate-record", &record_path);
    assert!(!record_check.status.success());

    let reordered = record_json(temp.path()).replace(
        "{\"id\":\"P-02\",\"selector\":\"non-red\"",
        "{\"selector\":\"non-red\",\"id\":\"P-02\"",
    );
    fs::write(&record_path, reordered).unwrap();
    let order_check = validate(temp.path(), "validate-record", &record_path);
    assert!(!order_check.status.success());

    let receipt = receipt_path(temp.path());
    let original = fs::read(&receipt).unwrap();
    let mut enum_mutation = original.clone();
    enum_mutation = replace_bytes(&enum_mutation, b"expectation=pass", b"expectation=bogus");
    fs::write(&receipt, enum_mutation).unwrap();
    let enum_check = validate(temp.path(), "validate-receipt", &receipt);
    assert!(!enum_check.status.success());

    let mut trailing = original;
    trailing.extend_from_slice(b"x");
    fs::write(&receipt, trailing).unwrap();
    let trailing_check = validate(temp.path(), "validate-receipt", &receipt);
    assert!(!trailing_check.status.success());
}

#[test]
fn real_binary_records_timeout_and_spawn_error_without_aborting() {
    let temp = TempDir::new().unwrap();
    let timeout = run_probe(
        temp.path(),
        "P-03",
        &shell_probe(if cfg!(windows) {
            "ping -n 3 127.0.0.1 >NUL"
        } else {
            "sleep 30"
        }),
        20,
    );
    assert!(
        timeout.status.success(),
        "{}",
        String::from_utf8_lossy(&timeout.stderr)
    );

    let missing_temp = TempDir::new().unwrap();
    let missing = run_probe(
        missing_temp.path(),
        "P-04",
        &argv(&["gal-test-first-missing-binary"]),
        2_000,
    );
    assert!(
        missing.status.success(),
        "{}",
        String::from_utf8_lossy(&missing.stderr)
    );
    let record = record_json(missing_temp.path());
    assert!(record.contains("\"termination\":\"spawn-error\""));
}

// `gal test-first-probe contract-digest <prompt> <task>` — a read-only
// operation that parses a task's Test-First contract with the real
// `pipeline::task_spec::parse_task_contract` and prints its
// `TaskContract::compute_digest()` value. The fixture below carries three
// task blocks: a `required` task with a complete contract, a
// `not-applicable` task, and a `required` task missing its mandatory `Seam`
// field (unparseable). EF-01's red evidence — nonzero exit with the usage
// string `usage: gal test-first-probe run` on the combined stream — is
// carried by every probe below at today's baseline, since the operation
// does not exist yet; no separate probe restates it, because that
// restatement inverts (goes red for the wrong reason) the moment the
// operation is added.
fn contract_digest_fixture(temp: &TempDir) -> (PathBuf, String, String, String, String) {
    let slug = "contract-digest-fixture";
    let plans_dir = temp.path().join(".dev").join("plans");
    fs::create_dir_all(&plans_dir).unwrap();
    let t_required = tid(41);
    let t_not_applicable = tid(42);
    let t_unparseable = tid(43);
    let prompt_text = format!(
        "# Plan Prompt: Contract Digest Fixture\n\n\
         Pipeline Contract: test-first-v1\n\n\
         ## Tasks\n\n\
         - [ ] {t_required} — Implement feature with a locked test-first contract.\n  \
         - Test-first: required\n  \
         - Seam: `crates/cli/src/lib.rs`\n  \
         - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`\n  \
         - Production Paths: `crates/cli/src/lib.rs`\n  \
         - Test Paths: `crates/cli/tests/test_feat.rs`\n  \
         - Scaffold: required\n\n\
         - [ ] {t_not_applicable} — Document feature changes.\n  \
         - Test-first: not-applicable — Documentation only change with no executable behavior\n  \
         - Non-red probe: `cargo test -p gal-cli`\n\n\
         - [ ] {t_unparseable} — Task missing the required Seam field.\n  \
         - Test-first: required\n  \
         - Expected failures: `EF-01;class=assertion;term=nonzero;stream=stderr;matcher_b64=ZmFpbGVk — assertion failed`\n  \
         - Production Paths: `crates/cli/src/lib.rs`\n  \
         - Test Paths: `crates/cli/tests/test_feat.rs`\n  \
         - Scaffold: required\n\n\
         ## Test Plan\n\n\
         | ID | desc |\n\
         | --- | --- |\n\
         | {t_required} | test feature |\n\
         | {t_not_applicable} | test doc |\n"
    );
    let prompt_path = plans_dir.join(format!("{slug}.prompt.md"));
    fs::write(&prompt_path, &prompt_text).unwrap();
    (
        prompt_path,
        slug.to_string(),
        t_required,
        t_not_applicable,
        t_unparseable,
    )
}

fn run_contract_digest(repo: &Path, prompt_path: &Path, task: &str) -> std::process::Output {
    run(
        repo,
        &[
            "test-first-probe".into(),
            "contract-digest".into(),
            prompt_path.to_string_lossy().into_owned(),
            task.into(),
        ],
    )
}

fn contract_digest_line(output: &std::process::Output) -> Option<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.starts_with("contract_digest="))
        .map(|line| line.strip_prefix("contract_digest=").unwrap().to_string())
}

/// Recursively snapshots every regular file under `root` as
/// `(relative path, sha256 hex)`, sorted, so a before/after comparison
/// catches any filesystem side effect anywhere in the tree.
fn snapshot_tree(root: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, out: &mut Vec<(String, String)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                walk(&path, root, out);
            } else {
                let bytes = fs::read(&path).unwrap();
                let digest = format!("{:x}", Sha256::digest(&bytes));
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.push((relative, digest));
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

#[test]
fn contract_digest_prints_expected_digest_for_required_task() {
    let temp = TempDir::new().unwrap();
    let (prompt_path, slug, t_required, _t_not_applicable, _t_unparseable) =
        contract_digest_fixture(&temp);
    let prompt_text = fs::read_to_string(&prompt_path).unwrap();
    let expected_digest = parse_task_contract(&prompt_text, &slug, &t_required)
        .unwrap()
        .compute_digest();

    let output = run_contract_digest(temp.path(), &prompt_path, &t_required);
    assert!(
        output.status.success(),
        "expected red today: operation does not exist yet; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = contract_digest_line(&output).expect("contract_digest= line");
    assert_eq!(printed, expected_digest);
    assert_eq!(printed.len(), 64);
    assert!(printed
        .chars()
        .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
}

#[test]
fn contract_digest_prints_expected_digest_for_not_applicable_task() {
    let temp = TempDir::new().unwrap();
    let (prompt_path, slug, _t_required, t_not_applicable, _t_unparseable) =
        contract_digest_fixture(&temp);
    let prompt_text = fs::read_to_string(&prompt_path).unwrap();
    let expected_digest = parse_task_contract(&prompt_text, &slug, &t_not_applicable)
        .unwrap()
        .compute_digest();

    let output = run_contract_digest(temp.path(), &prompt_path, &t_not_applicable);
    assert!(
        output.status.success(),
        "expected red today: operation does not exist yet; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = contract_digest_line(&output).expect("contract_digest= line");
    assert_eq!(printed, expected_digest);
}

#[test]
fn contract_digest_fails_closed_on_unknown_task() {
    let temp = TempDir::new().unwrap();
    let (prompt_path, slug, _t_required, _t_not_applicable, _t_unparseable) =
        contract_digest_fixture(&temp);
    let prompt_text = fs::read_to_string(&prompt_path).unwrap();
    let unknown_task = tid(99);
    assert_eq!(
        parse_task_contract(&prompt_text, &slug, &unknown_task),
        Err(TaskContractError::TaskNotFound(unknown_task.clone()))
    );

    let output = run_contract_digest(temp.path(), &prompt_path, &unknown_task);
    assert!(
        !output.status.success(),
        "an unknown task id must fail closed, not panic or succeed"
    );
    assert!(contract_digest_line(&output).is_none());
}

#[test]
fn contract_digest_fails_closed_on_unparseable_contract() {
    let temp = TempDir::new().unwrap();
    let (prompt_path, slug, _t_required, _t_not_applicable, t_unparseable) =
        contract_digest_fixture(&temp);
    let prompt_text = fs::read_to_string(&prompt_path).unwrap();
    assert!(matches!(
        parse_task_contract(&prompt_text, &slug, &t_unparseable),
        Err(TaskContractError::MissingField(_, "Seam"))
    ));

    let output = run_contract_digest(temp.path(), &prompt_path, &t_unparseable);
    assert!(
        !output.status.success(),
        "an unparseable contract must fail closed, not panic or succeed"
    );
    assert!(contract_digest_line(&output).is_none());
}

#[test]
fn contract_digest_writes_nothing() {
    let temp = TempDir::new().unwrap();
    let (prompt_path, _slug, t_required, t_not_applicable, _t_unparseable) =
        contract_digest_fixture(&temp);

    let before = snapshot_tree(temp.path());
    run_contract_digest(temp.path(), &prompt_path, &t_required);
    run_contract_digest(temp.path(), &prompt_path, &t_not_applicable);
    let after = snapshot_tree(temp.path());

    assert_eq!(
        before, after,
        "contract-digest must not write anything, unlike `run`"
    );
}

#[allow(clippy::too_many_arguments)]
fn run_probe_custom(
    repo: &Path,
    plan_slug: &str,
    task: &str,
    generation: &str,
    contract_digest: &str,
    phase: &str,
    expectation: &str,
    id: &str,
    argv_b64: &str,
) -> std::process::Output {
    run(
        repo,
        &[
            "test-first-probe".into(),
            "run".into(),
            plan_slug.into(),
            task.into(),
            generation.into(),
            contract_digest.into(),
            phase.into(),
            expectation.into(),
            id.into(),
            "non-red".into(),
            argv_b64.into(),
            "2000".into(),
        ],
    )
}

#[test]
fn probe_producer_constrains_identity_grammar_and_phase_expectation_pairs() {
    let temp = TempDir::new().unwrap();
    let probe_argv = shell_probe("exit 0");
    let valid_slug = "probe-plan";
    let valid_task = tid(26);
    let valid_gen = "1";
    let valid_digest = "a".repeat(64);
    let valid_phase = "test";
    let valid_exp = "pass";

    let invalid_cases = [
        (
            "../unsafe-slug",
            valid_task.as_str(),
            valid_gen,
            valid_digest.as_str(),
            valid_phase,
            valid_exp,
            "unsafe plan slug",
        ),
        (
            valid_slug,
            "task-26",
            valid_gen,
            valid_digest.as_str(),
            valid_phase,
            valid_exp,
            "noncanonical task",
        ),
        (
            valid_slug,
            valid_task.as_str(),
            "0",
            valid_digest.as_str(),
            valid_phase,
            valid_exp,
            "zero generation",
        ),
        (
            valid_slug,
            valid_task.as_str(),
            valid_gen,
            "shortdigest",
            valid_phase,
            valid_exp,
            "malformed digest length",
        ),
        (
            valid_slug,
            valid_task.as_str(),
            valid_gen,
            &"A".repeat(64),
            valid_phase,
            valid_exp,
            "malformed digest uppercase",
        ),
        (
            valid_slug,
            valid_task.as_str(),
            valid_gen,
            valid_digest.as_str(),
            "test",
            "green",
            "invalid phase/expectation pair (test, green)",
        ),
        (
            valid_slug,
            valid_task.as_str(),
            valid_gen,
            valid_digest.as_str(),
            "green-rerun",
            "red",
            "invalid phase/expectation pair (green-rerun, red)",
        ),
        (
            valid_slug,
            valid_task.as_str(),
            valid_gen,
            valid_digest.as_str(),
            "post-test",
            "pass",
            "invalid probe phase",
        ),
    ];

    for (slug, task, gen, digest, phase, exp, desc) in invalid_cases {
        let before_tree = snapshot_tree(temp.path());
        let output = run_probe_custom(
            temp.path(),
            slug,
            task,
            gen,
            digest,
            phase,
            exp,
            "P-01",
            &probe_argv,
        );
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let combined = format!("{stdout}\n{stderr}");

        assert!(
            !output.status.success(),
            "invalid identity ({desc}) must fail execution; output={combined}"
        );
        assert!(
            combined.contains("invalid artifact identity"),
            "invalid identity ({desc}) stderr/stdout must contain 'invalid artifact identity', got:\n{combined}"
        );

        let after_tree = snapshot_tree(temp.path());
        assert_eq!(
            before_tree, after_tree,
            "invalid identity ({desc}) must leave no staging directory, output blob, manifest, or receipt"
        );
    }
}

fn run_encode_argv(repo: &Path, args: &[&str]) -> std::process::Output {
    let mut cli_args = vec!["test-first-probe".to_string(), "encode-argv".to_string()];
    cli_args.extend(args.iter().map(|s| s.to_string()));
    run(repo, &cli_args)
}

fn encode_argv_line(output: &std::process::Output) -> Option<String> {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find(|line| line.starts_with("argv_b64="))
        .map(|line| line.strip_prefix("argv_b64=").unwrap().to_string())
}

#[test]
fn encode_argv_prints_canonical_encoding_for_arguments() {
    let temp = TempDir::new().unwrap();
    let args = ["cargo", "test", "-p", "gal-cli", "--", "some filter"];
    let string_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let expected_b64 = encode_argv_b64(&string_args).unwrap();

    let output = run_encode_argv(temp.path(), &args);
    assert!(
        output.status.success(),
        "expected red today: operation does not exist yet; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = encode_argv_line(&output).expect("argv_b64= line");
    assert_eq!(printed, expected_b64);

    let validate_result = run(
        temp.path(),
        &["test-first-probe".into(), "validate-argv".into(), printed],
    );
    assert!(validate_result.status.success());
}

#[test]
fn encode_argv_prints_canonical_encoding_for_single_argument() {
    let temp = TempDir::new().unwrap();
    let args = ["gal-test-binary"];
    let string_args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let expected_b64 = encode_argv_b64(&string_args).unwrap();

    let output = run_encode_argv(temp.path(), &args);
    assert!(
        output.status.success(),
        "expected red today: operation does not exist yet; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed = encode_argv_line(&output).expect("argv_b64= line");
    assert_eq!(printed, expected_b64);
}

#[test]
fn encode_argv_fails_on_empty_arguments() {
    let temp = TempDir::new().unwrap();
    let output = run_encode_argv(temp.path(), &[]);
    assert!(
        !output.status.success(),
        "encode-argv with no arguments must fail (argc must be positive)"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("argc must be positive"),
        "expected error message about argc on stderr, got: {stderr}"
    );
}

#[test]
fn encode_argv_writes_nothing() {
    let temp = TempDir::new().unwrap();
    let before = snapshot_tree(temp.path());
    let output = run_encode_argv(temp.path(), &["foo", "bar"]);
    assert!(
        output.status.success(),
        "expected red today: operation does not exist yet; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let after = snapshot_tree(temp.path());

    assert_eq!(
        before, after,
        "encode-argv must not write anything to the filesystem"
    );
}
