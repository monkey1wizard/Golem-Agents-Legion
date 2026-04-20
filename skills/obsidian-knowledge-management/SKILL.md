---
name: obsidian-knowledge-management
description: Knowledge management protocol for the Obsidian vault. Default to a PARA-based organization when no personal Guide is present, and defer to the Guide when it exists. Use when creating, editing, organizing, or archiving Obsidian notes, processing captures, extracting reusable knowledge, or managing cross-references between vault and codebase.
---

# Obsidian Knowledge Management Protocol

> **Guide Resolution**: When `<OBSIDIAN_GUIDE_MODE>` is `guide`, load `<OBSIDIAN_GUIDE_PATH>` before writing. When the mode is `auto`, load the guide only if the file exists. When the mode is `generic`, skip Guide loading and use the generic PARA-first rules in this skill.

> **Default Mode**: If no Guide is available, assume a standard PARA system: Projects, Areas, Resources, and Archives. Do not assume numbered folder prefixes, Slipbox folders, map folders, taxonomy files, or template files unless the user's Guide or vault clearly defines them.

## Step 0: Check Obsidian CLI Availability

Before any vault operation, run the availability check from the `obsidian-cli` skill:

```bash
obsidian vault="<OBSIDIAN_VAULT_NAME>" tags total
```

- **CLI available (exit 0)**: Use `obsidian` CLI for all I/O in this session.
- **CLI unavailable (non-zero / Obsidian closed)**: Fall back to Copilot file tools for the entire session. Follow the fallback message protocol defined in the `obsidian-cli` skill.

> The result of this check applies for the entire conversation session. Do not re-check on every command.

## Prerequisite: Semantic Search (Mandatory)

Before any vault operation, invoke the `local-first-search` skill and execute Phase 0 semantic search:

```bash
cd <LOCAL_SEARCH_PROJECT>
uv run python scripts/query.py semantic "<extracted keywords>" --limit 5
```

- This step is not optional and must run before any file-based read or write.
- If results return a similarity score above 0.55, read those notes before proceeding.
- Only after semantic search may you continue to Context Loading.

## Context Loading

Before any file creation, modification, or organization, resolve the active mode:

1. If `<OBSIDIAN_GUIDE_MODE>` is `guide`, read `<OBSIDIAN_GUIDE_PATH>`.
2. If `<OBSIDIAN_GUIDE_MODE>` is `auto` and `<OBSIDIAN_GUIDE_PATH>` exists, read it.
3. If the Guide is unavailable or the mode is `generic`, continue with the generic PARA-first rules in this skill.
4. Read taxonomy, templates, or system notes only if the user's Guide or actual vault structure points to them.

## Agent Mandates

- **No Over-interpretation**: Execute only what is explicitly requested.
- **Strict File Operation Protocol**: Only operate on files mentioned by the user. For any other file, explain which files you want to touch and why, then wait for approval.
- **Pre-Write Validation**: Validate output against the user's active Guide when present, otherwise validate against the generic rules in this skill.
- **Strict Terminology (Taiwanese/English only)**: Avoid Mainland Chinese terminology. Use Taiwanese terminology or fall back to English if unsure.
- **Link Integrity**: When renaming a file, search all backlinks and update them.
- **Strict Batch Protocol**: When processing multiple files, list all targets first, process one file per response, and wait for approval before the next.

## Available Scripts / Tools

When you encounter a PDF file in the vault, follow the `pdf` skill for extraction.

## Default Vault Model: PARA

If no personal Guide is present, use these default categories:

- **Projects**: Time-bound work, active deliverables, plans, and execution notes.
- **Areas**: Ongoing responsibilities, maintained domains, or recurring operational knowledge.
- **Resources**: Reusable knowledge, reference notes, literature notes, glossaries, cheatsheets, and long-lived learning material.
- **Archives**: Inactive or historical material that should be retained but not actively maintained.
- **Optional Inbox**: A capture area may exist, but it is not required. Only treat an inbox as canonical if the user or Guide defines one.

### Structure Resolution Rules

- If a Guide defines folder names, note types, template locations, or naming rules, follow the Guide.
- If no Guide exists, do not invent a custom hierarchy beyond PARA.
- Do not assume numbered prefixes, a Slipbox subtree, Map-of-Content folders, or any user-specific system folder layout.

## Data Validity Boundary

- **Valid**: Guide-defined folders, or the standard PARA categories when no Guide exists.
- **Ignore by default**: Hidden, log, temp, export, or legacy folders unless the user explicitly asks for them.

## Default Data Flow

When no Guide is present, use this flow:

1. Capture or receive raw material.
2. Sanitize and classify it.
3. Route it by intent:
   - Active outcome → Projects
   - Ongoing responsibility → Areas
   - Reusable knowledge or reference → Resources
   - Historical record → Archives
4. If the vault uses indexes, maps, or dashboards and they are explicitly present, update them. Otherwise do not invent them.

## File Naming Rules

### General

- Forbidden characters `: / \ ? * " < > |` must be replaced with `_` or `-`.
- Spaces should become `_` unless the user's Guide says otherwise.
- Remove decorative brackets when they do not carry meaning.

