---
name: plan-to-prompt
description: "GAL plan-to-prompt ($plan-to-prompt / /plan-to-prompt / generate prompt / 生成執行提示). Creates .dev/plans/<slug>.prompt.md from a reviewed source plan. Requires ENG_REVIEW: CLEAR + human approval in ## Approval. Output is the execution prompt consumed by $gal-pipeline."
---

# /plan-to-prompt

Create or refresh the execution prompt for a human-approved source plan.

## Role

Execution-prompt generator. Your job is to transform a stable source plan into the shared mutable execution work file that control-plane chat, GAL commands, and specialist write-back flows use for stateful workflow operations, while preserving execution-owned state during refresh.

## When to Use

- A source plan exists and no execution prompt has been created yet
- The source plan has changed significantly and the execution prompt should be regenerated after planning-stage reviews are complete and human approval is recorded
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

1. `.dev/plans/<slug>.md`
2. `<slug>.md` at the repo root
3. Search the repo for a file matching `*<slug>*.md` (excluding `.dev/` and any existing `.prompt.md`)

If the source plan cannot be found, tell the user and stop.

If `.dev/plans/<slug>.prompt.md` already exists (refresh case), read it too before updating.

## Step 1.5 - Approval Gate

Before generating or refreshing the execution prompt, inspect the source plan's `## Approval` section.

- `- Human approval: [approved]` is the required literal ready signal.
- `[pending]`, a missing human-approval line, or any other wording (including `[clear]`, equivalence claims, or ambiguous phrasing) means the gate is not satisfied.

If the human-approval gate is not satisfied, tell the user that `/plan-to-prompt` must stop until human approval is recorded in the source plan, and do not create or refresh the execution prompt.

## Step 1.7 — Non-English planLanguage: Reconcile, Inline Equivalence Stamp, Draft Exit

When the source plan carries a planning-authority metadata block (`planLanguage != en`, the EN-draft flow — see `workflows/coding.md` → Planning-Language Authority):

1. **On-entry reconcile preflight.** Compute the localized rendered-source hash; on mismatch, STOP and reconcile the hand-edited divergence back into the EN draft (diff-base = `render(EN draft)`), re-render, re-run `gal planning-stamp`. Do not generate the prompt from a stale/unreconciled localized source.
2. **Generate the English prompt from the EN draft.** The EN semantic draft is the stable planning-content source for meaning-translation into the English execution prompt. Carry human approval, review markers, and workflow-visible checkbox/task state from the localized source. Never re-translate from the localized text.
3. **Stamp the inline equivalence verdict, then delete the draft.** After `prompt-check` + the equivalence gate pass, run `gal planning-stamp --equivalence <prompt>` to overwrite the source plan's `prompt-hash` and `equivalence-verdict` fields in place (no `--receipt` argument, no sibling file). **Stamp the source plan first, then delete the EN draft** — never delete the draft before the stamp lands.
4. **Post-prompt refresh.** Once the draft is deleted the English prompt is the semantic authority. If the source plan's stable planning content is later hand-edited, the next refresh must rebuild the EN draft and re-stamp the inline equivalence fields, or return to the planning-stage reconcile — never a second translation straight from the localized source.

For `en`-prefix planLanguage there is no draft: generate the prompt directly from the single-file source plan.

## Step 2 — Create Execution Prompt

Create or update `.dev/plans/<slug>.prompt.md`.

This file is the single shared mutable execution-memory surface for the active task. Do not invent a separate chat-memory lane or any second execution-state file.
Execution-stage specialists write progress, retry state, review/test results, and resume markers here. After a pipeline task passes implement + test + review, `/gal pipeline` also synchronizes the task checkbox and commit note back to `.dev/plans/<slug>.md` and updates `.dev/state.md` session continuity.

**Use the execution-prompt template below as the output schema.** The template controls section names, order, and scaffold shape. Do NOT look for an external `templates/plan-prompt.md` file — the template is embedded here.

### Execution-Prompt Template

The execution prompt must contain exactly these sections in this order:

```markdown
# Plan Prompt: [Feature Name]

<!--
Generated from <source-plan-path>.
Output path: <repo>/.dev/plans/<slug>.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
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
## Deferred Follow-up
## Analyze
## Test Plan
## Test Results
## Review Results  (with ### Architecture Review, ### Business Review, ### Design Review, ### Engineering Review)
## Debug Log
```

Compression contract for the generated prompt:

- Keep the exact section order and heading text above.
- Compress by meaning, not by omission: remove human-oriented filler, repeated rationale, and prose that is not needed for execution.
- Preserve the parser anchors consumed by pipeline/finalize tooling: `## Status`, `Current Task:`, `## Tasks`, at least one `- [ ] T-NN`, `## Test Results`, `## Review Results`, and the four review subsections.
- The embedded template in this SKILL is the source of truth; any external `templates/plan-prompt.md` copy is documentation only.

