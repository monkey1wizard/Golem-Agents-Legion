# Coding Flow

The primary development workflow is command-driven, not dispatcher-state-driven.
Every AI agent follows the same artifact model: source plans live in `docs/plans/`, execution prompts live in `.dev/plans/`, repo continuity lives in `.dev/state.md`, and specialist commands write back to the active execution prompt.

Cross-model verification remains the default guardrail: planning critique, testing, and review should be done by different models whenever a separate capable model is available.

## Risk Weight

Not every change needs the same process. **Risk weight** determines how many guardrails to apply.

| Weight | When | Recommended Flow | Plan? | Architect | Designer | Analyst | Reviewer |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Trivial** | Typo fix, obvious bug, single-file edit | Direct implement, optional `/review`, optional `/qa` | No | No (consult OK) | No | No | No |
| **Standard** | Small feature, known-cause bug fix, 2–3 files | `/planning` → `/plan-to-prompt` → engineering review lane → implement → `/review` → conditional `/design-review` or `/cso` → `/qa` → `/ship` | Lightweight | Lite (default on) | No | No | Mandatory (lite) |
| **Strategic** | New feature, arch change, high-risk, cross-cutting | `/planning` → `/deep-planning` → `/plan-to-prompt` → full review pack → implement → `/review` → conditional `/design-review` or `/cso` → `/qa` → `/ship` | Full | Full (mandatory) | Full (mandatory) | Conditional | Full |

**Upgrade rule**: Any change can escalate to Strategic mid-flight if complexity exceeds expectations. Stop, refine the plan, and engage the full review pack.

Risk weight is about how many guardrails the task needs, not whether the task is morally "important." Trivial minimizes ceremony; Standard adds cheap structural protection; Strategic buys explicit review gates when a wrong move would be expensive.

## Execution Lifecycle

GAL's coding flow is expressed as artifact-producing command phases.

| Phase | Entry Signal | Commands | Main Artifacts |
| --- | --- | --- | --- |
| **Draft plan** | No active plan, or an existing plan needs reset | `/planning`, `/deep-planning`, `/plan-to-prompt` | `docs/plans/<slug>.md`, `.dev/plans/<slug>.prompt.md`, `.dev/state.md` active plan row |
| **Planning reviews** | Execution prompt exists, buildability not yet locked | Business, design, and engineering review lanes via provider or fallback golems | `## Open Questions`, `## Tasks`, `## Review Results`, `## Test Plan` |
| **Implementation** | Tasks exist and work remains | Manual execution or `/gal pipeline` | `## Status`, `## Tasks`, code changes |
| **Review-stage audits** | Implementation reached a meaningful checkpoint | `/review`, conditional `/design-review`, conditional `/cso`, `/qa`, `/qa-only` | `## Analyze`, `## Review Results`, `## Test Results` |
| **Wrap-up or ship** | Work is paused or ready to land | `/gal wrap-up`, `/ship`, `/land-and-deploy` | `### Handoff Notes`, `.dev/state.md`, `## Ship`, `## Deploy` |

The `Workflow:` field inside `## Status` is a **plan phase marker**, not a dispatcher-owned state machine. It may be useful for humans and specialist commands, but readiness is determined by the presence and contents of plan artifacts such as `## Tasks`, `## Analyze`, `## Review Results`, and `## Test Results`.

`/review` and `/design-review` both belong to the post-implementation review stage. `/review` audits correctness, completeness, and drift in the diff; `/design-review` audits the running UI against `DESIGN.md`; `/cso` is the security audit for branches that touch auth, data handling, input handling, or public API surface.

## Review Pack (Strategic)

Strategic weight uses a fixed core review pack plus conditional specialists.
Current default policy includes `designer` in every Strategic review, even for technical tasks.

### Always included

- **Architect-full**: trade-off analysis, over-engineering, bug surface, dependency pollution, public API risk.
- **Designer**: visual direction, UX flow, accessibility, interaction clarity, and design-system consistency. If the task has no meaningful UI surface, return a no-impact verdict instead of inventing issues.

### Conditionally included

- **Analyst**: Only when the task involves business rules, pricing/billing, permission/policy, notification behavior, onboarding/funnel, eligibility/approval, or any change to customer-visible outcomes.
- **Reviewer**: When implementation is large or touches security-sensitive code.
- **Debugger**: When the task involves complex integration or known fragile areas.

### Entry to implementation

The condition is: **all required reviewers in the pack APPROVE**. If analyst is not in the pack, analyst approval is not needed.

### When analyst is NOT needed (Strategic examples)

- Pure technical refactoring
- Infrastructure / build / CI changes
- Performance optimization
- Dependency upgrades
- Bug fixes where business semantics don't change

Strategic weight does not automatically imply analyst involvement. Strategic means technical or delivery risk; analyst is added only when the change also carries business or customer-facing meaning.

## Architect Modes

| Mode | Weight | Scope |
| --- | --- | --- |
| **Lite** | Standard (default on) | Structure risk only: cross-layer, DI/interface/public API, protected paths, obvious over-engineering |
| **Full** | Strategic (mandatory) | Complete trade-off review: architecture fit, complexity budget, trade-offs, bug surface, performance, security |
| **Consult** | Any | Human-initiated, no formal verdict. Ask architect for advice without entering the formal review pack. |

