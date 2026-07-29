//! Projection support: health check, frontmatter rendering, file/list/link helpers.

use super::*;
use gal_foundation::health::{DoctorFinding, HealthCheck};
use gal_foundation::platform::{create_dir_link, is_symlink_or_junction, remove_dir_link};
use serde_json::{Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const PROJECTION_LOCK_KEY: &str = "_galProjection";
const PROJECTION_LOCK_SCHEMA_VERSION: u64 = 1;
const MANAGED_SOURCE_ATTRIBUTION: &str = "sourceAttribution";

pub(crate) const MANAGED_AGENT_PATHS: &str = "agentProjectionPaths";
pub(crate) const MANAGED_CODEX_AGENT_PATHS: &str = "codexAgentProjectionPaths";
pub(crate) const MANAGED_COMMAND_PATHS: &str = "commandProjectionPaths";
pub(crate) const MANAGED_DISCUSS_SKILL_PATHS: &str = "discussSkillProjectionPaths";
pub(crate) const MANAGED_LEGACY_PATHS: &str = "legacyProjectionPaths";
pub(crate) const MANAGED_SKILL_PATHS: &str = "skillProjectionPaths";

/// GAL skills/commands whose absence from the Codex shared skill surface is
/// treated as an error (not just a warning) — the workflow cannot function
/// without them.
const WORKFLOW_CRITICAL_SKILLS: &[&str] = &["adversarial-review", "doc-sync", "install-gal"];

/// Warn when the total projected `description:` footprint reaches this many
/// characters — 87.5% of Codex's documented 8,000-char initial-list fallback
/// budget (used when the actual `<= 2% of context window` limit is unknown).
const CODEX_SKILL_BUDGET_WARN: usize = 7_000;

pub struct SkillsProjectionHealthCheck {
    shared_skills_root: PathBuf,
    /// Skill/command names the projection source currently declares (dynamic
    /// inventory). Empty means "just check the root exists" (back-compat with
    /// the pre-inventory constructor).
    required_names: Vec<String>,
    /// Prior-lockfile path, used to distinguish a GAL-managed item from a
    /// genuine user-owned collision. `None` skips the ownership check.
    lockfile_path: Option<PathBuf>,
    /// Canonical source root (the directory whose `<name>/SKILL.md` is the
    /// current projected content). When set, a projected shared skill whose
    /// `SKILL.md` differs from the canonical `<name>/SKILL.md` is reported as
    /// stale instead of silently passing. `None` skips the freshness check —
    /// a canonical file that does not exist is never a false stale report.
    canonical_source_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ManagedArtifactRegistry {
    lockfile_path: PathBuf,
    lockfile_root: Option<Map<String, Value>>,
    prior: BTreeMap<String, BTreeSet<String>>,
    next: BTreeMap<String, BTreeSet<String>>,
    /// Maps normalized path → source id recorded in the prior lockfile.
    /// Populated from `"sourceAttribution"` on load; used by per-source prune.
    source_attribution_prior: BTreeMap<String, String>,
    /// Maps normalized path → source id being built for the next lockfile write.
    source_attribution_next: BTreeMap<String, String>,
}

impl ManagedArtifactRegistry {
    pub(crate) fn load(lockfile_path: &Path, report: &mut ProjectionReport) -> Self {
        let mut registry = Self {
            lockfile_path: lockfile_path.to_path_buf(),
            ..Self::default()
        };
        if !lockfile_path.is_file() {
            return registry;
        }

        let raw = match fs::read_to_string(lockfile_path) {
            Ok(raw) => raw,
            Err(err) => {
                report.warnings.push(ProjectionWarning {
                    message: format!(
                        "could not read projection lockfile marker, skipped managed cleanup: {} ({err})",
                        lockfile_path.display()
                    ),
                });
                return registry;
            }
        };

        let root = match serde_json::from_str::<Value>(&raw) {
            Ok(Value::Object(root)) => root,
            Ok(_) => {
                report.warnings.push(ProjectionWarning {
                    message: format!(
                        "projection lockfile marker is not a JSON object, skipped managed cleanup: {}",
                        lockfile_path.display()
                    ),
                });
                return registry;
            }
            Err(err) => {
                report.warnings.push(ProjectionWarning {
                    message: format!(
                        "could not parse projection lockfile marker, skipped managed cleanup: {} ({err})",
                        lockfile_path.display()
                    ),
                });
                return registry;
            }
        };

        let mut projection = BTreeMap::new();
        let mut attribution = BTreeMap::new();
        if let Some(Value::Object(meta)) = root.get(PROJECTION_LOCK_KEY) {
            for category in [
                MANAGED_AGENT_PATHS,
                MANAGED_CODEX_AGENT_PATHS,
                MANAGED_COMMAND_PATHS,
                MANAGED_DISCUSS_SKILL_PATHS,
                MANAGED_LEGACY_PATHS,
                MANAGED_SKILL_PATHS,
            ] {
                projection.insert(
                    category.to_string(),
                    load_managed_artifact_set(meta, category),
                );
            }
            if let Some(Value::Object(attr_map)) = meta.get(MANAGED_SOURCE_ATTRIBUTION) {
                for (path, src) in attr_map {
                    if let Some(src_str) = src.as_str() {
                        attribution.insert(path.clone(), src_str.to_string());
                    }
                }
            }
        }

        registry.lockfile_root = Some(root);
        registry.prior = projection.clone();
        registry.next = projection;
        registry.source_attribution_prior = attribution.clone();
        registry.source_attribution_next = attribution;
        registry
    }

    pub(crate) fn can_mutate(&self, path: &Path) -> bool {
        // No prior lockfile → first-run; allow all mutations (backward-compat with is_gal_managed_file guard).
        if self.prior.is_empty() {
            return true;
        }
        let needle = normalize_managed_artifact_path(path);
        self.prior.values().any(|paths| paths.contains(&needle))
    }

    /// Positive test: was this exact path recorded as GAL-managed in the prior
    /// lockfile? Unlike [`can_mutate`], this is `false` on first run (empty prior),
    /// so a pre-existing path that GAL never wrote is treated as user-owned even
    /// before any lockfile exists — used to guard command-skill writes from
    /// clobbering a genuine user directory.
    pub(crate) fn is_managed(&self, path: &Path) -> bool {
        let needle = normalize_managed_artifact_path(path);
        self.prior.values().any(|paths| paths.contains(&needle))
    }

    pub(crate) fn replace_category(&mut self, category: &str) {
        if let Some(prior_paths) = self.prior.get(category) {
            for path in prior_paths {
                self.source_attribution_next.remove(path);
            }
        }
        self.next.insert(category.to_string(), BTreeSet::new());
    }

    pub(crate) fn mark(&mut self, category: &str, path: &Path) {
        self.next
            .entry(category.to_string())
            .or_default()
            .insert(normalize_managed_artifact_path(path));
    }

    /// Like [`mark`], but also records which source owns this path.
    /// Used by multi-source projection so per-source prune can
    /// distinguish "source removed from installed set" from "source absent this invocation".
    pub(crate) fn mark_with_source(&mut self, category: &str, path: &Path, source_id: &str) {
        let normalized = normalize_managed_artifact_path(path);
        self.next
            .entry(category.to_string())
            .or_default()
            .insert(normalized.clone());
        self.source_attribution_next
            .insert(normalized, source_id.to_string());
    }

    /// Return the source id that owned `path` according to the prior lockfile.
    /// Returns `None` when attribution was not recorded (single-source or pre-attribution lockfile).
    pub(crate) fn owning_source_of(&self, path: &Path) -> Option<&str> {
        let exact = normalize_managed_artifact_path(path);
        let skill = normalize_managed_artifact_path(&path.join("SKILL.md"));
        self.source_attribution_prior
            .get(&exact)
            .or_else(|| self.source_attribution_prior.get(&skill))
            .map(String::as_str)
    }

    fn prior_records_for_path(&self, path: &Path) -> Vec<PriorOwnershipRecord> {
        let candidates = [
            normalize_managed_artifact_path(path),
            normalize_managed_artifact_path(&path.join("SKILL.md")),
        ];
        let mut records = Vec::new();
        for normalized_path in candidates {
            let source_id = self.source_attribution_prior.get(&normalized_path).cloned();
            let mut matched_category = false;
            for (category, paths) in &self.prior {
                if paths.contains(&normalized_path) {
                    matched_category = true;
                    records.push(PriorOwnershipRecord {
                        category: Some(category.clone()),
                        path: normalized_path.clone(),
                        source_id: source_id.clone(),
                    });
                }
            }
            if !matched_category {
                if let Some(source_id) = source_id {
                    records.push(PriorOwnershipRecord {
                        category: None,
                        path: normalized_path,
                        source_id: Some(source_id),
                    });
                }
            }
        }
        records
    }

    pub(crate) fn persist(
        &self,
        dry_run: bool,
        report: &mut ProjectionReport,
    ) -> Result<(), AdapterError> {
        // Bootstrap a fresh root when no prior lockfile existed.
        let mut root = self.lockfile_root.clone().unwrap_or_default();
        if self.prior == self.next && self.source_attribution_prior == self.source_attribution_next
        {
            return Ok(());
        }

        let non_empty = self.next.values().any(|paths| !paths.is_empty());
        if non_empty {
            let mut meta = Map::new();
            meta.insert(
                "schemaVersion".into(),
                Value::Number(PROJECTION_LOCK_SCHEMA_VERSION.into()),
            );
            for category in [
                MANAGED_AGENT_PATHS,
                MANAGED_CODEX_AGENT_PATHS,
                MANAGED_COMMAND_PATHS,
                MANAGED_DISCUSS_SKILL_PATHS,
                MANAGED_LEGACY_PATHS,
                MANAGED_SKILL_PATHS,
            ] {
                let values = self
                    .next
                    .get(category)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(Value::String)
                    .collect();
                meta.insert(category.into(), Value::Array(values));
            }
            if !self.source_attribution_next.is_empty() {
                let attr_obj: Map<String, Value> = self
                    .source_attribution_next
                    .iter()
                    .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                    .collect();
                meta.insert(MANAGED_SOURCE_ATTRIBUTION.into(), Value::Object(attr_obj));
            }
            root.insert(PROJECTION_LOCK_KEY.into(), Value::Object(meta));
        } else {
            root.remove(PROJECTION_LOCK_KEY);
        }

        let rendered = serde_json::to_string_pretty(&Value::Object(root))
            .map_err(|err| AdapterError::Message(err.to_string()))?;
        if write_text(&self.lockfile_path, &rendered, dry_run)? {
            report.written_files.push(self.lockfile_path.clone());
        }
        Ok(())
    }
}

/// Normalize skill file content for drift comparison: unify CRLF/CR line
/// endings to LF and drop trailing whitespace, so a cosmetic line-ending or
/// trailing-newline difference between the projected copy and the canonical
/// source is not reported as staleness.
fn normalize_skill_content(content: &str) -> String {
    content
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end()
        .to_string()
}

impl SkillsProjectionHealthCheck {
    /// Back-compat constructor: only checks that the shared root itself exists
    /// and is a directory. Used where no dynamic inventory is available.
    pub fn with_path(shared_skills_root: PathBuf) -> Self {
        Self {
            shared_skills_root,
            required_names: Vec::new(),
            lockfile_path: None,
            canonical_source_root: None,
        }
    }

    /// Required-skill inventory constructor: `required_names` is the dynamic
    /// skill+command name list from the canonical root / projection source.
    /// `lockfile_path` enables the GAL-ownership check used to tell a real
    /// collision apart from a GAL-managed item.
    pub fn with_inventory(
        shared_skills_root: PathBuf,
        required_names: Vec<String>,
        lockfile_path: Option<PathBuf>,
    ) -> Self {
        Self {
            shared_skills_root,
            required_names,
            lockfile_path,
            canonical_source_root: None,
        }
    }

    /// Enable the canonical-source freshness comparison. `canonical_source_root`
    /// is the directory whose `<name>/SKILL.md` is the current projected source;
    /// a projected shared skill whose content differs is reported stale. Chainable
    /// so existing `with_inventory` / `with_path` call sites stay unchanged.
    pub fn with_canonical_source(mut self, canonical_source_root: PathBuf) -> Self {
        self.canonical_source_root = Some(canonical_source_root);
        self
    }

    /// Deletion discriminator: tells an externally-deleted item (still recorded
    /// in the prior GAL projection lockfile) apart from one GAL never projected
    /// or already pruned itself. `unknown` (rather than a guess) when no
    /// lockfile is available or it could not be read/parsed — never a false
    /// report.
    fn deletion_discriminator(
        &self,
        dir: &Path,
        registry: Option<&ManagedArtifactRegistry>,
    ) -> String {
        let Some(registry) = registry else {
            return "unknown — no projection lockfile available".to_string();
        };
        if !registry.is_managed(dir) {
            return "not projected / pruned".to_string();
        }
        match self
            .lockfile_path
            .as_deref()
            .and_then(|p| fs::metadata(p).ok())
            .and_then(|m| m.modified().ok())
        {
            Some(mtime) => format!(
                "external deletion (last GAL projection: {})",
                chrono::DateTime::<chrono::Local>::from(mtime).format("%Y-%m-%d %H:%M:%S")
            ),
            None => "external deletion (last GAL projection: time unknown)".to_string(),
        }
    }

    fn missing_finding(
        &self,
        name: &str,
        registry: Option<&ManagedArtifactRegistry>,
    ) -> DoctorFinding {
        let dir = self.shared_skills_root.join(name);
        let discriminator = self.deletion_discriminator(&dir, registry);
        let message = format!(
            "required GAL skill '{name}' is not projected at {} — {discriminator}",
            dir.display()
        );
        if WORKFLOW_CRITICAL_SKILLS.contains(&name) {
            DoctorFinding::error(message, "run `gal refresh`")
        } else {
            DoctorFinding::warning(message)
        }
    }

    fn registry(&self) -> Option<ManagedArtifactRegistry> {
        let lockfile_path = self.lockfile_path.as_ref()?;
        let mut throwaway = ProjectionReport::default();
        Some(ManagedArtifactRegistry::load(lockfile_path, &mut throwaway))
    }

    fn check_one(
        &self,
        name: &str,
        registry: Option<&ManagedArtifactRegistry>,
    ) -> Vec<DoctorFinding> {
        let dir = self.shared_skills_root.join(name);
        if !dir.exists() {
            return vec![self.missing_finding(name, registry)];
        }
        if is_symlink_or_junction(&dir) {
            return vec![DoctorFinding::warning(format!(
                "'{}' is still a legacy junction projection — run `gal refresh` to materialize it as a real directory",
                dir.display()
            ))];
        }

        let mut findings = Vec::new();
        let skill_file = dir.join("SKILL.md");
        match fs::read_to_string(&skill_file) {
            Ok(content) => {
                let frontmatter = parse_frontmatter(&content);
                if let Some(fm_name) = frontmatter.name.as_deref() {
                    if fm_name != name {
                        findings.push(DoctorFinding::warning(format!(
                            "'{}' frontmatter name '{}' does not match its directory name '{}'",
                            skill_file.display(),
                            fm_name,
                            name
                        )));
                    }
                }
                if let Some(stale) = self.stale_finding(name, &content) {
                    findings.push(stale);
                }
            }
            Err(_) => {
                findings.push(DoctorFinding::error(
                    format!("'{}' is missing or unreadable", skill_file.display()),
                    "run `gal refresh`",
                ));
            }
        }

        // Attribution is keyed differently by projection shape: a materialized
        // skill marks the directory path itself; a command-skill
        // (write_command_skill) marks the SKILL.md file path. Both are checked
        // — mirrors the same fix applied to remove_gal_command_skill.
        let is_gal_owned = is_gal_owned_real_path(&dir)
            || registry
                .map(|r| r.is_managed(&dir) || r.is_managed(&skill_file))
                .unwrap_or(false);
        if !is_gal_owned {
            findings.push(DoctorFinding::warning(format!(
                "'{}' exists but is not GAL-managed — a same-named user directory may be blocking projection",
                dir.display()
            )));
        }
        findings
    }

    /// Per-skill `description:` character contribution across every projected
    /// required item that has a readable `SKILL.md`, sorted largest-first. A
    /// missing/unreadable item or one with no description contributes nothing
    /// (its own finding already reports that separately).
    fn description_contributors(&self) -> Vec<(String, usize)> {
        let mut contributors: Vec<(String, usize)> = self
            .required_names
            .iter()
            .filter_map(|name| {
                let skill_file = self.shared_skills_root.join(name).join("SKILL.md");
                let content = fs::read_to_string(&skill_file).ok()?;
                let chars = parse_frontmatter(&content)
                    .description
                    .map(|d| d.chars().count())
                    .unwrap_or(0);
                (chars > 0).then_some((name.clone(), chars))
            })
            .collect();
        // Largest first; tie-break by name for deterministic output.
        contributors.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        contributors
    }

    /// Compare a projected shared skill's content against the canonical source
    /// `<canonical_source_root>/<name>/SKILL.md`. Returns a stale finding only
    /// when the canonical file exists and its normalized content differs — a
    /// missing canonical file is never a false stale report (freshness disabled
    /// for that item). Line endings + trailing whitespace are normalized so a
    /// CRLF/LF or trailing-newline difference is not mistaken for drift.
    fn stale_finding(&self, name: &str, projected: &str) -> Option<DoctorFinding> {
        let canonical_root = self.canonical_source_root.as_deref()?;
        // A projected shared skill can be sourced either from a flat
        // `<root>/<name>/SKILL.md` layout or from the canonical plugin root,
        // where skills live under `skills/<name>` and commands (projected as
        // skills for Codex, e.g. gal-pipeline) live under `commands/<name>`.
        // Take the first candidate that exists as the canonical source.
        let candidates = [
            canonical_root.join(name).join("SKILL.md"),
            canonical_root.join("skills").join(name).join("SKILL.md"),
            canonical_root.join("commands").join(name).join("SKILL.md"),
        ];
        let (canonical_file, canonical) = candidates
            .iter()
            .find_map(|p| fs::read_to_string(p).ok().map(|c| (p.clone(), c)))?;
        if normalize_skill_content(projected) == normalize_skill_content(&canonical) {
            return None;
        }
        Some(DoctorFinding::warning(format!(
            "projected skill '{name}' differs from current canonical source ({}) — the shared \
             projection is stale; run `gal refresh` (Codex: then restart / start a new thread)",
            canonical_file.display()
        )))
    }

    fn budget_finding(&self) -> Option<DoctorFinding> {
        let contributors = self.description_contributors();
        let total: usize = contributors.iter().map(|(_, c)| c).sum();
        if total < CODEX_SKILL_BUDGET_WARN {
            return None;
        }
        // Name the largest contributors so a user knows which skills to shorten first.
        let top = contributors
            .iter()
            .take(3)
            .map(|(name, chars)| format!("`{name}` ({chars} chars)"))
            .collect::<Vec<_>>()
            .join(", ");
        Some(DoctorFinding::warning(format!(
            "Codex skill description footprint is {total} chars, nearing the ~8,000-char \
             initial-list fallback context budget — Codex shortens descriptions and then \
             omits skills from the list; an omitted skill is still callable directly via \
             `$skill-name`. Largest contributors: {top}"
        )))
    }
}

impl HealthCheck for SkillsProjectionHealthCheck {
    fn name(&self) -> &str {
        "skills-projection"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        if !self.shared_skills_root.exists() {
            if self.required_names.is_empty() {
                return vec![DoctorFinding::warning(format!(
                    "shared skill projection not found: {} — run `gal refresh`",
                    self.shared_skills_root.display()
                ))];
            }
            let registry = self.registry();
            return self
                .required_names
                .iter()
                .map(|name| self.missing_finding(name, registry.as_ref()))
                .collect();
        }
        if !self.shared_skills_root.is_dir() {
            return vec![DoctorFinding::error(
                format!(
                    "shared skill projection is not a directory: {}",
                    self.shared_skills_root.display()
                ),
                "remove the path and rerun `gal refresh`",
            )];
        }

        let registry = self.registry();
        let mut findings: Vec<DoctorFinding> = self
            .required_names
            .iter()
            .flat_map(|name| self.check_one(name, registry.as_ref()))
            .collect();
        findings.extend(self.budget_finding());
        findings
    }
}

/// Normalize a Codex `[[skills.config]]` `path` value to a bare skill name for
/// comparison. Upstream documents this field inconsistently as either a skill
/// folder or its `SKILL.md` file — both shapes are accepted: the last path
/// component is taken after stripping a trailing `SKILL.md` segment.
fn normalize_config_disable_path(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches(['/', '\\']);
    let trimmed = trimmed
        .strip_suffix("/SKILL.md")
        .or_else(|| trimmed.strip_suffix("\\SKILL.md"))
        .unwrap_or(trimmed);
    Path::new(trimmed)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or(trimmed)
        .to_string()
}

/// Read-only, bounded scan of the **global** `~/.codex/config.toml`
/// `[[skills.config]]` table for an entry that disables (`enabled = false`) a
/// GAL-projected skill. Upstream project-local and sub-agent config variants
/// are documented but do not currently take effect (openai/codex#20210,
/// #14161), so only the global file has diagnostic value — this check is
/// deliberately scoped to it and does not attempt the broken variants.
pub struct CodexSkillsConfigCheck {
    config_path: PathBuf,
    gal_skill_names: Vec<String>,
}

impl CodexSkillsConfigCheck {
    pub fn new(config_path: PathBuf, gal_skill_names: Vec<String>) -> Self {
        Self {
            config_path,
            gal_skill_names,
        }
    }
}

impl HealthCheck for CodexSkillsConfigCheck {
    fn name(&self) -> &str {
        "codex-skills-config"
    }

    fn check(&self) -> Vec<DoctorFinding> {
        if !self.config_path.is_file() {
            return Vec::new();
        }
        let Ok(raw) = fs::read_to_string(&self.config_path) else {
            return Vec::new();
        };
        let parsed: toml::Value = match raw.parse() {
            Ok(value) => value,
            Err(err) => {
                return vec![DoctorFinding::warning(format!(
                    "could not parse '{}': {err}",
                    self.config_path.display()
                ))]
            }
        };

        let mut findings = Vec::new();

        // (1) `[[skills.config]]` enabled = false disable-scan.
        if let Some(entries) = parsed
            .get("skills")
            .and_then(|v| v.get("config"))
            .and_then(|v| v.as_array())
        {
            for entry in entries {
                let enabled = entry
                    .get("enabled")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                if enabled {
                    continue;
                }
                let Some(path) = entry.get("path").and_then(|v| v.as_str()) else {
                    continue;
                };
                let name = normalize_config_disable_path(path);
                if self.gal_skill_names.iter().any(|n| n == &name) {
                    findings.push(DoctorFinding::warning(format!(
                        "Codex skill '{name}' is disabled via `[[skills.config]]` (path = '{path}', \
                         enabled = false) in '{}' — note: project-local and sub-agent config variants \
                         do not currently take effect upstream, only the global config shown here does",
                        self.config_path.display()
                    )));
                }
            }
        }

        // (2) Codex Memories — advisory only. Accepts either `memory = true` or a
        // `[memory]` table with `enabled`. An ENABLED memory gets a risk advisory
        // distinguishing Codex's own store from GAL file memory; a disabled/absent
        // memory is deliberately NOT reported, so disabled Memories are never
        // blamed for GAL workflow drift (GAL authority is the repo `.dev/` files).
        if let Some(true) = codex_memory_enabled(&parsed) {
            findings.push(DoctorFinding::warning(
                "Codex Memories are enabled — advisory only: these are Codex's own memory store, \
                 NOT GAL file memory. GAL workflow authority is always the repo `.dev/` files \
                 (state.md + the active .prompt.md); do not rely on Codex Memories for workflow \
                 state, and do not enable them to fix GAL drift"
                    .to_string(),
            ));
        }

        // (3) `[mcp_servers.*]` — advisory only. Configured MCP servers are NOT the
        // same as graph/tools actually exposed in a session (capability-gated); this
        // context helps explain why structural-retrieval guidance may fall back.
        let mcp_servers = codex_mcp_server_names(&parsed);
        if !mcp_servers.is_empty() {
            findings.push(DoctorFinding::warning(format!(
                "Codex `[mcp_servers]` configures {} server(s): {} — advisory only: a configured \
                 MCP server is not the same as its graph/tools being exposed in a session; GAL \
                 guidance capability-gates on the tools actually available",
                mcp_servers.len(),
                mcp_servers.join(", ")
            )));
        }

        findings
    }
}

/// Read a Codex memory toggle. Accepts a top-level `memory = <bool>` or a
/// `[memory]` table carrying `enabled = <bool>`. Returns `None` when memory is
/// not configured at all (so an absent memory is never reported).
fn codex_memory_enabled(parsed: &toml::Value) -> Option<bool> {
    match parsed.get("memory")? {
        toml::Value::Boolean(b) => Some(*b),
        toml::Value::Table(t) => t.get("enabled").and_then(|v| v.as_bool()),
        _ => None,
    }
}

/// Collect the configured `[mcp_servers.*]` server names (sorted, deterministic).
fn codex_mcp_server_names(parsed: &toml::Value) -> Vec<String> {
    let mut names: Vec<String> = parsed
        .get("mcp_servers")
        .and_then(|v| v.as_table())
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    names
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Frontmatter {
    pub(crate) name: Option<String>,
    pub(crate) description: Option<String>,
    pub(crate) color: Option<String>,
    pub(crate) tools: Vec<String>,
}

pub(crate) fn parse_frontmatter(raw: &str) -> Frontmatter {
    let mut lines = raw.lines();
    if lines.next() != Some("---") {
        return Frontmatter::default();
    }

    let mut name = None;
    let mut description = None;
    let mut color = None;
    let mut tools = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            if key == "name" {
                name = Some(value.to_string());
            } else if key == "description" {
                description = Some(value.to_string());
            } else if key == "color" {
                color = Some(value.to_string());
            } else if key == "tools" {
                tools = value
                    .trim_matches(['[', ']'])
                    .split(',')
                    .map(|part| part.trim().trim_matches('"').trim_matches('\''))
                    .filter(|part| !part.is_empty())
                    .map(str::to_string)
                    .collect();
            }
        }
    }

    Frontmatter {
        name,
        description,
        color,
        tools,
    }
}

pub(crate) fn strip_frontmatter(raw: &str) -> String {
    let mut lines = raw.lines();
    if lines.next() != Some("---") {
        return raw.trim().to_string();
    }

    let mut body = String::new();
    let mut in_frontmatter = true;
    for line in raw.lines().skip(1) {
        if in_frontmatter {
            if line.trim() == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        body.push_str(line);
        body.push('\n');
    }
    body.trim().to_string()
}

pub(crate) fn render_opencode_agent(_name: &str, frontmatter: &Frontmatter, body: &str) -> String {
    let mut permissions = Vec::new();
    for tool in &frontmatter.tools {
        match tool.as_str() {
            "read" => {
                push_permission(&mut permissions, "read");
                push_permission(&mut permissions, "list");
            }
            "search" => {
                push_permission(&mut permissions, "read");
                push_permission(&mut permissions, "list");
                push_permission(&mut permissions, "grep");
                push_permission(&mut permissions, "glob");
            }
            "list" => {
                push_permission(&mut permissions, "list");
            }
            "edit" => {
                push_permission(&mut permissions, "edit");
            }
            "execute" => {
                push_permission(&mut permissions, "bash");
            }
            _ => {}
        }
    }

    if permissions.is_empty() {
        push_permission(&mut permissions, "read");
        push_permission(&mut permissions, "list");
    }

    let description = frontmatter
        .description
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "GAL golem agent".into());

    let mut lines = vec![
        GAL_MANAGED_FILE_HEADER.to_string(),
        "---".into(),
        "description: |".into(),
    ];
    for line in description.lines() {
        lines.push(format!("  {line}"));
    }
    lines.push("mode: subagent".into());
    if let Some(color) = &frontmatter.color {
        lines.push(format!("color: {color}"));
    }
    lines.push("permission:".into());
    for permission in permissions {
        lines.push(format!("  {permission}: allow"));
    }
    lines.push("---".into());
    lines.push(String::new());
    lines.push(body.trim().to_string());
    lines.push(String::new());
    lines.join("\n")
}

pub(crate) fn render_gemini_command(_name: &str, frontmatter: &Frontmatter, body: &str) -> String {
    let description = frontmatter
        .description
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "GAL command".into());
    let prompt = format!(
        "User command arguments, if any: {{{{args}}}}\n\n{}",
        body.trim()
    );
    format!(
        "{header}\ndescription = \"{description}\"\nprompt = '''\n{prompt}\n'''\n",
        header = GAL_MANAGED_FILE_HEADER,
        description = escape_toml_basic_string(&description),
        prompt = prompt.replace("'''", "\\'\\'\\'")
    )
}

