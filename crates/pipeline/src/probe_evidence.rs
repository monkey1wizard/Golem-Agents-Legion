//! probe_evidence: canonical R4 ABI probe evidence schema and codecs.

use thiserror::Error;

use super::{decode_base64url_unpadded, encode_base64url_unpadded, lp};

pub const RUNNER_CONTRACT: &str = "test-first-probe-v1";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProbeEvidenceError {
    #[error("invalid header: {0}")]
    InvalidHeader(String),
    #[error("invalid JSON record: {0}")]
    InvalidJson(String),
    #[error("canonicality violation: {0}")]
    CanonicalityViolation(String),
    #[error("identity mismatch: {0}")]
    IdentityMismatch(String),
    #[error("invalid argv: {0}")]
    InvalidArgv(String),
    #[error("invalid environment: {0}")]
    InvalidEnv(String),
    #[error("invalid output: {0}")]
    InvalidOutput(String),
    #[error("base64 decode error: {0}")]
    Base64Error(String),
    #[error("duplicate record ID: {0}")]
    DuplicateRecord(String),
    #[error("records not sorted by ID: {0}")]
    UnsortedRecords(String),
    #[error("record observed result is not-run: {0}")]
    NotRunRecord(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeSelector {
    Acceptance,
    NonRed,
}

impl ProbeSelector {
    pub fn as_str(&self) -> &'static str {
        match self {
            ProbeSelector::Acceptance => "acceptance",
            ProbeSelector::NonRed => "non-red",
        }
    }

    pub fn parse(s: &str) -> Result<Self, ProbeEvidenceError> {
        match s {
            "acceptance" => Ok(ProbeSelector::Acceptance),
            "non-red" => Ok(ProbeSelector::NonRed),
            _ => Err(ProbeEvidenceError::InvalidJson(format!(
                "invalid selector domain: '{s}' (expected 'acceptance' or 'non-red')"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservedResult {
    Pass,
    Fail,
    NotRun,
}

impl ObservedResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            ObservedResult::Pass => "pass",
            ObservedResult::Fail => "fail",
            ObservedResult::NotRun => "not-run",
        }
    }

    pub fn parse(s: &str) -> Result<Self, ProbeEvidenceError> {
        match s {
            "pass" => Ok(ObservedResult::Pass),
            "fail" => Ok(ObservedResult::Fail),
            "not-run" => Ok(ObservedResult::NotRun),
            _ => Err(ProbeEvidenceError::InvalidJson(format!(
                "invalid observed domain: '{s}' (expected 'pass', 'fail', or 'not-run')"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureClass {
    Assertion,
    ErrorCode,
    Status,
    Exception,
    Diagnostic,
}

impl FailureClass {
    pub fn as_str(&self) -> &'static str {
        match self {
            FailureClass::Assertion => "assertion",
            FailureClass::ErrorCode => "error-code",
            FailureClass::Status => "status",
            FailureClass::Exception => "exception",
            FailureClass::Diagnostic => "diagnostic",
        }
    }

    pub fn parse(s: &str) -> Result<Self, ProbeEvidenceError> {
        match s {
            "assertion" => Ok(FailureClass::Assertion),
            "error-code" => Ok(FailureClass::ErrorCode),
            "status" => Ok(FailureClass::Status),
            "exception" => Ok(FailureClass::Exception),
            "diagnostic" => Ok(FailureClass::Diagnostic),
            _ => Err(ProbeEvidenceError::InvalidJson(format!(
                "invalid failure_class domain: '{s}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Termination {
    Exit(i32),
    Signal(i32),
    Timeout,
    SpawnError,
}

impl Termination {
    pub fn to_canonical_string(&self) -> String {
        match self {
            Termination::Exit(code) => format!("exit:{code}"),
            Termination::Signal(sig) => format!("signal:{sig}"),
            Termination::Timeout => "timeout".to_string(),
            Termination::SpawnError => "spawn-error".to_string(),
        }
    }

    pub fn parse(s: &str) -> Result<Self, ProbeEvidenceError> {
        if s == "timeout" {
            return Ok(Termination::Timeout);
        }
        if s == "spawn-error" {
            return Ok(Termination::SpawnError);
        }
        if let Some(rest) = s.strip_prefix("exit:") {
            let code = parse_canonical_i32(rest).map_err(|e| {
                ProbeEvidenceError::InvalidJson(format!(
                    "invalid termination exit code '{rest}': {e}"
                ))
            })?;
            return Ok(Termination::Exit(code));
        }
        if let Some(rest) = s.strip_prefix("signal:") {
            let sig = parse_canonical_i32(rest).map_err(|e| {
                ProbeEvidenceError::InvalidJson(format!("invalid termination signal '{rest}': {e}"))
            })?;
            return Ok(Termination::Signal(sig));
        }
        Err(ProbeEvidenceError::InvalidJson(format!(
            "invalid termination grammar: '{s}'"
        )))
    }
}

fn parse_canonical_i32(s: &str) -> Result<i32, String> {
    if s.is_empty() {
        return Err("empty number string".to_string());
    }
    if s.len() > 1 && s.starts_with('0') {
        return Err("leading zero not allowed".to_string());
    }
    if s.len() > 2 && s.starts_with("-0") {
        return Err("leading zero not allowed".to_string());
    }
    s.parse::<i32>().map_err(|e| e.to_string())
}

// ── Framing functions ───────────────────────────────────────────────────────────

pub fn encode_argv_frame(argv: &[String]) -> Result<Vec<u8>, ProbeEvidenceError> {
    if argv.is_empty() {
        return Err(ProbeEvidenceError::InvalidArgv(
            "argc must be positive".to_string(),
        ));
    }
    for (i, arg) in argv.iter().enumerate() {
        if arg.contains('\0') {
            return Err(ProbeEvidenceError::InvalidArgv(format!(
                "argv[{i}] contains NUL byte"
            )));
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&lp(b"test-first-argv-v1"));
    out.extend_from_slice(&(argv.len() as u64).to_be_bytes());
    for arg in argv {
        out.extend_from_slice(&lp(arg.as_bytes()));
    }
    Ok(out)
}

pub fn decode_argv_frame(frame: &[u8]) -> Result<Vec<String>, ProbeEvidenceError> {
    let mut cursor: usize = 0;
    if frame.len() < 8 {
        return Err(ProbeEvidenceError::InvalidArgv(
            "argv frame too short".to_string(),
        ));
    }
    let tag_len_u64 = u64::from_be_bytes(frame[0..8].try_into().unwrap());
    let tag_len = usize::try_from(tag_len_u64).map_err(|_| {
        ProbeEvidenceError::InvalidArgv("tag_len exceeds usize capacity".to_string())
    })?;
    cursor += 8;
    let tag_end = cursor
        .checked_add(tag_len)
        .ok_or_else(|| ProbeEvidenceError::InvalidArgv("tag_len offset overflowed".to_string()))?;
    if frame.len() < tag_end {
        return Err(ProbeEvidenceError::InvalidArgv(
            "argv tag incomplete".to_string(),
        ));
    }
    let tag = &frame[cursor..tag_end];
    if tag != b"test-first-argv-v1" {
        return Err(ProbeEvidenceError::InvalidArgv(format!(
            "invalid argv tag: {:?}",
            String::from_utf8_lossy(tag)
        )));
    }
    cursor = tag_end;

    let argc_offset_end = cursor
        .checked_add(8)
        .ok_or_else(|| ProbeEvidenceError::InvalidArgv("argc offset overflowed".to_string()))?;
    if frame.len() < argc_offset_end {
        return Err(ProbeEvidenceError::InvalidArgv(
            "argv count incomplete".to_string(),
        ));
    }
    let argc_u64 = u64::from_be_bytes(frame[cursor..argc_offset_end].try_into().unwrap());
    let argc = usize::try_from(argc_u64)
        .map_err(|_| ProbeEvidenceError::InvalidArgv("argc exceeds usize capacity".to_string()))?;
    cursor = argc_offset_end;

    if argc == 0 {
        return Err(ProbeEvidenceError::InvalidArgv(
            "argc must be positive (> 0)".to_string(),
        ));
    }
    let remaining_bytes = frame.len() - cursor;
    if argc > remaining_bytes / 8 {
        return Err(ProbeEvidenceError::InvalidArgv(format!(
            "argc {argc} cannot be satisfied by remaining {remaining_bytes} bytes"
        )));
    }

    let mut argv = Vec::with_capacity(argc);
    for _ in 0..argc {
        let arg_len_offset_end = cursor.checked_add(8).ok_or_else(|| {
            ProbeEvidenceError::InvalidArgv("arg_len offset overflowed".to_string())
        })?;
        if frame.len() < arg_len_offset_end {
            return Err(ProbeEvidenceError::InvalidArgv(
                "arg len incomplete".to_string(),
            ));
        }
        let arg_len_u64 = u64::from_be_bytes(frame[cursor..arg_len_offset_end].try_into().unwrap());
        let arg_len = usize::try_from(arg_len_u64).map_err(|_| {
            ProbeEvidenceError::InvalidArgv("arg_len exceeds usize capacity".to_string())
        })?;
        cursor = arg_len_offset_end;

        let arg_end = cursor.checked_add(arg_len).ok_or_else(|| {
            ProbeEvidenceError::InvalidArgv("arg_len offset overflowed".to_string())
        })?;
        if frame.len() < arg_end {
            return Err(ProbeEvidenceError::InvalidArgv(
                "arg bytes incomplete".to_string(),
            ));
        }
        let arg_bytes = &frame[cursor..arg_end];
        cursor = arg_end;
        let s = std::str::from_utf8(arg_bytes)
            .map_err(|e| ProbeEvidenceError::InvalidArgv(format!("arg is not valid UTF-8: {e}")))?;
        if s.contains('\0') {
            return Err(ProbeEvidenceError::InvalidArgv(
                "arg contains NUL byte".to_string(),
            ));
        }
        argv.push(s.to_string());
    }
    if cursor != frame.len() {
        return Err(ProbeEvidenceError::InvalidArgv(format!(
            "trailing bytes after argv frame: {} unparsed bytes",
            frame.len() - cursor
        )));
    }
    Ok(argv)
}

pub fn encode_argv_b64(argv: &[String]) -> Result<String, ProbeEvidenceError> {
    let frame = encode_argv_frame(argv)?;
    Ok(encode_base64url_unpadded(&frame))
}

pub fn decode_argv_b64(b64: &str) -> Result<(Vec<String>, Vec<u8>), ProbeEvidenceError> {
    let frame = decode_base64url_unpadded(b64)
        .map_err(|e| ProbeEvidenceError::Base64Error(format!("invalid argv_b64 base64url: {e}")))?;
    let argv = decode_argv_frame(&frame)?;
    Ok((argv, frame))
}

pub fn compute_env_digest(entries: &[(&str, &str)]) -> Result<String, ProbeEvidenceError> {
    let mut sorted = entries.to_vec();
    sorted.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    for window in sorted.windows(2) {
        if window[0].0 == window[1].0 {
            return Err(ProbeEvidenceError::InvalidEnv(format!(
                "duplicate env key: {}",
                window[0].0
            )));
        }
    }
    if cfg!(windows) {
        // Ordinal ASCII fold choice: to_ascii_uppercase() ensures predictable byte-for-byte ASCII uppercase normalization without Unicode locale-dependent folding rules.
        let mut case_folded: Vec<(String, &str)> = entries
            .iter()
            .map(|(k, _)| (k.to_ascii_uppercase(), *k))
            .collect();
        case_folded.sort_by(|a, b| a.0.cmp(&b.0));
        for window in case_folded.windows(2) {
            if window[0].0 == window[1].0 {
                return Err(ProbeEvidenceError::InvalidEnv(format!(
                    "case-fold env key collision: '{}' and '{}'",
                    window[0].1, window[1].1
                )));
            }
        }
    }
    let mut out = Vec::new();
    out.extend_from_slice(&lp(b"test-first-env-v1"));
    out.extend_from_slice(&(sorted.len() as u64).to_be_bytes());
    for (k, v) in &sorted {
        if k.contains('\0') || v.contains('\0') {
            return Err(ProbeEvidenceError::InvalidEnv(
                "env key or value contains NUL byte".to_string(),
            ));
        }
        out.extend_from_slice(&lp(k.as_bytes()));
        out.extend_from_slice(&lp(v.as_bytes()));
    }
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&out);
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn create_output_envelope(stdout: &[u8], stderr: &[u8]) -> (Vec<u8>, String, String) {
    let cap = 8usize
        .saturating_add(stdout.len())
        .saturating_add(8)
        .saturating_add(stderr.len());
    let mut envelope = Vec::with_capacity(cap);
    envelope.extend_from_slice(&(stdout.len() as u64).to_be_bytes());
    envelope.extend_from_slice(stdout);
    envelope.extend_from_slice(&(stderr.len() as u64).to_be_bytes());
    envelope.extend_from_slice(stderr);

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&envelope);
    let digest = format!("{:x}", hasher.finalize());
    let output_ref = format!("outputs/{digest}.bin");
    (envelope, digest, output_ref)
}

pub fn verify_output_envelope(
    envelope: &[u8],
    expected_digest: &str,
) -> Result<(), ProbeEvidenceError> {
    if envelope.len() < 8 {
        return Err(ProbeEvidenceError::InvalidOutput(
            "output envelope too short".to_string(),
        ));
    }
    let stdout_len_u64 = u64::from_be_bytes(envelope[0..8].try_into().unwrap());
    let stdout_len = usize::try_from(stdout_len_u64).map_err(|_| {
        ProbeEvidenceError::InvalidOutput("stdout_len exceeds usize capacity".to_string())
    })?;
    let stderr_len_offset = 8usize.checked_add(stdout_len).ok_or_else(|| {
        ProbeEvidenceError::InvalidOutput("stdout_len offset overflowed".to_string())
    })?;
    let stderr_len_end = stderr_len_offset.checked_add(8).ok_or_else(|| {
        ProbeEvidenceError::InvalidOutput("stderr_len offset overflowed".to_string())
    })?;
    if envelope.len() < stderr_len_end {
        return Err(ProbeEvidenceError::InvalidOutput(
            "output envelope stdout truncated".to_string(),
        ));
    }
    let stderr_len_u64 = u64::from_be_bytes(
        envelope[stderr_len_offset..stderr_len_end]
            .try_into()
            .unwrap(),
    );
    let stderr_len = usize::try_from(stderr_len_u64).map_err(|_| {
        ProbeEvidenceError::InvalidOutput("stderr_len exceeds usize capacity".to_string())
    })?;
    let total_expected_len = stderr_len_end.checked_add(stderr_len).ok_or_else(|| {
        ProbeEvidenceError::InvalidOutput("output envelope total length overflowed".to_string())
    })?;
    if envelope.len() != total_expected_len {
        return Err(ProbeEvidenceError::InvalidOutput(
            "output envelope length mismatch or trailing bytes".to_string(),
        ));
    }

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(envelope);
    let computed = format!("{:x}", hasher.finalize());
    if computed != expected_digest {
        return Err(ProbeEvidenceError::InvalidOutput(format!(
            "output digest mismatch: computed '{computed}', expected '{expected_digest}'"
        )));
    }
    Ok(())
}

pub fn compute_probe_identity_bytes(
    id: &str,
    selector: ProbeSelector,
    argv_frame: &[u8],
    env_digest: &str,
    expected_failure_ref: Option<&str>,
    timeout_ms: u32,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&lp(id.as_bytes()));
    out.extend_from_slice(&lp(selector.as_str().as_bytes()));
    out.extend_from_slice(&lp(argv_frame));
    out.extend_from_slice(&lp(env_digest.as_bytes()));
    out.extend_from_slice(&lp(expected_failure_ref.unwrap_or("").as_bytes()));
    out.extend_from_slice(&(timeout_ms as u64).to_be_bytes());
    out
}

/// Cross-receipt comparison projection: the probe identity minus the `env_digest`
/// limb. `test-first-v1` deliberately assigns the red receipt to a dispatched TESTER
/// and the green rerun to the in-process ORCHESTRATOR, so the two receipts are
/// produced by different process trees and can never share an ambient environment.
/// Comparing full identities across them therefore always fails. `env_digest` stays
/// recorded in every record, and each receipt still self-verifies its own
/// `probe_set_digest` against the unchanged env-inclusive formula above.
pub fn compute_probe_comparison_bytes(
    id: &str,
    selector: ProbeSelector,
    argv_frame: &[u8],
    expected_failure_ref: Option<&str>,
    timeout_ms: u32,
) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&lp(id.as_bytes()));
    out.extend_from_slice(&lp(selector.as_str().as_bytes()));
    out.extend_from_slice(&lp(argv_frame));
    out.extend_from_slice(&lp(expected_failure_ref.unwrap_or("").as_bytes()));
    out.extend_from_slice(&(timeout_ms as u64).to_be_bytes());
    out
}

/// R4 defines this digest over the identity records "after bytewise `id` sorting",
/// so the sort belongs to the digest definition rather than to each caller. Every
/// identity begins with `LP(id)` and every validated id is exactly four bytes
/// (`P-NN`), so the `u64be` length prefix is identical across identities and a
/// bytewise sort of the whole identity blob orders by `id`. Sorting here makes the
/// helper order-invariant, so a writer that assembles identities in probe-declaration
/// order cannot silently produce a digest the verifier will not reproduce.
pub fn compute_probe_set_digest(identities: &[Vec<u8>]) -> String {
    let mut sorted: Vec<&Vec<u8>> = identities.iter().collect();
    sorted.sort_by(|a, b| a.as_slice().cmp(b.as_slice()));

    let mut out = Vec::new();
    out.extend_from_slice(&lp(b"test-first-probe-set-v1"));
    out.extend_from_slice(&(sorted.len() as u64).to_be_bytes());
    for id_bytes in sorted {
        out.extend_from_slice(&lp(id_bytes));
    }
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&out);
    format!("{:x}", hasher.finalize())
}

// ── JSON Escaping & Parsing ─────────────────────────────────────────────────────

pub fn escape_rfc8259(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\x08' => out.push_str("\\b"),
            '\x0C' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ── ProbeRecord ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeRecord {
    pub id: String,
    pub selector: ProbeSelector,
    pub argv: Vec<String>,
    pub argv_b64: String,
    pub env_digest: String,
    pub expected_failure_ref: Option<String>,
    pub observed: ObservedResult,
    pub failure_class: Option<FailureClass>,
    pub termination: Termination,
    pub timeout_ms: u32,
    pub output_ref: String,
    pub output_digest: String,
}

impl ProbeRecord {
    pub fn compute_identity_bytes(&self) -> Result<Vec<u8>, ProbeEvidenceError> {
        let (_, frame) = decode_argv_b64(&self.argv_b64)?;
        Ok(compute_probe_identity_bytes(
            &self.id,
            self.selector,
            &frame,
            &self.env_digest,
            self.expected_failure_ref.as_deref(),
            self.timeout_ms,
        ))
    }

    /// Env-excluded projection of this record's identity, for comparison across
    /// receipts authored by different actors. See `compute_probe_comparison_bytes`.
    pub fn compute_comparison_bytes(&self) -> Result<Vec<u8>, ProbeEvidenceError> {
        let (_, frame) = decode_argv_b64(&self.argv_b64)?;
        Ok(compute_probe_comparison_bytes(
            &self.id,
            self.selector,
            &frame,
            self.expected_failure_ref.as_deref(),
            self.timeout_ms,
        ))
    }

    pub fn to_canonical_json(&self) -> String {
        let mut s = String::new();
        s.push('{');
        s.push_str("\"id\":");
        s.push_str(&escape_rfc8259(&self.id));
        s.push_str(",\"selector\":");
        s.push_str(&escape_rfc8259(self.selector.as_str()));
        s.push_str(",\"argv_b64\":");
        s.push_str(&escape_rfc8259(&self.argv_b64));
        s.push_str(",\"env_digest\":");
        s.push_str(&escape_rfc8259(&self.env_digest));
        s.push_str(",\"expected_failure_ref\":");
        match &self.expected_failure_ref {
            Some(r) => s.push_str(&escape_rfc8259(r)),
            None => s.push_str("null"),
        }
        s.push_str(",\"observed\":");
        s.push_str(&escape_rfc8259(self.observed.as_str()));
        s.push_str(",\"failure_class\":");
        match &self.failure_class {
            Some(fc) => s.push_str(&escape_rfc8259(fc.as_str())),
            None => s.push_str("null"),
        }
        s.push_str(",\"termination\":");
        s.push_str(&escape_rfc8259(&self.termination.to_canonical_string()));
        s.push_str(",\"timeout_ms\":");
        s.push_str(&self.timeout_ms.to_string());
        s.push_str(",\"output_ref\":");
        s.push_str(&escape_rfc8259(&self.output_ref));
        s.push_str(",\"output_digest\":");
        s.push_str(&escape_rfc8259(&self.output_digest));
        s.push('}');
        s
    }

    pub fn parse_canonical_json(json_str: &str) -> Result<ProbeRecord, ProbeEvidenceError> {
        let v: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| ProbeEvidenceError::InvalidJson(format!("invalid JSON syntax: {e}")))?;

        let obj = v.as_object().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("record must be a JSON object".to_string())
        })?;

        if obj.len() != 11 {
            return Err(ProbeEvidenceError::InvalidJson(format!(
                "record object must have exactly 11 fields, found {}",
                obj.len()
            )));
        }

        let raw_keys = extract_json_keys(json_str)?;
        let expected_keys = [
            "id",
            "selector",
            "argv_b64",
            "env_digest",
            "expected_failure_ref",
            "observed",
            "failure_class",
            "termination",
            "timeout_ms",
            "output_ref",
            "output_digest",
        ];
        if raw_keys != expected_keys {
            return Err(ProbeEvidenceError::CanonicalityViolation(format!(
                "JSON field order mismatch: got {raw_keys:?}, expected {expected_keys:?}"
            )));
        }

        let id_val = obj.get("id").unwrap();
        let id_str = id_val
            .as_str()
            .ok_or_else(|| ProbeEvidenceError::InvalidJson("id must be a string".to_string()))?;
        validate_probe_id(id_str)?;

        let selector_val = obj.get("selector").unwrap();
        let selector_str = selector_val.as_str().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("selector must be a string".to_string())
        })?;
        let selector = ProbeSelector::parse(selector_str)?;

        let argv_b64_val = obj.get("argv_b64").unwrap();
        let argv_b64_str = argv_b64_val.as_str().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("argv_b64 must be a string".to_string())
        })?;
        let (argv, _) = decode_argv_b64(argv_b64_str)?;

        let env_digest_val = obj.get("env_digest").unwrap();
        let env_digest_str = env_digest_val.as_str().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("env_digest must be a string".to_string())
        })?;
        validate_lower_hex_64(env_digest_str, "env_digest")?;

        let ef_val = obj.get("expected_failure_ref").unwrap();
        let expected_failure_ref = if ef_val.is_null() {
            if selector == ProbeSelector::Acceptance {
                return Err(ProbeEvidenceError::InvalidJson(
                    "expected_failure_ref must not be null for acceptance selector".to_string(),
                ));
            }
            None
        } else if let Some(ef_str) = ef_val.as_str() {
            if selector == ProbeSelector::NonRed {
                return Err(ProbeEvidenceError::InvalidJson(
                    "expected_failure_ref must be null for non-red selector".to_string(),
                ));
            }
            validate_expected_failure_ref(ef_str)?;
            Some(ef_str.to_string())
        } else {
            return Err(ProbeEvidenceError::InvalidJson(
                "expected_failure_ref must be string or null".to_string(),
            ));
        };

        let observed_val = obj.get("observed").unwrap();
        let observed_str = observed_val.as_str().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("observed must be a string".to_string())
        })?;
        let observed = ObservedResult::parse(observed_str)?;

        let fc_val = obj.get("failure_class").unwrap();
        let failure_class = if fc_val.is_null() {
            if observed == ObservedResult::Fail {
                return Err(ProbeEvidenceError::InvalidJson(
                    "failure_class must not be null when observed is fail".to_string(),
                ));
            }
            None
        } else if let Some(fc_str) = fc_val.as_str() {
            if observed != ObservedResult::Fail {
                return Err(ProbeEvidenceError::InvalidJson(
                    "failure_class must be null unless observed is fail".to_string(),
                ));
            }
            Some(FailureClass::parse(fc_str)?)
        } else {
            return Err(ProbeEvidenceError::InvalidJson(
                "failure_class must be string or null".to_string(),
            ));
        };

        let term_val = obj.get("termination").unwrap();
        let term_str = term_val.as_str().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("termination must be a string".to_string())
        })?;
        let termination = Termination::parse(term_str)?;

        validate_class_termination_matrix(observed, failure_class, &termination)?;

        let timeout_val = obj.get("timeout_ms").unwrap();
        let timeout_u64 = timeout_val.as_u64().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("timeout_ms must be an integer".to_string())
        })?;
        if !(1..=4294967295).contains(&timeout_u64) {
            return Err(ProbeEvidenceError::InvalidJson(format!(
                "timeout_ms out of range 1..=4294967295: {timeout_u64}"
            )));
        }
        let timeout_ms = timeout_u64 as u32;

        let out_ref_val = obj.get("output_ref").unwrap();
        let out_ref_str = out_ref_val.as_str().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("output_ref must be a string".to_string())
        })?;

        let out_digest_val = obj.get("output_digest").unwrap();
        let out_digest_str = out_digest_val.as_str().ok_or_else(|| {
            ProbeEvidenceError::InvalidJson("output_digest must be a string".to_string())
        })?;
        validate_lower_hex_64(out_digest_str, "output_digest")?;

        let expected_ref = format!("outputs/{out_digest_str}.bin");
        if out_ref_str != expected_ref {
            return Err(ProbeEvidenceError::InvalidJson(format!(
                "output_ref '{out_ref_str}' does not match expected '{expected_ref}'"
            )));
        }

        let record = ProbeRecord {
            id: id_str.to_string(),
            selector,
            argv,
            argv_b64: argv_b64_str.to_string(),
            env_digest: env_digest_str.to_string(),
            expected_failure_ref,
            observed,
            failure_class,
            termination,
            timeout_ms,
            output_ref: out_ref_str.to_string(),
            output_digest: out_digest_str.to_string(),
        };

        let canonical_str = record.to_canonical_json();
        if canonical_str != json_str {
            return Err(ProbeEvidenceError::CanonicalityViolation(
                "JSON string is not canonical representation".to_string(),
            ));
        }

        Ok(record)
    }
}

