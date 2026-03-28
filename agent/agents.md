# Agents

Golem agent definitions for the Coding Flow state machine.
These are `.agent.md` files for VS Code Copilot custom agents.

## Golem Classifications

| Classification | Bound to State Machine? | Agents |
| --- | --- | --- |
| **Workflow** | Yes — bound to specific states | planner, implementer, tester, reviewer, verifier |
| **Utility** | No — callable at any tier | debugger, scribe |
| **Domain** | No — bound to workflow, not single state | architect, analyst, librarian (cross-workflow capable) |

## Why GAL Uses 10 Agents

GAL keeps these roles separate on purpose.

- Smaller prompts keep responsibilities legible and reduce context waste.
- Independent tester/reviewer/verifier roles make verification more credible than one mega-agent doing everything.
- Tiering means only the needed subset is active for a task; the system does not expect all 10 roles every time.
- Domain and utility roles remain reusable across workflows without forcing state transitions.

## Agents → Workflow States

| Agent | Classification | Workflow State | Purpose |
| --- | --- | --- | --- |
| [golem-planner](golem-planner.agent.md) | Workflow | PLAN | Analyze requirements, produce plan files |
| [golem-architect](golem-architect.agent.md) | Domain | DISCUSS / consult | Adversarial plan review — trade-offs, over-engineering, bugs |
| [golem-analyst](golem-analyst.agent.md) | Domain | DISCUSS (conditional) | Business logic review — ROI, domain correctness, user impact |
| [golem-implementer](golem-implementer.agent.md) | Workflow | IMPLEMENT | Execute approved plans with atomic commits |
| [golem-tester](golem-tester.agent.md) | Workflow | TEST | Write tests from spec only (never reads implementation) |
| [golem-reviewer](golem-reviewer.agent.md) | Workflow | REVIEW | Review for bugs, security, architecture, conventions |
| [golem-verifier](golem-verifier.agent.md) | Workflow | VERIFY | Goal-backward verification + plan lifecycle ending |
| [golem-debugger](golem-debugger.agent.md) | Utility | *(any)* | Scientific method bug investigation |
| [golem-scribe](golem-scribe.agent.md) | Utility | *(any)* | End-of-day diary + shutdown enforcer |
| [golem-librarian](golem-librarian.agent.md) | Domain | *(any)* | Obsidian vault writes — inbox processing, knowledge extraction |

## DISCUSS: Review Pack (T2 Only)

T2 uses a **composable review pack** — not a fixed dual-review. The pack is assembled per task:

- **Architect-full** (always): trade-off analysis, over-engineering, bug surface, public API risk
- **Analyst** (conditional): only when task involves business rules, pricing, permissions, or customer-visible changes
- **Others** (optional): reviewer, debugger — added when task type warrants it

Entry to IMPLEMENT requires **all pack members APPROVE**. If analyst is not in the pack, analyst approval is not needed.

T0/T1 skip DISCUSS entirely. T1 gets architect-lite by default (structure risk only).

## Direct Agent Invocation

| Type | Allowed? | Rule |
| --- | --- | --- |
| **Consult** (Domain) | Yes | Read-only advice, no formal verdict. e.g., `gal ask architect` |
| **Utility** | Yes | Independent of workflow state. e.g., `gal run debugger`, `gal run scribe` |
| **Workflow-bound** | Consult-only unless already bound | Explicit targeting is allowed for consultation, but state transitions still go through workflow control. |
| **Librarian** | Yes | Vault writes on demand. e.g., `@golem-librarian inbox`, `@golem-librarian extract`. Requires `start-implementation` for vault writes. |

Consult output is advice, not an APPROVE/REVIEW verdict. Formal verdicts come from DISCUSS/REVIEW states only.
Workflow golems may be explicitly named, but doing so does not skip PLAN, TEST, REVIEW, or VERIFY gates.

## Model Role Enforcement

Per [model-roles.md](../model-roles.md):

- **Tester must be a different model from implementer** — independent verification
- **Reviewer should differ from implementer** — fresh perspective
- **Reviewer model tier ≥ implementer model tier** — reviewer must be at least as capable

## Activation Principles

- **Minimum viable set**: Only activate golems needed for the current tier and task.
- **Context budget ~15%**: Each golem's loaded context (agent prompt + project files) should stay under ~15% of available context window.
- **Independent operation**: Each golem can operate with only its agent file + `.dev/project.md` + the current plan. No golem depends on another golem's chat history.

## Installation

These agents are symlinked to `~/.copilot/agents/` by `scripts/Setup-Machine.ps1`.
VS Code Copilot discovers them as custom agents in Agent Mode.

## Research Flow

The [Research Flow](../workflows/research.md) is an independent workflow for research-driven tasks.
It uses the same agents but with different activation rules:

| Research State | Agents | Notes |
| --- | --- | --- |
| RESEARCH | User / any Frontier model | No dedicated research golem |
| SYNTHESIZE | User / any Frontier model | Organize raw findings |
| REVIEW (R2) | architect, analyst (conditional) | Same review pack logic as Coding Flow |
| DOCUMENT (repo) | User / any model | Direct write to `docs/research/` |
| DOCUMENT (vault) | librarian | Requires `start-implementation` |

Research Flow can run in parallel with Coding Flow. Research output feeds into plans or vault knowledge.

## Curfew System

All agents enforce a shutdown boundary defined in `conventions/curfew.md`:

```text
... normal work ... ──── 22:00 ──── shutdown window ──── 23:00 ──── hard curfew
                       │                                    │
                       └─ only @golem-scribe active ──────────┘ all agents refuse
```

- **22:00**: Non-scribe agents block if today's diary is unwritten. Redirect to `@golem-scribe`.
- **23:00**: ALL agents refuse work, including scribe. No exceptions.
- **Override**: User can say "override curfew" — single-use, does not persist.