pub(crate) fn render_opencode_command(name: &str, frontmatter: &Frontmatter, body: &str) -> String {
    let description = frontmatter
        .description
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "GAL command".into());
    let rendered_body = if name == "git-commit-msg" {
        "Use BASELINE below for the type(scope) prefix (it is a no-hijack path/status classifier that never reads the diff body), then write a subject describing what actually changed using PLANS's Goal for staged plan files and CHANGES's hunk headers for code files — do not read the raw diff yourself. Do not ship BASELINE's generic subject verbatim. Keep the type(scope) prefix unless the context clearly contradicts it. Add up to three body bullets only for broader changes. Do not add explanations, markdown fences, reasoning tags, JSON, or any extra prose. If the output reports No staged changes., return that text exactly. Apply extra instructions if provided: $ARGUMENTS\n\n!`gal commit-msg --context`".to_string()
    } else {
        body.trim().to_string()
    };

    let mut lines = vec![
        GAL_MANAGED_FILE_HEADER.to_string(),
        "---".into(),
        "description: |".into(),
    ];
    for line in description.lines() {
        lines.push(format!("  {line}"));
    }
    lines.push("---".into());
    lines.push(String::new());
    lines.push("User command arguments, if any: $ARGUMENTS".into());
    lines.push(String::new());
    lines.push(rendered_body);
    lines.push(String::new());
    lines.join("\n")
}

