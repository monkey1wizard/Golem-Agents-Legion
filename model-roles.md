# Model Roles

Define roles by **what they do**, not by which model they are.
When you switch AI tools, update the mapping table — everything else stays the same.

Default principle: formal cross-checks should use a different model from the one that authored the artifact whenever practical.

## Roles

| Role | Purpose | Key Trait |
| --- | --- | --- |
| PLANNER | Analyze requirements, produce plan files | Broad reasoning, architecture awareness |
| ARCHITECT | Adversarial plan review — trade-offs, over-engineering, bugs | Critical thinking, minimalism, direct communication |
| ANALYST | Business logic review — ROI, domain correctness, user impact | Commercial awareness, domain expertise |
| DESIGNER | Review visual design, UX flow, accessibility, and design-system consistency | Experience design judgment, user empathy |
| RESEARCHER | Investigate unknowns, synthesize findings, and prepare research outputs | Evidence gathering, source attribution, synthesis |
| CODER | Write implementation code following a plan | Code generation, refactoring |
| TESTER | Write tests from plan spec + public API only | Spec-driven, usually basic unit/integration coverage |
| REVIEWER | Review code for bugs, security, style | Higher-level critical eye, different perspective |
| SCRIBE | End-of-day diary, shutdown enforcement | Summarization, Obsidian integration |
| LIBRARIAN | Obsidian vault writes — inbox processing, knowledge extraction | Guide.md compliance, knowledge classification |
| LOCAL | Tasks requiring privacy or local language | Runs on-device, no data leaves machine |

## Model Tiers

Models are classified by capability tier. This drives activation rules.

| Tier | Label | Roles | When to Use |
| --- | --- | --- | --- |
| 1 | **Frontier** | PLANNER, ARCHITECT, ANALYST, DESIGNER, RESEARCHER | Strong reasoning required — planning, trade-off analysis, business logic, design critique, research synthesis |
| 2 | **Standard** | CODER, TESTER, REVIEWER | Execution tasks — implementation, testing, code review |
| 3 | **Low-cost** | SCRIBE, LIBRARIAN, LOCAL | Low-stakes tasks — diary writing, vault writes, quick notes, offline |

### Tier Rules

- **Reviewer model tier ≥ implementer (CODER) model tier** — the reviewer must be at least as capable as the code it reviews.
- **Reviewer model tier ≥ tester model tier** — review should be at least as capable as test authoring, and usually stronger.
- **Architect-full requires Tier 1** — trade-off analysis needs frontier reasoning.
- **Architect-lite can use Tier 2** — structural checks don't need full reasoning power.
- **Within the same tier, prefer model diversity** — different models catch different blind spots.

## Routing Rules

1. **PLAN review is a different-model check** — `PLANNER` and `ARCHITECT` must be different models.
2. **CODER and TESTER must be different models** — independent verification
3. **REVIEWER should differ from CODER** — fresh perspective catches blind spots
4. **REVIEWER should also differ from TESTER when practical** — review is a higher-level check than test generation
5. **DESIGNER should differ from CODER when used as a formal reviewer** — keep experience review independent from implementation
6. **RESEARCHER owns research and synthesis** — `ARCHITECT`, `ANALYST`, and `DESIGNER` join later as review specialists
7. **LIBRARIAN** follows Guide.md for all vault writes — any model tier works, but must load Guide.md context first
8. **LOCAL** is for privacy-sensitive data or Traditional Chinese tasks
9. When switching tools, update the **Current Mapping** table below only

## Architect Activation by Tier

| Coding Flow Tier | Architect Mode | Model Tier Required |
| --- | --- | --- |
| T0 (Trivial) | Off (consult OK) | — |
| T1 (Standard) | Lite (default on) | Tier 2+ |
| T2 (Strategic) | Full (mandatory) | Tier 1 |

## Review Pack Rules (T2)

The T2 review pack has a fixed core plus conditional specialists:

| Reviewer | When Included | Verdict Required? |
| --- | --- | --- |
| **Architect-full** | Always | Yes — APPROVE required |
| **DESIGNER** | Always | Yes — APPROVE required |
| **Analyst** | Business rules, pricing, permissions, customer-visible changes | Yes — when included |
| **Reviewer** | Large implementation, security-sensitive code | Yes — when included |
| **Debugger** | Complex integration, known fragile areas | Advisory only |

Entry to IMPLEMENT: **all required reviewers APPROVE**. If analyst is not in pack, analyst approval not needed.

## Typical Workflow (Single Developer)

```text
1. PLAN    → Use a frontier-tier model (interactive or async), then cross-check it with a different model in T2
2. IMPLEMENT → Use a standard-tier coding agent — follow the approved plan
3. TEST    → Use a DIFFERENT model — feed it plan + public interfaces only; this is usually basic unit/integration coverage
4. REVIEW  → Use a DIFFERENT model again — bugs, security, architecture; this should be a higher-level check than TEST
5. VERIFY  → Run full test suite, confirm all plan items implemented
```

---

## Your Setup

Copy [`model-roles.example.md`](model-roles.example.md) to `model-roles.local.md` and customize
with your own machines, models, and tools. The local file is git-ignored.
