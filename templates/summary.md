# Summary Template

Template for `<repo>/.dev/summary.md` — compact project summary for AI context loading.

Agents read this file first. It must be short enough to consume in full without triggering further reads. Write it as if briefing a capable new engineer who has never seen the repo — in 500 words or less.

## File Template

```markdown
# [Project Name] — Summary

*Last updated: YYYY-MM-DD*

## Current State

[1–2 paragraphs. What the project is, where it stands today, and what the active thrust is. No history — present state only.]

## Key Decisions

| Decision | Choice | Rationale |
| --- | --- | --- |
| [Topic] | [What was decided] | [Why — one line] |
| [Topic] | [What was decided] | [Why — one line] |

## Active Threads

| Thread | Status | Blocker |
| --- | --- | --- |
| [Plan or investigation name] | in-progress | [none / description] |

## What Agents Should Know

- [Convention or invariant every agent must respect in this repo]
- [File or path that should never be edited without human approval]
- [Naming rule, pattern, or constraint that is easy to violate by accident]

## Canonical Docs

| Purpose | Path |
| --- | --- |
| Architecture | `docs/architecture.md` |
| Active plans | `docs/plans/` |
| Requirements | `.dev/requirements.md` |
| Roadmap | `.dev/roadmap.md` |
```

## Usage Rules

1. Keep the entire file under 500 words. If it grows beyond that, archive old decisions into `project.md` and trim this file.
2. **Current State** is the only section written in prose. All other sections use tables or brief bullets.
3. Update **Active Threads** at the end of every session. A thread stays here until its plan reaches DONE.
4. **What Agents Should Know** is the most critical section — list only things that are easy to miss and costly to get wrong.
5. This file supplements `project.md`. `project.md` holds stable project context; `summary.md` holds current-moment state. Agents read `summary.md` to orient quickly and only open `project.md` when they need full architecture or history.
