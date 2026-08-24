---
name: deep-planning
description: "GAL deep-planning ($deep-planning / /deep-planning / 深度規劃 / architect review / structural plan). Expand-then-converge architectural review: architect always, analyst/designer conditional. Required for Protected Paths or structural changes. Writes ARCH_REVIEW: CLEAR before $refining-plan may proceed."
---

# /deep-planning

Refine planning-stage material into a formal source plan.

## Role

Planning refiner. Your job is to take rough planning material and converge it into a formal, scoped, review-ready source plan.
This command includes an architect review pass before the plan is treated as implementation-ready.

## When to Use

- A source plan exists but needs restructuring, splitting, or convergence
- The input is an external planning document, research memo, architecture draft, or other planning-stage text document
- The user wants another pass before prompt generation or planning-stage review lanes
- The task changes shared structure, dependencies, public interfaces, or other architecture-sensitive areas

## Order Contract

**Expand FIRST, converge AFTER** — this ordering is intentional and must not be reversed. This is the deep-planning instance of the shared **Planning Order Principle** ([`workflows/coding.md`](../../workflows/coding.md) → Planning Reviews) — understand → diverge (expand) → converge; never converge-first; depth-scaled. `/deep-planning` runs the deepest form of the loop.

Phase A (Steps 1–2) is deep design **expansion**: surface data-flows, state transitions, failure modes, trust boundaries, and the full option space without pruning. Phase B (Steps 3a–3e) is **convergence**: apply the minimalism ladder and adversarial reviews to cut the expanded design back to the minimum correct implementation.

**Rationale:** models trend toward over-design during expansion. Placing the minimalism challenge and adversarial reviews *after* expansion — not before — ensures convergence pruning acts on the full design surface rather than suppressing ideas before they are evaluated. The converge gate is the shear that cuts, not a filter applied at intake.

## Step 1 — Read Planning Inputs

Read every planning-stage document the user identifies. This may include:

- `.dev/plans/*.md`
- `.dev/research/*.md`
- design notes
- external or migrated planning documents

Before using an optional capability, resolve its state through the five-state preflight in [optional-capabilities.md](../../conventions/optional-capabilities.md).

- graphify: if `graphify-out/GRAPH_REPORT.md` exists, use it as structural context; if not, degrade to native codebase reading without prompting for graph generation. If `graphify-out/GAL_GRAPHIFY_VERSION.txt` exists and the current graphify version no longer matches the stamped version while the report is not newer than the stamp, treat the report as stale-by-tool-version and continue without it.

Read `graphify-out/GRAPH_REPORT.md` if it exists. Use god nodes, communities, and surprising connections as structural context for the plan.

If `graphify-out/GAL_GRAPHIFY_VERSION.txt` exists and the `graphify` CLI is available, compare the stamped version to the current `graphify --version` output. When the versions differ and `GRAPH_REPORT.md` is not newer than the stamp file, do not rely on the report for structural context and continue with native codebase reading.

Prefer convergence over interrogation. Ask a focused question only when the plan cannot be made review-ready without resolving a security, irreversible scope, or taste decision.

## Step 2 — Phase A: Expand

Write or update `.dev/plans/<plan-slug>.md` using `templates/plan.md`.

**Phase A is expansion**: surface the full design space — data-flows, state transitions, failure modes, trust boundaries, option space. Do not prune during expansion. Convergence happens in Step 3.

For structural/multi-file/Protected-Path plans, populate the `## Diagrams` section with a text flowchart or description that aids comprehension (architecture, data-flow, state, flow). Depth-scaled: obvious local-fix plans may omit diagrams. Canonical form is monospaced text (terminal-readable, consistent with `workflows/coding.md`). Mermaid is also accepted inside `.dev/plans/`.

Before resolving language, read `~/.gal/config/config.json` as a mandatory first step when it exists. If `planLanguage` is set and there is no explicit per-invocation override, the plan output MUST use that language.

Resolve the output language for the source plan using this precedence order:

1. explicit per-invocation language directive at the top level of the user's request (for example `in zh-tw`, `in en`, or "write in English")
2. machine-local `planLanguage` from `~/.gal/config/config.json` (following the `workingHours` precedent)
3. auto-detect the narrative language of the user's request
4. fallback default `en`

