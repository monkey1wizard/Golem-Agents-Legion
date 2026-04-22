---
name: plan-to-prompt
description: "Create .dev/plans/<plan-slug>.prompt.md from a source plan using the standard execution-prompt template."
---

# /plan-to-prompt

Create or refresh the execution prompt for a source plan.

## Role

Execution-prompt generator. Your job is to transform a stable source plan into the mutable execution work file that GAL and specialist commands use for stateful workflow operations, while preserving execution-owned state during refresh.

## When to Use

- A source plan exists and no execution prompt has been created yet
- The source plan has changed significantly and the execution prompt should be regenerated after planning-stage reviews are complete
- An execution prompt needs to be generated or refreshed without advancing downstream workflow by side effect

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
- Carry forward planning-stage review content from the source plan into the matching prompt sections, including `## Review Results > ### Architecture Review`, `## Review Results > ### Engineering Review`, and `## Approval > Architect review`.
- The source plan is expected to already contain the implementation contract from `/refining-plan`; seed the execution prompt from that `## Tasks`, `## Test Plan`, and engineering review content.
- In refresh mode, preserve `## Tasks`, `## Test Plan`, and `## Review Results > ### Engineering Review` from the existing execution prompt only when those sections have execution-state changes that would be lost by replacement; otherwise refresh them from the source plan.

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

The execution prompt is a **machine-readable work file** and must be entirely in **English**. All section headers, scaffold text, status markers, and content prose must be in English. When the source plan is in a non-English language, **translate** content while creating the prompt. Do not produce mixed-language output.

### Scope Guard

This command only creates or refreshes the execution prompt. It must not decide scope, invent engineering tasks or a test matrix on its own, add a review verdict that does not exist in the source plan, or invoke downstream workflow steps.

When refreshing an existing prompt, do not wipe completed tasks, review history, or handoff notes unless the user explicitly asks for a reset.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the `File` column points at `.dev/plans/<slug>.prompt.md`.

## Step 4 — Handoff

Tell the user:

- which execution prompt was created or updated
- whether existing mutable sections were preserved or reset
- whether the prompt is generated only, or already runnable because the source plan already supplied the implementation contract or the existing prompt preserved it
- if required execution-owned sections are still placeholders, state that implementation cannot start yet because the source plan still needs `/refining-plan` before the prompt can be regenerated into a runnable state
- do this as readiness guidance only; do not name or invoke any downstream command in this handoff
