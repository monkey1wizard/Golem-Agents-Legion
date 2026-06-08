# Diary Template

Used by `@golem-notewriter` diary mode to generate daily work diaries in Obsidian.

## Frontmatter

```yaml
---
type: work-diary
date: "{{YYYY-MM-DD}}"
weekday: "{{ddd}}"
repos:
  - "{{repo-name}}"
tags:
  - work-diary
---
```

## Body Structure

```markdown
# Work Journal {{YYYY-MM-DD}}（{{Weekday}}）

## Summary

One or two sentences summarizing today's progress.

## Development

### {{repo-name}}

- <type>: <description> (commit: <short-hash>)
- <type>: <description> (commit: <short-hash>)

> Current status: <from .dev/state.md>

## Notes & Research

- Items from scratch log that aren't commit-linked

## Decisions

- **<decision>**: <rationale>

## Other

- Meetings, reading, non-dev activities
- or (None)

## Tomorrow

- [ ] Carry-forward tasks from state.md
- [ ] Planned next steps

> [!tip] Atomizable
> If any section contains reusable knowledge, extract it to a reusable note in the user's configured knowledge area.
```

## Diary Rules

- **Language**: Traditional Chinese for structure headings; content follows source language (commit messages stay as-is)
- **Punctuation**: Full-width for CJK text `，`、`。`、`：`; half-width for English/code
- **No emoji** in content — only in the shutdown confirmation banner
- **Commit hashes**: Include short hash for traceability
- **Empty sections**: Write `（無）` instead of omitting the heading — consistent structure for search

## Storage

- **Active**: `<OBSIDIAN_DIARY_DIR>/YYYYMMDD_Work_Diary.md`
- **Archive**: `<OBSIDIAN_ARCHIVE_DIR>/YYYY-MM/` (monthly, on 1st of next month)
