//! Integration & specification tests for probe evidence schema, R4 ABI, codecs, golden vectors, and mutation matrix.

use pipeline::task_spec::probe_evidence::*;
use pipeline::task_spec::{decode_base64url_unpadded, encode_base64url_unpadded, lp};

fn tid(n: u32) -> String {
    format!("T-{:03}", n)
}

fn sample_argv() -> Vec<String> {
    vec![
        "cargo".to_string(),
        "test".to_string(),
        "-p".to_string(),
        "pipeline".to_string(),
    ]
}

fn sample_env() -> Vec<(&'static str, &'static str)> {
    vec![("CARGO_PKG_NAME", "pipeline"), ("RUST_BACKTRACE", "1")]
}

fn sample_output() -> (Vec<u8>, String, String) {
    create_output_envelope(b"test output pass\n", b"no errors\n")
}

fn sample_record_red() -> ProbeRecord {
    let argv = sample_argv();
    let argv_b64 = encode_argv_b64(&argv).unwrap();
    let env_digest = compute_env_digest(&sample_env()).unwrap();
    let (_, output_digest, output_ref) = sample_output();

    ProbeRecord {
        id: "P-01".to_string(),
        selector: ProbeSelector::Acceptance,
        argv,
        argv_b64,
        env_digest,
        expected_failure_ref: Some("EF-01".to_string()),
        observed: ObservedResult::Fail,
        failure_class: Some(FailureClass::Assertion),
        termination: Termination::Exit(1),
        timeout_ms: 5000,
        output_ref,
        output_digest,
    }
}

fn sample_record_green() -> ProbeRecord {
    let mut rec = sample_record_red();
    rec.observed = ObservedResult::Pass;
    rec.failure_class = None;
    rec.termination = Termination::Exit(0);
    rec
}

fn sample_record_non_red_pass() -> ProbeRecord {
    let argv = sample_argv();
    let argv_b64 = encode_argv_b64(&argv).unwrap();
    let env_digest = compute_env_digest(&sample_env()).unwrap();
    let (_, output_digest, output_ref) = sample_output();

    ProbeRecord {
        id: "P-01".to_string(),
        selector: ProbeSelector::NonRed,
        argv,
        argv_b64,
        env_digest,
        expected_failure_ref: None,
        observed: ObservedResult::Pass,
        failure_class: None,
        termination: Termination::Exit(0),
        timeout_ms: 5000,
        output_ref,
        output_digest,
    }
}

fn sample_receipt_red() -> ProbeReceipt {
    let rec = sample_record_red();
    let id_bytes = rec.compute_identity_bytes().unwrap();
    let psd = compute_probe_set_digest(&[id_bytes]);

    ProbeReceipt {
        plan: "feat-test-first-pipeline".to_string(),
        task: tid(15),
        generation: 1,
        contract_digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_string(),
        phase: "test".to_string(),
        expectation: "red".to_string(),
        runner_contract: RUNNER_CONTRACT.to_string(),
        probe_set_digest: psd,
        verdict: "fail".to_string(),
        records: vec![rec],
    }
}

fn sample_receipt_green() -> ProbeReceipt {
    let rec = sample_record_green();
    let id_bytes = rec.compute_identity_bytes().unwrap();
    let psd = compute_probe_set_digest(&[id_bytes]);

    ProbeReceipt {
        plan: "feat-test-first-pipeline".to_string(),
        task: tid(15),
        generation: 1,
        contract_digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_string(),
        phase: "green-rerun".to_string(),
        expectation: "green".to_string(),
        runner_contract: RUNNER_CONTRACT.to_string(),
        probe_set_digest: psd,
        verdict: "pass".to_string(),
        records: vec![rec],
    }
}

fn sample_receipt_non_red_pass() -> ProbeReceipt {
    let rec = sample_record_non_red_pass();
    let id_bytes = rec.compute_identity_bytes().unwrap();
    let psd = compute_probe_set_digest(&[id_bytes]);

    ProbeReceipt {
        plan: "feat-test-first-pipeline".to_string(),
        task: tid(15),
        generation: 1,
        contract_digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_string(),
        phase: "test".to_string(),
        expectation: "pass".to_string(),
        runner_contract: RUNNER_CONTRACT.to_string(),
        probe_set_digest: psd,
        verdict: "pass".to_string(),
        records: vec![rec],
    }
}

