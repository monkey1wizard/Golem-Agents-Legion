# Model Roles

Define roles by **what they do**, not by which model they are.
When you switch AI tools, update the mapping table — everything else stays the same.

## Roles

| Role | Purpose | Key Trait |
| --- | --- | --- |
| PLANNER | Analyze requirements, produce plan files | Broad reasoning, architecture awareness |
| ARCHITECT | Adversarial plan review — trade-offs, over-engineering, bugs | Critical thinking, minimalism, direct communication |
| ANALYST | Business logic review — ROI, domain correctness, user impact | Commercial awareness, domain expertise |
| CODER | Write implementation code following a plan | Code generation, refactoring |
| TESTER | Write tests from plan spec + public API only | Spec-driven, does NOT read implementation |
| REVIEWER | Review code for bugs, security, style | Critical eye, different perspective |
| SCRIBE | End-of-day diary, shutdown enforcement | Summarization, Obsidian integration |
| LOCAL | Tasks requiring privacy or local language | Runs on-device, no data leaves machine |

## Model Tiers

Models are classified by capability tier. This drives activation rules.

| Tier | Label | Roles | When to Use |
| --- | --- | --- | --- |
| 1 | **Frontier** | PLANNER, ARCHITECT, ANALYST | Strong reasoning required — planning, trade-off analysis, business logic |
| 2 | **Standard** | CODER, TESTER, REVIEWER | Execution tasks — implementation, testing, code review |
| 3 | **Low-cost** | SCRIBE, LOCAL | Low-stakes tasks — diary writing, quick notes, offline |

### Tier Rules

- **Reviewer model tier ≥ implementer (CODER) model tier** — the reviewer must be at least as capable as the code it reviews.
- **Architect-full requires Tier 1** — trade-off analysis needs frontier reasoning.
- **Architect-lite can use Tier 2** — structural checks don't need full reasoning power.
- **Within the same tier, prefer model diversity** — different models catch different blind spots.

## Routing Rules

1. **CODER and TESTER must be different models** — independent verification
2. **REVIEWER should differ from CODER** — fresh perspective catches blind spots
3. **LOCAL** is for privacy-sensitive data or Traditional Chinese tasks
4. When switching tools, update the **Current Mapping** table below only

## Architect Activation by Tier

| Coding Flow Tier | Architect Mode | Model Tier Required |
| --- | --- | --- |
| T0 (Trivial) | Off (consult OK) | — |
| T1 (Standard) | Lite (default on) | Tier 2+ |
| T2 (Strategic) | Full (mandatory) | Tier 1 |

## Review Pack Rules (T2)

The T2 review pack is composed per task, not fixed:

| Reviewer | When Included | Verdict Required? |
| --- | --- | --- |
| **Architect-full** | Always | Yes — APPROVE required |
| **Analyst** | Business rules, pricing, permissions, customer-visible changes | Yes — when included |
| **Reviewer** | Large implementation, security-sensitive code | Yes — when included |
| **Debugger** | Complex integration, known fragile areas | Advisory only |

Entry to IMPLEMENT: **all required reviewers APPROVE**. If analyst is not in pack, analyst approval not needed.

## Typical Workflow (Single Developer)

```text
1. PLAN    → Use a frontier-tier model (interactive or async)
2. IMPLEMENT → Use a standard-tier coding agent — follow the approved plan
3. TEST    → Use a DIFFERENT model — feed it plan + public interfaces only
4. REVIEW  → Use a DIFFERENT model again — bugs, security, architecture
5. VERIFY  → Run full test suite, confirm all plan items implemented
```

---

## Your Setup

Copy [`model-roles.example.md`](model-roles.example.md) to `model-roles.local.md` and customize
with your own machines, models, and tools. The local file is git-ignored.
