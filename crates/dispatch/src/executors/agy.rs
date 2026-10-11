//! Antigravity CLI (agy) executor adapter.
//!
//! The v1 invocation remains stdin based. V2 adds native JSON output and reads
//! session identity only from stdout captured for that executor attempt.

use super::{
    Adapter, AdapterInvocation, AdapterInvocationInput, SessionEvidence, SessionEvidenceError,
    SpecDelivery,
};
use serde::de::{MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;

const MAX_NATIVE_OUTPUT_BYTES: usize = 64 * 1024;

pub struct AgyAdapter;

impl Adapter for AgyAdapter {
    fn executor_name(&self) -> &str {
        "agy"
    }

    fn build_invocation(&self, input: &AdapterInvocationInput) -> AdapterInvocation {
        build_args(input, None, false)
    }

    fn build_v2_invocation(
        &self,
        input: &AdapterInvocationInput,
        resumed_session_id: Option<&str>,
    ) -> AdapterInvocation {
        build_args(input, resumed_session_id, true)
    }

    fn supports_effort(&self) -> bool {
        true
    }

    fn extract_session_id(&self, stdout: &str) -> Option<String> {
        parse_native_output(stdout, None)
            .ok()
            .map(|evidence| evidence.session_id)
    }

    fn parse_session_evidence(
        &self,
        stdout: &str,
        resumed_session_id: Option<&str>,
    ) -> Result<Option<SessionEvidence>, SessionEvidenceError> {
        parse_native_output(stdout, resumed_session_id).map(Some)
    }
}

fn build_args(
    input: &AdapterInvocationInput,
    resumed_session_id: Option<&str>,
    v2: bool,
) -> AdapterInvocation {
    let mut args = vec![
        "--dangerously-skip-permissions".to_string(),
        "--add-dir".to_string(),
        input.workdir.to_string_lossy().into_owned(),
    ];
    if let Some(session_id) = resumed_session_id {
        args.push("--conversation".to_string());
        args.push(session_id.to_string());
    }
    if !input.model.is_empty() {
        args.push("--model".to_string());
        args.push(input.model.to_string());
    }
    if let Some(effort) = input.effort {
        args.push("--effort".to_string());
        args.push(effort.to_string());
    }
    if let Some(secs) = input.timeout_secs {
        args.push("--print-timeout".to_string());
        args.push(format!("{secs}s"));
    }
    if v2 {
        args.push("--output-format".to_string());
        args.push("json".to_string());
    }
    AdapterInvocation {
        args,
        delivery: SpecDelivery::Stdin,
    }
}

#[derive(Default)]
struct NativeTerminal {
    conversation_id: Option<String>,
    status: Option<String>,
    conversation_id_count: usize,
    status_count: usize,
}

impl<'de> Deserialize<'de> for NativeTerminal {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct TerminalVisitor;
        impl<'de> Visitor<'de> for TerminalVisitor {
            type Value = NativeTerminal;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an Agy terminal JSON object")
            }
            fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
            where
                M: MapAccess<'de>,
            {
                let mut terminal = NativeTerminal::default();
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "conversation_id" => {
                            terminal.conversation_id_count += 1;
                            terminal.conversation_id = Some(map.next_value()?);
                        }
                        "status" => {
                            terminal.status_count += 1;
                            terminal.status = Some(map.next_value()?);
                        }
                        _ => {
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(terminal)
            }
        }
        deserializer.deserialize_map(TerminalVisitor)
    }
}

fn parse_native_output(
    stdout: &str,
    resumed_session_id: Option<&str>,
) -> Result<SessionEvidence, SessionEvidenceError> {
    if stdout.len() > MAX_NATIVE_OUTPUT_BYTES {
        return Err(SessionEvidenceError::Oversized);
    }
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Err(SessionEvidenceError::MissingTerminalObject);
    }
    let objects: Vec<&str> = trimmed
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('{'))
        .collect();
    if objects.len() > 1 {
        return Err(SessionEvidenceError::DuplicateTerminalObject);
    }
    let line = trimmed.lines().last().unwrap_or_default().trim();
    if !line.starts_with('{') {
        return if trimmed.contains('{') {
            Err(SessionEvidenceError::Truncated)
        } else {
            Err(SessionEvidenceError::MissingTerminalObject)
        };
    }
    let mut deserializer = serde_json::Deserializer::from_str(line);
    let terminal = NativeTerminal::deserialize(&mut deserializer)
        .map_err(|_| SessionEvidenceError::MalformedJson)?;
    deserializer
        .end()
        .map_err(|_| SessionEvidenceError::MalformedJson)?;
    if terminal.conversation_id_count > 1 || terminal.status_count > 1 {
        return Err(SessionEvidenceError::DuplicateTerminalObject);
    }
    let session_id = terminal
        .conversation_id
        .ok_or(SessionEvidenceError::MissingField("conversation_id"))?;
    let status = terminal
        .status
        .ok_or(SessionEvidenceError::MissingField("status"))?;
    if !is_uuid(&session_id) {
        return Err(SessionEvidenceError::InvalidField("conversation_id"));
    }
    if status != "SUCCESS" {
        return Err(SessionEvidenceError::InvalidField("status"));
    }
    if resumed_session_id.is_some_and(|requested| requested != session_id) {
        return Err(SessionEvidenceError::MismatchedSession);
    }
    Ok(SessionEvidence { session_id, status })
}

