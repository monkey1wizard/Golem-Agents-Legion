---
name: plan-to-prompt
description: "Create .dev/plans/<plan-slug>.prompt.md from a source plan using the standard execution-prompt template."
---

# /plan-to-prompt

Create or refresh the execution prompt for a source plan.

## Role

Execution-prompt generator. Your job is to transform a stable source plan into the mutable execution work file that GAL and specialist commands use for stateful workflow operations.

## When to Use

- A source plan exists and no execution prompt has been created yet
- The source plan has changed materially and the execution prompt should be regenerated before reviews begin

## Step 0 — Select Plan

If the user specified a plan slug, use it. Otherwise:

1. Read `.dev/state.md` `## Active Plans` only to collect possible plan names or slugs.
2. Derive the slug from each plan's name (lowercase, hyphens for spaces) or `File` column.
3. For each slug, check the actual filesystem path `.dev/plans/<slug>.prompt.md`.
4. Treat the filesystem as authoritative: if the prompt file is missing on disk, that execution prompt does **not** exist yet, even if `.dev/state.md` says it exists.
5. Candidates are slugs whose prompt file is missing on disk and whose source plan can be found.
6. If exactly one candidate remains, use it. If multiple candidates exist, **ask the user** which plan to create an execution prompt for. If no candidate exists, tell the user and stop.

Never silently pick a plan when multiple plans still need execution prompts.
Never conclude that an execution prompt already exists from `.dev/state.md` alone.

## Step 1 — Find the Source Plan

Look for the source plan in this order:

1. `docs/plans/<slug>.md`
2. `<slug>.md` at the repo root
3. Search the repo for a file matching `*<slug>*.md` (excluding `.dev/` and any existing `.prompt.md`)

If the source plan cannot be found, tell the user and stop.

If `.dev/plans/<slug>.prompt.md` already exists (refresh case), read it too before updating.

## Step 2 — Create Execution Prompt

Create or update `.dev/plans/<slug>.prompt.md`.

**Use the execution-prompt template below as the output schema.** The template controls section names, order, and scaffold shape. Do NOT look for an external `templates/plan-prompt.md` file — the template is embedded here.

### Execution-Prompt Template

The execution prompt must contain exactly these sections in this order:

```markdown
# Plan Prompt: [Feature Name]

<!--
Generated from <source-plan-path>.
Output path: <repo>/.dev/plans/<slug>.prompt.md
This is the mutable execution work file consumed by /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal
## Requirements
## Approach
## Files to Create or Modify
## Test Cases
## Success Criteria
## Risks
## Open Questions
## Approval
---
## Status          (with ### Deviations table and ### Handoff Notes)
## Tasks
## Analyze
## Test Plan
## Test Results
## Review Results  (with ### Architecture Review, ### Business Review, ### Design Review, ### Engineering Review)
## Debug Log
```

### Prompt Creation Rules

- Copy the stable planning content from the source plan by **semantic mapping**, not by mirroring the source plan's heading layout.
- If the source plan uses non-standard headings (e.g. `## Context`, `## Scope`, `## Delivery Strategy`, `## Steps`), map their content into the correct sections above. Typical mappings:
  - `## Context` / `## Delivery Strategy` → `## Approach`
  - `## Scope — In-Scope` → `## Requirements` + `## Approach`
  - `## Scope — Out-of-Scope` → `## Approach` (out-of-scope paragraph)
  - `## Steps (Roadmap)` → note in `## Status` Step count; do not create a non-standard section
- If the source plan already contains execution-style sections (drift), salvage their content into the matching standard mutable sections instead of copying the non-standard structure.
- `## Status`, `## Analyze`, `## Test Results`, `### Deviations`, `### Handoff Notes` are mutable execution-state sections. Initialize them from the template scaffold unless refreshing an existing prompt whose progress should be preserved.
- `## Open Questions` — carry forward existing `OQ-NNN` items. Format: `- [ ] OQ-NNN — description *(raised by: source)*`
- `## Review Results`, `## Test Plan`, and `## Tasks` may carry forward existing planning-stage content when it belongs in the matching standard section.
- Carry forward architecture review content from the source plan into `## Review Results > ### Architecture Review` and `## Approval > Architect review`.

### Status Section Scaffold

Initialize `## Status` as:

```text
Workflow: DRAFT
Step: 0 of N
Last activity: YYYY-MM-DD — prompt generated from source plan
Next step: [from source plan or "run plan reviews"]
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0
```

### Language Rule

The execution prompt is a **machine-readable artifact** and must be entirely in **English**. All section headers, scaffold text, status markers, and content prose must be in English. When the source plan is in a non-English language, **translate** content while creating the prompt. Do not produce mixed-language output.

### Scope Guard

This command only creates or refreshes the execution prompt. It must not decide scope, initialize engineering tasks, produce a test matrix, or write `<!-- ENG_REVIEW: CLEAR -->`.

When refreshing an existing prompt, do not wipe completed tasks, review history, or handoff notes unless the user explicitly asks for a reset.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the `File` column points at `.dev/plans/<slug>.prompt.md`.

## Step 4 — Handoff

Tell the user:

- which execution prompt was created or updated
- whether existing mutable sections were preserved or reset
- that the next step is usually a planning-stage review lane