pub(crate) fn push_permission(permissions: &mut Vec<&'static str>, value: &'static str) {
    if !permissions.contains(&value) {
        permissions.push(value);
    }
}

pub(crate) fn list_named_children(root: &Path) -> Result<Vec<String>, AdapterError> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            names.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    names.sort();
    Ok(names)
}

pub(crate) fn list_files_with_extension(
    root: &Path,
    suffix: &str,
) -> Result<Vec<PathBuf>, AdapterError> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && entry
                .path()
                .file_name()
                .and_then(OsStr::to_str)
                .map(|name| name.ends_with(suffix))
                .unwrap_or(false)
        {
            files.push(entry.path());
        }
    }
    files.sort();
    Ok(files)
}

pub(crate) fn list_command_dirs(root: &Path) -> Result<Vec<PathBuf>, AdapterError> {
    if !root.is_dir() {
        return Ok(Vec::new());
    }
    let mut dirs = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        if path.join("SKILL.md").is_file() || path.join("SKILL.template.md").is_file() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

pub(crate) fn read_markdown_required(path: &Path) -> Result<String, AdapterError> {
    fs::read_to_string(path).map_err(AdapterError::from)
}

pub(crate) fn bake_command_content(
    command_dir: &Path,
    repo_root: &Path,
) -> Result<String, AdapterError> {
    let template = command_dir.join("SKILL.template.md");
    if !template.is_file() {
        return read_markdown_required(&command_dir.join("SKILL.md"));
    }

    let mut baked = read_markdown_required(&template)?;
    baked = baked.replace("{{GAL_ROOT}}", &repo_root.display().to_string());

    let local_override = command_dir.join("SKILL.local.md");
    if !local_override.is_file() {
        return Ok(baked);
    }

    let local = read_markdown_required(&local_override)?;
    if local.trim().is_empty() {
        return Ok(baked);
    }

    Ok(format!(
        "{}\n\n<!-- GAL LOCAL OVERRIDE START -->\n<!-- Source: SKILL.local.md (gitignored machine-local overlay) -->\n{}\n<!-- GAL LOCAL OVERRIDE END -->\n",
        baked.trim_end(),
        local.trim()
    ))
}

pub(crate) fn escape_toml_basic_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn prune_stale_links<'a>(
    root: &Path,
    keep_names: impl IntoIterator<Item = &'a str>,
    dry_run: bool,
    report: &mut ProjectionReport,
    registry: &mut ManagedArtifactRegistry,
    installed_source_ids: &[&str],
) -> Result<(), AdapterError> {
    if !root.is_dir() {
        return Ok(());
    }

    let keep: Vec<&str> = keep_names.into_iter().collect();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if keep.contains(&name) {
            continue;
        }

        let path = entry.path();
        let symlink = is_symlink_or_junction(&path);

        // Per-source guard: if the owning source is still in the installed set,
        // do not prune even when the name is absent from this invocation.
        // This prevents cross-source misfire when a source is temporarily absent.
        let prior_records = registry.prior_records_for_path(&path);
        let owning_source_installed = registry
            .owning_source_of(&path)
            .map(|src| installed_source_ids.contains(&src))
            .unwrap_or(false);
        if owning_source_installed {
            retain_prior_records(registry, &prior_records);
            continue;
        }

        // Authorize removal when the entry is positively GAL-identified by content
        // (header marker or golem-agent frontmatter — the a2 escape hatch, so even
        // a pre-lockfile orphan the fail-safe would otherwise protect is cleaned),
        // when it is a GAL-managed symlink the lockfile permits mutating,
        // when it is a plain file the prior lockfile explicitly recorded as written by GAL
        // (is_managed is false on first-run/empty-prior, preventing user-file deletion),
        // or when it is a materialized real directory the prior lockfile recorded (a
        // materialized skill copy carries no content marker of its own — attribution
        // is the only proof, same fail-safe semantics as the plain-file case).
        let authorized = is_gal_owned_real_path(&path)
            || (symlink && registry.can_mutate(&path))
            || (!symlink && !path.is_dir() && registry.is_managed(&path))
            || (!symlink && path.is_dir() && registry.is_managed(&path));
        if authorized {
            if !dry_run {
                if symlink {
                    remove_link(&path)?;
                } else if path.is_dir() {
                    fs::remove_dir_all(&path)?;
                } else {
                    fs::remove_file(&path)?;
                }
            }
            report.removed_paths.push(path);
        } else {
            retain_prior_records(registry, &prior_records);
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct PriorOwnershipRecord {
    category: Option<String>,
    path: String,
    source_id: Option<String>,
}

fn retain_prior_records(registry: &mut ManagedArtifactRegistry, records: &[PriorOwnershipRecord]) {
    for record in records {
        if let Some(category) = &record.category {
            registry
                .next
                .entry(category.clone())
                .or_default()
                .insert(record.path.clone());
        }
        if let Some(source_id) = &record.source_id {
            registry
                .source_attribution_next
                .insert(record.path.clone(), source_id.clone());
        }
    }
}

pub(crate) fn prune_stale_command_links(
    root: &Path,
    active_names: &[&str],
    dry_run: bool,
    report: &mut ProjectionReport,
    registry: &ManagedArtifactRegistry,
) -> Result<(), AdapterError> {
    if !root.is_dir() {
        return Ok(());
    }

    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if active_names.contains(&name)
            || !is_gal_command_link(&path, &path_needle_for_commands())
            || !registry.can_mutate(&path)
        {
            continue;
        }
        remove_link_if_present(&path, dry_run, report)?;
    }
    Ok(())
}

pub(crate) fn prune_legacy_gal_prefixed_dirs(
    root: &Path,
    active_names: &[&str],
    dry_run: bool,
    report: &mut ProjectionReport,
    registry: &ManagedArtifactRegistry,
) -> Result<(), AdapterError> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let Some(name) = path.file_name().and_then(OsStr::to_str) else {
            continue;
        };
        if !name.starts_with("gal-") || active_names.contains(&name) || !registry.can_mutate(&path)
        {
            continue;
        }
        if !dry_run {
            fs::remove_dir_all(&path)?;
        }
        report.removed_paths.push(path);
    }
    Ok(())
}

