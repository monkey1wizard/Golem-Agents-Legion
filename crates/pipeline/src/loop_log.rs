//! Pipeline loop error logger — append-only NDJSON forensics for the dispatch
//! loop.
//!
//! `executor-logs/` only captures headless dispatch STDOUT/STDERR. When a phase
//! has no dispatch attempt log (such as when running in-process), the errors the
//! orchestrator hits (naming-gate blocks, test failures, compile errors,
//! out-of-scope writes, retries) have no durable record. This module is the
//! deterministic writer for a machine-greppable loop-log that covers that blind
//! spot.
//!
//! It is **failure-only** (callers do not log pass detail), machine-local
//! forensics (the caller writes under a gitignored dir), and never logs raw
//! secrets — [`filter_secrets`] masks obvious token/secret material as defense
//! in depth. The writer is append-only and never rewrites prior lines.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Severity, aligned to the `structured-logging` convention: `Warning` =
/// recoverable / degraded, `Error` = a failure needing attention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopLevel {
    Info,
    Warning,
    Error,
}

impl LoopLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            LoopLevel::Info => "info",
            LoopLevel::Warning => "warning",
            LoopLevel::Error => "error",
        }
    }
}

/// One structured loop event. `time` is caller-supplied (ISO-8601) so the writer
/// stays pure — the clock is an input, not a hidden side effect. `kind` is one
/// of the documented event classes (`naming-gate-block`, `test-fail`,
/// `compile-error`, `executor-no-receipt`, `executor-timeout`,
/// `boundary-violation`, `degrade`, `retry`). `log_ptr` optionally points at the
/// matching `executor-logs/` file.
#[derive(Debug, Clone)]
pub struct LoopEvent {
    pub time: String,
    pub task: String,
    pub phase: String,
    pub role: String,
    pub kind: String,
    pub level: LoopLevel,
    pub msg: String,
    pub log_ptr: Option<String>,
}

/// Map a dispatch `terminal_state` string to a loop-log `(kind, level)`.
/// `completed` is Info (one outcome line, not verbose pass detail);
/// `no-receipt`/`disconnected-partial` are Error; the timeout family and
/// `unavailable` are Warning (recoverable/degraded). Unknown states default to
/// a generic degrade Warning so nothing is silently dropped.
pub fn classify_dispatch(terminal_state: &str) -> (&'static str, LoopLevel) {
    match terminal_state {
        "completed" => ("dispatch-completed", LoopLevel::Info),
        "no-receipt" => ("executor-no-receipt", LoopLevel::Error),
        "disconnected-partial" => ("executor-disconnected", LoopLevel::Error),
        "timeout" | "timeout-no-output" | "timeout-midrun" => {
            ("executor-timeout", LoopLevel::Warning)
        }
        "unavailable" => ("executor-unavailable", LoopLevel::Warning),
        _ => ("degrade", LoopLevel::Warning),
    }
}

/// Append one event as an NDJSON line to `<dir>/<run>.ndjson`, creating `dir`.
/// The message is secret-filtered. Append-only: existing lines are never
/// rewritten. Returns the file path written.
pub fn append_event(dir: &Path, run: &str, event: &LoopEvent) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{run}.ndjson"));
    let mut line = to_ndjson_line(event);
    line.push('\n');
    let mut f = OpenOptions::new().create(true).append(true).open(&path)?;
    f.write_all(line.as_bytes())?;
    Ok(path)
}

/// Serialize one event to a single NDJSON object line (hand-rolled to avoid a
/// serde_json dependency — the schema is small and fixed).
pub fn to_ndjson_line(e: &LoopEvent) -> String {
    let mut s = String::from("{");
    push_field(&mut s, "time", &e.time, true);
    push_field(&mut s, "task", &e.task, false);
    push_field(&mut s, "phase", &e.phase, false);
    push_field(&mut s, "role", &e.role, false);
    push_field(&mut s, "kind", &e.kind, false);
    push_field(&mut s, "level", e.level.as_str(), false);
    push_field(&mut s, "msg", &filter_secrets(&e.msg), false);
    if let Some(ptr) = &e.log_ptr {
        push_field(&mut s, "log_ptr", ptr, false);
    }
    s.push('}');
    s
}

fn push_field(s: &mut String, key: &str, val: &str, first: bool) {
    if !first {
        s.push(',');
    }
    s.push('"');
    s.push_str(key);
    s.push_str("\":\"");
    s.push_str(&escape_json(val));
    s.push('"');
}