pub fn validate_class_termination_matrix(
    observed: ObservedResult,
    failure_class: Option<FailureClass>,
    termination: &Termination,
) -> Result<(), ProbeEvidenceError> {
    match observed {
        ObservedResult::Pass => {
            if termination != &Termination::Exit(0) {
                return Err(ProbeEvidenceError::InvalidJson(format!(
                    "observed pass requires termination exit:0, got '{}'",
                    termination.to_canonical_string()
                )));
            }
            if failure_class.is_some() {
                return Err(ProbeEvidenceError::InvalidJson(
                    "observed pass requires failure_class to be null".to_string(),
                ));
            }
        }
        ObservedResult::Fail => {
            let fc = failure_class.ok_or_else(|| {
                ProbeEvidenceError::InvalidJson(
                    "observed fail requires non-null failure_class".to_string(),
                )
            })?;
            match fc {
                // R16 term tokens are `nonzero`, `exit:N`, and `signal:N`. `nonzero`
                // and `exit:N` are distinct tokens, so `exit:N` deliberately admits
                // any code including zero and is not narrowed here. R16 also states
                // that `spawn-error`, `timeout`, and `not-run` never prove red or
                // pass, so those terminations are rejected for every failure class.
                FailureClass::Assertion | FailureClass::Diagnostic => match termination {
                    Termination::Exit(0) => {
                        return Err(ProbeEvidenceError::InvalidJson(format!(
                            "failure_class '{}' requires nonzero termination, got exit:0",
                            fc.as_str()
                        )));
                    }
                    Termination::Exit(_) => {}
                    _ => {
                        return Err(ProbeEvidenceError::InvalidJson(format!(
                            "failure_class '{}' requires nonzero exit termination, got '{}'",
                            fc.as_str(),
                            termination.to_canonical_string()
                        )));
                    }
                },
                FailureClass::ErrorCode | FailureClass::Status => match termination {
                    Termination::Exit(_) => {}
                    _ => {
                        return Err(ProbeEvidenceError::InvalidJson(format!(
                            "failure_class '{}' requires exit:N termination, got '{}'",
                            fc.as_str(),
                            termination.to_canonical_string()
                        )));
                    }
                },
                FailureClass::Exception => match termination {
                    Termination::Exit(0) => {
                        return Err(ProbeEvidenceError::InvalidJson(
                            "failure_class 'exception' requires nonzero termination or signal:N, got exit:0".to_string(),
                        ));
                    }
                    Termination::Exit(_) | Termination::Signal(_) => {}
                    _ => {
                        return Err(ProbeEvidenceError::InvalidJson(format!(
                            "failure_class 'exception' requires nonzero exit or signal:N termination, got '{}'",
                            termination.to_canonical_string()
                        )));
                    }
                },
            }
        }
        ObservedResult::NotRun => {
            return Err(ProbeEvidenceError::NotRunRecord(
                "observed not-run is invalid evidence".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_probe_id(id: &str) -> Result<(), ProbeEvidenceError> {
    if id.len() != 4 || !id.starts_with("P-") {
        return Err(ProbeEvidenceError::InvalidJson(format!(
            "invalid probe id '{id}' (must be P-01..P-99 format)"
        )));
    }
    let digits = &id[2..];
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(ProbeEvidenceError::InvalidJson(format!(
            "invalid probe id '{id}' (must be P-01..P-99 format)"
        )));
    }
    let num: u32 = digits
        .parse()
        .map_err(|_| ProbeEvidenceError::InvalidJson(format!("invalid probe id '{id}'")))?;
    if !(1..=99).contains(&num) {
        return Err(ProbeEvidenceError::InvalidJson(format!(
            "invalid probe id '{id}' (must be P-01..P-99 format)"
        )));
    }
    Ok(())
}

fn validate_expected_failure_ref(r: &str) -> Result<(), ProbeEvidenceError> {
    if r.len() != 5 || !r.starts_with("EF-") {
        return Err(ProbeEvidenceError::InvalidJson(format!(
            "invalid expected_failure_ref '{r}' (must be EF-01..EF-99 format)"
        )));
    }
    let digits = &r[3..];
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(ProbeEvidenceError::InvalidJson(format!(
            "invalid expected_failure_ref '{r}' (must be EF-01..EF-99 format)"
        )));
    }
    let num: u32 = digits.parse().map_err(|_| {
        ProbeEvidenceError::InvalidJson(format!("invalid expected_failure_ref '{r}'"))
    })?;
    if !(1..=99).contains(&num) {
        return Err(ProbeEvidenceError::InvalidJson(format!(
            "invalid expected_failure_ref '{r}' (must be EF-01..EF-99 format)"
        )));
    }
    Ok(())
}

fn validate_lower_hex_64(s: &str, field: &str) -> Result<(), ProbeEvidenceError> {
    if s.len() != 64 || !s.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')) {
        return Err(ProbeEvidenceError::InvalidJson(format!(
            "{field} must be 64 lowercase hex characters, got '{s}'"
        )));
    }
    Ok(())
}

fn extract_json_keys(json_str: &str) -> Result<Vec<&str>, ProbeEvidenceError> {
    let mut keys = Vec::new();
    let mut rest = json_str.trim();
    if !rest.starts_with('{') || !rest.ends_with('}') {
        return Err(ProbeEvidenceError::InvalidJson(
            "must start with {{ and end with }}".to_string(),
        ));
    }
    rest = &rest[1..rest.len() - 1];
    while !rest.is_empty() {
        rest = rest.trim_start();
        if !rest.starts_with('"') {
            return Err(ProbeEvidenceError::InvalidJson(
                "expected quoted key".to_string(),
            ));
        }
        let end_quote = find_closing_quote(rest)?;
        let key = &rest[1..end_quote];
        keys.push(key);
        rest = rest[end_quote + 1..].trim_start();
        if !rest.starts_with(':') {
            return Err(ProbeEvidenceError::InvalidJson(
                "expected : after key".to_string(),
            ));
        }
        rest = rest[1..].trim_start();
        let value_end = find_json_value_end(rest)?;
        rest = rest[value_end..].trim_start();
        if rest.starts_with(',') {
            rest = &rest[1..];
        } else if !rest.is_empty() {
            return Err(ProbeEvidenceError::InvalidJson(
                "expected comma or end of object".to_string(),
            ));
        }
    }
    Ok(keys)
}

fn find_closing_quote(s: &str) -> Result<usize, ProbeEvidenceError> {
    let mut escaped = false;
    for (i, c) in s.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if c == '\\' {
            escaped = true;
        } else if c == '"' {
            return Ok(i);
        }
    }
    Err(ProbeEvidenceError::InvalidJson(
        "unterminated string".to_string(),
    ))
}

fn find_json_value_end(s: &str) -> Result<usize, ProbeEvidenceError> {
    if s.starts_with('"') {
        let end_quote = find_closing_quote(s)?;
        return Ok(end_quote + 1);
    }
    let mut depth_obj = 0;
    let mut depth_arr = 0;
    let mut in_str = false;
    let mut escaped = false;
    for (i, c) in s.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if in_str {
            if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth_obj += 1,
            '}' => {
                if depth_obj == 0 && depth_arr == 0 {
                    return Ok(i);
                }
                depth_obj -= 1;
            }
            '[' => depth_arr += 1,
            ']' => {
                if depth_arr == 0 && depth_obj == 0 {
                    return Ok(i);
                }
                depth_arr -= 1;
            }
            ',' if depth_obj == 0 && depth_arr == 0 => return Ok(i),
            _ => {}
        }
    }
    Ok(s.len())
}