pub(crate) fn prune_stale_managed_files(
    root: &Path,
    extension: &str,
    active_names: &[&str],
    dry_run: bool,
    report: &mut ProjectionReport,
    registry: &ManagedArtifactRegistry,
) -> Result<(), AdapterError> {
    if !root.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_file() {
            continue;
        }
        let path = entry.path();
        let is_extension_match = path
            .extension()
            .and_then(OsStr::to_str)
            .map(|value| value.eq_ignore_ascii_case(extension))
            .unwrap_or(false);
        if !is_extension_match || !is_gal_managed_file(&path) || !registry.can_mutate(&path) {
            continue;
        }
        let Some(name) = path.file_stem().and_then(OsStr::to_str) else {
            continue;
        };
        if active_names.contains(&name) {
            continue;
        }
        remove_managed_file_if_present(&path, dry_run, report, registry)?;
    }
    Ok(())
}

pub(crate) fn ensure_dir(path: &Path, dry_run: bool) -> Result<(), AdapterError> {
    if dry_run || path.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(path)?;
    Ok(())
}

/// Project a baked command as a runtime skill at `<skills_root>/<name>/SKILL.md`.
///
/// A baked command (`SKILL.md` with `name`/`description` frontmatter) is already
/// a valid skill, so projection is a verbatim write — this exposes the GAL
/// command as a skill on runtimes whose capability unit is a skill, not a slash
/// command (Copilot `/gal-status`, Codex `$gal-status`).
///
/// Preserves a genuine user directory at the same name: a real (non-symlink)
/// directory that GAL never recorded (`!is_managed`, false even on first run) and
/// whose `SKILL.md` differs from our output is left untouched with a warning (the
/// a2 user-content guard). Returns the written path so the caller can record it in
/// the registry, or `None` when preserved.
pub(crate) fn write_command_skill(
    skills_root: &Path,
    name: &str,
    content: &str,
    dry_run: bool,
    report: &mut ProjectionReport,
    registry: &ManagedArtifactRegistry,
) -> Result<Option<PathBuf>, AdapterError> {
    let dir = skills_root.join(name);
    let skill_file = dir.join("SKILL.md");

    if dir.exists() && !is_symlink_or_junction(&dir) && !registry.is_managed(&skill_file) {
        let normalized = format!("{}\n", content.trim_end());
        let is_ours = fs::read_to_string(&skill_file)
            .ok()
            .map(|existing| existing == normalized)
            .unwrap_or(false);
        if !is_ours {
            report.warnings.push(ProjectionWarning {
                message: format!("preserved user-owned path: {}", dir.display()),
            });
            return Ok(None);
        }
    }

    if is_symlink_or_junction(&dir) && !dry_run {
        remove_link(&dir)?;
    }
    if write_text(&skill_file, content, dry_run)? {
        report.written_files.push(skill_file.clone());
    }
    Ok(Some(skill_file))
}