### Prompt Creation Rules

- Copy the stable planning content from the source plan by **semantic mapping**, not by mirroring the source plan's heading layout.
- When the source plan is not in English, translate by **meaning**, not by literal phrasing. Preserve intent, constraints, and decision logic while rewriting for concise execution-oriented English.
- If the source plan uses non-standard headings (e.g. `## Context`, `## Scope`, `## Delivery Strategy`, `## Steps`), map their content into the correct sections above. Typical mappings:
  - `## Context` / `## Delivery Strategy` → `## Approach`
  - `## Scope — In-Scope` → `## Requirements` + `## Approach`
  - `## Scope — Out-of-Scope` → `## Approach` (out-of-scope paragraph)
  - `## Steps (Roadmap)` → note in `## Status` Step count; do not create a non-standard section
- If the source plan already contains execution-style sections (drift), salvage their content into the matching standard mutable sections instead of copying the non-standard structure.
- `## Status`, `## Tasks`, `## Deferred Follow-up`, `## Analyze`, `## Test Plan`, `## Test Results`, `## Review Results`, `## Debug Log`, `### Deviations`, and `### Handoff Notes` are execution-owned sections of the shared mutable work file. Initialize them from the template scaffold only when creating a new prompt; in refresh mode, preserve existing execution-state content unless the user explicitly asked for a reset.
- `.dev/plans/<slug>.md` remains the planning-stage source and human-readable task checklist after prompt generation. Do not overwrite it from prompt refresh, but do preserve any pipeline-synchronized task checkbox and commit notes.
- `## Tasks` is the blocking task list only. Move optional, deferred, or non-blocking follow-up items into `## Deferred Follow-up` instead of leaving them inside `## Tasks`.
- `## Open Questions` — carry forward existing `OQ-NN` items. Format: `- [ ] OQ-NN — description *(raised by: source)*`
- Carry forward planning-stage review content from the source plan into the matching prompt sections, including `## Review Results > ### Architecture Review`, `## Review Results > ### Engineering Review`, and `## Approval > Architect review`.
- Carry forward `## Approval` exactly from the source plan, including all four fields — `Human approval`, `Architect review`, `Design review`, `Business review` — and any recorded reason tails.
- The source plan is expected to already contain the implementation contract from `/refining-plan`; seed the first execution prompt from that `## Tasks`, `## Test Plan`, and engineering review content.
- In refresh mode, preserve execution-owned sections from the existing prompt as the authoritative mutable state. Refresh the stable planning sections from the source plan, and only backfill missing execution placeholders from the source plan's implementation contract when that does not overwrite existing execution history.

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

The execution prompt is a **machine-readable work file** and must be entirely in **English**. All section headers, scaffold text, status markers, and content prose must be in English. When the source plan is in a non-English language, translate content into concise execution-oriented English by **meaning translation**, not literal translation. Do not produce mixed-language output.

Use aggressive token compression because the prompt is written for AI consumption, not human readability. Prefer compact wording, direct imperatives, short labels, and deduplicated phrasing while preserving the full behavioral contract needed for execution.

### Scope Guard

This command only creates or refreshes the execution prompt. It must not decide scope, invent engineering tasks or a test matrix on its own, add a review verdict or approval state that does not exist in the source plan, or invoke downstream workflow steps.

When refreshing an existing prompt, do not wipe completed tasks, review history, or handoff notes unless the user explicitly asks for a reset.

## Step 2.5 - Prompt Receipt Gate

After creating or refreshing `.dev/plans/<slug>.prompt.md`, run:

```powershell
gal prompt-check .dev/plans/<slug>.prompt.md
```

Read the receipt. A passing receipt is required before handoff. If the receipt is `fail` or `not-run`, stop and repair the prompt anchors before reporting readiness.
The default evidence path is `.dev/pipeline/receipts/<plan-scope-key>/prompt-check.receipt.md`.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the `File` column points at `.dev/plans/<slug>.prompt.md`.

## Step 4 — Handoff

Tell the user:

- which execution prompt was created or updated
- whether existing mutable sections were preserved or reset
- whether the prompt is generated only, or already runnable because the source plan already supplied the implementation contract, human approval is recorded, or the existing prompt preserved the needed execution state
- if required execution-owned sections are still placeholders, state that implementation cannot start yet because the source plan still needs `/refining-plan` before the prompt can be regenerated into a runnable state
- if generation was blocked by missing human approval, state that the source plan must record human approval in `## Approval` before this command can run
- do this as readiness guidance only; do not name or invoke any downstream command in this handoff
