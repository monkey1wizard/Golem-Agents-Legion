---
name: planning
description: "GAL planning ($planning / /planning / 規劃 / 新計劃 / plan this / make a plan). Creates .dev/plans/<type>-<slug>.md source plan from a new request. Does NOT generate the execution prompt — use $deep-planning when structural review is needed, complete any placeholder implementation contract per plugins/gal-core/templates/plan.md, then use $plan-to-prompt after its gates pass."
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

### Non-English planLanguage: Single-File Source Plan

When the resolved language is **not** English (`planLanguage` normalizes to a non-`en` prefix), write one source plan at `.dev/plans/<plan-slug>.md`. Write narrative prose in the resolved language. Keep machine anchors in English, including section headings, file paths, commands, code spans, task and test IDs, review-verdict literals, and metadata keys. The source plan is the semantic authority. Do not create an EN draft, render a second file, or run `gal planning-stamp` for new plans.

For English (`en`-prefix) `planLanguage`, use the same single-file source plan. Legacy plans that already carry a `gal:planning-authority` block remain subject to the existing localized-metadata validation; do not remove or rewrite legacy metadata as part of ordinary planning.

Source plans are human-readable artifacts. Prefer charts, tables, diagrams, trees, and flow maps when they reduce reread cost.

The source plan is the human-readable plan document for:

- goal and rationale
- scope boundaries
- requirements
- approach
- risks and open questions
- approval state

If assumptions remain unresolved, record them in `## Open Questions` with stable `OQ-NN` IDs marked as raised by `planning`. OQ IDs use two-digit zero-padded format — see `conventions/open-questions.md`.

For structural/multi-file/Protected-Path plans, populate the `## Diagrams` section with a Mermaid `flowchart TD` or description that aids comprehension. Use `text-flowcharts` for terminal plain-text diagrams. Depth-scaled: obvious local-fix plans may leave this section empty or omit it entirely.

Do not create `.dev/plans/<plan-slug>.prompt.md` in this command.

## Step 3 — Update Repo State

Update `.dev/state.md` `## Active Plans` so the row points at `.dev/plans/<plan-slug>.md` until `/plan-to-prompt` creates the execution prompt.

## Step 4 — Handoff

Before handoff, run:

```powershell
gal planning-check .dev/plans/<plan-slug>.md
```

Read the complete receipt. If this is the outer workflow invocation, create one stable `invocation_id` and set the consumed format-repair round count to `0`. Otherwise carry both forward without reset. Preserve both across nested gate reruns and entries, with a maximum of two consumed rounds. For an eligible, bound `planning` receipt whose failed or not-run rows are all `format`, follow [Bounded Format Recovery](../../conventions/token-budget.md#bounded-format-recovery): invoke the repair helper with that receipt's exact target, receipt, repair, gate, mode, gate attempt, and repair round values, then rerun `gal planning-check` on the same source plan and require a fresh verified pass. Catalogued recovery requires the source plan to be tracked, committed, clean, and have no staged entries because `gal format-repair` refuses other target states. If this precondition does not hold, stop and ask the owner to commit the source plan. Then rerun `gal planning-check` and apply recovery only to a fresh eligible receipt. This stop consumes no round. A document round consumes one count after the helper exits, whether it succeeds or fails. When both rounds are consumed, follow [Bounded Format Recovery's stop report table](../../conventions/token-budget.md#stop-and-next-action) and do not repair again.

For any eligible phase-payload write-back in this invocation, pass `--format-repair-context '{"invocation_id":"<invocation_id>","round":<next_round>}'` only to `gal pipeline <prompt>`. Set `<next_round>` to the consumed count plus one. When both rounds are consumed, omit the option. Never pass it to executor raw arguments or `gal format-repair`. A payload round consumes a count only when the runtime prints `FORMAT_REPAIR`. Ambiguity refusal prints no such line and consumes no round.

Any `owner` row, missing or ambiguous marker/content, stale or unbound evidence, helper refusal/failure, failed rerun, or uncertainty stops handoff. For an `owner` row or helper refusal/failure, follow [Bounded Format Recovery's stop report table](../../conventions/token-budget.md#stop-and-next-action). Missing or ambiguous markers/content and stale or unbound evidence are not formatting defects and stop recovery. Recovery may repair supported formatting only. It cannot create missing content, approval, or evidence. Preserve the failed receipt and source language reconciliation rules. Do not use a failed receipt as a pass.
The default evidence path is `.dev/pipeline/<plan-scope-key>/planning-check.receipt.md`.


Tell the user:

- which source plan file was written
- the chosen `plan-slug`
- whether the plan needs `/deep-planning` for architect review, and whether `## Tasks` or `## Test Plan` still has placeholders that a producer must complete per `plugins/gal-core/templates/plan.md`; `/refining-plan` is one available producer
- whether the plan should invoke a specific domain lane directly against the source plan when business or design review is needed without an architect-led `/deep-planning` pass
- whether the next concern is architecture convergence or completing the implementation contract
