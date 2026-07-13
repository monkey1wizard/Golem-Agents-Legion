//! Per-runtime plugin manifest + instruction-corpus renderers.

use super::*;
use projection::tool_map::{abstract_tools_to_claude_names, InvocationMode};
use std::fs;
use std::path::Path;

/// Filter agent frontmatter for Claude compatibility.
///
/// Keeps only Claude-compatible keys: name, description, model, effort, maxTurns,
/// tools, disallowedTools, skills, memory, background, isolation (if worktree).
///
/// The `tools:` value is converted from GAL abstract tokens (`read`, `execute`,
/// `search`, `edit`, `web`) to Claude Code valid tool names using the single
/// mapping source in `projection::tool_map`. Build-mode expansion is used because
/// agent registration does not lock write access — the runtime permission is
/// determined by the subagent invocation mode at call time.
///
/// The `name: golem-*` frontmatter line is preserved verbatim; it is the identity
/// key used by the adopt/prune mechanism.
pub(crate) fn filter_agent_for_claude(agent_path: &Path) -> Result<String, RenderError> {
    let content = fs::read_to_string(agent_path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.len() < 3 || lines[0] != "---" {
        // No frontmatter, return as-is
        return Ok(content);
    }

    // Find closing ---
    let mut closing_idx = None;
    for (i, line) in lines.iter().enumerate().skip(1) {
        if *line == "---" {
            closing_idx = Some(i);
            break;
        }
    }

    let closing_idx = match closing_idx {
        Some(idx) => idx,
        None => return Ok(content), // No closing, return as-is
    };

    let allowed_keys = [
        "name",
        "description",
        "model",
        "effort",
        "maxTurns",
        "tools",
        "disallowedTools",
        "skills",
        "memory",
        "background",
        "isolation",
    ];

    let mut filtered_frontmatter = Vec::new();
    for line in &lines[1..closing_idx] {
        if let Some(colon_pos) = line.find(':') {
            let key = line[..colon_pos].trim();
            if !allowed_keys.contains(&key) {
                continue;
            }
            if key == "isolation" {
                let value = line[colon_pos + 1..].trim();
                if value == "worktree" || value == "\"worktree\"" || value == "'worktree'" {
                    filtered_frontmatter.push(line.to_string());
                }
            } else if key == "tools" {
                // Convert abstract GAL tool tokens to Claude Code valid tool names.
                // Build-mode is used for registration; consult-mode clamping is applied
                // by the caller when launching a subagent in consult context.
                let raw_value = line[colon_pos + 1..].trim();
                let abstract_tokens: Vec<&str> = raw_value
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                let claude_names =
                    abstract_tools_to_claude_names(&abstract_tokens, InvocationMode::Build);
                if claude_names.is_empty() {
                    // No valid tokens — omit the tools line rather than emitting `tools: []`.
                    continue;
                }
                let joined = claude_names.join(", ");
                filtered_frontmatter.push(format!("tools: [{joined}]"));
            } else {
                filtered_frontmatter.push(line.to_string());
            }
        }
    }

    let body = &lines[closing_idx + 1..];
    let mut result = Vec::new();
    result.push("---".to_string());
    result.extend(filtered_frontmatter);
    result.push("---".to_string());
    result.extend(body.iter().map(|s| s.to_string()));

    Ok(result.join("\n"))
}

/// Render Claude plugin manifest (.claude-plugin/plugin.json).
pub(crate) fn render_claude_plugin_manifest(
    claude_plugin_dir: &Path,
    _components: &ScannedComponents,
) -> Result<(), RenderError> {
    // The `agents`/`skills`/`commands` manifest fields are PATHS in Claude's schema,
    // not component-name lists. Emitting name arrays makes Claude resolve each name as a
    // path, fail ("Path not found"), and reject the whole manifest so the plugin never
    // loads (verified with `claude plugin validate`). Omit them entirely: the plugin
    // already ships `agents/`, `skills/`, and `commands/` directories at its root, which
    // Claude auto-discovers. No static `version` (optional; absence is a cosmetic
    // validator warning only, never load-blocking).
    let manifest_json = serde_json::json!({
        "name": "gal",
        "description": "GAL - Golem Agents Legion",
    });

    let manifest_path = claude_plugin_dir.join("plugin.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json)?,
    )?;

    Ok(())
}

/// Render Codex plugin manifest (.codex-plugin/plugin.json).
pub(crate) fn render_codex_plugin_manifest(codex_plugin_dir: &Path) -> Result<(), RenderError> {
    let manifest_json = serde_json::json!({
        "name": "gal",
        "version": generate_plugin_version(),
        "description": "GAL - Golem Agents Legion for Codex"
    });

    let manifest_path = codex_plugin_dir.join("plugin.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json)?,
    )?;

    Ok(())
}

/// Generate a plugin version string: 1.0.0-timestamp.hash
/// Format matches the PowerShell oracle: timestamp from current time, hash from package content.
/// For initial implementation, we use a simplified version with timestamp + UUID-based suffix.
pub(crate) fn generate_plugin_version() -> String {
    let formatted_time = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
    let uuid_suffix = uuid::Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(8)
        .collect::<String>();

    format!("1.0.0-{}.{}", formatted_time, uuid_suffix)
}

/// Render Copilot manifest (copilot-manifest.json at root).
/// Copilot will not load commands/ unless the path is explicitly defined
/// in a root-level manifest.
pub(crate) fn render_copilot_manifest(
    temp_dir: &Path,
    _components: &ScannedComponents,
) -> Result<(), RenderError> {
    // Generate version string: 1.0.0-timestamp.hash
    let version = generate_plugin_version();

    let manifest_json = serde_json::json!({
        "name": "gal",
        "displayName": "Golem Agents Legion",
        "version": version,
        "description": "Golem Agents Legion plugin for GitHub Copilot CLI",
        "components": {
            "agents": "agents/",
            "skills": "skills/",
            "commands": "commands/",
            "mcpConfig": ".mcp.json"
        }
    });

    let manifest_path = temp_dir.join("copilot-manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json)?,
    )?;

    Ok(())
}

/// Render AGY plugin manifest (plugin.json at root).
pub(crate) fn render_agy_plugin_manifest(
    temp_dir: &Path,
    components: &ScannedComponents,
) -> Result<(), RenderError> {
    // No static `version`: agy reads this plugin live via a junction to the canonical
    // root and does not version-cache, so a hardcoded version is misleading. Kept
    // absent for honesty + parity with the codex/copilot dynamic-version intent.
    let manifest_json = serde_json::json!({
        "name": "gal",
        "description": "GAL - Golem Agents Legion",
        "agents": components.agents.iter().map(|a| format!("agy-agents/{}.agent.md", a.name)).collect::<Vec<_>>(),
        "skills": components.skills.iter().map(|s| format!("skills/{}", s.name)).collect::<Vec<_>>(),
    });

    let manifest_path = temp_dir.join("plugin.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json)?,
    )?;

    Ok(())
}

/// Render instruction corpus (rules/gal.md) from conventions/ and workflows/.
pub(crate) fn render_instruction_corpus(
    rules_dir: &Path,
    source_root: &Path,
) -> Result<(), RenderError> {
    let mut corpus = String::new();
    corpus.push_str("# GAL Instruction Corpus\n\n");
    corpus.push_str("## Conventions\n\n");

    // Collect conventions
    let conventions_dir = source_root.join("conventions");
    if conventions_dir.exists() {
        for entry in fs::read_dir(&conventions_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".md") {
                        let content = fs::read_to_string(entry.path())?;
                        corpus.push_str(&format!("### {}\n\n", name));
                        corpus.push_str(&content);
                        corpus.push_str("\n\n");
                    }
                }
            }
        }
    }

    corpus.push_str("## Workflows\n\n");

    // Collect workflows
    let workflows_dir = source_root.join("workflows");
    if workflows_dir.exists() {
        for entry in fs::read_dir(&workflows_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".md") {
                        let content = fs::read_to_string(entry.path())?;
                        corpus.push_str(&format!("### {}\n\n", name));
                        corpus.push_str(&content);
                        corpus.push_str("\n\n");
                    }
                }
            }
        }
    }

    let corpus_path = rules_dir.join("gal.md");
    fs::write(&corpus_path, corpus)?;

    Ok(())
}
