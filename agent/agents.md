# Agents

Golem agent definitions for the development workflow state machine.
These are `.agent.md` files for VS Code Copilot custom agents.

## Agents → Workflow States

| Agent | Workflow State | Purpose |
| --- | --- | --- |
| [planner](planner.agent.md) | PLAN | Analyze requirements, produce plan files |
| [architect](architect.agent.md) | DISCUSS | Adversarial plan review — trade-offs, over-engineering, bugs |
| [analyst](analyst.agent.md) | DISCUSS | Business logic review — ROI, domain correctness, user impact |
| [implementer](implementer.agent.md) | IMPLEMENT | Execute approved plans with atomic commits |
| [tester](tester.agent.md) | TEST | Write tests from spec only (never reads implementation) |
| [reviewer](reviewer.agent.md) | CROSS_REVIEW | Review for bugs, security, architecture, conventions |
| [verifier](verifier.agent.md) | VERIFY | Goal-backward verification of plan achievement |
| [debugger](debugger.agent.md) | *(utility)* | Scientific method bug investigation |
| [scribe](scribe.agent.md) | *(utility)* | End-of-day diary + shutdown enforcer |

## DISCUSS: Dual Review Dynamic

The planner's output is reviewed by two independent perspectives:

```text
User request → Planner (creates plan) ─┬→ Architect (technical review)
                  ↑                     └→ Analyst  (business review)
                  │                            │           │
                  └──── revise if REVISE ───────┴───────────┘
                                               │
                                  Both APPROVE → IMPLEMENT
```

- **Planner**: "Here's how we build it" — solution-focused
- **Architect**: "Here's why that won't work" — technical trade-offs, over-engineering, bugs
- **Analyst**: "Here's why that won't sell" — business logic, user impact, revenue risk
- Both architect and analyst can issue APPROVE / REVISE / REJECT verdicts
- Both must APPROVE before proceeding — either can send the plan back

## Model Role Enforcement

Per `model-roles.md`:

- **TESTER must be a different model from IMPLEMENTER** (CODER)
- **REVIEWER should be a different model from IMPLEMENTER** (CODER)

This ensures independent verification — not just re-running the same model's logic.

## Installation

These agents are symlinked to `~/.copilot/agents/` by `scripts/Setup-Machine.ps1`.
VS Code Copilot discovers them as custom agents in Agent Mode.

## Design Principles

- **Borrowed from GSD**: Goal-backward verification, scientific debugging, plan-as-prompt
- **Unique to Golem**: Independent tester (spec-only), `.dev/` context system, model role separation, curfew enforcement
- **Tool-agnostic**: Agent logic is in Markdown, not tied to any runtime

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
