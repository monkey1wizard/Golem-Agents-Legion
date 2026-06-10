# Agents

Specialist agent definitions for GAL's agent-owned execution model.
These are `.agent.md` files for VS Code Copilot custom agents.

## Specialist Classifications

| Classification | How It Is Activated | Agents |
| --- | --- | --- |
| **Pipeline** | Invoked by `/gal pipeline` or other specialist workflows | implementer, tester, auditor, verifier |
| **Utility** | Callable at any tier | debugger, notewriter, dockeeper |
| **Domain** | Consulted directly by commands or users | architect, analyst, designer, researcher, releaser |

## Why GAL Uses 12 Agents

GAL keeps these roles separate on purpose.

- Smaller prompts keep responsibilities legible and reduce context waste.
- Independent tester/auditor/verifier roles make verification more credible than one mega-agent doing everything.
- Pipeline roles stay narrow and execution-focused.
- Domain and utility roles remain reusable across workflows without requiring a dispatcher-owned state machine.

## Agents And Responsibilities

| Agent | Classification | Invocation | Purpose |
| --- | --- | --- | --- |
| [golem-architect](golem-architect.agent.md) | Domain | Consult / deep-planning review | Adversarial plan review — trade-offs, over-engineering, bugs |
| [golem-analyst](golem-analyst.agent.md) | Domain | Consult / conditional planning review | Business logic review — ROI, domain correctness, user impact |
| [golem-designer](golem-designer.agent.md) | Domain | Consult / design execution / live audit | Own the design system, variant exploration, design-to-code build, and live UI audit |
| [golem-researcher](golem-researcher.agent.md) | Domain | `/gal research`, `/gal deep-research`, or direct consult | Local-first research, cross-source synthesis, and reference-ready findings |
| [golem-implementer](golem-implementer.agent.md) | Pipeline | `/gal pipeline` | Execute approved plans with atomic commits |
| [golem-tester](golem-tester.agent.md) | Pipeline | `/gal pipeline` or direct verification | Run spec-driven tests and real-browser QA |
| [golem-auditor](golem-auditor.agent.md) | Pipeline | `/gal pipeline` or direct audit | Audit for deep performance, security, and other high-confidence branch risks |
| [golem-verifier](golem-verifier.agent.md) | Pipeline | `/gal pipeline` | Goal-backward verification + plan lifecycle ending |
| [golem-debugger](golem-debugger.agent.md) | Utility | Any time | Scientific method bug investigation with internal freeze discipline |
| [golem-notewriter](golem-notewriter.agent.md) | Utility | Any time | Obsidian writes, private captures, diary, shutdown ritual, and knowledge extraction |
| [golem-dockeeper](golem-dockeeper.agent.md) | Utility | Pipeline closeout, reconcile, or direct drift audit | Maintain the doc structure map, detect stale docs, and coordinate doc sync |
| [golem-releaser](golem-releaser.agent.md) | Domain | Direct release prep / deploy / doc sync | Release prep, deploy orchestration, and documentation sync |

## Planning Reviews

`/deep-planning` always runs an architect review before a source plan is treated as implementation-ready.

- **Architect** (always in `/deep-planning`): trade-off analysis, over-engineering, bug surface, dependency pollution, and public API risk
- **Analyst** (conditional): auto-activates when content touches business rules, pricing, permissions, or customer-visible behavior
- **Designer** (conditional): auto-activates when content touches customer-facing flows, layout, states, components, or accessibility-sensitive interactions

Each of these domain lanes can also be invoked directly against the source plan outside of `/deep-planning`.

Planning-stage security review remains part of architect's job in `/deep-planning`; `golem-auditor` is reserved for auditing implemented changes.

`auditor`, `debugger`, and `releaser` remain implementation-stage specialists. They are not default planning reviewers.

## Direct Agent Invocation

| Type | Allowed? | Rule |
| --- | --- | --- |
| **Consult** (Domain) | Yes | Read-only advice, no formal verdict. e.g., `/gal [ask architect]` |
| **Utility** | Yes | Independent helper. e.g., `/gal [run debugger]`, `/gal [run notewriter]` |
| **Pipeline** | Yes | Pipeline agents may still be invoked directly when the task is clearly bounded to their specialist contract. |
| **Notewriter** | Yes | Vault writes on demand. e.g., `@golem-notewriter private-capture`, `@golem-notewriter extract`. Durable knowledge writes still require `start-implementation`. |

Typical direct use examples:

- `/gal [ask designer]`
- `/gal [golem-researcher]`
- `@golem-auditor audit this branch`
- `@golem-releaser prepare release`

Consult output is advice unless the named agent's contract explicitly includes formal write-back for its specialist stage.
Workflow specialists may be explicitly named, but doing so does not skip PLAN, TEST, REVIEW, or VERIFY gates.

## Model Role Enforcement

Per [workflows/coding.md](../workflows/coding.md) — Model Roles and Per-Phase Assignment:

- **Tester must be a different model from implementer** — independent verification
- **Auditor should differ from implementer** — fresh perspective
- **Auditor should be capable enough to audit what the implementer produced** — prefer stronger or equal capability

## Activation Principles

- **Minimum viable set**: Only activate specialists needed for the current workflow stage and task.
- **Context budget ~15%**: Each specialist's loaded context (agent prompt + project files) should stay under ~15% of available context window.
- **Independent operation**: Each specialist can operate with only its agent file + `.dev/project.md` + the current plan. No specialist depends on another specialist's chat history.

## Installation

These agents are symlinked to `~/.copilot/agents/` by `gal setup`.
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
| DOCUMENT (vault) | notewriter | Private capture is lightweight; durable knowledge still requires `start-implementation` |

Research Flow can run in parallel with Coding Flow. Research output feeds into plans or vault knowledge.

## Working Hours

All agents enforce a shutdown boundary defined in `~/.copilot/gal/conventions/working-hours.md` when working hours are enabled in local configuration:

```text
... Working Hours ... ──── After Hours ──── Wrap-up Time ─────── Hard Stop
                               │                                     │
                               └─ only @golem-notewriter owns ritual ┘ all agents refuse
```

- **Working Hours off**: All agents proceed normally.
- **After Hours**: Outside the preferred workday, agents may proceed until Wrap-up Time.
- **Wrap-up Time**: Non-notewriter agents block if today's diary is unwritten. Redirect to `@golem-notewriter`.
- **Hard Stop**: ALL agents stop and use the exact Hard Stop message from `conventions/working-hours.md`, including notewriter.
- **Override**: User can say `override working hours` — single-use, does not persist.