fn escape_json(v: &str) -> String {
    let mut out = String::with_capacity(v.len());
    for c in v.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// Sensitive key words; a value immediately following one (after `=`/`:`/space)
/// is masked.
const SENSITIVE_KEYS: &[&str] = &[
    "token",
    "secret",
    "password",
    "passwd",
    "api_key",
    "apikey",
    "api-key",
    "authorization",
    "bearer",
    "github_token",
    "anthropic_api_key",
    "openai_api_key",
];

/// Redact obvious secret/PII material from a message before it lands in the log.
/// Conservative defense-in-depth (failure-only logging already limits exposure):
/// (1) a token that follows a sensitive key word + separator is masked; (2) any
/// standalone run of ≥ 32 token-characters (`[A-Za-z0-9+/_=-]`) is masked as a
/// likely key/token. Not a tokenizer — a minimal guard.
pub fn filter_secrets(msg: &str) -> String {
    let lowered = msg.to_ascii_lowercase();
    // Pass 1: mask the value after a sensitive key + separator.
    let mut masked = String::with_capacity(msg.len());
    let bytes = msg.as_bytes();
    let mut i = 0usize;
    while i < msg.len() {
        let after_key = SENSITIVE_KEYS.iter().find_map(|k| {
            if lowered[i..].starts_with(k) {
                Some(k.len())
            } else {
                None
            }
        });
        if let Some(klen) = after_key {
            // Emit the key, then skip a single separator run, then mask the value token.
            masked.push_str(&msg[i..i + klen]);
            let mut j = i + klen;
            // separator: any of = : space tab " '
            let sep_start = j;
            while j < msg.len() && matches!(bytes[j], b'=' | b':' | b' ' | b'\t' | b'"' | b'\'') {
                j += 1;
            }
            if j > sep_start {
                masked.push_str(&msg[sep_start..j]);
                // mask the value (until whitespace or quote)
                let val_start = j;
                while j < msg.len()
                    && !matches!(bytes[j], b' ' | b'\t' | b'\n' | b'"' | b'\'' | b',')
                {
                    j += 1;
                }
                if j > val_start {
                    masked.push_str("<redacted>");
                }
                i = j;
                continue;
            }
            i += klen;
            continue;
        }
        // copy one UTF-8 char
        let ch_len = utf8_len(bytes[i]);
        masked.push_str(&msg[i..i + ch_len]);
        i += ch_len;
    }
    // Pass 2: mask long token-character runs.
    mask_long_runs(&masked)
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

fn mask_long_runs(s: &str) -> String {
    let is_tok = |c: char| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '_' | '=' | '-');
    let mut out = String::with_capacity(s.len());
    let mut run = String::new();
    for c in s.chars() {
        if is_tok(c) {
            run.push(c);
        } else {
            flush_run(&mut out, &mut run);
            out.push(c);
        }
    }
    flush_run(&mut out, &mut run);
    out
}

fn flush_run(out: &mut String, run: &mut String) {
    if run.len() >= 32 {
        out.push_str("<redacted>");
    } else {
        out.push_str(run);
    }
    run.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ev(msg: &str) -> LoopEvent {
        LoopEvent {
            time: "2026-06-18T00:00:00Z".into(),
            task: "T-XX".into(),
            phase: "test".into(),
            role: "TESTER".into(),
            kind: "test-fail".into(),
            level: LoopLevel::Error,
            msg: msg.into(),
            log_ptr: Some(".dev/executor-logs/x.log".into()),
        }
    }

    #[test]
    fn append_writes_wellformed_ndjson_line() {
        let tmp = TempDir::new().unwrap();
        let path = append_event(tmp.path(), "run1", &ev("two tests failed")).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        assert!(body.ends_with('\n'));
        let line = body.trim_end();
        // All fields present, valid-looking single-object NDJSON.
        for needle in [
            "\"time\":\"2026-06-18T00:00:00Z\"",
            "\"task\":\"T-XX\"",
            "\"phase\":\"test\"",
            "\"role\":\"TESTER\"",
            "\"kind\":\"test-fail\"",
            "\"level\":\"error\"",
            "\"msg\":\"two tests failed\"",
            "\"log_ptr\":\".dev/executor-logs/x.log\"",
        ] {
            assert!(line.contains(needle), "missing {needle} in {line}");
        }
        assert!(line.starts_with('{') && line.ends_with('}'));
    }

    #[test]
    fn planted_secret_is_filtered_out() {
        let secret = "ghp_AbCdEf0123456789AbCdEf0123456789xyz";
        let msg = format!("dispatch failed: token={secret} during spawn");
        let line = to_ndjson_line(&ev(&msg));
        assert!(
            !line.contains(secret),
            "raw secret leaked into log line: {line}"
        );
        assert!(line.contains("<redacted>"));
    }

    #[test]
    fn classify_dispatch_maps_each_terminal_state() {
        assert_eq!(
            classify_dispatch("completed"),
            ("dispatch-completed", LoopLevel::Info)
        );
        assert_eq!(
            classify_dispatch("no-receipt"),
            ("executor-no-receipt", LoopLevel::Error)
        );
        assert_eq!(
            classify_dispatch("disconnected-partial"),
            ("executor-disconnected", LoopLevel::Error)
        );
        assert_eq!(
            classify_dispatch("timeout"),
            ("executor-timeout", LoopLevel::Warning)
        );
        assert_eq!(
            classify_dispatch("timeout-no-output"),
            ("executor-timeout", LoopLevel::Warning)
        );
        assert_eq!(
            classify_dispatch("timeout-midrun"),
            ("executor-timeout", LoopLevel::Warning)
        );
        assert_eq!(
            classify_dispatch("unavailable"),
            ("executor-unavailable", LoopLevel::Warning)
        );
        // Unknown → generic degrade Warning (never silently dropped).
        assert_eq!(classify_dispatch("weird"), ("degrade", LoopLevel::Warning));
    }

    #[test]
    fn append_only_keeps_prior_lines() {
        let tmp = TempDir::new().unwrap();
        append_event(tmp.path(), "run2", &ev("first")).unwrap();
        let path = append_event(tmp.path(), "run2", &ev("second")).unwrap();
        let body = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = body.lines().collect();
        assert_eq!(lines.len(), 2, "append must keep both lines");
        assert!(lines[0].contains("first"));
        assert!(lines[1].contains("second"));
    }
}
