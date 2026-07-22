# Agents

Specialist agent definitions for GAL's agent-owned execution model.
These are `.agent.md` files for VS Code Copilot custom agents.

## Specialist Classifications

Two user-facing invocability classes:

| Classification | How It Is Activated | Agents |
| --- | --- | --- |
| **Directly callable** | Bare `/gal <role>` → `MODE: direct` (isolated). Four of these six also support `/gal discuss <role>` (`CONSULT_MODE: in-context`): architect, analyst, designer, releaser. debugger and steward are direct-only (no discuss). | architect, analyst, designer, releaser, debugger, steward |
| **Orchestrated-only** | Bare call → `COMMAND: error`; runs only via `/gal pipeline`, `/gal finalize`, `/gal research`, or `/gal deep-research` | implementer, tester, auditor, researcher |

## Why GAL Uses 10 Agents

GAL keeps these roles separate on purpose.

- Smaller prompts keep responsibilities legible and reduce context waste.
- Independent tester/auditor roles plus orchestrator-owned goal-backward verification make verification more credible than one mega-agent doing everything. (End-of-run verification is owned by the ORCHESTRATOR, not a standalone agent — see the checking-role triangle in `workflows/coding.md`.)
- Pipeline roles stay narrow and execution-focused.
- Domain and utility roles remain reusable across workflows without requiring a dispatcher-owned state machine.

## Agents And Responsibilities

| Agent | Classification | Invocation | Purpose |
| --- | --- | --- | --- |
| [golem-architect](golem-architect.agent.md) | Directly callable | `/gal architect` / `/gal discuss architect` / deep-planning review | Adversarial plan review — trade-offs, over-engineering, bugs |
| [golem-analyst](golem-analyst.agent.md) | Directly callable | `/gal analyst` / `/gal discuss analyst` / conditional planning review | Business logic review — ROI, domain correctness, user impact |
| [golem-designer](golem-designer.agent.md) | Directly callable | `/gal designer` / `/gal discuss designer` / design execution / live audit | Own the design system, variant exploration, design-to-code build, and live UI audit |
| [golem-researcher](golem-researcher.agent.md) | Orchestrated-only | `/gal research` or `/gal deep-research` | Local-first research, cross-source synthesis, and reference-ready findings |
| [golem-implementer](golem-implementer.agent.md) | Orchestrated-only | `/gal pipeline` | Execute approved plans with atomic commits |
| [golem-tester](golem-tester.agent.md) | Orchestrated-only | `/gal pipeline` | Run spec-driven tests and real-browser QA |
| [golem-auditor](golem-auditor.agent.md) | Orchestrated-only | `/gal pipeline` or `/gal finalize` | Audit for deep performance, security, and other high-confidence branch risks (single task) |
| [golem-debugger](golem-debugger.agent.md) | Directly callable | `/gal debugger` (no `discuss`) | Scientific method bug investigation with internal freeze discipline |
| [golem-steward](golem-steward.agent.md) | Directly callable | `/gal steward` (no `discuss`), planning open, refining end, or pipeline closeout | First-class documentation-structure steward: structure map, code→doc drift, knowledge-extraction → docs/, docs/ + .dev/plans/ structural hygiene, figure sync (`.dev` state + lifecycle = ORCHESTRATOR, not steward) |
| [golem-releaser](golem-releaser.agent.md) | Directly callable | `/gal releaser` / `/gal discuss releaser` | Planning-stage release-flow designer: researches API/MCP/CICD tools, designs a release/devops flow, emits design advice for `/planning` to generate a `release-` plan. Does not execute. |

## Planning Reviews

`/deep-planning` always runs an architect review before a source plan is treated as implementation-ready.

- **Architect** (always in `/deep-planning`): trade-off analysis, over-engineering, bug surface, dependency pollution, and public API risk
- **Analyst** (conditional): auto-activates when content touches business rules, pricing, permissions, or customer-visible behavior
- **Designer** (conditional): auto-activates when content touches customer-facing flows, layout, states, components, or accessibility-sensitive interactions

