# Diary Template

Used by `@scribe` to generate daily work diaries in Obsidian.

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
  - "{{repo-tag}}"
---
```

## Body Structure

```markdown
# {{YYYY-MM-DD}} ({{ddd}}) Work Diary

## 📋 Summary
<!-- 3-5 sentence overview of the day -->

## 💻 Development
### {{repo-name}}
- **Branch**: `{{branch}}`
- **Commits**:
  - `abc1234` — commit message
  - `def5678` — commit message
- **Changes Summary**: What was accomplished in this repo

## 📝 Notes & Research
- Items from scratch log that aren't commit-linked

## 🔀 Decisions
- Key decisions made and their reasoning

## 📦 Other
- Meetings, reading, non-dev activities

## 📌 Tomorrow
- [ ] Carry-forward tasks
- [ ] Planned next steps

> [!tip] 可原子化
> If any section contains reusable knowledge, extract it to a permanent note in `20_Concepts/` or `21_Literature/`.
```

## Storage

- **Active**: `10_Projects/Work_Journal/YYYYMMDD_Work_Diary.md`
- **Archive**: `30_Archives/Work_Journal/YYYY-MM/` (monthly, on 1st of next month)
