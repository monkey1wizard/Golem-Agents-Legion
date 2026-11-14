use crate::usage::{RecordKind, StoredRecord};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, io, path::Path};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionBinding {
    pub criterion_id: String,
    pub spec_sha256: String,
    pub membership_sha256: String,
    pub artifact_set_sha256: String,
    pub artifact_snapshot_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionMapping {
    pub tp_id: String,
    pub task_id: String,
    pub gate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriterionSource {
    Test,
    OrchestratorCheck,
    GoalVerification,
    OwnerAcceptance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionSecurity {
    pub rule_or_threat: String,
    pub pass_condition: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionDeclaration {
    pub criterion_id: String,
    pub requirements: BTreeSet<String>,
    pub gate_id: String,
    pub pass_condition: String,
    pub source_kind: CriterionSource,
    pub target: Option<String>,
    pub security: Option<CriterionSecurity>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionTestPlan {
    pub id: String,
    pub test_type: String,
    pub covers: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CriterionPayload {
    pub schema_version: u32,
    pub declaration: CriterionDeclaration,
    pub test_plans: BTreeSet<CriterionTestPlan>,
    pub mappings: BTreeSet<CriterionMapping>,
}

impl CriterionPayload {
    pub fn spec_sha256(&self) -> Result<String, serde_json::Error> {
        let mut value = serde_json::to_value(self)?;
        normalize_json(&mut value);
        Ok(crate::usage::digest_bytes(&serde_json::to_vec(&value)?))
    }
}

fn normalize_json(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::String(text) => *text = text.replace("\r\n", "\n").replace('\r', "\n"),
        serde_json::Value::Array(items) => items.iter_mut().for_each(normalize_json),
        serde_json::Value::Object(fields) => fields.values_mut().for_each(normalize_json),
        _ => {}
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ArtifactState {
    Absent,
    File { mode: u32, content_digest: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactSnapshot {
    pub entries: std::collections::BTreeMap<String, ArtifactState>,
}

impl ArtifactSnapshot {
    pub fn digest(&self) -> String {
        crate::usage::digest_bytes(&serde_json::to_vec(self).expect("snapshot serialization"))
    }
}

pub fn artifact_snapshot(repo: &Path, paths: &BTreeSet<String>) -> io::Result<ArtifactSnapshot> {
    use std::path::Component;
    let root = fs::canonicalize(repo)?;
    let mut entries = std::collections::BTreeMap::new();
    for name in paths {
        let path = Path::new(name);
        if name.is_empty()
            || name.contains(['*', '?', '[', ']', '\\', ':'])
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid artifact path",
            ));
        }
        let mut current = root.clone();
        let mut absent = false;
        for component in path.components() {
            current.push(component);
            match fs::symlink_metadata(&current) {
                Ok(metadata) if metadata.file_type().is_symlink() => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "artifact path contains a link",
                    ))
                }
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    absent = true;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        let state = if absent {
            ArtifactState::Absent
        } else {
            let metadata = fs::metadata(&current)?;
            if !metadata.is_file() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "artifact is not a file",
                ));
            }
            #[cfg(unix)]
            let mode = {
                use std::os::unix::fs::PermissionsExt;
                if metadata.permissions().mode() & 0o111 != 0 {
                    0o100755
                } else {
                    0o100644
                }
            };
            #[cfg(not(unix))]
            let mode = 0o100644;
            let output = std::process::Command::new("git")
                .arg("-C")
                .arg(&root)
                .arg("hash-object")
                .arg(format!("--path={name}"))
                .arg("--")
                .arg(name)
                .output()?;
            if !output.status.success() {
                return Err(io::Error::other(
                    String::from_utf8_lossy(&output.stderr).into_owned(),
                ));
            }
            ArtifactState::File {
                mode,
                content_digest: crate::usage::digest_bytes(&output.stdout),
            }
        };
        entries.insert(name.clone(), state);
    }
    Ok(ArtifactSnapshot { entries })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactValidity {
    pub snapshot: ArtifactSnapshot,
    pub source_evidence_sha256: String,
    pub superseded: bool,
}

impl ArtifactValidity {
    /// Creates a validity record bound to the snapshot and source evidence.
    pub fn new(snapshot: ArtifactSnapshot, source_evidence: &[u8]) -> Self {
        Self {
            snapshot,
            source_evidence_sha256: crate::usage::digest_bytes(source_evidence),
            superseded: false,
        }
    }

    pub fn check(
        &mut self,
        current: &ArtifactSnapshot,
        source_evidence: &[u8],
    ) -> Result<CriterionStatus, &'static str> {
        if crate::usage::digest_bytes(source_evidence) != self.source_evidence_sha256 {
            return Err("integrity-failure");
        }
        self.superseded |= self.snapshot != *current;
        Ok(if self.superseded {
            CriterionStatus::Superseded
        } else {
            CriterionStatus::Pass
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CriterionStatus {
    Pass,
    Fail,
    Unverified,
    Superseded,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CriterionRow {
    pub mapping: CriterionMapping,
    pub status: CriterionStatus,
}

pub fn criterion_status(
    expected: &BTreeSet<CriterionMapping>,
    rows: &[CriterionRow],
) -> CriterionStatus {
    if expected.is_empty() {
        return CriterionStatus::Unverified;
    }
    let mut seen = BTreeSet::new();
    for row in rows {
        if !expected.contains(&row.mapping) || !seen.insert(&row.mapping) {
            return CriterionStatus::Unverified;
        }
    }
    if rows
        .iter()
        .any(|row| row.status == CriterionStatus::Superseded)
    {
        CriterionStatus::Superseded
    } else if rows.iter().any(|row| row.status == CriterionStatus::Fail) {
        CriterionStatus::Fail
    } else if seen.len() != expected.len()
        || rows
            .iter()
            .any(|row| row.status == CriterionStatus::Unverified)
    {
        CriterionStatus::Unverified
    } else {
        CriterionStatus::Pass
    }
}

/// An immutable event with source-specific facts in its content.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EvidenceRecord {
    pub schema_version: u32,
    pub event_id: String,
    pub operation_id: String,
    pub run_id: String,
    pub plan_generation: Option<String>,
    pub task_id: Option<String>,
    pub attempt_id: Option<String>,
    pub parent_attempt_id: Option<String>,
    pub phase: String,
    pub source_kind: String,
    pub source_version: String,
    pub recorded_at_unix_ms: u64,
    pub content_hash: String,
    pub content: serde_json::Value,
}

impl StoredRecord for EvidenceRecord {
    const KIND: RecordKind = RecordKind::Evidence;
    fn schema_version(&self) -> u32 {
        self.schema_version
    }
    fn run_id(&self) -> &str {
        &self.run_id
    }
    fn record_id(&self) -> &str {
        &self.event_id
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesignRosterVersion {
    pub design_roster_version: Option<String>,
    pub reason: Option<String>,
}

/// Read the version line outside fenced code in the governance roster.
pub fn read_design_roster_version(repo: &Path) -> io::Result<DesignRosterVersion> {
    let path = repo.join("plugins/gal-core/conventions/governance-report.md");
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(roster(None, "roster-absent")),
        Err(e) if e.kind() == io::ErrorKind::InvalidData => {
            return Ok(roster(None, "roster-malformed"))
        }
        Err(e) => return Err(e),
    };
    let mut fence: Option<(char, usize)> = None;
    let mut lines = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if let Some(ch) = trimmed.chars().next().filter(|c| *c == '`' || *c == '~') {
            let count = trimmed.chars().take_while(|c| *c == ch).count();
            if count >= 3 {
                match fence {
                    None => {
                        fence = Some((ch, count));
                        continue;
                    }
                    Some((open, length))
                        if ch == open && count >= length && trimmed[count..].trim().is_empty() =>
                    {
                        fence = None;
                        continue;
                    }
                    _ => {}
                }
            }
        }
        if fence.is_none() && trimmed.starts_with("Roster-Version") {
            lines.push(line);
        }
    }
    if lines.is_empty() {
        return Ok(roster(None, "roster-absent"));
    }
    if lines.len() != 1 {
        return Ok(roster(None, "roster-malformed"));
    }
    let Some(token) = lines[0].strip_prefix("Roster-Version: ") else {
        return Ok(roster(None, "roster-malformed"));
    };
    if token.is_empty()
        || token.len() > 64
        || !token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
    {
        return Ok(roster(None, "roster-malformed"));
    }
    Ok(DesignRosterVersion {
        design_roster_version: Some(token.to_owned()),
        reason: None,
    })
}

fn roster(version: Option<String>, reason: &str) -> DesignRosterVersion {
    DesignRosterVersion {
        design_roster_version: version,
        reason: Some(reason.to_owned()),
    }
}
