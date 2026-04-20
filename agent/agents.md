# Agents

Specialist agent definitions for GAL's command-driven execution model.
These are `.agent.md` files for VS Code Copilot custom agents.

## Specialist Classifications

| Classification | How It Is Activated | Agents |
| --- | --- | --- |
| **Pipeline** | Invoked by `/gal pipeline` or other specialist workflows | implementer, tester, reviewer, verifier |
| **Utility** | Callable at any tier | debugger, scribe |
| **Domain** | Consulted directly by commands or users | architect, analyst, designer, researcher, librarian |

## Why GAL Uses 11 Agents

GAL keeps these roles separate on purpose.

- Smaller prompts keep responsibilities legible and reduce context waste.
- Independent tester/reviewer/verifier roles make verification more credible than one mega-agent doing everything.
- Pipeline roles stay narrow and execution-focused.
- Domain and utility roles remain reusable across workflows without requiring a dispatcher-owned state machine.

## Agents And Responsibilities

| Agent | Classification | Invocation | Purpose |
| --- | --- | --- | --- |
| [golem-architect](golem-architect.agent.md) | Domain | Consult / deep-planning review | Adversarial plan review — trade-offs, over-engineering, bugs |
| [golem-analyst](golem-analyst.agent.md) | Domain | Consult / conditional planning review | Business logic review — ROI, domain correctness, user impact |
| [golem-designer](golem-designer.agent.md) | Domain | Consult / conditional planning review / design review workflows | Review visual design, UX flow, accessibility, and design-system consistency |
| [golem-researcher](golem-researcher.agent.md) | Domain | `/gal research`, `/gal deep-research`, or direct consult | Local-first research, cross-source synthesis, and reference-ready findings |
| [golem-implementer](golem-implementer.agent.md) | Pipeline | `/gal pipeline` | Execute approved plans with atomic commits |
| [golem-tester](golem-tester.agent.md) | Pipeline | `/gal pipeline` | Write tests from spec only (never reads implementation) |
| [golem-reviewer](golem-reviewer.agent.md) | Pipeline | `/gal pipeline` or review workflows | Review for bugs, security, architecture, conventions |
| [golem-verifier](golem-verifier.agent.md) | Pipeline | `/gal pipeline` | Goal-backward verification + plan lifecycle ending |
| [golem-debugger](golem-debugger.agent.md) | Utility | Any time | Scientific method bug investigation |
| [golem-scribe](golem-scribe.agent.md) | Utility | Any time | End-of-day diary + shutdown enforcer |
| [golem-librarian](golem-librarian.agent.md) | Domain | Consult / vault workflows | Obsidian vault writes — inbox processing, knowledge extraction |

## Planning Reviews

`/deep-planning` runs an architect review by default before a source plan is treated as implementation-ready.

- **Architect** (default in `/deep-planning`): trade-off analysis, over-engineering, bug surface, dependency pollution, and public API risk
- **Analyst** (conditional): add when the plan changes business rules, pricing, permissions, or customer-visible behavior
- **Designer** (conditional): add when the plan changes customer-facing flows, layout, states, components, or accessibility-sensitive interactions

`reviewer` and `debugger` remain implementation-stage specialists. They are not default planning reviewers.

## Direct Agent Invocation

| Type | Allowed? | Rule |
| --- | --- | --- |
| **Consult** (Domain) | Yes | Read-only advice, no formal verdict. e.g., `/gal [ask architect]` |
| **Utility** | Yes | Independent helper. e.g., `/gal [run debugger]`, `/gal [run scribe]` |
| **Pipeline** | Consult-only by direct invocation | Full execution authority comes from `/gal pipeline` or another specialist workflow, not dispatcher state. |
| **Librarian** | Yes | Vault writes on demand. e.g., `@golem-librarian inbox`, `@golem-librarian extract`. Requires `start-implementation` for vault writes. |

Typical direct use examples:

- `/gal [ask designer]`
- `/gal [golem-researcher]`

Consult output is advice, not an APPROVE/REVIEW verdict. Formal verdicts come from DISCUSS/REVIEW states only.
Workflow specialists may be explicitly named, but doing so does not skip PLAN, TEST, REVIEW, or VERIFY gates.

## Model Role Enforcement

Per [model-roles.md](../model-roles.md):

- **Tester must be a different model from implementer** — independent verification
- **Reviewer should differ from implementer** — fresh perspective
- **Reviewer should be capable enough to review what the implementer produced** — prefer stronger or equal capability

## Activation Principles

- **Minimum viable set**: Only activate specialists needed for the current workflow stage and task.
- **Context budget ~15%**: Each specialist's loaded context (agent prompt + project files) should stay under ~15% of available context window.
- **Independent operation**: Each specialist can operate with only its agent file + `.dev/project.md` + the current plan. No specialist depends on another specialist's chat history.

## Installation

These agents are symlinked to `~/.copilot/agents/` by `scripts/Setup-Machine.ps1`.
VS Code Copilot discovers them as custom agents in Agent Mode.

## Research Flow

The [Research Flow](../workflows/research.md) is an independent workflow for research-driven tasks.
It uses a dedicated researcher golem plus shared review and vault-writing roles:

| Research State | Agents | Notes |
| --- | --- | --- |
| RESEARCH | researcher | Local-first investigation and evidence gathering |
| SYNTHESIZE | researcher | Organize raw findings, identify gaps and trade-offs |
| CROSS-REVIEW | researcher | Cross-check sources for consensus, contradiction, and bias |
| VERIFY | Independent verifier model | Reverse-check every retained reference before documentation |
| DOCUMENT (repo) | User / any model | Direct write to `docs/research/` |
| DOCUMENT (vault) | librarian | Requires `start-implementation` |

Research Flow can run in parallel with Coding Flow. Research output feeds into plans or vault knowledge.

## Curfew System

All agents enforce a shutdown boundary defined in `~/.copilot/gal/conventions/curfew.md`:

```text
... normal work ... ──── 22:00 ──── shutdown window ──── 23:00 ──── hard curfew
                       │                                    │
                       └─ only @golem-scribe active ──────────┘ all agents refuse
```

- **22:00**: Non-scribe agents block if today's diary is unwritten. Redirect to `@golem-scribe`.
- **22:00-23:00 with diary already written**: Non-scribe agents may offer `/gal wrap-up` once, but only run it after explicit user confirmation.
- **23:00**: ALL agents stop and use the exact hard-curfew message from `conventions/curfew.md`, including scribe.
- **Override**: User can say "override curfew" — single-use, does not persist.