fn is_uuid(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() == 36
        && [8, 13, 18, 23].iter().all(|&index| bytes[index] == b'-')
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| [8, 13, 18, 23].contains(&index) || byte.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn input<'a>(workdir: &'a std::path::Path) -> AdapterInvocationInput<'a> {
        AdapterInvocationInput {
            model: "m",
            workdir,
            spec: "spec",
            effort: None,
            mcp_disable_servers: &[],
            timeout_secs: None,
        }
    }

    const UUID: &str = "176f1141-b606-47aa-a47b-76f2a6147623";

    #[test]
    fn parses_new_and_resumed_session() {
        let output =
            format!(r#"{{"conversation_id":"{UUID}","status":"SUCCESS","response":"ok"}}"#);
        let parsed = parse_native_output(&output, None).unwrap();
        assert_eq!(parsed.session_id, UUID);
        assert_eq!(parsed.status, "SUCCESS");
        assert_eq!(
            parse_native_output(&output, Some(UUID)).unwrap().session_id,
            UUID
        );
        assert_eq!(
            parse_native_output(&output, Some("00000000-0000-0000-0000-000000000000")),
            Err(SessionEvidenceError::MismatchedSession)
        );
        assert_eq!(
            AgyAdapter.parse_session_evidence(&output, None).unwrap(),
            Some(SessionEvidence {
                session_id: UUID.to_string(),
                status: "SUCCESS".to_string(),
            })
        );
        assert_eq!(
            AgyAdapter
                .parse_session_evidence(&output, Some(UUID))
                .unwrap(),
            Some(SessionEvidence {
                session_id: UUID.to_string(),
                status: "SUCCESS".to_string(),
            })
        );
    }

    #[test]
    fn rejects_bounded_parser_failures() {
        for fixture in [
            "",
            "plain output",
            r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"SUCCESS""#,
            r#"{"conversation_id":"bad","status":"SUCCESS"}"#,
            r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"FAILURE"}"#,
            r#"{"conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","conversation_id":"176f1141-b606-47aa-a47b-76f2a6147623","status":"SUCCESS"}"#,
        ] {
            assert!(
                parse_native_output(fixture, None).is_err(),
                "accepted {fixture:?}"
            );
        }

        // Bounded size check
        assert_eq!(
            parse_native_output(&"x".repeat(MAX_NATIVE_OUTPUT_BYTES + 1), None),
            Err(SessionEvidenceError::Oversized)
        );

        // Missing field checks
        assert_eq!(
            parse_native_output(r#"{"status":"SUCCESS"}"#, None),
            Err(SessionEvidenceError::MissingField("conversation_id"))
        );
        assert_eq!(
            parse_native_output(&format!(r#"{{"conversation_id":"{UUID}"}}"#), None),
            Err(SessionEvidenceError::MissingField("status"))
        );

        // Duplicate terminal objects across lines
        let duplicate_lines = format!(
            r#"{{"conversation_id":"{UUID}","status":"SUCCESS"}}
{{"conversation_id":"{UUID}","status":"SUCCESS"}}"#
        );
        assert_eq!(
            parse_native_output(&duplicate_lines, None),
            Err(SessionEvidenceError::DuplicateTerminalObject)
        );

        // Truncated terminal output
        let truncated = format!(
            r#"{{"conversation_id":"{UUID}","status":"SUCCESS"}}
truncated tail"#
        );
        assert_eq!(
            parse_native_output(&truncated, None),
            Err(SessionEvidenceError::Truncated)
        );

        // Adapter trait error propagation
        assert!(matches!(
            AgyAdapter.parse_session_evidence("", None),
            Err(SessionEvidenceError::MissingTerminalObject)
        ));
    }

    #[test]
    fn v1_argv_is_unchanged_and_v2_requests_json() {
        let adapter = AgyAdapter;
        let wd = PathBuf::from("/wd");
        assert_eq!(
            adapter.build_invocation(&input(&wd)).args,
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                "/wd",
                "--model",
                "m"
            ]
        );
        assert_eq!(
            adapter.build_v2_invocation(&input(&wd), Some(UUID)).args,
            vec![
                "--dangerously-skip-permissions",
                "--add-dir",
                "/wd",
                "--conversation",
                UUID,
                "--model",
                "m",
                "--output-format",
                "json"
            ]
        );
    }

    #[test]
    fn session_parsing_does_not_consult_brain_directories() {
        let stdout = format!(r#"{{"conversation_id":"{UUID}","status":"SUCCESS"}}"#);
        assert_eq!(
            AgyAdapter.extract_session_id(&stdout).as_deref(),
            Some(UUID)
        );
    }
}