#[test]
fn golden_vector_bytes_and_digests() {
    // Argv framing golden
    let argv = vec!["arg1".to_string(), "arg2".to_string()];
    let argv_frame = encode_argv_frame(&argv).unwrap();
    let argv_b64 = encode_argv_b64(&argv).unwrap();
    assert_eq!(argv_frame.len(), 8 + 18 + 8 + 8 + 4 + 8 + 4);
    assert!(!argv_b64.contains('='));

    // Env digest golden
    let env_digest = compute_env_digest(&[("K1", "V1"), ("K2", "V2")]).unwrap();
    assert_eq!(env_digest.len(), 64);
    assert!(env_digest
        .chars()
        .all(|c| matches!(c, '0'..='9' | 'a'..='f')));

    // Output envelope golden
    let (env_bytes, out_digest, out_ref) = create_output_envelope(b"out", b"err");
    assert_eq!(env_bytes.len(), 8 + 3 + 8 + 3);
    assert_eq!(out_digest.len(), 64);
    assert_eq!(out_ref, format!("outputs/{out_digest}.bin"));
    verify_output_envelope(&env_bytes, &out_digest).unwrap();

    // Probe set digest golden
    let rec = sample_record_red();
    let id_bytes = rec.compute_identity_bytes().unwrap();
    let psd = compute_probe_set_digest(&[id_bytes]);
    assert_eq!(psd.len(), 64);

    // Record JSON canonical golden
    let json = rec.to_canonical_json();
    assert!(json.starts_with("{\"id\":\"P-01\","));
    assert!(json.contains(",\"selector\":\"acceptance\","));
    let parsed_rec = ProbeRecord::parse_canonical_json(&json).unwrap();
    assert_eq!(parsed_rec, rec);

    // Receipt golden bytes
    let receipt = sample_receipt_red();
    let receipt_bytes = receipt.to_receipt_bytes();
    assert!(receipt_bytes.ends_with(b"\n"));
    let parsed_receipt = ProbeReceipt::parse_receipt_bytes(&receipt_bytes).unwrap();
    assert_eq!(parsed_receipt, receipt);
}

#[test]
fn roundtrip_valid_receipts() {
    // 1. (test, red)
    let r_red = sample_receipt_red();
    let bytes_red = r_red.to_receipt_bytes();
    let parsed_red = ProbeReceipt::parse_receipt_bytes(&bytes_red).unwrap();
    assert_eq!(parsed_red, r_red);

    // 2. (green-rerun, green)
    let r_green = sample_receipt_green();
    let bytes_green = r_green.to_receipt_bytes();
    let parsed_green = ProbeReceipt::parse_receipt_bytes(&bytes_green).unwrap();
    assert_eq!(parsed_green, r_green);

    // 3. (test, pass)
    let r_pass = sample_receipt_non_red_pass();
    let bytes_pass = r_pass.to_receipt_bytes();
    let parsed_pass = ProbeReceipt::parse_receipt_bytes(&bytes_pass).unwrap();
    assert_eq!(parsed_pass, r_pass);
}

#[test]
fn rejects_malformed_headers() {
    let valid_bytes = sample_receipt_red().to_receipt_bytes();
    let valid_str = String::from_utf8(valid_bytes).unwrap();

    // Out of order header
    let bad_order = valid_str.replace(
        &format!("plan=feat-test-first-pipeline\ntask={}\n", tid(15)),
        &format!("task={}\nplan=feat-test-first-pipeline\n", tid(15)),
    );
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(bad_order.as_bytes()),
        Err(ProbeEvidenceError::InvalidHeader(_))
    ));

    // Missing header
    let missing_header = valid_str.replace(
        "contract_digest=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\n",
        "",
    );
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(missing_header.as_bytes()),
        Err(ProbeEvidenceError::InvalidHeader(_))
    ));

    // Invalid phase/expectation pair
    let bad_pair = valid_str.replace(
        "phase=test\nexpectation=red\n",
        "phase=test\nexpectation=green\n",
    );
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(bad_pair.as_bytes()),
        Err(ProbeEvidenceError::InvalidHeader(_))
    ));

    // Invalid verdict
    let bad_verdict = valid_str.replace("verdict=fail\n", "verdict=pass\n");
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(bad_verdict.as_bytes()),
        Err(ProbeEvidenceError::InvalidHeader(_))
    ));

    // CRLF line endings
    let crlf = valid_str.replace('\n', "\r\n");
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(crlf.as_bytes()),
        Err(ProbeEvidenceError::CanonicalityViolation(_))
    ));

    // Missing blank line
    let no_blank = valid_str.replace(
        "verdict=fail\n\nprobe_record_b64=",
        "verdict=fail\nprobe_record_b64=",
    );
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(no_blank.as_bytes()),
        Err(ProbeEvidenceError::InvalidHeader(_))
    ));

    // Trailing content after record
    let trailing = format!("{valid_str}extra_line\n");
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(trailing.as_bytes()),
        Err(ProbeEvidenceError::InvalidHeader(_) | ProbeEvidenceError::CanonicalityViolation(_))
    ));
}