Treat an explicit language directive as valid only when it is a top-level instruction for this invocation's output language. Do not treat the same string as a language switch when it appears inside described content, quoted examples, or requested documentation text.

Write the human-readable source plan entirely in the resolved language. Keep technical identifiers, file paths, command names, code snippets, and other literal machine-facing tokens untranslated. Do not awkwardly mix English prose with the resolved language inside the same narrative section.

This command may:

- narrow an over-scoped plan
- merge supporting material into one plan
- clarify architecture or task boundaries
- rewrite requirements so later reviews can operate on stable semantics
- prepare the plan for business, design, or engineering review lanes without naming a specific collaborative-tool command

Do not create or mutate `.dev/plans/<plan-slug>.prompt.md` here.

## Step 3 — Phase B: Converge (Minimalism + Adversarial Reviews)

Phase B runs after the full design is on paper. Its job is to cut the expanded plan back to the minimum correct implementation through two gates: the minimalism challenge (Step 3a*) and adversarial reviews (Steps 3a–3e).

### Step 3a* — Minimalism Challenge (All Elements, Pointer-Only)

Before running the domain reviews, apply the minimalism ladder from [`conventions/minimalism.md`](../../conventions/minimalism.md) to every proposed mechanism, file, abstraction, and dependency in the expanded plan. Stop at the first YES for each item. Remove elements that fail the ladder; record genuine uncertainty as `## Open Questions`.

Failing to run this pass before the adversarial reviews is itself an adversarial finding (the architect may flag it).

### Step 3a — Architect Review (Always Mandatory)

Read `plugins/gal-core/agents/golem-architect.agent.md` and apply its review standards to the converged source plan.

Write architecture review narrative in the same resolved language used for the source plan. Keep technical identifiers, file paths, command names, code snippets, and other literal machine-facing tokens untranslated.

Write the architect outcome back into the source plan:

- `## Review Results > ### Architecture Review`
- `## Approval > Architect review`

Map the architect verdict to the `Architect review` field deterministically, with no model inference: `APPROVE` → `[clear]`, blocking → `[blocked]`.

When the architect verdict is `APPROVE`, also emit `<!-- ARCH_REVIEW: CLEAR -->` on its own line inside `### Architecture Review`. This is the machine-readable signal (parallel to `<!-- ENG_REVIEW: CLEAR -->`) that the Architectural Escalation Fence / Protected Paths gate is satisfied — the protected-path gate binds to this verdict, not to the `/deep-planning` command, so a recorded CLEAR architect review (from here or a direct `/gal architect` write-back) lets implementation proceed without re-running `/deep-planning`.

If the architect review finds blocking issues, write `## Approval > Architect review: [blocked]`, keep the plan in deep-planning, and do not emit the marker. Revise the source plan before recommending `/plan-to-prompt`.

Architect review is the mandatory deep-planning gate before `/plan-to-prompt`.

### Step 3b — Designer Review (Content-Triggered)

If the source plan touches customer-facing flows, layout, states, components, or accessibility, run the design review lane concurrently against the source plan through the configured collaborative tool or fallback golem.

Write any design-review narrative back in the same resolved language used for the source plan, while preserving literal technical identifiers and code snippets, targeting `## Review Results > ### Design Review`. Write the verdict to `## Approval > Design review`, mapped deterministically with no model inference: approve → `[clear]`, block → `[blocked]`, lane not triggered by this content → `[not-requested]`.

### Step 3c — Analyst Review (Content-Triggered)

If the source plan touches business rules, pricing, permissions, notifications, onboarding, or eligibility, run the business review lane concurrently against the source plan through the configured collaborative tool or fallback golem.

Write any business-review narrative back in the same resolved language used for the source plan, while preserving literal technical identifiers and code snippets, targeting `## Review Results > ### Business Review`. Write the verdict to `## Approval > Business review`, mapped deterministically with no model inference: approve → `[clear]`, block → `[blocked]`, lane not triggered by this content → `[not-requested]`.

## Step 3d — OQ-Completion Gate + Internalization

