---
name: plan-to-prompt
description: "GAL plan-to-prompt ($plan-to-prompt / /plan-to-prompt / generate prompt / 生成執行提示). Creates .dev/plans/<slug>.prompt.md from a reviewed source plan. Requires a passing gal refining-check, ENG_REVIEW: CLEAR, and human approval in ## Approval, regardless of which producer completed the contract. Output is the execution prompt consumed by $gal-pipeline."
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

## Step 1.6 — Implementation Contract Gate

Before generating or refreshing the prompt, require all three conditions:

- `## Tasks` and `## Test Plan` contain no placeholders and follow the implementation-contract task shape in `plugins/gal-core/templates/plan.md`.
- `## Review Results > ### Engineering Review` records the completed STAGE 3.5 dual-lens review and ends with `<!-- ENG_REVIEW: CLEAR -->`.
- A passing `gal refining-check` receipt verifies the current source plan.

Any producer may complete the implementation contract. `/refining-plan` is one available producer. The producer does not change these gates. If any condition is missing or fails, stop and report what remains before creating or refreshing the prompt.

## Step 1.7 — Source Language and Prompt Compilation

For a non-English source plan, compile the English execution prompt directly from the single Markdown source plan. Preserve its meaning in English while carrying approval state, review markers, and workflow-visible checkbox and task state. Do not create an EN draft, reconcile hashes, or write an equivalence stamp. If the source plan is later edited, refresh the prompt directly from the current source plan.

For legacy source plans that carry a `gal:planning-authority` block, retain that block and run the existing localized-metadata validation. It does not make an EN draft the semantic authority or require a new equivalence stamp.

For `en`-prefix `planLanguage`, compile the prompt from the same single-file source plan.

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
## Preconditions       (optional; present only when the source plan has it)
## Owner Acceptance    (optional; present only when the source plan has it)
## Open Questions
## Approval
---
## Status          (with ### Deviations table and ### Handoff Notes)
## Tasks
## Verification Map
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
- Preserve the parser anchors consumed by pipeline/finalize tooling: `## Status`, `Current Task:`, `## Tasks`, at least one `- [ ] T-NN`, `## Test Results`, `## Review Results`, and the four review subsections. **Every `## Requirements` entry keeps its source-plan checkbox prefix — `- [ ] R<n> — ` or `- [x] R<n> — `, verbatim.** `finalize-check`'s `finalize-review-shape` row counts `## Requirements` entries with the regex `^- \[[ xX]\] R\d+ — `; a compressed entry that drops the leading `- [ ]` (e.g. `- R1 — ...`) is invisible to that count and silently desyncs the Finalize Review table's expected row count from the real requirement count at finalize time. This is a compression-immune anchor, not filler — do not strip it even when trimming Requirements prose for length.
- Preserve each task's `**Verification Impact**` JSON object and copy the source `## Verification Map` rows. Keep exactly the Boolean keys `contract_docs`, `code_logic`, `cli_surface`, and `cross_boundary`, with at least one true value. Their required layers are `contract-doc`, `unit`, `cli`, and `integration` respectively.
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
- `## Preconditions` and `## Owner Acceptance` — when the source plan has either section, copy it **byte-identical** into the prompt: same heading, same table, same cell text, no compression, translation, or reflow. When the source plan lacks a section, omit that heading from the prompt. Do not invent either section.
- `## Open Questions` — carry forward existing `OQ-NN` items. Format: `- [ ] OQ-NN — description *(raised by: source)*`
- Carry forward planning-stage review content from the source plan into the matching prompt sections, including `## Review Results > ### Architecture Review`, `## Review Results > ### Engineering Review`, and `## Approval > Architect review`.
- Carry forward `## Approval` exactly from the source plan, including all four fields — `Human approval`, `Architect review`, `Design review`, `Business review` — and any recorded reason tails.
- The source plan must already contain the implementation contract; seed the first execution prompt from its `## Tasks`, `## Test Plan`, and engineering review content, regardless of which producer completed it.
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

Read the complete receipt. A passing receipt is required before handoff. If this is the outer workflow invocation, create one stable `invocation_id` and set the consumed format-repair round count to `0`. Otherwise carry both forward without reset. For an eligible, bound `prompt` receipt whose failed or not-run rows are all `format`, follow [Bounded Format Recovery](../../conventions/token-budget.md#bounded-format-recovery): invoke the helper with the receipt's exact target, receipt, repair, gate, mode, gate attempt, and repair round values, then rerun `gal prompt-check` on the same execution prompt and verify a fresh standalone pass. Catalogued recovery requires the execution prompt to be tracked, committed, clean, and have no staged entries because `gal format-repair` refuses other target states. If this precondition does not hold, stop and ask the owner to commit the execution prompt. Then rerun `gal prompt-check` and apply recovery only to a fresh eligible receipt. This stop consumes no round. A document round consumes one count after the helper exits, whether it succeeds or fails. The shared ceiling is two rounds across entries. When both rounds are consumed, follow [Bounded Format Recovery's stop report table](../../conventions/token-budget.md#stop-and-next-action) and do not repair again. Any `owner` row, missing or ambiguous marker/content, stale or unbound evidence, helper refusal/failure, failed rerun, or uncertainty stops handoff. For an `owner` row or helper refusal/failure, follow [Bounded Format Recovery's stop report table](../../conventions/token-budget.md#stop-and-next-action). Missing or ambiguous markers/content and stale or unbound evidence are not formatting defects and stop recovery. Recovery repairs supported formatting only. It cannot create missing prompt content, human approval, or evidence. Preserve approval and source-language reconciliation requirements. Do not use a failed receipt as a pass.

For any eligible phase-payload write-back in this invocation, pass `--format-repair-context '{"invocation_id":"<invocation_id>","round":<next_round>}'` only to `gal pipeline <prompt>`, where `<next_round>` is the consumed count plus one. When both rounds are consumed, omit the option. Never pass it to executor raw arguments or `gal format-repair`. A payload round consumes a count only when the runtime prints `FORMAT_REPAIR`. Ambiguity refusal prints no such line and consumes no round.
The default evidence path is `.dev/pipeline/<plan-scope-key>/prompt-check.receipt.md`.
Before trusting the stored pass, run the bound executable with `doctor --verify-receipt <receipt> --expect-scope standalone` and require exit 0. Follow [`conventions/self-bootstrap.md`](../../conventions/self-bootstrap.md) for explicit executable roots and package-manager ownership; never resolve `gal` again through `PATH` after handoff.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the `File` column points at `.dev/plans/<slug>.prompt.md`.

## Step 4 — Handoff

Tell the user:

- which execution prompt was created or updated
- whether existing mutable sections were preserved or reset
- whether the prompt is generated only, or already runnable because the source plan already supplied the implementation contract, human approval is recorded, or the existing prompt preserved the needed execution state
- if required execution-owned sections are still placeholders, state that the implementation contract needs completion per `plugins/gal-core/templates/plan.md` before the prompt can be regenerated into a runnable state; `/refining-plan` is one available producer
- if generation was blocked by missing human approval, state that the source plan must record human approval in `## Approval` before this command can run
- do this as readiness guidance only; do not name or invoke any downstream command in this handoff