#[test]
fn rejects_malformed_record_json() {
    let rec = sample_record_red();
    let json = rec.to_canonical_json();

    // Key out of order
    let bad_order = json.replace(
        "\"id\":\"P-01\",\"selector\":\"acceptance\"",
        "\"selector\":\"acceptance\",\"id\":\"P-01\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&bad_order),
        Err(ProbeEvidenceError::CanonicalityViolation(_))
    ));

    // Unknown field
    let unknown_field = json.replace("{\"id\":\"P-01\",", "{\"id\":\"P-01\",\"extra\":123,");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&unknown_field),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Wrong type for integer
    let wrong_type = json.replace("\"timeout_ms\":5000,", "\"timeout_ms\":\"5000\",");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&wrong_type),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Non-null expected_failure_ref for non-red selector
    let rec_non_red = sample_record_non_red_pass();
    let json_non_red = rec_non_red.to_canonical_json();
    let invalid_ef = json_non_red.replace(
        "\"expected_failure_ref\":null",
        "\"expected_failure_ref\":\"EF-01\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&invalid_ef),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Null expected_failure_ref for acceptance selector
    let invalid_ef_null = json.replace(
        "\"expected_failure_ref\":\"EF-01\"",
        "\"expected_failure_ref\":null",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&invalid_ef_null),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Non-null failure_class when observed is pass
    let rec_green = sample_record_green();
    let json_green = rec_green.to_canonical_json();
    let invalid_fc =
        json_green.replace("\"failure_class\":null", "\"failure_class\":\"assertion\"");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&invalid_fc),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Null failure_class when observed is fail
    let invalid_fc_null = json.replace("\"failure_class\":\"assertion\"", "\"failure_class\":null");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&invalid_fc_null),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Invalid termination leading zero
    let invalid_term = json.replace("\"termination\":\"exit:1\"", "\"termination\":\"exit:01\"");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&invalid_term),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));
}

#[test]
fn rejects_mutations() {
    // Base64url padding mutation (adding =)
    let rec = sample_record_red();
    let json = rec.to_canonical_json();
    let b64_padded = format!("{}=\n", encode_base64url_unpadded(json.as_bytes()));
    assert!(decode_base64url_unpadded(b64_padded.trim()).is_err());

    // Base64url alphabet mutation (using + instead of -)
    let bad_b64 = "abc+def_ghi";
    assert!(decode_base64url_unpadded(bad_b64).is_err());

    // Argv frame count mutation
    let mut bad_frame = encode_argv_frame(&["a".to_string(), "b".to_string()]).unwrap();
    // mutate argc from 2 to 3
    bad_frame[26] = 3;
    assert!(matches!(
        decode_argv_frame(&bad_frame),
        Err(ProbeEvidenceError::InvalidArgv(_))
    ));

    // Output envelope trailing bytes mutation
    let (mut env_bytes, digest, _) = sample_output();
    env_bytes.push(0xff);
    assert!(matches!(
        verify_output_envelope(&env_bytes, &digest),
        Err(ProbeEvidenceError::InvalidOutput(_))
    ));
}

