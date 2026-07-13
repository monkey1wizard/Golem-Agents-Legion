# Requirements Template

Template for `<repo>/.dev/requirements.md` — persistent product requirements baseline.

This file lives outside any single plan. It captures what the product must do and what it must not do, independent of how the work is scheduled. Plan files reference requirements; they do not duplicate them.

## File Template

```markdown
# Requirements

## Functional Requirements

| ID | Requirement | Status | Source |
| --- | --- | --- | --- |
| F-01 | [Observable behavior the product must have] | stable | [origin — spec, conversation, user research] |
| F-02 | [Observable behavior the product must have] | evolving | [origin] |

Status values: `stable` (unlikely to change) · `evolving` (still being refined) · `deferred` (agreed to skip for now)

## Non-Functional Requirements

| ID | Requirement | Threshold | Status |
| --- | --- | --- | --- |
| NF-01 | [Performance, scalability, security, reliability, etc.] | [Measurable threshold] | stable |

## Out of Scope

The following are explicitly excluded from this product's requirements:

- [Thing that might be assumed but is not being built] — [reason]
- [Feature deferred to a future phase] — [reason]

## Open Questions

- [ ] [Unresolved requirement that needs a decision before it can be implemented]

## Change Log

| Date | Change | Author |
| --- | --- | --- |
| YYYY-MM-DD | [What was added, changed, or deferred] | [human or AI session] |
```

## Usage Rules

1. This file captures **product-level** requirements. Do not record plan-specific acceptance criteria here — those belong in the plan's `## Requirements` section.
2. Every requirement needs an `ID`. Plan files reference requirements by ID (e.g., "implements F-01").
3. Mark requirements `deferred` rather than deleting them — deleting loses the rationale.
4. When a plan changes a requirement, update this file and add a row to the Change Log.
5. Non-functional requirements must have a measurable threshold, not just a category name.
