//! Per-runtime plugin manifest + instruction-corpus renderers.

use super::*;
use projection::tool_map::{abstract_tools_to_claude_names, InvocationMode};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
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
    version_stamp: &str,
) -> Result<(), RenderError> {
    // The `agents`/`skills`/`commands` manifest fields are PATHS in Claude's schema,
    // not component-name lists. Emitting name arrays makes Claude resolve each name as a
    // path, fail ("Path not found"), and reject the whole manifest so the plugin never
    // loads (verified with `claude plugin validate`). Omit them entirely: the plugin
    // already ships `agents/`, `skills/`, and `commands/` directories at its root, which
    // Claude auto-discovers. `version` carries the same content-digest stamp
    // (`<gal_version>-g<digest>`) as the Codex and root manifests — computed once
    // per render in `render_to_temp` and passed in here rather than recomputed.
    let manifest_json = serde_json::json!({
        "name": "gal",
        "version": version_stamp,
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
pub(crate) fn render_codex_plugin_manifest(
    codex_plugin_dir: &Path,
    version_stamp: &str,
    hooks: &serde_json::Value,
) -> Result<(), RenderError> {
    let manifest_json = serde_json::json!({
        "name": "gal",
        "version": version_stamp,
        "description": "GAL - Golem Agents Legion for Codex",
        "author": {
            "name": "GAL"
        },
        "interface": {
            "displayName": "Golem Agents Legion",
            "shortDescription": "GAL workflow commands and golem agents"
        },
        "hooks": hooks
    });

    let manifest_path = codex_plugin_dir.join("plugin.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json)?,
    )?;

    Ok(())
}

/// Generate the plugin version stamp: `<gal_version>-g<first-12-lowercase-hex>`
/// of the portable-render content digest rooted at `render_root`.
///
/// `gal_version` is the running GAL binary's semver (for example
/// `env!("CARGO_PKG_VERSION")`), passed in rather than hardcoded so every
/// manifest rendered by one build carries the same version prefix.
///
/// Personal skill and personal MCP server names are resolved from the same
/// machine-local paths `merge_personal_skills`/`merge_personal_mcp` read
/// (`gal_foundation::paths::gal_local_skills_root` /
/// `gal_local_mcp_path`), so the digest feeding the stamp excludes exactly
/// the content those merges may have added. A portable source therefore
/// yields the same stamp whether it is rendered on a machine with no
/// personal layer or one with a populated personal layer.
///
/// `core_skill_names` must be the skill set scanned from the source **before**
/// `merge_personal_skills` runs, and core MCP server names are read from the
/// source `mcp.json`. Both merges are core-wins, so a personal entry whose name
/// collides with a core one never enters the render tree — see
/// [`personal_only_names`] for why excluding it anyway would corrupt the stamp.
pub(crate) fn generate_plugin_version(
    gal_version: &str,
    render_root: &Path,
    source_root: &Path,
    core_skill_names: &HashSet<String>,
    codex_hooks_bytes: &[u8],
) -> String {
    let exclude_skill_names = personal_only_names(&personal_skill_names(), core_skill_names);
    let exclude_mcp_server_names = personal_only_names(
        &personal_mcp_server_names(),
        &core_mcp_server_names(source_root),
    );
    let digest = compute_portable_digest_with_codex_hooks(
        render_root,
        &exclude_skill_names,
        &exclude_mcp_server_names,
        codex_hooks_bytes,
    );
    let short: String = digest.chars().take(12).collect();
    format!("{gal_version}-g{short}")
}

fn compute_portable_digest_with_codex_hooks(
    render_root: &Path,
    exclude_skill_names: &HashSet<String>,
    exclude_mcp_server_names: &HashSet<String>,
    codex_hooks_bytes: &[u8],
) -> String {
    let base = compute_portable_digest(render_root, exclude_skill_names, exclude_mcp_server_names);
    let mut hasher = Sha256::new();
    hasher.update(base.as_bytes());
    hasher.update([0]);
    hasher.update(b"codex/hooks.json");
    hasher.update([0]);
    hasher.update(normalize_to_lf(codex_hooks_bytes));
    hex::encode(hasher.finalize())
}

/// Names present in `personal` but absent from `core`.
///
/// `merge_personal_skills` and `merge_personal_mcp` are both core-wins: a
/// personal entry whose name collides with a core one is skipped and never
/// reaches the render tree. Excluding such a name from the digest would strip
/// the **core** entry instead, so the stamp would depend on whether the
/// rendering machine happens to have a personal entry of that name — the exact
/// guarantee the stamp exists to provide. Only names the personal layer
/// genuinely added belong in an exclusion set.
fn personal_only_names(personal: &HashSet<String>, core: &HashSet<String>) -> HashSet<String> {
    personal.difference(core).cloned().collect()
}

/// MCP server names declared by the portable source `mcp.json` (`{"servers": …}`).
/// These are the core names `merge_personal_mcp` protects under core-wins. An
/// absent or unparseable source file yields an empty set (fail-safe).
fn core_mcp_server_names(source_root: &Path) -> HashSet<String> {
    let Ok(content) = fs::read_to_string(source_root.join("mcp.json")) else {
        return HashSet::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return HashSet::new();
    };
    value
        .get("servers")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

/// Skill directory names present under the machine-local personal skills root
/// (`~/.gal/local/skills`). Mirrors the directory scan in `merge_personal_skills`
/// without mutating any render state — used only to exclude personal skill
/// content from the version-stamp digest. An absent or unreadable root yields
/// an empty set (fail-safe, matches `merge_personal_skills`).
fn personal_skill_names() -> HashSet<String> {
    let Some(root) = gal_foundation::paths::gal_local_skills_root().filter(|p| p.exists()) else {
        return HashSet::new();
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return HashSet::new();
    };
    entries
        .flatten()
        .filter(|entry| entry.path().join("SKILL.md").exists())
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

/// MCP server names present in the machine-local personal MCP file
/// (`~/.gal/local/mcp.json`). Mirrors the server-name read in
/// `merge_personal_mcp` without mutating any render state — used only to
/// exclude personal MCP server entries from the version-stamp digest. An
/// absent file or parse failure yields an empty set (fail-safe, matches
/// `merge_personal_mcp`).
fn personal_mcp_server_names() -> HashSet<String> {
    let Some(path) = gal_foundation::paths::gal_local_mcp_path().filter(|p| p.exists()) else {
        return HashSet::new();
    };
    let Ok(content) = fs::read_to_string(&path) else {
        return HashSet::new();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) else {
        return HashSet::new();
    };
    value
        .get("servers")
        .and_then(|v| v.as_object())
        .map(|obj| obj.keys().cloned().collect())
        .unwrap_or_default()
}

/// True for the fixed relative path (forward-slash) of one of the three
/// stamped manifests — excluded from the digest because they embed the very
/// stamp this digest produces.
fn is_stamped_manifest(relative: &str) -> bool {
    matches!(
        relative,
        ".claude-plugin/plugin.json" | ".codex-plugin/plugin.json" | "plugin.json"
    )
}

/// True when `relative` (forward-slash) is the top-level directory
/// `dir_name` itself, or a path under it.
fn is_under_top_level_dir(relative: &str, dir_name: &str) -> bool {
    relative == dir_name || relative.starts_with(&format!("{dir_name}/"))
}

/// True when `relative` (forward-slash) is `skills/<name>/...` for a `<name>`
/// present in `exclude_skill_names`.
fn is_excluded_skill(relative: &str, exclude_skill_names: &HashSet<String>) -> bool {
    let Some(rest) = relative.strip_prefix("skills/") else {
        return false;
    };
    let Some(skill_name) = rest.split('/').next() else {
        return false;
    };
    exclude_skill_names.contains(skill_name)
}

/// Recursively collect every file under `current` as a path relative to
/// `root`. An unreadable directory contributes no entries (fail-safe — the
/// digest is best-effort over whatever the render tree actually contains).
fn collect_relative_files(root: &Path, current: &Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            collect_relative_files(root, &path, out);
        } else if file_type.is_file() {
            if let Ok(rel) = path.strip_prefix(root) {
                out.push(rel.to_path_buf());
            }
        }
    }
}

/// Byte-level CRLF -> LF normalization. Operates on raw bytes (not `str`) so
/// it never assumes UTF-8 content.
fn normalize_to_lf(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\r' && i + 1 < bytes.len() && bytes[i + 1] == b'\n' {
            out.push(b'\n');
            i += 2;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    out
}

/// Parse `bytes` as the rendered `mcp.json`/`.mcp.json` shape
/// (`{"mcpServers": {...}}`), remove every key in `exclude_names` from the
/// `mcpServers` map, and re-serialize through one fixed serializer.
///
/// The re-serialization is unconditional: it runs even when `exclude_names` is
/// empty and even when no name matched. Hashing raw file bytes in one case and
/// re-serialized bytes in another would make the digest depend on the file's
/// formatting rather than its content, so the same portable source would stamp
/// differently on a machine with a personal MCP layer (re-serialized) than on
/// one without (raw) — the render writes these files pretty-printed, while the
/// re-serialization is compact.
///
/// Returns `None` (caller falls back to the original bytes) only on parse
/// failure — fail-safe, matches the rest of the merge/digest machinery.
fn canonicalize_mcp_json(bytes: &[u8], exclude_names: &HashSet<String>) -> Option<Vec<u8>> {
    let mut value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if let Some(servers) = value.get_mut("mcpServers").and_then(|v| v.as_object_mut()) {
        for name in exclude_names {
            servers.remove(name);
        }
    }
    serde_json::to_vec(&value).ok()
}

/// Compute a stable content digest of a portable GAL render tree rooted at
/// `render_root`: sorted relative paths (forward slashes, for cross-platform
/// stability), file bytes with CRLF normalized to LF, folded into a SHA-256
/// hash and returned as lowercase hex.
///
/// Excluded from the walk:
/// - the three stamped manifests (`.claude-plugin/plugin.json`,
///   `.codex-plugin/plugin.json`, root `plugin.json`).
/// - the entire top-level `bin/` directory (the host-OS-native `gal` binary,
///   present only on machine-local installs).
/// - `skills/<name>/` for every `<name>` in `exclude_skill_names`.
/// - the `mcpServers` entries named in `exclude_mcp_server_names`, stripped
///   from `mcp.json` and `.mcp.json` before hashing. Those two files are
///   always re-serialized through one fixed serializer before hashing, with
///   or without exclusions, so their contribution to the digest depends on
///   their content and never on their on-disk formatting.
///
/// Given the same render tree and the same exclusion sets, this is
/// deterministic. A single changed byte in any included file, or an
/// unexcluded added/removed file, changes the digest.
pub(crate) fn compute_portable_digest(
    render_root: &Path,
    exclude_skill_names: &HashSet<String>,
    exclude_mcp_server_names: &HashSet<String>,
) -> String {
    let mut relative_paths = Vec::new();
    collect_relative_files(render_root, render_root, &mut relative_paths);

    let mut entries: Vec<(String, std::path::PathBuf)> = relative_paths
        .into_iter()
        .map(|rel| {
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            (rel_str, rel)
        })
        .filter(|(rel_str, _)| !is_stamped_manifest(rel_str))
        .filter(|(rel_str, _)| !is_under_top_level_dir(rel_str, "bin"))
        .filter(|(rel_str, _)| !is_excluded_skill(rel_str, exclude_skill_names))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (rel_str, rel) in &entries {
        let Ok(bytes) = fs::read(render_root.join(rel)) else {
            continue;
        };
        let content = if rel_str == "mcp.json" || rel_str == ".mcp.json" {
            canonicalize_mcp_json(&bytes, exclude_mcp_server_names).unwrap_or(bytes)
        } else {
            bytes
        };
        let normalized = normalize_to_lf(&content);
        hasher.update(rel_str.as_bytes());
        hasher.update([0u8]);
        hasher.update(&normalized);
        hasher.update([0u8]);
    }

    hex::encode(hasher.finalize())
}

/// Render the root Agent Plugins manifest (plugin.json at root).
pub(crate) fn render_root_plugin_manifest(
    temp_dir: &Path,
    _components: &ScannedComponents,
    version_stamp: &str,
) -> Result<(), RenderError> {
    // Antigravity discovers agents and skills by directory and ignores manifest arrays.
    // Keep the root manifest in the Agent Plugins shape so the same manifest is also
    // consumable by runtimes that read the canonical root. `version` carries the
    // same content-digest stamp as the Claude and Codex manifests.
    let manifest_json = serde_json::json!({
        "$schema": "https://agent-plugins.org/schemas/1.0.0/plugin.schema.json",
        "name": "gal",
        "version": version_stamp,
        "description": "GAL - Golem Agents Legion",
    });

    let manifest_path = temp_dir.join("plugin.json");
    fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest_json)?,
    )?;

    Ok(())
}

/// Render the standard Agent Plugins `mcp.json` (root, no leading dot) from an
/// already-merged `mcpServers` map — the same map written to Claude Code's
/// `.mcp.json` after the personal-layer merge.
///
/// The output object has exactly two keys: `$schema` (the Agent Plugins MCP
/// schema URL) and `mcpServers`. Server entries pass through unchanged except
/// for the `type` field: `"http"` is rewritten to `"streamable-http"` (the
/// Agent Plugins spec's transport tag); `"stdio"` and `"sse"` — and any entry
/// with no `type` field at all — pass through untouched.
pub(crate) fn render_standard_mcp_manifest(
    mcp_servers: &serde_json::Map<String, serde_json::Value>,
    target_path: &Path,
) -> Result<(), RenderError> {
    let mut mapped = serde_json::Map::with_capacity(mcp_servers.len());
    for (name, value) in mcp_servers {
        let mut server = value.clone();
        if let Some(obj) = server.as_object_mut() {
            if obj.get("type").and_then(|v| v.as_str()) == Some("http") {
                obj.insert(
                    "type".to_string(),
                    serde_json::Value::String("streamable-http".to_string()),
                );
            }
        }
        mapped.insert(name.clone(), server);
    }

    let manifest_json = serde_json::json!({
        "$schema": "https://agent-plugins.org/schemas/1.0.0/mcp.schema.json",
        "mcpServers": mapped,
    });

    fs::write(target_path, serde_json::to_string_pretty(&manifest_json)?)?;

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

#[cfg(test)]
mod codex_manifest_tests {
    use super::*;
    use tempfile::TempDir;

    fn name_set(names: &[&str]) -> HashSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    #[test]
    fn personal_only_names_drops_core_wins_collisions() {
        // Regression: both personal merges are core-wins, so a colliding
        // personal name never reaches the render tree. Excluding it would strip
        // the core entry from the digest and make the stamp depend on the
        // machine's personal layer.
        let personal = name_set(&["shared-name", "personal-only"]);
        let core = name_set(&["shared-name", "core-only"]);

        let excluded = personal_only_names(&personal, &core);

        assert_eq!(
            excluded,
            name_set(&["personal-only"]),
            "only genuinely-added personal names may be excluded from the digest"
        );
    }

    #[test]
    fn personal_only_names_empty_personal_layer_excludes_nothing() {
        let excluded = personal_only_names(&HashSet::new(), &name_set(&["core-only"]));
        assert!(excluded.is_empty());
    }

    #[test]
    fn core_mcp_server_names_reads_source_servers_and_is_failsafe() {
        let dir = TempDir::new().unwrap();
        assert!(
            core_mcp_server_names(dir.path()).is_empty(),
            "absent source mcp.json must yield an empty set"
        );

        fs::write(dir.path().join("mcp.json"), "{ not json").unwrap();
        assert!(
            core_mcp_server_names(dir.path()).is_empty(),
            "unparseable source mcp.json must yield an empty set"
        );

        fs::write(
            dir.path().join("mcp.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "servers": {
                    "core-a": { "type": "http", "url": "https://a.example" },
                    "core-b": { "type": "stdio", "command": "b" }
                }
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            core_mcp_server_names(dir.path()),
            name_set(&["core-a", "core-b"])
        );
    }

    #[test]
    fn digest_ignores_formatting_of_mcp_files() {
        // Regression: the render writes mcp.json pretty-printed. Hashing raw
        // bytes when there is nothing to exclude, but re-serialized bytes when
        // there is, would make the stamp depend on the personal layer's mere
        // presence rather than on portable content.
        let servers = serde_json::json!({
            "mcpServers": { "core-srv": { "type": "http", "url": "https://core.example" } }
        });

        let pretty_dir = TempDir::new().unwrap();
        fs::write(
            pretty_dir.path().join("mcp.json"),
            serde_json::to_string_pretty(&servers).unwrap(),
        )
        .unwrap();

        let compact_dir = TempDir::new().unwrap();
        fs::write(
            compact_dir.path().join("mcp.json"),
            serde_json::to_string(&servers).unwrap(),
        )
        .unwrap();

        assert_eq!(
            compute_portable_digest(pretty_dir.path(), &HashSet::new(), &HashSet::new()),
            compute_portable_digest(compact_dir.path(), &HashSet::new(), &HashSet::new()),
            "the same MCP content must digest identically regardless of on-disk formatting"
        );
    }

    #[test]
    fn codex_plugin_manifest_carries_author_and_interface() {
        let dir = TempDir::new().unwrap();
        let codex_plugin_dir = dir.path().join(".codex-plugin");
        fs::create_dir_all(&codex_plugin_dir).unwrap();
        render_codex_plugin_manifest(
            &codex_plugin_dir,
            "1.0.0-gabc123abc123",
            &serde_json::json!({}),
        )
        .unwrap();
        let raw = fs::read_to_string(codex_plugin_dir.join("plugin.json")).unwrap();
        let json: serde_json::Value = serde_json::from_str(&raw).unwrap();

        assert_eq!(json["name"], "gal");
        assert_eq!(json["version"], "1.0.0-gabc123abc123");
        assert_eq!(
            json["author"]["name"], "GAL",
            "Codex spec requires author.name: {raw}"
        );
        assert_eq!(
            json["interface"]["displayName"], "Golem Agents Legion",
            "Codex spec requires interface.displayName: {raw}"
        );
        assert_eq!(
            json["interface"]["shortDescription"],
            "GAL workflow commands and golem agents"
        );
    }
}