// ── ProbeReceipt ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeReceipt {
    pub plan: String,
    pub task: String,
    pub generation: u32,
    pub contract_digest: String,
    pub phase: String,
    pub expectation: String,
    pub runner_contract: String,
    pub probe_set_digest: String,
    pub verdict: String,
    pub records: Vec<ProbeRecord>,
}

impl ProbeReceipt {
    pub fn to_receipt_bytes(&self) -> Vec<u8> {
        let mut out = String::new();
        out.push_str(&format!("plan={}\n", self.plan));
        out.push_str(&format!("task={}\n", self.task));
        out.push_str(&format!("generation={}\n", self.generation));
        out.push_str(&format!("contract_digest={}\n", self.contract_digest));
        out.push_str(&format!("phase={}\n", self.phase));
        out.push_str(&format!("expectation={}\n", self.expectation));
        out.push_str(&format!("runner_contract={}\n", self.runner_contract));
        out.push_str(&format!("probe_set_digest={}\n", self.probe_set_digest));
        out.push_str(&format!("verdict={}\n", self.verdict));
        out.push('\n');

        let mut sorted_records = self.records.clone();
        sorted_records.sort_by(|a, b| a.id.as_bytes().cmp(b.id.as_bytes()));

        for rec in &sorted_records {
            let json_str = rec.to_canonical_json();
            let b64 = encode_base64url_unpadded(json_str.as_bytes());
            out.push_str(&format!("probe_record_b64={b64}\n"));
        }

        out.into_bytes()
    }

