# Roadmap Template

Template for `<repo>/.dev/roadmap.md` — project-level delivery picture.

This file shows the phased delivery plan across milestones. It is not a task list — plan files handle individual task execution. The roadmap answers "what ships when and in what order."

## File Template

```markdown
# Roadmap

## Current Phase

**Phase [N]: [Name]** — [one-line description of this phase's thrust]

## Phases

### Phase 1: [Name]

Status: [not-started | in-progress | complete]
Target: [YYYY-MM-DD or "undefined"]

Key deliverables:
- [ ] [Deliverable 1 — user-observable capability or milestone]
- [ ] [Deliverable 2]

Plans:
- [ ] [`.dev/plans/plan-name.prompt.md`] — [brief purpose]

### Phase 2: [Name]

Status: not-started
Target: undefined

Key deliverables:
- [ ] [Deliverable]

## Milestones

| Milestone | Phase | Target Date | Status |
| --- | --- | --- | --- |
| [Milestone name] | Phase 1 | YYYY-MM-DD | not-started |

## Deferred Work

The following was considered and intentionally pushed out of scope:

- [Feature or phase] — [reason deferred, and condition that would re-activate it]

## Change Log

| Date | Change | Author |
| --- | --- | --- |
| YYYY-MM-DD | [Phase added, reordered, or scope changed] | [human or AI session] |
```

## Usage Rules

1. Add a new `### Phase N` block when a new phase is scoped. Do not reuse phase numbers.
2. Mark deliverables complete (`- [x]`) when the plan that implements them reaches DONE.
3. Move phases to **Deferred Work** rather than deleting them when they are cut.
4. The **Milestones** table is optional — use it only when external commitments (releases, deadlines) exist.
5. This file does not replace plan files. Link related plans under each phase for traceability.