/// Remove a previously-projected command-skill or materialized skill at
/// `<skills_root>/<name>` when it is no longer active, so it does not linger in
/// a shared skills dir (`~/.agents/skills`, read by codex+opencode+copilot) and
/// reintroduce a double-load or leak as a zombie. Removes a legacy symlink, or a
/// GAL-written real directory — authorized either by lockfile attribution (a
/// command-skill marks the `SKILL.md` file path; a materialized skill marks the
/// directory path itself, so both are checked) or by a positive GAL content
/// marker (the a2 escape hatch, same as [`prune_stale_links`]). A genuine user
/// directory (neither attributed nor marked) is always left untouched.
pub(crate) fn remove_gal_command_skill(
    skills_root: &Path,
    name: &str,
    repo_root: &Path,
    registry: &ManagedArtifactRegistry,
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<(), AdapterError> {
    let dir = skills_root.join(name);
    if is_symlink_or_junction(&dir) {
        // Only a GAL-owned repo-link is removed; a user's own symlink (e.g. to an
        // external skill) is preserved.
        if is_gal_repo_link(&dir, repo_root) {
            remove_link_if_present(&dir, dry_run, report)?;
        }
    } else if dir.is_dir()
        && (registry
            .prior_records_for_path(&dir)
            .iter()
            .any(|record| record.category.is_some())
            || is_gal_owned_real_path(&dir))
    {
        if !dry_run {
            fs::remove_dir_all(&dir)?;
        }
        report.removed_paths.push(dir);
    }
    Ok(())
}

pub(crate) fn write_text(path: &Path, content: &str, dry_run: bool) -> Result<bool, AdapterError> {
    let normalized = format!("{}\n", content.trim_end());
    if fs::read_to_string(path)
        .ok()
        .map(|existing| existing == normalized)
        .unwrap_or(false)
    {
        return Ok(false);
    }
    if !dry_run {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, normalized.as_bytes())?;
    }
    Ok(true)
}

