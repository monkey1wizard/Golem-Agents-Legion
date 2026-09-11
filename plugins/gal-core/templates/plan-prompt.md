# Plan Prompt: [Feature Name]

<!--
Documentation copy only. The execution-prompt source of truth is the embedded
template inside commands/plan-to-prompt/SKILL.template.md.
Keep this file synchronized with that embedded template.
-->

<!--
Generated from .dev/plans/<plan-slug>.md.
Output path: <repo>/.dev/plans/<plan-slug>.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal

[Copy from source plan]

## Requirements

- [ ] [Copy from source plan]

## Approach

[Compress by meaning; keep only execution-relevant constraints, ordering, and scope guards]

## Files to Create or Modify

- `[path/to/file]` — [Copy from source plan]

## Test Cases

- [ ] [Copy from source plan]

## Success Criteria

- [ ] [Copy from source plan]

## Risks

- [Copy from source plan]

## Open Questions

- [ ] OQ-01 — [Open question description] *(raised by: command)*

<!-- Format: - [ ] OQ-NN — description *(raised by: command)* -->
<!-- Resolved: - [x] OQ-NN — description *(raised by: command, resolved by: engineering-review-lane)* -->

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

[Stable planning content above this divider is seeded from the source plan and may be refreshed from planning-stage changes.]

---

[Mutable execution-owned state below this divider is shared by control-plane chat and specialist workflows. Preserve it on refresh unless the user explicitly requests a reset.]

[Execution-stage agents write progress, retry state, review/test results, and resume markers here. After a task passes implement + test + review, /gal pipeline also syncs the source-plan checkbox, commit note, and .dev/state.md continuity row.]

## Status

Workflow: DRAFT
Step: 0 of N
Last activity: YYYY-MM-DD — prompt created
Next step: [begin implementation workflow]
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0

<!-- Durable resume markers above this line drive /gal pipeline, /gal status, and /gal whats-next. -->

### Deviations

| Step | Plan Said | Actually Did | Why |
| --- | --- | --- | --- |

<!--
  R7 dispute deviation format (Why column ASCII payload for probe/implementation disputes):
  dispute_id=T-NN-DNN;generation=N;contract_digest=<sha256>;lock_refs_b64=<base64url>;evidence_refs_b64=<base64url>;basis=<contract-incomplete|contract-contradictory|contract-nonunique|probe-assertion|probe-fixture|probe-command|probe-failure-class|behavior-missing|behavior-wrong>;classification=<probe-defect|implementation-defect|contract-ambiguous>;status=<open|resolved>;retry_before=N;retry_after=N
-->

### Test-First Generations

| Task | Generation | Contract Digest | Reason |
| --- | --- | --- | --- |

### Handoff Notes

[Context from /gal wrap-up — key insights, unresolved questions, current hypothesis]

## Tasks

[Copied from the source plan after `/refining-plan` marks Engineering Review CLEAR. Implementation updates completion state only — do not rewrite task semantics.]

<!--
  Task format locked by /refining-plan:
  - [ ] T-NN — <short description>
    - Test-first: required
    - Seam: <public seam signature or location>
    - Expected failures: EF-01;class=<closed-token>;term=<nonzero|exit:N|signal:N>;stream=<stdout|stderr|combined>;matcher_b64=<unpadded-base64url-literal> — <prose>
    - Production Paths: `path/to/file`
    - Test Paths: `path/to/test`
    - Scaffold: required|not-required

  For tasks where test-first workflow is not applicable:
  - [ ] T-NN — <short description>
    - Test-first: not-applicable — <technical rationale>
-->

<!-- Format: - [ ] T-NN — task description (Verify: how to confirm done) -->

## Analyze

[Written by golem-auditor — verdict: CLEAR | DRIFT-OPEN | NOT-RUN]

<!-- Sole writer: golem-auditor. golem-releaser, /gal status, /gal whats-next consume verdict only — they do not recalculate drift. -->

## Test Plan

[Copied from the source plan after `/refining-plan` marks ENG_REVIEW CLEAR]

## Test Results

[Written by tester specialist after TEST phase]

## Review Results

### Architecture Review

Pending.

### Business Review

Pending.

### Design Review

Pending.

### Engineering Review

Pending.

[Written by planning-stage and review specialists in their owned subsections]

## Debug Log

[Written by debugger specialist if debugging occurs during this plan]
