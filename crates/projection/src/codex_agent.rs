//! Codex subagent TOML serializer for golem agent files.
//!
//! Converts a `*.agent.md` frontmatter+body pair into the TOML format that
//! Codex reads from `~/.codex/agents/<name>.toml`. The `model`,
//! `model_reasoning_effort`, and `sandbox_mode` values are supplied by the
//! caller rather than read from the agent file so that executor-routing can
//! override them at projection time.

/// Golem names that are consult-eligible for the discuss skill projection.
/// Planning-stage consult roles: architect, analyst, designer, releaser.
pub(crate) const CONSULT_GOLEM_NAMES: &[&str] = &[
    "golem-architect",
    "golem-analyst",
    "golem-designer",
    "golem-releaser",
];

/// Parsed minimal agent metadata extracted from a `*.agent.md` file.
#[derive(Debug, PartialEq, Clone)]
pub struct AgentMeta {
    pub name: String,
    pub description: String,
    /// The charter body (everything after the closing `---` of the frontmatter).
    pub body: String,
}

/// Serialize an `AgentMeta` into a Codex TOML string.
///
/// The output format expected by Codex:
/// ```toml
/// name = "golem-architect"
/// description = "..."
/// developer_instructions = """
/// ...body...
/// """
/// model = "gpt-5.4"
/// model_reasoning_effort = "medium"
/// sandbox_mode = "read-only"
/// ```
pub fn serialize_codex_toml(
    meta: &AgentMeta,
    model: &str,
    reasoning_effort: Option<&str>,
    sandbox_mode: &str,
) -> String {
    let body = escape_toml_multiline(&meta.body);
    let description = escape_toml_string(&meta.description);
    let name = escape_toml_string(&meta.name);
    let model_escaped = escape_toml_string(model);
    let sandbox_escaped = escape_toml_string(sandbox_mode);

    let mut lines = vec![
        format!("name = \"{name}\""),
        format!("description = \"{description}\""),
        format!("developer_instructions = \"\"\"\n{body}\n\"\"\""),
        format!("model = \"{model_escaped}\""),
    ];

    if let Some(effort) = reasoning_effort {
        lines.push(format!(
            "model_reasoning_effort = \"{}\"",
            escape_toml_string(effort)
        ));
    }
    lines.push(format!("sandbox_mode = \"{sandbox_escaped}\""));

    lines.join("\n") + "\n"
}

/// Parse the minimal metadata from a `*.agent.md` file.
///
/// Returns `None` when the file has no recognisable YAML frontmatter or is
/// missing the required `name` field.
pub fn parse_agent_meta(content: &str) -> Option<AgentMeta> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() < 3 || lines[0] != "---" {
        return None;
    }
    // Find closing ---
    let closing = lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| **l == "---")
        .map(|(i, _)| i)?;

    let mut name = String::new();
    let mut description = String::new();

    for line in &lines[1..closing] {
        if let Some(rest) = line.strip_prefix("name:") {
            name = rest.trim().to_owned();
        } else if let Some(rest) = line.strip_prefix("description:") {
            description = rest.trim().to_owned();
        }
    }

    if name.is_empty() {
        return None;
    }

    // Everything after the closing --- is the charter body.
    let body = lines[closing + 1..].join("\n");

    Some(AgentMeta {
        name,
        description,
        body: body.trim_start_matches('\n').to_owned(),
    })
}

/// Escape a single-line TOML basic string value (content between `"…"`).
fn escape_toml_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Escape content for use inside a TOML multiline basic string (`"""…"""`).
///
/// The only sequence that needs handling is `"""` — we escape the final quote
/// so that three consecutive double-quotes cannot close the string prematurely.
fn escape_toml_multiline(s: &str) -> String {
    s.replace("\"\"\"", "\"\"\\\"")
}

/// Extract the `<role>…</role>` section from a charter body, trimmed.
/// When the tags are absent, returns the full body (fallback for non-XML agent files).
pub fn extract_role_section(body: &str) -> &str {
    if let Some(start) = body.find("<role>") {
        let inner_start = start + "<role>".len();
        if let Some(rel_end) = body[inner_start..].find("</role>") {
            return body[inner_start..inner_start + rel_end].trim();
        }
    }
    body.trim()
}