pub(crate) fn remove_link_if_present(
    path: &Path,
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<(), AdapterError> {
    if !is_symlink_or_junction(path) {
        return Ok(());
    }
    if !dry_run {
        remove_link(path)?;
    }
    report.removed_paths.push(path.to_path_buf());
    Ok(())
}

pub(crate) fn remove_managed_file_if_present(
    path: &Path,
    dry_run: bool,
    report: &mut ProjectionReport,
    registry: &ManagedArtifactRegistry,
) -> Result<(), AdapterError> {
    if !is_gal_managed_file(path) || !registry.can_mutate(path) {
        return Ok(());
    }
    if !dry_run {
        fs::remove_file(path)?;
    }
    report.removed_paths.push(path.to_path_buf());
    Ok(())
}

pub(crate) fn ensure_dir_link(
    target: &Path,
    link: &Path,
    replace: bool,
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<(), AdapterError> {
    if same_path(link, target) {
        return Ok(());
    }
    if is_symlink_or_junction(link) {
        if dry_run {
            report.created_links.push(link.to_path_buf());
            return Ok(());
        }
        remove_dir_link(link)?;
    } else if link.exists() {
        // Adopt a GAL-owned real path (file→symlink migration) without requiring
        // --replace; preserve only genuine user-owned paths (no GAL content marker
        // and not a GAL-projected golem agent).
        if !replace && !is_gal_owned_real_path(link) {
            report.warnings.push(ProjectionWarning {
                message: format!("preserved user-owned path: {}", link.display()),
            });
            return Ok(());
        }
        let backup = backup_path(link);
        if !dry_run {
            if backup.exists() {
                if backup.is_dir() {
                    fs::remove_dir_all(&backup)?;
                } else {
                    fs::remove_file(&backup)?;
                }
            }
            fs::rename(link, &backup)?;
        }
        report.removed_paths.push(backup);
    }
    if !dry_run {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)?;
        }
        create_dir_link(target, link)?;
    }
    report.created_links.push(link.to_path_buf());
    Ok(())
}