#[test]
fn rejects_duplicates_unsorted_and_not_run() {
    let rec1 = sample_record_red();
    let mut rec2 = rec1.clone();
    rec2.id = "P-02".to_string();

    let id1 = rec1.compute_identity_bytes().unwrap();
    let id2 = rec2.compute_identity_bytes().unwrap();
    let psd = compute_probe_set_digest(&[id1, id2]);

    // Unsorted records (line for P-02 before line for P-01)
    let json1 = rec1.to_canonical_json();
    let b64_1 = encode_base64url_unpadded(json1.as_bytes());
    let json2 = rec2.to_canonical_json();
    let b64_2 = encode_base64url_unpadded(json2.as_bytes());

    let task = tid(15);
    let header_prefix = format!(
        "plan=feat-test-first-pipeline\ntask={task}\ngeneration=1\ncontract_digest=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef\nphase=test\nexpectation=red\nrunner_contract=test-first-probe-v1\nprobe_set_digest={psd}\nverdict=fail\n\n"
    );

    let unsorted_bytes =
        format!("{header_prefix}probe_record_b64={b64_2}\nprobe_record_b64={b64_1}\n");
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(unsorted_bytes.as_bytes()),
        Err(ProbeEvidenceError::UnsortedRecords(_))
    ));

    // Duplicate record (P-01, P-01)
    let dup_bytes = format!("{header_prefix}probe_record_b64={b64_1}\nprobe_record_b64={b64_1}\n");
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(dup_bytes.as_bytes()),
        Err(ProbeEvidenceError::DuplicateRecord(_))
    ));

    // NotRun observed result -> unconditionally invalid evidence regardless of header verdict
    let mut rec_nr = rec1.clone();
    rec_nr.observed = ObservedResult::NotRun;
    rec_nr.failure_class = None;
    let nr_id = rec_nr.compute_identity_bytes().unwrap();
    let nr_psd = compute_probe_set_digest(&[nr_id]);

    let nr_receipt_pass = ProbeReceipt {
        plan: "feat-test-first-pipeline".to_string(),
        task: tid(15),
        generation: 1,
        contract_digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_string(),
        phase: "test".to_string(),
        expectation: "red".to_string(),
        runner_contract: RUNNER_CONTRACT.to_string(),
        probe_set_digest: nr_psd.clone(),
        verdict: "pass".to_string(),
        records: vec![rec_nr.clone()],
    };
    let nr_bytes_pass = nr_receipt_pass.to_receipt_bytes();
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(&nr_bytes_pass),
        Err(ProbeEvidenceError::NotRunRecord(_))
    ));

    let nr_receipt_fail = ProbeReceipt {
        plan: "feat-test-first-pipeline".to_string(),
        task: tid(15),
        generation: 1,
        contract_digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"
            .to_string(),
        phase: "test".to_string(),
        expectation: "red".to_string(),
        runner_contract: RUNNER_CONTRACT.to_string(),
        probe_set_digest: nr_psd,
        verdict: "fail".to_string(),
        records: vec![rec_nr],
    };
    let nr_bytes_fail = nr_receipt_fail.to_receipt_bytes();
    assert!(matches!(
        ProbeReceipt::parse_receipt_bytes(&nr_bytes_fail),
        Err(ProbeEvidenceError::NotRunRecord(_))
    ));
}

#[test]
fn same_generation_identity_validation_and_mismatch() {
    let red = sample_receipt_red();
    let green = sample_receipt_green();

    // Valid same-generation match
    validate_same_generation_identity(&red, &green).unwrap();

    // Plan mismatch (plan isolation)
    let mut red_plan = red.clone();
    red_plan.plan = "other-plan".to_string();
    assert!(matches!(
        validate_same_generation_identity(&red_plan, &green),
        Err(ProbeEvidenceError::IdentityMismatch(_))
    ));

    // Task mismatch
    let mut red_task = red.clone();
    red_task.task = tid(16);
    assert!(matches!(
        validate_same_generation_identity(&red_task, &green),
        Err(ProbeEvidenceError::IdentityMismatch(_))
    ));

    // Generation mismatch
    let mut red_gen = red.clone();
    red_gen.generation = 2;
    assert!(matches!(
        validate_same_generation_identity(&red_gen, &green),
        Err(ProbeEvidenceError::IdentityMismatch(_))
    ));

    // Contract digest mismatch
    let mut red_cd = red.clone();
    red_cd.contract_digest =
        "aaaa456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    assert!(matches!(
        validate_same_generation_identity(&red_cd, &green),
        Err(ProbeEvidenceError::IdentityMismatch(_))
    ));
}