    pub fn parse_receipt_bytes(bytes: &[u8]) -> Result<ProbeReceipt, ProbeEvidenceError> {
        if bytes.contains(&b'\r') {
            return Err(ProbeEvidenceError::CanonicalityViolation(
                "receipt contains CRLF or CR line endings (must be LF only)".to_string(),
            ));
        }

        let content = std::str::from_utf8(bytes).map_err(|e| {
            ProbeEvidenceError::InvalidHeader(format!("receipt is not valid UTF-8: {e}"))
        })?;

        if !content.ends_with('\n') {
            return Err(ProbeEvidenceError::CanonicalityViolation(
                "receipt must end with a final LF character".to_string(),
            ));
        }

        let lines: Vec<&str> = content.split('\n').collect();
        let lines = &lines[..lines.len() - 1];

        if lines.len() < 11 {
            return Err(ProbeEvidenceError::InvalidHeader(
                "receipt must have at least 9 headers, 1 blank line, and 1 record line".to_string(),
            ));
        }

        let plan = parse_header_line(lines[0], "plan=")?;
        let task = parse_header_line(lines[1], "task=")?;

        let gen_str = parse_header_line(lines[2], "generation=")?;
        if gen_str.starts_with('0') || gen_str.starts_with('-') {
            return Err(ProbeEvidenceError::InvalidHeader(
                "generation must be a positive u32 integer with no leading zero".to_string(),
            ));
        }
        let generation: u32 = gen_str.parse().map_err(|e| {
            ProbeEvidenceError::InvalidHeader(format!(
                "invalid generation integer '{gen_str}': {e}"
            ))
        })?;
        if generation == 0 {
            return Err(ProbeEvidenceError::InvalidHeader(
                "generation must be > 0".to_string(),
            ));
        }

        let contract_digest = parse_header_line(lines[3], "contract_digest=")?;
        validate_lower_hex_64(&contract_digest, "contract_digest")?;

        let phase = parse_header_line(lines[4], "phase=")?;
        let expectation = parse_header_line(lines[5], "expectation=")?;

        match (phase.as_str(), expectation.as_str()) {
            ("test", "red") | ("green-rerun", "green") | ("test", "pass") => {}
            _ => {
                return Err(ProbeEvidenceError::InvalidHeader(format!(
                    "invalid phase/expectation pair: ('{phase}', '{expectation}')"
                )));
            }
        }

        let runner_contract = parse_header_line(lines[6], "runner_contract=")?;
        if runner_contract != RUNNER_CONTRACT {
            return Err(ProbeEvidenceError::InvalidHeader(format!(
                "invalid runner_contract '{runner_contract}' (expected '{RUNNER_CONTRACT}')"
            )));
        }

        let probe_set_digest = parse_header_line(lines[7], "probe_set_digest=")?;
        validate_lower_hex_64(&probe_set_digest, "probe_set_digest")?;

        let verdict = parse_header_line(lines[8], "verdict=")?;
        if verdict != "pass" && verdict != "fail" {
            return Err(ProbeEvidenceError::InvalidHeader(format!(
                "invalid verdict '{verdict}' (expected 'pass' or 'fail')"
            )));
        }

        if !lines[9].is_empty() {
            return Err(ProbeEvidenceError::InvalidHeader(format!(
                "line 10 must be empty separator line, found '{}'",
                lines[9]
            )));
        }

        let mut records = Vec::new();
        let mut identities = Vec::new();

        for (idx, line) in lines[10..].iter().enumerate() {
            if line.is_empty() {
                return Err(ProbeEvidenceError::CanonicalityViolation(format!(
                    "unexpected blank line at record index {idx}"
                )));
            }
            let b64 = line.strip_prefix("probe_record_b64=").ok_or_else(|| {
                ProbeEvidenceError::InvalidHeader(format!(
                    "record line must start with 'probe_record_b64=', found '{line}'"
                ))
            })?;

            let json_bytes = decode_base64url_unpadded(b64).map_err(|e| {
                ProbeEvidenceError::Base64Error(format!("invalid probe_record_b64 base64url: {e}"))
            })?;
            let json_str = std::str::from_utf8(&json_bytes).map_err(|e| {
                ProbeEvidenceError::InvalidJson(format!("decoded record is not valid UTF-8: {e}"))
            })?;

            let rec = ProbeRecord::parse_canonical_json(json_str)?;

            if rec.observed == ObservedResult::NotRun {
                return Err(ProbeEvidenceError::NotRunRecord(rec.id.clone()));
            }

            match expectation.as_str() {
                "red" | "green" => {
                    if rec.selector != ProbeSelector::Acceptance {
                        return Err(ProbeEvidenceError::InvalidJson(format!(
                            "record {} selector must be acceptance for expectation '{expectation}'",
                            rec.id
                        )));
                    }
                }
                "pass" if rec.selector != ProbeSelector::NonRed => {
                    return Err(ProbeEvidenceError::InvalidJson(format!(
                        "record {} selector must be non-red for expectation 'pass'",
                        rec.id
                    )));
                }
                _ => {}
            }

            let id_bytes = rec.compute_identity_bytes()?;
            records.push(rec);
            identities.push(id_bytes);
        }

        if records.is_empty() {
            return Err(ProbeEvidenceError::InvalidHeader(
                "receipt must contain at least one probe record".to_string(),
            ));
        }

        for window in records.windows(2) {
            if window[0].id.as_bytes() >= window[1].id.as_bytes() {
                if window[0].id == window[1].id {
                    return Err(ProbeEvidenceError::DuplicateRecord(window[0].id.clone()));
                } else {
                    return Err(ProbeEvidenceError::UnsortedRecords(format!(
                        "record {} appears before {}",
                        window[0].id, window[1].id
                    )));
                }
            }
        }

        let computed_psd = compute_probe_set_digest(&identities);
        if computed_psd != probe_set_digest {
            return Err(ProbeEvidenceError::CanonicalityViolation(format!(
                "probe_set_digest mismatch: computed '{computed_psd}', header has '{probe_set_digest}'"
            )));
        }

        let all_pass = records.iter().all(|r| r.observed == ObservedResult::Pass);
        let expected_verdict = if all_pass { "pass" } else { "fail" };
        if verdict != expected_verdict {
            return Err(ProbeEvidenceError::InvalidHeader(format!(
                "verdict mismatch: computed '{expected_verdict}', header has '{verdict}'"
            )));
        }

        Ok(ProbeReceipt {
            plan,
            task,
            generation,
            contract_digest,
            phase,
            expectation,
            runner_contract,
            probe_set_digest,
            verdict,
            records,
        })
    }
}

