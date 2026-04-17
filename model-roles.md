# Model Roles

Define roles by **what they do**, not by which model they are.
When you switch AI tools, update the mapping table — everything else stays the same.

Default principle: formal cross-checks should use a different model from the one that authored the artifact whenever practical.

## Roles

| Role | Purpose | Key Trait |
| --- | --- | --- |
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

## Routing Rules

1. **Planning review should be a different-model check** — the model that critiques a plan should differ from the one that drafted it whenever practical.
2. **CODER and TESTER must be different models** — independent verification
3. **REVIEWER should differ from CODER** — fresh perspective catches blind spots
4. **REVIEWER should also differ from TESTER when practical** — review is a higher-level check than test generation
5. **DESIGNER should differ from CODER when used as a formal reviewer** — keep experience review independent from implementation
6. **RESEARCHER owns research and synthesis** — `ARCHITECT`, `ANALYST`, and `DESIGNER` join later as review specialists
7. **LIBRARIAN** follows Guide.md for all vault writes — any model tier works, but must load Guide.md context first
8. **LOCAL** is for privacy-sensitive data or Traditional Chinese tasks
9. When switching tools, update the **Current Mapping** table below only

## Planning Review Rules

`/deep-planning` is the default architect-reviewed planning pass before prompt materialization.

| Reviewer | When Included | Verdict Required? |
| --- | --- | --- |
| **ARCHITECT** | Every `/deep-planning` pass | Yes — review required before `/plan-to-prompt` |
| **ANALYST** | Business rules, pricing, permissions, customer-visible logic | Yes — when included |
| **DESIGNER** | Customer-facing flows, layout, states, component systems, accessibility-sensitive work | Yes — when included |

Implementation-stage `REVIEWER` and `DEBUGGER` remain separate specialists. They do not replace planning review.

## Typical Workflow (Single Developer)

```text
1. `/planning` or `/deep-planning` → Use a frontier-class model (interactive or async) to produce the initial plan
2. Plan reviews → Use different models for architect, design, or business critiques when practical
3. IMPLEMENT → Use a standard coding agent — follow the approved plan
4. TEST → Use a DIFFERENT model — feed it plan + public interfaces only; this is usually basic unit/integration coverage
5. REVIEW → Use a DIFFERENT model again — bugs, security, architecture; this should be a higher-level check than TEST
6. VERIFY → Run full test suite, confirm all plan items implemented
```

---

## Your Setup

Copy [`model-roles.example.md`](model-roles.example.md) to `model-roles.local.md` and customize
with your own machines, models, and tools. The local file is git-ignored.