#[test]
fn mutation_matrix_names_first_rejected_invariant() {
    let red_bytes = sample_receipt_red().to_receipt_bytes();
    let red_str = String::from_utf8(red_bytes).unwrap();
    let task = tid(15);

    let matrix = vec![
        (
            "header_order",
            red_str.replace(
                &format!("plan=feat-test-first-pipeline\ntask={task}"),
                &format!("task={task}\nplan=feat-test-first-pipeline"),
            ),
            "invalid header: expected header line starting with 'plan='",
        ),
        (
            "invalid_phase",
            red_str.replace(
                "phase=test\nexpectation=red",
                "phase=test\nexpectation=green",
            ),
            "invalid header: invalid phase/expectation pair",
        ),
        (
            "invalid_generation",
            red_str.replace("generation=1", "generation=0"),
            "invalid header: generation must be a positive u32 integer",
        ),
        (
            "line_ending_crlf",
            red_str.replace('\n', "\r\n"),
            "canonicality violation: receipt contains CRLF",
        ),
        (
            "missing_blank_line",
            red_str.replace(
                "verdict=fail\n\nprobe_record_b64=",
                "verdict=fail\nprobe_record_b64=",
            ),
            "invalid header: receipt must have at least 9 headers",
        ),
    ];

    for (name, mutated, expected_fragment) in matrix {
        let err = ProbeReceipt::parse_receipt_bytes(mutated.as_bytes()).unwrap_err();
        let err_msg = err.to_string();
        assert!(
            err_msg.contains(expected_fragment),
            "mutation vector '{name}' failed: expected error containing '{expected_fragment}', got '{err_msg}'"
        );
    }
}