/// Generate a Codex `discuss-<name>` SKILL.md that injects the activation-core into
/// the current session without spawning a separate subagent.
///
/// The caller writes this to `~/.codex/skills/discuss-<name>/SKILL.md`.
pub fn generate_discuss_skill_md(meta: &AgentMeta) -> String {
    let role_core = extract_role_section(&meta.body);
    let discuss_name = format!("discuss-{}", meta.name);
    format!(
        "---\nname: {discuss_name}\ndescription: \"Enter in-context consult mode for {name}. \
Loads activation-core into current session (no subagent spawn). \
Label response [{name} \u{00b7} in-context].\"\n---\n\n\
# {discuss_name}\n\n\
You are activating `{name}` in **in-context** mode.\n\n\
This is not a subagent spawn \u{2014} you are the current assistant, taking on the role.\n\
Label your response `[{name} \u{00b7} in-context]`.\n\n\
## Activation Core\n\n{role_core}\n",
        discuss_name = discuss_name,
        name = meta.name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool_map::{invocation_mode_to_codex_sandbox, InvocationMode};

    fn example_meta() -> AgentMeta {
        AgentMeta {
            name: "golem-architect".to_owned(),
            description: "Strict technical architect.".to_owned(),
            body: "You review plans.\n\nBe adversarial.".to_owned(),
        }
    }

    #[test]
    fn serialize_produces_valid_toml_section() {
        let out = serialize_codex_toml(&example_meta(), "gpt-5.4", Some("medium"), "read-only");
        assert!(out.starts_with("name = \"golem-architect\"\n"));
        assert!(!out.contains(&["[", "agent", "]"].concat()));
        assert!(out.contains("description = \"Strict technical architect.\""));
        assert!(out.contains("model = \"gpt-5.4\""));
        assert!(out.contains("model_reasoning_effort = \"medium\""));
        assert!(out.contains("sandbox_mode = \"read-only\""));
        assert!(out.contains("developer_instructions = \"\"\"\n"));
    }

    #[test]
    fn serialize_no_reasoning_effort_omits_field() {
        let out = serialize_codex_toml(&example_meta(), "gpt-5.4", None, "read-only");
        assert!(
            !out.contains("model_reasoning_effort"),
            "must omit when None: {out}"
        );
    }

    #[test]
    fn serialize_body_in_multiline_string() {
        let out = serialize_codex_toml(&example_meta(), "m", None, "s");
        assert!(
            out.contains("You review plans."),
            "charter body must appear in output"
        );
    }

    #[test]
    fn serialize_escapes_triple_quote_in_body() {
        let meta = AgentMeta {
            name: "g".to_owned(),
            description: "d".to_owned(),
            body: "before \"\"\" after".to_owned(),
        };
        let out = serialize_codex_toml(&meta, "m", None, "s");
        // The raw unescaped "before """ after" must not appear in the body section.
        assert!(
            !out.contains("before \"\"\" after"),
            "unescaped triple-quote in body must be escaped: {out}"
        );
        // The escaped form ""\" must appear where the body's """ was.
        assert!(
            out.contains("\"\"\\\""),
            "escaped form must be present: {out}"
        );
    }

    #[test]
    fn parse_extracts_name_and_description() {
        let content = "---\nname: golem-architect\ndescription: The architect.\ntools: [read]\n---\nbody content\n";
        let meta = parse_agent_meta(content).expect("should parse");
        assert_eq!(meta.name, "golem-architect");
        assert_eq!(meta.description, "The architect.");
        assert_eq!(meta.body, "body content");
    }

    #[test]
    fn parse_returns_none_without_frontmatter() {
        assert!(parse_agent_meta("no frontmatter here").is_none());
    }

    #[test]
    fn parse_returns_none_without_name() {
        let content = "---\ndescription: d\n---\nbody\n";
        assert!(parse_agent_meta(content).is_none());
    }

    #[test]
    fn parse_body_excludes_frontmatter() {
        let content = "---\nname: g\n---\ncharter starts here\n";
        let meta = parse_agent_meta(content).unwrap();
        assert_eq!(meta.body, "charter starts here");
        assert!(
            !meta.body.contains("---"),
            "frontmatter must not leak into body"
        );
    }

    #[test]
    fn extract_role_section_returns_inner_content() {
        let body = "prefix\n<role>\nrole content here\n</role>\nsuffix";
        assert_eq!(extract_role_section(body), "role content here");
    }

    #[test]
    fn extract_role_section_falls_back_to_full_body() {
        let body = "no role tags here";
        assert_eq!(extract_role_section(body), "no role tags here");
    }

    #[test]
    fn generate_discuss_skill_contains_in_context_label_and_core() {
        let meta = AgentMeta {
            name: "golem-architect".to_owned(),
            description: "Strict architect.".to_owned(),
            body: "<role>\nCheck trade-offs.\n</role>\nAppendix here.".to_owned(),
        };
        let skill = generate_discuss_skill_md(&meta);
        assert!(
            skill.contains("discuss-golem-architect"),
            "discuss name in skill: {skill}"
        );
        assert!(
            skill.contains("[golem-architect · in-context]"),
            "in-context label: {skill}"
        );
        assert!(
            skill.contains("Check trade-offs."),
            "role core injected: {skill}"
        );
        assert!(
            !skill.contains("Appendix here."),
            "appendix must be excluded: {skill}"
        );
    }

    #[test]
    fn consult_names_are_consult_roles() {
        for name in CONSULT_GOLEM_NAMES {
            assert!(
                name.starts_with("golem-"),
                "must be full golem name: {name}"
            );
        }
        assert!(CONSULT_GOLEM_NAMES.contains(&"golem-architect"));
        assert!(CONSULT_GOLEM_NAMES.contains(&"golem-analyst"));
        assert!(CONSULT_GOLEM_NAMES.contains(&"golem-designer"));
        assert!(CONSULT_GOLEM_NAMES.contains(&"golem-releaser"));
        assert!(
            !CONSULT_GOLEM_NAMES.contains(&"golem-auditor"),
            "auditor is orchestrated-only"
        );
    }

    #[test]
    fn consult_sandbox_mode_is_read_only() {
        assert_eq!(
            invocation_mode_to_codex_sandbox(InvocationMode::Consult),
            "read-only"
        );
    }

    #[test]
    fn build_sandbox_mode_is_workspace_write() {
        assert_eq!(
            invocation_mode_to_codex_sandbox(InvocationMode::Build),
            "workspace-write"
        );
    }
}
