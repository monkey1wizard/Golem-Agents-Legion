//! Abstract tool token → runtime permission mapping (single source of truth).
//!
//! Maps the agent frontmatter `tools:` abstract vocabulary (`read|execute|search|edit|web`)
//! to the concrete permission representation required by each supported runtime:
//!
//! - **Claude**: a list of valid Claude Code tool names (`Read,Grep,Glob,Edit,Write,WebFetch,WebSearch`)
//! - **Codex**: a single `sandbox_mode` string (`"read-only"` or `"workspace-write"`)
//!
//! The mapping takes an `InvocationMode` so the same agent (e.g. designer, which
//! declares `edit`) is correctly clamped to read-only during a consult invocation.

/// Invocation mode for a golem role.
///
/// Controls whether write-capable tools are included in the Claude tool list
/// and which Codex `sandbox_mode` is selected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvocationMode {
    /// Planning/consult mode: read-only; Codex `sandbox_mode = "read-only"`.
    Consult,
    /// Implementation/build mode: full write access; Codex `sandbox_mode = "workspace-write"`.
    Build,
}

/// Claude Code tool names that write or execute (suppressed in `Consult` mode).
const WRITE_CAPABLE_CLAUDE_TOOLS: &[&str] = &["Edit", "Write", "Bash"];

/// Full per-token expansion to Claude Code valid tool names (build-mode baseline).
///
/// Rows are `(abstract_token, &[claude_tool_names])`.
/// Unknown tokens return an empty slice via [`abstract_token_to_claude_tools`].
const TOKEN_CLAUDE_TOOLS: &[(&str, &[&str])] = &[
    ("read", &["Read"]),
    ("search", &["Read", "Grep", "Glob"]),
    ("edit", &["Read", "Grep", "Glob", "Edit", "Write"]),
    ("execute", &["Bash"]),
    ("web", &["WebFetch", "WebSearch"]),
];

/// Map a single abstract tool token to Claude Code tool names (build-mode expansion).
///
/// Returns an empty slice for tokens not listed in [`TOKEN_CLAUDE_TOOLS`].
pub fn abstract_token_to_claude_tools(token: &str) -> &'static [&'static str] {
    for &(tok, tools) in TOKEN_CLAUDE_TOOLS {
        if tok == token {
            return tools;
        }
    }
    &[]
}

/// Expand an abstract tool token set to Claude Code valid tool names.
///
/// In `Consult` mode, write-capable tools (`Edit`, `Write`, `Bash`) are omitted
/// so consult roles stay read-only regardless of the declared abstract set.
/// In `Build` mode, the full expansion is returned.
///
/// Duplicates are removed; ordering follows declaration order in [`TOKEN_CLAUDE_TOOLS`].
pub fn abstract_tools_to_claude_names(tokens: &[&str], mode: InvocationMode) -> Vec<&'static str> {
    let mut result: Vec<&'static str> = Vec::new();
    for &token in tokens {
        for &name in abstract_token_to_claude_tools(token) {
            if mode == InvocationMode::Consult && WRITE_CAPABLE_CLAUDE_TOOLS.contains(&name) {
                continue;
            }
            if !result.contains(&name) {
                result.push(name);
            }
        }
    }
    result
}

/// Map invocation mode to the Codex `sandbox_mode` value.
///
/// The mode — not the abstract tool set — determines the Codex sandbox:
/// consult roles are always `"read-only"`; build roles are `"workspace-write"`.
pub fn invocation_mode_to_codex_sandbox(mode: InvocationMode) -> &'static str {
    match mode {
        InvocationMode::Consult => "read-only",
        InvocationMode::Build => "workspace-write",
    }
}

