<!--
  PLAN HYGIENE (applies to this file and all plans derived from it):
  • A plan is a SPECIFICATION, not a changelog. No process noise, self-correction breadcrumbs,
    or F-numbered wrong-guesses belong here. If a section needs updating, rewrite the whole
    section; do not annotate over stale text.
  • Language: follow token-budget.md language policy (.dev/plans/ follows per-invocation chain).
  • Diagrams (depth-scaled): include a ## Diagrams section for structural/multi-file/Protected-Path
    plans where a diagram aids comprehension. Obvious local-fix plans may omit it.
  • Localized metadata (non-English planLanguage only): see the sample block below. English
    (en-prefix) plans OMIT it — the plan is its own EN source. See workflows/coding.md →
    Planning-Language Authority.
-->

<!--
  LOCALIZED METADATA SAMPLE — non-English planLanguage only.
  A localized source plan is rendered from an EN semantic draft and carries a
  planning-authority metadata block written/refreshed by the internal
  `gal planning-stamp` (never hand-authored). English (en-prefix) plans have no
  draft and OMIT this block. Machine anchors stay English (keep, no rename); only
  narrative prose is localized.

  The block is an HTML comment whose first line is the planning-authority marker
  (the literal "gal" colon "planning-authority"), followed by these six fields
  (relative repo paths / language code / hex hashes / verdict literal) — shown
  here with the marker intentionally spelled out so this sample is not parsed
  as a real block. `prompt-hash` and `equivalence-verdict` carry the
  equivalence proof inline — there is no separate `.equiv.md` receipt file.
  Before `/plan-to-prompt` has run, both are non-empty placeholders (`none` /
  `pending`); `gal planning-stamp --equivalence <prompt>` overwrites them with
  the live prompt hash and `EQUIVALENT` once the equivalence gate passes:

    <marker>
    semantic-draft: .dev/plans/<slug>.en.md
    planLanguage: zh-TW
    draft-hash: <sha256-hex>
    rendered-source-hash: <sha256-hex>
    prompt-hash: none
    equivalence-verdict: pending
-->

# Plan: [Feature Name]

<!--
  Approval fields — fixed order, closed per-field vocabularies:
  • Human approval: [pending|approved]
  • Architect review: [pending|clear|blocked|not-required]
  • Design review: [not-requested|clear|blocked]
  • Business review: [not-requested|clear|blocked]
  Grammar: `- <label>: [<token>]`, optionally followed by ` — <reason>` (non-empty prose).
  `Architect review: [clear]` requires a standalone `<!-- ARCH_REVIEW: CLEAR -->` marker elsewhere in this plan; either side alone fails.
-->

## Approval

- Human approval: [pending]
- Architect review: [pending]
- Design review: [not-requested]
- Business review: [not-requested]

## Goal

[What this change accomplishes and why it matters — stated as a truth that must hold when done]

## Requirements

- [ ] [Requirement 1 — observable behavior]
- [ ] [Requirement 2 — observable behavior]

## Diagrams

<!-- Depth-scaled: include for structural/multi-file/Protected-Path plans where a diagram aids
     comprehension (architecture, data-flow, state, flow). Obvious local-fix plans may omit
     this section entirely. Use text flowcharts (terminal-readable, consistent with coding.md);
     mermaid is also accepted inside .dev/plans/. -->

[text flowchart or description, or remove this section for simple local-fix plans]

## Files to Create or Modify

- `[path/to/file]` — [purpose]

## Test Cases

- [Test case 1 — input → expected output]
- [Test case 2 — edge case]

## Success Criteria

- [Observable truth 1 — what must be TRUE when done]
- [Observable truth 2]

## Risks

- [Risk that might require escalation to DISCUSS state]

## Open Questions

- [ ] OQ-01 [H|A|F] — [Open question description] *(raised by: planning)*

<!-- Format: - [ ] OQ-NN [H|A|F] — description *(raised by: command)* -->
<!-- ID format: OQ-NN is a ZERO-PADDED TWO-DIGIT number (OQ-01…OQ-99). THREE-DIGIT form OQ-NNN
     is NEVER valid. Canonical rule: conventions/open-questions.md#id-format. -->
<!-- Class tag (authority class — canonical rule: conventions/open-questions.md):
     H = human authority (taste/value/trust/scope) — only the human closes; an override-able default does NOT close H.
     A = architect authority (technical trade-off) — architect role closes with recorded rationale (doubt → H).
     F = false OQ (one forced answer) — closed with evidence.
     An UNTAGGED OQ defaults to H. -->
<!-- Internalization (OQ-completion gate): when an OQ is resolved, fold the decision into the
     plan body (Approach / Requirements / relevant section) and DELETE the OQ entry, leaving the
     closer-class + rationale in `## Review Results` / git (no `[x] resolved by …` breadcrumbs or
     `see OQ-NN` cross-references). Closing is class-gated: `/refining-plan` NEVER closes (gate-check
     only); the architect role closes A/F via `/deep-planning` or `/gal architect`; the human closes H.
     `/refining-plan` refuses to run while any OQ is unresolved; when all are resolved this reads `None`. -->

## Approach

### Step 1: [Action]

- **Files**: `path/to/file`
- **What**: [Concrete description]
- **Verify**: [How to confirm this step is done]

Prefer human-readable structure in source plans: use charts, tables, diagrams, trees, or flow maps when they reduce reread cost. Use the `text-flowcharts` skill when a branchy process is easier to understand as a terminal-readable flow.

### Step 2: [Action]

- **Files**: `path/to/file`
- **What**: [Concrete description]
- **Verify**: [How to confirm this step is done]

## Review Results

### Architecture Review

Pending.

### Business Review

Pending.

### Design Review

Pending.

### Engineering Review

Pending.

## Preconditions

<!-- Optional: delete this section when the owner must supply nothing. IDs are PC-NN.
     Every cell is non-empty, not a placeholder (TBD, TODO, pending, N/A, ..., any <...> text),
     and at most 200 characters outside backticks. `Check command` must contain a backticked command. -->

| ID | Requirement | Check command | Expected result | How to satisfy |
| --- | --- | --- | --- | --- |

## Test Plan

Pending.

## Tasks

Pending.

## Owner Acceptance

<!-- Optional: delete this section unless a check needs human judgment or a human action. At most 3 items, IDs are OA-NN.
     Every cell is non-empty, not a placeholder (TBD, TODO, pending, N/A, ..., any <...> text),
     and at most 200 characters outside backticks. `What to check` must contain a backticked path, command, or URL.
     Each item lets the owner decide pass or fail without a follow-up question. -->

| ID | What to check | Expected result | Pass/fail rule | Why not automated |
| --- | --- | --- | --- | --- |