pub(crate) fn same_path(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

pub(crate) fn backup_path(path: &Path) -> PathBuf {
    let name = path
        .file_name()
        .and_then(OsStr::to_str)
        .map(|value| format!("{value}.bak"))
        .unwrap_or_else(|| "backup.bak".into());
    path.with_file_name(name)
}

pub(crate) fn is_gal_managed_file(path: &Path) -> bool {
    fs::read_to_string(path)
        .ok()
        .and_then(|content| content.lines().next().map(str::to_string))
        .map(|first| first == GAL_MANAGED_FILE_HEADER)
        .unwrap_or(false)
}

/// Content-marker test for an existing real (non-symlink) path during adopt.
///
/// A file is GAL-managed when its first line is [`GAL_MANAGED_FILE_HEADER`].
/// A directory is GAL-managed when it shallowly contains at least one
/// GAL-managed file — this lets the projection re-adopt a directory a prior
/// projection era materialized as real files (file→symlink migration) without
/// ever adopting a genuine user directory (which carries no GAL header).
pub(crate) fn is_gal_managed_path(path: &Path) -> bool {
    if path.is_dir() {
        fs::read_dir(path)
            .ok()
            .map(|entries| {
                entries.flatten().any(|entry| {
                    let child = entry.path();
                    child.is_file() && is_gal_managed_file(&child)
                })
            })
            .unwrap_or(false)
    } else {
        is_gal_managed_file(path)
    }
}

/// Does this file look like a GAL-projected golem agent?
///
/// Agent projections are verbatim copies of `plugins/gal-core/agents/<name>.agent.md`:
/// YAML frontmatter whose `name:` value is a `golem-*` agent. They carry **no**
/// [`GAL_MANAGED_FILE_HEADER`] (the first line is `---`), so the header check
/// misses them. Detect by the golem-agent frontmatter signature instead — a
/// content identity, not a path/directory exclusion.
pub(crate) fn is_gal_projected_agent_file(path: &Path) -> bool {
    let Ok(content) = fs::read_to_string(path) else {
        return false;
    };
    let mut lines = content.lines();
    if lines.next() != Some("---") {
        return false;
    }
    for line in lines.take(15) {
        if line == "---" {
            break;
        }
        if let Some(rest) = line.strip_prefix("name:") {
            return rest
                .trim()
                .trim_matches(|c| c == '"' || c == '\'')
                .starts_with("golem-");
        }
    }
    false
}

/// Positive GAL-ownership test for an existing real path, used by both adopt
/// ([`ensure_dir_link`]) and prune ([`prune_stale_links`]).
///
/// True when the path carries a GAL content marker ([`is_gal_managed_path`]) or
/// is a GAL-projected golem agent ([`is_gal_projected_agent_file`]). This is the
/// sole criterion for re-adopting / pruning a GAL real-file residue: it is a
/// positive content identification (the sanctioned a2 escape hatch), never a
/// path/directory exclusion and never an unconditional overwrite.
pub(crate) fn is_gal_owned_real_path(path: &Path) -> bool {
    is_gal_managed_path(path) || (path.is_file() && is_gal_projected_agent_file(path))
}

pub(crate) fn is_gal_repo_link(path: &Path, repo_root: &Path) -> bool {
    if !is_symlink_or_junction(path) {
        return false;
    }
    match (fs::canonicalize(path), fs::canonicalize(repo_root)) {
        (Ok(target), Ok(root)) => target.starts_with(root),
        _ => false,
    }
}

pub(crate) fn path_needle_for_commands() -> String {
    format!(
        "{}commands{}",
        std::path::MAIN_SEPARATOR,
        std::path::MAIN_SEPARATOR
    )
}

pub(crate) fn is_gal_command_link(path: &Path, repo_root_fragment: &str) -> bool {
    if !is_symlink_or_junction(path) {
        return false;
    }
    fs::canonicalize(path)
        .ok()
        .and_then(|target| target.to_str().map(str::to_string))
        .map(|target| target.contains(repo_root_fragment))
        .unwrap_or(false)
}

pub(crate) fn remove_link(path: &Path) -> io::Result<()> {
    // A directory junction / dir-symlink reports `is_dir() == false` via
    // `symlink_metadata` on Windows (a reparse point is a symlink, not a dir), so
    // gating link removal on `is_dir()` sent directory junctions to `fs::remove_file`
    // → `DeleteFile` refuses a directory reparse point with ERROR_ACCESS_DENIED
    // (os error 5). Route any reparse point through `remove_dir_link` (rmdir — removes
    // the link, not the target), falling back to `remove_file` for a file symlink.
    if is_symlink_or_junction(path) {
        return remove_dir_link(path).or_else(|_| fs::remove_file(path));
    }
    let metadata = path.symlink_metadata()?;
    if metadata.file_type().is_dir() {
        remove_dir_link(path)
    } else {
        fs::remove_file(path)
    }
}

pub(crate) fn remove_if_gal_owned_dir(
    path: &Path,
    dry_run: bool,
    report: &mut ProjectionReport,
    registry: &ManagedArtifactRegistry,
) -> Result<(), AdapterError> {
    if !path.exists() {
        return Ok(());
    }
    if !registry.can_mutate(path) {
        return Ok(());
    }
    if is_symlink_or_junction(path) {
        if !dry_run {
            remove_dir_link(path)?;
        }
        report.removed_paths.push(path.to_path_buf());
        return Ok(());
    }
    if path.is_dir() {
        let marker = path.join(".gal-managed");
        if marker.exists() || path.file_name().and_then(OsStr::to_str) == Some("skills") {
            if !dry_run {
                fs::remove_dir_all(path)?;
            }
            report.removed_paths.push(path.to_path_buf());
        }
    }
    Ok(())
}

fn load_managed_artifact_set(meta: &Map<String, Value>, category: &str) -> BTreeSet<String> {
    meta.get(category)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn normalize_managed_artifact_path(path: &Path) -> String {
    let rendered = path.to_string_lossy().replace('/', "\\");
    #[cfg(windows)]
    {
        rendered.to_ascii_lowercase()
    }
    #[cfg(not(windows))]
    {
        rendered
    }
}

/// Recursively copy a skill directory from `source` to `target`, writing files
/// only when content differs (byte-exact comparison per file).
///
/// Returns `true` when at least one file was written (i.e. the target changed),
/// `false` when every file was already byte-identical (idempotent second call).
///
/// If `target` is a symlink or NTFS junction (ReparsePoint) it is removed first
/// so the copy lands in a real directory. Ownership decisions are the caller's
/// responsibility — this helper copies unconditionally.
///
/// When `dry_run` is `true` no filesystem writes occur; the return value reflects
/// what would have been written.
pub(crate) fn materialize_skill_dir(
    source: &Path,
    target: &Path,
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<bool, AdapterError> {
    if is_symlink_or_junction(target) {
        report.removed_paths.push(target.to_path_buf());
        if !dry_run {
            remove_link(target)?;
        } else {
            // In dry_run the junction would be replaced — that is definitely a change.
            return Ok(true);
        }
    }
    copy_dir_recursive(source, target, dry_run, report)
}

fn copy_dir_recursive(
    source: &Path,
    target: &Path,
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<bool, AdapterError> {
    if !dry_run {
        fs::create_dir_all(target)?;
    }
    let mut any_written = false;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let src_path = entry.path();
        let tgt_path = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            if copy_dir_recursive(&src_path, &tgt_path, dry_run, report)? {
                any_written = true;
            }
        } else if copy_file_if_changed(&src_path, &tgt_path, dry_run, report)? {
            any_written = true;
        }
    }
    Ok(any_written)
}

fn copy_file_if_changed(
    src: &Path,
    dst: &Path,
    dry_run: bool,
    report: &mut ProjectionReport,
) -> Result<bool, AdapterError> {
    let src_bytes = fs::read(src)?;
    if dst.is_file() {
        if let Ok(dst_bytes) = fs::read(dst) {
            if dst_bytes == src_bytes {
                return Ok(false);
            }
        }
    }
    if !dry_run {
        if let Some(parent) = dst.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(dst, &src_bytes)?;
        report.written_files.push(dst.to_path_buf());
    }
    Ok(true)
}