Each of these domain lanes can also be invoked directly against the source plan outside of `/deep-planning`.

Planning-stage security review remains part of architect's job in `/deep-planning`; `golem-auditor` is reserved for auditing implemented changes.

`auditor` and `debugger` are not default planning review lanes — `auditor` audits implemented changes; `debugger` investigates bugs.

## Direct Agent Invocation

| Type | Allowed? | Rule |
| --- | --- | --- |
| **Directly callable** (architect, analyst, designer, releaser, debugger, steward) | Yes | Bare `/gal <role>` → `MODE: direct`, isolated. Read-only advice, no formal verdict, unless the agent's contract states otherwise. |
| **Discuss** (architect, analyst, designer, releaser only) | Yes | `/gal discuss <role>` → `MODE: direct` + `CONSULT_MODE: in-context`. debugger/steward do not support `discuss`. |
| **Orchestrated-only** (implementer, tester, auditor, researcher) | No, bare-call | Reachable only via `/gal pipeline`, `/gal finalize`, `/gal research`, or `/gal deep-research`. A bare or `discuss` call returns `COMMAND: error` with a role-specific recovery `ACTION`. |

Typical direct use examples:

- `/gal designer`
- `/gal discuss releaser`
- `/gal debugger`

Direct-call output is advice unless the named agent's contract explicitly includes formal write-back for its specialist stage.
Workflow specialists are never invoked as bare `/gal <role>` calls — they run under `/gal pipeline` or `/gal finalize`, which does not skip PLAN, TEST, REVIEW, or VERIFY gates.

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

These agent definitions live in `plugins/gal-core/agents/`. Projecting them into
a specific runtime's agent surface (for example `~/.copilot/agents/`, where
VS Code Copilot discovers them as custom agents in Agent Mode) is no longer part
of the `gal` workflow product.

## Research Flow

The [Research Flow](../workflows/research.md) is an independent workflow for research-driven tasks.
It uses a dedicated researcher golem plus shared review and vault-writing roles:

| Research State | Agents | Notes |
| --- | --- | --- |
| RESEARCH | researcher | Local-first investigation and evidence gathering |
| SYNTHESIZE | researcher | Organize raw findings, identify gaps and trade-offs |
| CROSS-REVIEW | researcher | Cross-check sources for consensus, contradiction, and bias |
| VERIFY | Independent model (reference check) | Reverse-check every retained reference before documentation |
| DOCUMENT (repo) | User / any model | Direct write to `.dev/research/` |
| DOCUMENT (external notes) | machine-local note backend | Optional external-note routing only when explicitly configured |

Research Flow can run in parallel with Coding Flow. Research output feeds into plans or vault knowledge.

## Working Hours

All agents enforce a shutdown boundary defined in `~/.copilot/gal/conventions/working-hours.md` when working hours are enabled in local configuration:

```text
... Working Hours ... ──── After Hours ──── Wrap-up Time ─────── Hard Stop
                               │                                     │
                               └─ reminder-only shutdown window ┘ all agents refuse
```

- **Working Hours off**: All agents proceed normally.
- **After Hours**: Outside the preferred workday, agents may proceed until Wrap-up Time.
- **Wrap-up Time**: Agents do not auto-run wrap-up. Offer `/gal wrap-up` once when applicable.
- **Hard Stop**: ALL agents stop and use the exact Hard Stop message from `conventions/working-hours.md`.
- **Pipeline-phase exemption**: a golem dispatched with `DISPATCH_KIND: pipeline-phase` (or a `PIPELINE_PHASE` marker) is machine self-driving and **skips its working-hours refusal** — it proceeds across Wrap-up Time and Hard Stop (see `conventions/working-hours.md` → Pipeline Execution Exemption). A **direct interactive call** (no pipeline-phase context) stays bounded by the rows above.
- **Override**: User can say `override working hours` — single-use, does not persist.