fn parse_header_line(line: &str, prefix: &str) -> Result<String, ProbeEvidenceError> {
    let val = line.strip_prefix(prefix).ok_or_else(|| {
        ProbeEvidenceError::InvalidHeader(format!(
            "expected header line starting with '{prefix}', found '{line}'"
        ))
    })?;
    if val.is_empty() {
        return Err(ProbeEvidenceError::InvalidHeader(format!(
            "header '{prefix}' value must not be empty"
        )));
    }
    Ok(val.to_string())
}

// ── Same-Generation Identity ────────────────────────────────────────────────────

pub fn validate_same_generation_identity(
    red_receipt: &ProbeReceipt,
    green_receipt: &ProbeReceipt,
) -> Result<(), ProbeEvidenceError> {
    if red_receipt.plan != green_receipt.plan {
        return Err(ProbeEvidenceError::IdentityMismatch(format!(
            "plan mismatch: red='{}', green='{}'",
            red_receipt.plan, green_receipt.plan
        )));
    }
    if red_receipt.task != green_receipt.task {
        return Err(ProbeEvidenceError::IdentityMismatch(format!(
            "task mismatch: red='{}', green='{}'",
            red_receipt.task, green_receipt.task
        )));
    }
    if red_receipt.generation != green_receipt.generation {
        return Err(ProbeEvidenceError::IdentityMismatch(format!(
            "generation mismatch: red='{}', green='{}'",
            red_receipt.generation, green_receipt.generation
        )));
    }
    if red_receipt.contract_digest != green_receipt.contract_digest {
        return Err(ProbeEvidenceError::IdentityMismatch(format!(
            "contract_digest mismatch: red='{}', green='{}'",
            red_receipt.contract_digest, green_receipt.contract_digest
        )));
    }
    if red_receipt.runner_contract != green_receipt.runner_contract {
        return Err(ProbeEvidenceError::IdentityMismatch(format!(
            "runner_contract mismatch: red='{}', green='{}'",
            red_receipt.runner_contract, green_receipt.runner_contract
        )));
    }
    // `probe_set_digest` is deliberately NOT compared across receipts: it folds
    // in each probe's `env_digest`, and the red receipt is authored by a
    // dispatched TESTER while the green rerun is authored by the in-process
    // ORCHESTRATOR, so the two never share an ambient environment. Each receipt
    // still self-verifies its own digest at parse time. The per-record loop below
    // compares the env-excluded projection instead.
    if red_receipt.records.len() != green_receipt.records.len() {
        return Err(ProbeEvidenceError::IdentityMismatch(format!(
            "record count mismatch: red={}, green={}",
            red_receipt.records.len(),
            green_receipt.records.len()
        )));
    }
    for (r_rec, g_rec) in red_receipt.records.iter().zip(green_receipt.records.iter()) {
        let r_id = r_rec.compute_comparison_bytes()?;
        let g_id = g_rec.compute_comparison_bytes()?;
        if r_id != g_id {
            return Err(ProbeEvidenceError::IdentityMismatch(format!(
                "record {} identity mismatch between red and green",
                r_rec.id
            )));
        }
    }
    Ok(())
}