#[test]
fn class_termination_matrix_validation_and_mutations() {
    let rec = sample_record_red();
    let json = rec.to_canonical_json();

    // Assertion with exit:0 -> invalid
    let bad_assertion_exit0 = json.replace(
        "\"failure_class\":\"assertion\",\"termination\":\"exit:1\"",
        "\"failure_class\":\"assertion\",\"termination\":\"exit:0\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&bad_assertion_exit0),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Diagnostic with exit:0 -> invalid
    let bad_diag_exit0 = json.replace(
        "\"failure_class\":\"assertion\",\"termination\":\"exit:1\"",
        "\"failure_class\":\"diagnostic\",\"termination\":\"exit:0\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&bad_diag_exit0),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // ErrorCode with signal:9 -> invalid
    let bad_error_code_signal = json.replace(
        "\"failure_class\":\"assertion\",\"termination\":\"exit:1\"",
        "\"failure_class\":\"error-code\",\"termination\":\"signal:9\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&bad_error_code_signal),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Status with timeout -> invalid
    let bad_status_timeout = json.replace(
        "\"failure_class\":\"assertion\",\"termination\":\"exit:1\"",
        "\"failure_class\":\"status\",\"termination\":\"timeout\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&bad_status_timeout),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Exception with exit:0 -> invalid
    let bad_exception_exit0 = json.replace(
        "\"failure_class\":\"assertion\",\"termination\":\"exit:1\"",
        "\"failure_class\":\"exception\",\"termination\":\"exit:0\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&bad_exception_exit0),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // Pass with exit:1 -> invalid
    let rec_green = sample_record_green();
    let json_green = rec_green.to_canonical_json();
    let bad_pass_exit1 =
        json_green.replace("\"termination\":\"exit:0\"", "\"termination\":\"exit:1\"");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&bad_pass_exit1),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));
}

#[test]
fn argv_framing_bounds_and_overflow_mutations() {
    // Argc larger than remaining bytes
    let mut frame = Vec::new();
    frame.extend_from_slice(&lp(b"test-first-argv-v1"));
    frame.extend_from_slice(&100_000u64.to_be_bytes()); // argc = 100,000 but no arg data
    assert!(matches!(
        decode_argv_frame(&frame),
        Err(ProbeEvidenceError::InvalidArgv(msg)) if msg.contains("cannot be satisfied by remaining")
    ));

    // Arg len exceeding remaining frame bytes (frame length check)
    let mut frame_incomplete_arg = Vec::new();
    frame_incomplete_arg.extend_from_slice(&lp(b"test-first-argv-v1"));
    frame_incomplete_arg.extend_from_slice(&1u64.to_be_bytes()); // argc = 1
    frame_incomplete_arg.extend_from_slice(&100u64.to_be_bytes()); // arg_len = 100 but no arg data
    assert!(matches!(
        decode_argv_frame(&frame_incomplete_arg),
        Err(ProbeEvidenceError::InvalidArgv(msg)) if msg.contains("arg bytes incomplete")
    ));
}

#[test]
fn strict_2digit_domain_validation_mutations() {
    let rec = sample_record_red();
    let json = rec.to_canonical_json();

    // 1-digit probe id P-1 -> rejected
    let p_1_digit = json.replace("\"id\":\"P-01\"", "\"id\":\"P-1\"");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&p_1_digit),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // 3-digit probe id P-100 -> rejected
    let p_3_digit = json.replace("\"id\":\"P-01\"", "\"id\":\"P-100\"");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&p_3_digit),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // 0-valued probe id P-00 -> rejected
    let p_zero = json.replace("\"id\":\"P-01\"", "\"id\":\"P-00\"");
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&p_zero),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // 1-digit EF ref EF-1 -> rejected
    let ef_1_digit = json.replace(
        "\"expected_failure_ref\":\"EF-01\"",
        "\"expected_failure_ref\":\"EF-1\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&ef_1_digit),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // 3-digit EF ref EF-100 -> rejected
    let ef_3_digit = json.replace(
        "\"expected_failure_ref\":\"EF-01\"",
        "\"expected_failure_ref\":\"EF-100\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&ef_3_digit),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));

    // 0-valued EF ref EF-00 -> rejected
    let ef_zero = json.replace(
        "\"expected_failure_ref\":\"EF-01\"",
        "\"expected_failure_ref\":\"EF-00\"",
    );
    assert!(matches!(
        ProbeRecord::parse_canonical_json(&ef_zero),
        Err(ProbeEvidenceError::InvalidJson(_))
    ));
}

#[test]
fn checked_arithmetic_and_frame_length_mutations() {
    // Incomplete tag_len (frame length check)
    let mut frame_incomplete_tag = Vec::new();
    frame_incomplete_tag.extend_from_slice(&100u64.to_be_bytes());
    assert!(matches!(
        decode_argv_frame(&frame_incomplete_tag),
        Err(ProbeEvidenceError::InvalidArgv(msg)) if msg.contains("argv tag incomplete")
    ));

    // Near-usize::MAX tag_len (arithmetic overflow protection)
    let mut frame_overflow_tag = Vec::new();
    frame_overflow_tag.extend_from_slice(&(u64::MAX - 4).to_be_bytes());
    assert!(matches!(
        decode_argv_frame(&frame_overflow_tag),
        Err(ProbeEvidenceError::InvalidArgv(_))
    ));

    // Truncated stdout_len in output envelope (envelope length check)
    let mut env_truncated_stdout = Vec::new();
    env_truncated_stdout.extend_from_slice(&100u64.to_be_bytes());
    assert!(matches!(
        verify_output_envelope(
            &env_truncated_stdout,
            "0000000000000000000000000000000000000000000000000000000000000000"
        ),
        Err(ProbeEvidenceError::InvalidOutput(msg)) if msg.contains("output envelope stdout truncated")
    ));

    // Near-usize::MAX stdout_len (arithmetic overflow protection)
    let mut env_overflow_stdout = Vec::new();
    env_overflow_stdout.extend_from_slice(&(u64::MAX - 4).to_be_bytes());
    assert!(matches!(
        verify_output_envelope(
            &env_overflow_stdout,
            "0000000000000000000000000000000000000000000000000000000000000000"
        ),
        Err(ProbeEvidenceError::InvalidOutput(_))
    ));

    // Near-usize::MAX stderr_len (arithmetic overflow protection)
    let mut env_overflow_stderr = Vec::new();
    env_overflow_stderr.extend_from_slice(&0u64.to_be_bytes()); // stdout_len = 0
    env_overflow_stderr.extend_from_slice(&(u64::MAX - 4).to_be_bytes()); // stderr_len near max
    assert!(matches!(
        verify_output_envelope(
            &env_overflow_stderr,
            "0000000000000000000000000000000000000000000000000000000000000000"
        ),
        Err(ProbeEvidenceError::InvalidOutput(_))
    ));
}

#[test]
fn windows_environment_key_case_collision_rejection() {
    let result = compute_env_digest(&[("PATH", "trusted"), ("Path", "attacker")]);
    if cfg!(windows) {
        assert!(matches!(
            result,
            Err(ProbeEvidenceError::InvalidEnv(msg)) if msg.contains("case-fold env key collision")
        ));
    } else {
        assert!(result.is_ok());
    }
}

#[test]
fn class_termination_forbidden_domains_are_rejected() {
    // R16: `assertion` and `diagnostic` require `nonzero`, `exception` permits
    // `nonzero|signal:N`, and `spawn-error`/`timeout` never prove red or pass.
    let json = sample_record_red().to_canonical_json();
    let anchor = "\"failure_class\":\"assertion\",\"termination\":\"exit:1\"";

    // assertion and diagnostic admit neither signal:N nor timeout nor spawn-error.
    for class in ["assertion", "diagnostic"] {
        for term in ["signal:9", "signal:0", "timeout", "spawn-error"] {
            let mutated = json.replace(
                anchor,
                &format!("\"failure_class\":\"{class}\",\"termination\":\"{term}\""),
            );
            assert!(
                matches!(
                    ProbeRecord::parse_canonical_json(&mutated),
                    Err(ProbeEvidenceError::InvalidJson(_))
                ),
                "class {class} with termination {term} must be rejected"
            );
        }
    }

    // exception admits nonzero exit and signal:N, but not timeout or spawn-error.
    for term in ["timeout", "spawn-error"] {
        let mutated = json.replace(
            anchor,
            &format!("\"failure_class\":\"exception\",\"termination\":\"{term}\""),
        );
        assert!(
            matches!(
                ProbeRecord::parse_canonical_json(&mutated),
                Err(ProbeEvidenceError::InvalidJson(_))
            ),
            "class exception with termination {term} must be rejected"
        );
    }
    for term in ["signal:9", "signal:0", "exit:2"] {
        let mutated = json.replace(
            anchor,
            &format!("\"failure_class\":\"exception\",\"termination\":\"{term}\""),
        );
        assert!(
            ProbeRecord::parse_canonical_json(&mutated).is_ok(),
            "class exception with termination {term} must be accepted per R16"
        );
    }

    // error-code and status require exit:N, which R16 keeps distinct from
    // `nonzero`, so exit:0 stays admissible and signals do not.
    for class in ["error-code", "status"] {
        let ok = json.replace(
            anchor,
            &format!("\"failure_class\":\"{class}\",\"termination\":\"exit:0\""),
        );
        assert!(
            ProbeRecord::parse_canonical_json(&ok).is_ok(),
            "class {class} with exit:0 must be accepted per R16 exit:N token"
        );
        let bad = json.replace(
            anchor,
            &format!("\"failure_class\":\"{class}\",\"termination\":\"signal:9\""),
        );
        assert!(
            matches!(
                ProbeRecord::parse_canonical_json(&bad),
                Err(ProbeEvidenceError::InvalidJson(_))
            ),
            "class {class} with signal:9 must be rejected"
        );
    }
}

#[test]
fn probe_set_digest_is_order_invariant() {
    let mut rec_a = sample_record_red();
    rec_a.id = "P-01".to_string();
    let mut rec_b = sample_record_red();
    rec_b.id = "P-02".to_string();

    let id_a = rec_a.compute_identity_bytes().unwrap();
    let id_b = rec_b.compute_identity_bytes().unwrap();

    let sorted = compute_probe_set_digest(&[id_a.clone(), id_b.clone()]);
    let reversed = compute_probe_set_digest(&[id_b, id_a]);
    assert_eq!(
        sorted, reversed,
        "R4 defines probe_set_digest after bytewise id sorting, so caller order must not matter"
    );
}