OQ IDs use two-digit zero-padded format (`OQ-NN`, never `OQ-NNN`) — see `conventions/open-questions.md`.

Before deep-planning can hand off to `/refining-plan`, the source plan's `## Open Questions` must be **fully resolved**, each entry closed by its **authority class** (canonical rule: [`conventions/open-questions.md`](../../conventions/open-questions.md)): the **architect** classifies and closes **Class A** (technical trade-off, with recorded rationale) and **Class F** (false OQ — proven single answer); **Class H** (human authority — taste / value / trust boundary / scope) is surfaced and recommended but **only the human closes it** — an override-able default does **not** close an H entry; **doubt → H**. An unresolved OQ blocks refining.

When an OQ is resolved, **internalize it**: fold the decision into the plan body (Approach / Requirements / the relevant section) and **remove the OQ entry**. Do not leave `[x] resolved by …` breadcrumbs or `see OQ-NN` cross-references — git history is the audit trail; the plan body stays single-source and readable. When all are resolved, `## Open Questions` reads `None`.

## Step 3e — Steward Doc-Structure Adversarial Review

Read `plugins/gal-core/agents/golem-steward.agent.md` and apply its doc-structure review to the **plan document itself** — not to code, not to knowledge extraction (that is a `/gal finalize` responsibility; deep-planning has no implementation knowledge to extract yet).

Steward scope at this step is limited to:

- Is the plan file at the correct path (`.dev/plans/<slug>.md`) and slug format?
- Is the plan structure complete (all required sections present, no orphan or duplicate in `.dev/plans/`)?
- Is the plan language consistent (no English/zh-TW mixing in narrative prose)?
- Are diagrams present and in sync with the plan content when the plan warrants them (depth-scaled)?
- Is the plan free of discussion drift and brand/name residue (old names embedded in prose)?

Write the steward outcome back into the source plan `## Review Results` as `### Documentation Structure Review (steward)`. If the steward review finds issues, revise the plan before emitting Step 3d's OQ-completion gate.

**Lifecycle note:** "plan's durable knowledge must land in `docs/` before the plan file may be deleted" is a `/gal finalize` gate (Sequence 5 evidence-gate), not a deep-planning concern. Deep-planning has no implementation knowledge to extract; that gate fires at finalize closeout.

## Step 3f — Non-English planLanguage: Reconcile Into The EN Draft

When `planLanguage != en` (see `workflows/coding.md` → Planning-Language Authority), deep-planning refines the **EN semantic draft** (`.dev/plans/<slug>.en.md`), never the localized source directly — the EN draft is the single semantic authority.

**Reconcile diff-base (fixed).** On entry, compute the localized rendered-source hash and compare it to the metadata. On mismatch (the human hand-edited the localized source), the diff-base is **`render(EN draft)`** — diff the current localized source against a fresh render of the current EN draft, treat the divergence as human semantic intent, and merge it back into the EN draft. Do not store a third last-rendered snapshot; the EN draft plus a fresh render is the only comparison base.

After the architect/analyst/designer review write-backs and the reconcile, re-render the localized source and run the internal `gal planning-stamp` so the metadata `draft-hash` / `rendered-source-hash` are re-written deterministically. Review-verdict markers stay English on the localized surface (keep, no rename).

## Step 4 — Update Repo State

If `.dev/state.md` tracks the active plan, keep it pointed at the source plan until `/plan-to-prompt` runs.

## Step 5 — Handoff

Before handoff, run:

```powershell
gal planning-check .dev/plans/<plan-slug>.md
```

Read the receipt. If the receipt is `fail` or `not-run`, stop and fix the source plan before handoff.
The default evidence path is `.dev/pipeline/receipts/<plan-scope-key>/planning-check.receipt.md`.


Tell the user:

- what changed in the source plan
- whether architect review is clear or still blocking
- whether the next action is recording human approval then `/plan-to-prompt`, another deep-planning pass, or a review lane through the configured collaborative tool or fallback golem
- if `## Tasks` and `## Test Plan` still remain placeholders in the source plan, the next step is `/refining-plan`; after the plan is converged, record human approval before `/plan-to-prompt`
