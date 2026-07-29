---
name: planning
description: "GAL planning ($planning / /planning / 規劃 / 新計劃 / plan this / make a plan). Creates .dev/plans/<type>-<slug>.md source plan from a new request. Does NOT generate the execution prompt — follow with $deep-planning (if structural) then $refining-plan then $plan-to-prompt."
---

# /planning

Create or replace a formal source plan for a new feature, sprint, or initiative.

## Role

Planning lead. Your job is to turn the request into a clean, human-readable source plan that captures scope, rationale, and requirements without prematurely binding execution-state details.

## When to Use

- Starting a new feature or sprint
- When `/gal whats-next` reports no active plan
- When an informal request should become a formal source plan in `.dev/plans/`
- When you want a formal source plan without starting a question-heavy discovery workflow

## Step 1 — Gather Inputs

Read `.dev/project.md` if it exists.

Read the current request and any directly referenced files. Prefer a low-interruption planning pass: infer reasonable defaults, write assumptions explicitly, and record unresolved items in `## Open Questions`.

Read `graphify-out/GRAPH_REPORT.md` if it exists. Use communities and surprising connections to judge whether the request crosses module boundaries or hides coupling that should be called out in scope.

If `graphify-out/GAL_GRAPHIFY_VERSION.txt` exists and the `graphify` CLI is available, compare the stamped version to the current `graphify --version` output. When the versions differ and `GRAPH_REPORT.md` is not newer than the stamp file, treat the graphify report as stale-by-tool-version: do not rely on it for scope judgment, and continue with native codebase reading. If the report is newer than the stamp file, keep treating it as advisory context.

Escalate to a direct user question only when one of these is true:

- the decision affects security or trust boundaries
- the choice is effectively irreversible once implementation begins
- the plan depends on a genuine taste decision with no repo precedent

Do not route the user to legacy planning command names from this workflow. If later planning-stage review is needed, describe it as a review lane (business, design, engineering), not as a legacy planning command name.
Treat `/deep-planning` as the architect-reviewed planning pass when the plan needs structural challenge before `/plan-to-prompt`.

## Step 1a — Understand → Diverge → Converge

Follow the shared **Planning Order Principle** ([`workflows/coding.md`](../../workflows/coding.md) → Planning Reviews) — never converge-first; depth-scaled. At `/planning` depth it is deliberately shallow:

1. **Understand (shallow)** — read the README + relevant `docs/` and skim the directly-related code to ground the plan in what actually exists. Deep code-flow tracing is deferred to `/deep-planning` (+ graphify). Docs are advisory; when docs and code disagree, the running code decides. Capture any web / official-doc sources as **correct URLs recorded in the plan** for later deep reading — do not inline-trace them now.
2. **Diverge (light)** — enumerate the candidate approaches as a list; do not prune while listing.
3. **Converge** — apply the minimalism ladder in [`conventions/minimalism.md`](../../conventions/minimalism.md) to each candidate and proposed element, stopping at the first rung that eliminates it (**reuse what already exists** / std-lib / native-platform / installed-dep / one-line first). Elements no rung eliminates enter the plan; genuinely-uncertain ones go to `## Open Questions`.

If the design space is genuinely open or structural, **escalate to `/deep-planning`** rather than forcing a shallow converge. A trivial local fix may collapse this to a quick self-check.

Do not proceed to Step 2 with any element that does not survive the converge step.

## Step 2 — Produce Source Plan

Write or update `.dev/plans/<plan-slug>.md` using `templates/plan.md`.

Before resolving language, read `~/.gal/config/config.json` as a mandatory first step when it exists. If `planLanguage` is set and there is no explicit per-invocation override, the plan output MUST use that language.

Resolve the output language for the source plan using this precedence order:

1. explicit per-invocation language directive at the top level of the user's request (for example `in zh-tw`, `in en`, or "write in English")
2. machine-local `planLanguage` from `~/.gal/config/config.json` (following the `workingHours` precedent)
3. auto-detect the narrative language of the user's request
4. fallback default `en`

Treat an explicit language directive as valid only when it is a top-level instruction for this invocation's output language. Do not treat the same string as a language switch when it appears inside described content, quoted examples, or requested documentation text.

Write the human-readable source plan entirely in the resolved language. Keep technical identifiers, file paths, command names, code snippets, and other literal machine-facing tokens untranslated. Do not awkwardly mix English prose with the resolved language inside the same narrative section.

### Non-English planLanguage: EN Draft + Localized Render

When the resolved language is **not** English (`planLanguage` normalizes to a non-`en` prefix), the source plan is a localized render, not the semantic authority (see `workflows/coding.md` → Planning-Language Authority):

1. **Create the EN semantic draft** at `.dev/plans/<plan-slug>.en.md` — the sole planning-stage semantic authority, written in English, holding the full technical meaning.
2. **Render the localized source plan** `.dev/plans/<plan-slug>.md` from the EN draft: narrative prose in the resolved language, machine anchors (headings, paths, IDs, verdict literals, metadata keys) kept English (no rename). Attach the planning-authority metadata block (see `templates/plan.md`).
3. **Stamp the hashes** by running the internal `gal planning-stamp` after the render, so the localized metadata block's `draft-hash` and `rendered-source-hash` are written deterministically — never hand-compute hashes.

**Reconcile preflight (on re-entry).** If the EN draft already exists and the localized source has been hand-edited, compute the localized rendered-source hash and compare it to the metadata. On mismatch, STOP at a read-only reconcile preflight before any rewrite: diff the current localized source against a fresh render of the EN draft, merge the human's semantic intent back into the EN draft, then re-render the localized source and re-run `gal planning-stamp`. Never silently overwrite the user's localized edits.

For English (`en`-prefix) planLanguage there is no draft: `.dev/plans/<plan-slug>.md` is itself the EN source plan (single-file fast path).

Source plans are human-readable artifacts. Prefer charts, tables, diagrams, trees, and flow maps when they reduce reread cost.

The source plan is the human-readable plan document for:

- goal and rationale
- scope boundaries
- requirements
- approach
- risks and open questions
- approval state

If assumptions remain unresolved, record them in `## Open Questions` with stable `OQ-NN` IDs marked as raised by `planning`. OQ IDs use two-digit zero-padded format — see `conventions/open-questions.md`.

For structural/multi-file/Protected-Path plans, populate the `## Diagrams` section with a text flowchart or description that aids comprehension. Depth-scaled: obvious local-fix plans may leave this section empty or omit it entirely.

Do not create `.dev/plans/<plan-slug>.prompt.md` in this command.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the row points at `.dev/plans/<plan-slug>.md` until `/plan-to-prompt` creates the execution prompt.

## Step 4 — Handoff

Before handoff, run:

```powershell
gal planning-check .dev/plans/<plan-slug>.md --receipt .dev/pipeline/receipts/planning-check.receipt.md
```

Read the receipt. If the receipt is `fail` or `not-run`, stop and fix the source plan before handoff.


Tell the user:

- which source plan file was written
- the chosen `plan-slug`
- whether the plan should go through `/deep-planning` for architect review first, or can move on to `/refining-plan` to lock the implementation contract
- whether the plan should invoke a specific domain lane directly against the source plan when business or design review is needed without an architect-led `/deep-planning` pass
- whether the next concern is architecture convergence or locking the implementation contract
