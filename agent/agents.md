# Agents

Golem agent definitions for the Coding Flow state machine.
These are `.agent.md` files for VS Code Copilot custom agents.

## Golem Classifications

| Classification | Bound to State Machine? | Agents |
| --- | --- | --- |
| **Workflow** | Yes — bound to specific states | planner, implementer, tester, reviewer, verifier |
| **Utility** | No — callable at any tier | debugger, scribe |
| **Domain** | No — bound to workflow, not single state | architect, analyst (cross-workflow capable) |

## Agents → Workflow States

| Agent | Classification | Workflow State | Purpose |
| --- | --- | --- | --- |
| [planner](planner.agent.md) | Workflow | PLAN | Analyze requirements, produce plan files |
| [architect](architect.agent.md) | Domain | DISCUSS / consult | Adversarial plan review — trade-offs, over-engineering, bugs |
| [analyst](analyst.agent.md) | Domain | DISCUSS (conditional) | Business logic review — ROI, domain correctness, user impact |
| [implementer](implementer.agent.md) | Workflow | IMPLEMENT | Execute approved plans with atomic commits |
| [tester](tester.agent.md) | Workflow | TEST | Write tests from spec only (never reads implementation) |
| [reviewer](reviewer.agent.md) | Workflow | REVIEW | Review for bugs, security, architecture, conventions |
| [verifier](verifier.agent.md) | Workflow | VERIFY | Goal-backward verification + plan lifecycle ending |
| [debugger](debugger.agent.md) | Utility | *(any)* | Scientific method bug investigation |
| [scribe](scribe.agent.md) | Utility | *(any)* | End-of-day diary + shutdown enforcer |

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
| **Workflow-bound** | No | State transitions via `gal next` only. implementer/tester/reviewer/verifier cannot be called directly to change state. |

Consult output is advice, not an APPROVE/REVIEW verdict. Formal verdicts come from DISCUSS/REVIEW states only.

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

## Curfew System

All agents enforce a shutdown boundary defined in `conventions/curfew.md`:

```text
... normal work ... ──── 22:00 ──── shutdown window ──── 23:00 ──── hard curfew
                       │                                    │
                       └─ only @scribe active ───────────────┘ all agents refuse
```

- **22:00**: Non-scribe agents block if today's diary is unwritten. Redirect to `@scribe`.
- **23:00**: ALL agents refuse work, including scribe. No exceptions.
- **Override**: User can say "override curfew" — single-use, does not persist.