This split exists because architect review is most useful when it is frequent enough to catch drift but not so heavy that trivial work pays a Strategic-weight tax.

## Scope Fence (Trivial/Standard)

Trivial and Standard weight changes lack the full strategic review pack. To prevent accidental architecture damage, a dual-layer defence applies.

### Layer 1 — Implementer Scope Fence (Prevention)

The implementer's instruction set includes a **Trivial/Standard prohibited operations list**. Triggering any item requires stopping and requesting a Strategic upgrade:

- Create or delete project files (.csproj, .sln, package.json, etc.)
- Add or remove package dependencies
- Move files across architecture layers
- Create new interfaces or abstract base classes
- Modify DI registrations or service composition
- Change public API signatures used by 2+ consumers
- Introduce new design patterns
- Modify shared/core/base classes used by 3+ consumers

### Layer 2 — Standard Mandatory Reviewer (Detection)

Standard weight's reviewer is mandatory (not optional), but checks only two dimensions: **correctness + architecture**. This catches Scope Fence violations the implementer missed without the overhead of a full OWASP scan.

## Session Safety Mode

`/guard` is a high-risk-only safety mode. Use it when the work touches production systems, live data, shared risky config, or any task where you want `/careful` plus a strict edit boundary from `/freeze` in one command.

For normal feature work, `/careful` is the lighter default. `/guard` should not be treated as a universal readiness step.

## Protected Paths

Each repo's `.dev/project.md` contains a `## Protected Paths` section listing architecture-critical files. Touching any protected path during Trivial or Standard work **automatically triggers a Strategic upgrade**.

## Plan Lifecycle: Transient Task Memory

Plans are temporary work files, not permanent records. `docs/plans/` is a staging area.

### Lifecycle

1. **`/planning` creates the source plan** → `docs/plans/<type>-<slug>.md`
2. **`/deep-planning` refines the source plan when needed** → keeps scope and rationale review-ready
3. **`/plan-to-prompt` materializes the execution prompt** → `.dev/plans/<type>-<slug>.prompt.md`
4. **The execution prompt self-tracks progress** → `## Status` carries phase markers, step, deviations, and decisions
5. **Implementation updates progress** during execution (not `.dev/state.md`)
6. **Testing and review write results** → execution prompt `## Test Results`, `## Review Results`, `## Analyze`
7. **Verification confirms the goal** → extracts knowledge to `docs/`, marks the plan ready for closure
8. **Plan is deleted after lifecycle closure** → task memory returns to zero, no orphaned state

### Plan Filename Convention

```text
docs/plans/<type>-<slug>.md
.dev/plans/<type>-<slug>.prompt.md
```

Type prefixes: `feat-`, `fix-`, `refactor-`, `sec-`, `perf-`, `infra-`

### Safety Rules

- **Never delete a plan before verification and handoff are complete** — the plan is the spec; early deletion breaks review, QA, and resumption.
- **Plan is pipeline, docs/ is sink** — valuable knowledge flows from the plan into long-lived docs. The plan itself is disposable only after that extraction.

## State File Role

`.dev/state.md` is a **global index + session continuity** file, not a per-task tracker.

| Responsibility | Where |
| --- | --- |
| Per-task phase marker, step progress, deviations | **Plan file** `## Status` |
| Active plans index (which branches have active plans) | **state.md** |
| Repo-level blockers, cross-plan decisions | **state.md** |
| Session continuity (last session, stopped at, next step) | **state.md** |

## Context Handoff

Before switching worktrees or ending a session:

1. Compress key context into the plan's `## Status > ### Handoff Notes`
2. Update `.dev/state.md` Session Continuity section
3. Commit changes to the current branch

This is manually triggered — the AI does not know when you're switching context unless you record it.

## Direct Agent Invocation

| Type | Allowed | Examples |
| --- | --- | --- |
| **Consult** | Yes — read-only or scoped advice, no implied phase transition | `/gal [ask architect]`, `/gal [ask analyst]` |
| **Utility** | Yes — independent helper | `/gal [run debugger]`, `/gal [run scribe]` |
| **Pipeline** | Direct invocation is consult-only | `/gal [golem-tester]`, `/gal [golem-reviewer]` for scoped advice; full execution authority comes from `/gal pipeline` |

Consult output is advice, not a formal APPROVE or REVIEW verdict. Formal outcomes come from the specialist commands and the artifacts they write.

## Per-Phase Model Assignment

Model assignment is defined in [model-roles.md](../model-roles.md).

Key rules:

- **The planning author and architect should differ when practical** — cross-check the plan before implementation
- **The planning author and designer should differ when designer is a formal reviewer** — keep design critique independent from plan authorship
- **Implementer and tester must be different models** — independent verification
- **Reviewer should differ from implementer** — fresh perspective
- **Reviewer should also differ from tester when practical** — review should be higher-level than test generation
- **Reviewer model tier ≥ implementer model tier** — the reviewer must be at least as capable
- **Reviewer model tier ≥ tester model tier** — review should be at least as capable as test generation and usually stronger