/// Rewrite a Claude agent file's `tools:` frontmatter from abstract GAL tokens to
/// valid Claude Code tool names.
///
/// Kept as a pure `String → String` transform so it can be used both by the
/// projection write path (`crates/projection`) and by the gal-engine render path
/// (`crates/gal-engine`).
///
/// Only the `tools:` key is touched; every other line passes through verbatim.
/// If there is no frontmatter, the content is returned unchanged.
pub fn rewrite_agent_tools_for_claude(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 3 || lines[0] != "---" {
        return content.to_owned();
    }

    let closing_idx = match lines.iter().enumerate().skip(1).find(|(_, l)| **l == "---") {
        Some((i, _)) => i,
        None => return content.to_owned(),
    };

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    out.push("---".to_owned());
    for line in &lines[1..closing_idx] {
        if let Some(colon) = line.find(':') {
            let key = line[..colon].trim();
            if key == "tools" {
                let raw_value = line[colon + 1..].trim();
                let abstract_tokens: Vec<&str> = raw_value
                    .trim_matches(|c| c == '[' || c == ']')
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();
                let claude_names =
                    abstract_tools_to_claude_names(&abstract_tokens, InvocationMode::Build);
                if !claude_names.is_empty() {
                    let joined = claude_names.join(", ");
                    out.push(format!("tools: [{joined}]"));
                }
                // unknown tokens → omit the tools line entirely
                continue;
            }
        }
        out.push((*line).to_owned());
    }
    out.push("---".to_owned());
    for line in &lines[closing_idx + 1..] {
        out.push((*line).to_owned());
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── abstract_token_to_claude_tools ─────────────────────────────────────────

    #[test]
    fn read_token_maps_to_read() {
        assert_eq!(abstract_token_to_claude_tools("read"), &["Read"]);
    }

    #[test]
    fn search_token_maps_to_read_grep_glob() {
        assert_eq!(
            abstract_token_to_claude_tools("search"),
            &["Read", "Grep", "Glob"]
        );
    }

    #[test]
    fn edit_token_maps_to_read_grep_glob_edit_write() {
        assert_eq!(
            abstract_token_to_claude_tools("edit"),
            &["Read", "Grep", "Glob", "Edit", "Write"]
        );
    }

    #[test]
    fn execute_token_maps_to_bash() {
        assert_eq!(abstract_token_to_claude_tools("execute"), &["Bash"]);
    }

    #[test]
    fn web_token_maps_to_webfetch_websearch() {
        assert_eq!(
            abstract_token_to_claude_tools("web"),
            &["WebFetch", "WebSearch"]
        );
    }

    #[test]
    fn unknown_token_returns_empty() {
        assert!(abstract_token_to_claude_tools("unknown").is_empty());
        assert!(abstract_token_to_claude_tools("").is_empty());
    }

    // ── abstract_tools_to_claude_names ────────────────────────────────────────

    #[test]
    fn build_mode_edit_includes_write_capable_tools() {
        let names = abstract_tools_to_claude_names(&["edit"], InvocationMode::Build);
        assert!(names.contains(&"Edit"));
        assert!(names.contains(&"Write"));
        assert!(names.contains(&"Read"));
    }

    #[test]
    fn consult_mode_edit_suppresses_write_capable_tools() {
        let names = abstract_tools_to_claude_names(&["edit"], InvocationMode::Consult);
        assert!(
            !names.contains(&"Edit"),
            "Edit must be suppressed in consult mode"
        );
        assert!(
            !names.contains(&"Write"),
            "Write must be suppressed in consult mode"
        );
        assert!(names.contains(&"Read"), "Read must remain in consult mode");
    }

    #[test]
    fn consult_mode_execute_returns_empty() {
        let names = abstract_tools_to_claude_names(&["execute"], InvocationMode::Consult);
        assert!(names.is_empty(), "Bash must be suppressed in consult mode");
    }

    #[test]
    fn build_mode_execute_includes_bash() {
        let names = abstract_tools_to_claude_names(&["execute"], InvocationMode::Build);
        assert!(names.contains(&"Bash"));
    }

    #[test]
    fn web_token_unaffected_by_mode() {
        let consult = abstract_tools_to_claude_names(&["web"], InvocationMode::Consult);
        let build = abstract_tools_to_claude_names(&["web"], InvocationMode::Build);
        assert_eq!(consult, build, "web maps identically in both modes");
        assert!(consult.contains(&"WebFetch"));
        assert!(consult.contains(&"WebSearch"));
    }

    #[test]
    fn all_claude_names_are_valid() {
        let valid: &[&str] = &[
            "Read",
            "Grep",
            "Glob",
            "Edit",
            "Write",
            "WebFetch",
            "WebSearch",
            "Bash",
        ];
        for &(_, tools) in TOKEN_CLAUDE_TOOLS {
            for &tool in tools {
                assert!(valid.contains(&tool), "unexpected Claude tool name: {tool}");
            }
        }
    }

    #[test]
    fn no_duplicates_in_multi_token_expansion() {
        // Both "read" and "search" expand to "Read" — result must deduplicate.
        let names = abstract_tools_to_claude_names(&["read", "search"], InvocationMode::Build);
        let read_count = names.iter().filter(|&&n| n == "Read").count();
        assert_eq!(
            read_count, 1,
            "Read must appear exactly once despite two tokens expanding to it"
        );
    }

    #[test]
    fn empty_token_list_returns_empty() {
        assert!(abstract_tools_to_claude_names(&[], InvocationMode::Consult).is_empty());
        assert!(abstract_tools_to_claude_names(&[], InvocationMode::Build).is_empty());
    }

    // ── invocation_mode_to_codex_sandbox ──────────────────────────────────────

    #[test]
    fn consult_mode_maps_to_read_only() {
        assert_eq!(
            invocation_mode_to_codex_sandbox(InvocationMode::Consult),
            "read-only"
        );
    }

    #[test]
    fn build_mode_maps_to_workspace_write() {
        assert_eq!(
            invocation_mode_to_codex_sandbox(InvocationMode::Build),
            "workspace-write"
        );
    }

    #[test]
    fn codex_sandbox_independent_of_token_set() {
        // sandbox_mode is determined solely by invocation mode, not by abstract tokens.
        let consult_with_edit = invocation_mode_to_codex_sandbox(InvocationMode::Consult);
        let consult_with_read = invocation_mode_to_codex_sandbox(InvocationMode::Consult);
        assert_eq!(consult_with_edit, consult_with_read);

        let build_with_read = invocation_mode_to_codex_sandbox(InvocationMode::Build);
        let build_with_web = invocation_mode_to_codex_sandbox(InvocationMode::Build);
        assert_eq!(build_with_read, build_with_web);
    }

    // ── rewrite_agent_tools_for_claude ────────────────────────────────────────

    #[test]
    fn rewrite_converts_abstract_tools_to_claude_names() {
        let input = "---\nname: golem-test\ntools: [read, search]\n---\nbody\n";
        let out = rewrite_agent_tools_for_claude(input);
        assert!(out.contains("tools: [Read, Grep, Glob]"), "got: {out}");
        assert!(out.contains("name: golem-test"), "name must be preserved");
    }

    #[test]
    fn rewrite_unknown_tokens_omit_tools_line() {
        let input = "---\nname: golem-x\ntools: [bad_token]\n---\nbody\n";
        let out = rewrite_agent_tools_for_claude(input);
        assert!(
            !out.contains("tools:"),
            "unknown token must omit tools line"
        );
    }

    #[test]
    fn rewrite_no_frontmatter_returned_unchanged() {
        let input = "just body content\n";
        assert_eq!(rewrite_agent_tools_for_claude(input), input);
    }

    #[test]
    fn rewrite_preserves_non_tools_frontmatter_lines() {
        let input = "---\nname: golem-a\ndescription: A role\ntools: [read]\n---\n";
        let out = rewrite_agent_tools_for_claude(input);
        assert!(out.contains("name: golem-a"));
        assert!(out.contains("description: A role"));
        assert!(out.contains("tools: [Read]"), "read must become Read");
    }
}