### Default PARA Naming

If no Guide defines naming rules:

- **Projects**: Use clear project-oriented names such as `Project_Name.md` or `Project_Name_Log.md`.
- **Areas**: Use durable topic names such as `Area_Topic.md`.
- **Resources**: Use descriptive note names such as `Type_Keyword.md` or `Topic_Reference.md`.
- **Archives**: Preserve the existing filename when possible; add a date prefix only if it clarifies chronology.

### Optional Resource Type Prefixes

Use type prefixes only when they improve clarity, or when the user's Guide expects them:

| Type | Purpose |
| --- | --- |
| `Concept` | Definitions, principles, or phenomena |
| `Strategy` | Action plans or methodologies |
| `Model` | Causal or input-to-output structures |
| `Pattern` | Repeatable solutions or recurring phenomena |
| `Case` | Concrete examples or evidence |
| `Data` | Datasets, statistics, or reference data |
| `Resource` | Cheatsheets, references, or curated utilities |
| `Tool` | Software, services, or utilities |
| `Work` | Creative works or finished outputs |

### Atomicity Principles

- Universal knowledge that is reusable across contexts should usually be split into its own reusable note.
- Strongly coupled material may stay together when splitting it would reduce clarity.

## Frontmatter Standards

If the user's Guide or vault provides templates, use them. Otherwise use minimal frontmatter only when it adds retrieval value.

### Generic Reusable Note

```yaml
---
source: "[[Related_Note]]"
type: resource
updated: "YYYY-MM-DD"
tags:
  - domain-tag
  - specific-tag
---
```

### Generic External Reference Note

```yaml
---
source: {{url}}
author: {{author}}
type: reference
published: {{date}}
tags:
  - domain-tag
  - specific-tag
---
```

### Generic Project Note

```yaml
---
type: project
updated: "YYYY-MM-DD"
tags:
  - active-project
---
```

### Key Frontmatter Rules

- `source`: Use a wiki link if the source exists in the vault, a URL if external, `ai-synthesis` for AI-generated content without a specific vault source, or `personal-observation` for personal data.
- `author`: Plain text only.
- `tags`: English, kebab-case, unless the Guide defines a different tagging contract.
- Template comments are meta-instructions. Follow them, then remove them from final output.

## Workflow: Capture Processing

### Triage Logic

If the user has a Guide-defined inbox or capture flow, follow it. Otherwise use this default routing:

1. Historical or inactive material → Archives.
2. Active deliverable or project support material → Projects.
3. Ongoing responsibility or recurring domain note → Areas.
4. Reusable knowledge, references, or learning material → Resources.

### Content Sanitization

Remove only non-knowledge clutter such as banners, CTAs, unrelated navigation, decorative footers, or decorative images.

Preserve disclosures, relevant sponsorship notes, diagrams, charts, and evidence-bearing images.

### Atomization Process

1. Identify the knowledge-bearing parts of the source.
2. Extract reusable concepts, models, cases, or references.
3. Transform them into separate notes in the Guide-defined knowledge area, or Resources by default.
4. Link back to source notes when the source exists in the vault.
5. Archive or retain the source according to user intent or Guide rules.

### Tagging

1. Analyze the content semantically and assign a small set of useful tags.
2. Write tags to frontmatter only when the vault already uses tags or the Guide requires them.
3. If the user has a taxonomy file, follow it. Otherwise do not invent a mandatory taxonomy document.

### Index Or Map Maintenance

- If the user's Guide defines indexes, maps, dashboards, or MOCs, update them when relevant.
- If no such structure exists, do not create one by default.

## Formatting & Style

### Link Format

- Use wiki links for internal vault notes.
- Use standard Markdown links only for external files or code-repository references.

### Language

- Match the source language when reasonable.
- Default Chinese output should be Traditional Chinese.
- Use Taiwanese terminology rather than Mainland Chinese terminology.
- Keep English terms when there is no precise Traditional Chinese equivalent.

### Punctuation

- In CJK text, use full-width punctuation.
- Do not use semicolons.
- Use half-width punctuation for code, URLs, and fully English sentences.

### Content Style

- No emoji unless needed for warnings.
- Prefer strong contextual links over link spam.
- Every internal link must point to an existing file, or be omitted until the file exists.
- Use headings rather than oversized flat bullet lists for long content.

### References Section

Every reusable or reference-heavy note should end with:

```markdown
## References

- [External Title](URL)
- [[Internal_Note]]
```

## Note Structure Quick Reference

### Reusable Knowledge Note

```markdown
# Title

Brief definition or context.

## Content

### Core Definition / Mechanism
...

### Execution / Tactics
#### 1.
...

---
## Contextual Links

- **Related Concepts**：[[Related_Note]]
- **Applications / Cases**：[[Case_Note]]
- **Related Projects**：[[Project_Note]]

## References
```

### Optional Index Note

```markdown
# Topic_Name

Use this only when the user's Guide or vault already uses index pages, dashboards, or MOCs.

## Related Notes
- [[Reusable_Note]]：Brief context.

## References
- [[Reference_Note]]：Why it matters.

## Related Projects
- [[Project_Note]]：Status/relevance.
```
