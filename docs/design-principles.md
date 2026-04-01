# Design Principles

This document captures the durable architectural rationale behind Golem Agents Legion (GAL).
It is the canonical home for decisions that should outlive any single bootstrap or implementation plan.

## Why GAL Exists

GAL exists to stop methodology from being trapped inside one AI tool, one machine, or one long chat thread.

The design addresses four recurring failures:

- Tool lock-in: skills, agents, and instructions tied to one product become dead weight when that product changes.
- Machine fragmentation: workstation-specific setup drifts unless the methodology lives in version-controlled source.
- Session amnesia: important working context disappears when a chat resets unless it is written back to files.
- Workflow inconsistency: plan, implementation, testing, review, and verification degrade when there is no shared state machine.

## Core Position

GAL treats Markdown as the durable asset and tool config as generated infrastructure.

- Methodology lives in versioned source files under this repo.
- Tool adapters are disposable outputs that can be regenerated.
- Per-repo context belongs to the target repo, not to GAL itself.
- Task memory is temporary and should be absorbed into permanent docs before deletion.

This is why the repo optimizes for portability and explicit state, not for tool-specific convenience.

## Operating Principles

1. Knowledge in Markdown, not product config.
2. Canonical sources stay human-readable and tool-agnostic.
3. Generated adapters are replaceable and should never become the source of truth.
4. Workflow rules should be explicit enough that different tools can execute them consistently.
5. Temporary task memory should collapse back into canonical docs once work is complete.
6. Human operators remain the orchestrator, and agents are specialists rather than autonomous owners of process.

## Borrowed Concepts, Not Borrowed Implementations

GAL borrows patterns from other systems without depending on their runtime models or codebases.

| Source | Borrowed Concept | Why It Matters In GAL |
| --- | --- | --- |
| GSD | Phase-based workflow, explicit state, verification gates | Gives GAL a rigorous execution lifecycle without inheriting GSD's package or prompt model |
| OmO | Role-first model routing | Lets GAL map responsibilities to model classes instead of hard-coding one vendor or model |
| LangGraph | Stateful workflow with checkpoints | Reinforces that workflows are state machines with recoverable progress, not ad hoc chats |

## What GAL Optimizes For

### Portability over local optimization

A change that is elegant for one tool but harms Copilot and Gemini parity is usually a bad GAL change.

### Single-source-of-truth docs over prompt sprawl

Workflow logic, guardrails, and setup topology should be documented once, then referenced or generated elsewhere.

### Durable reasoning over historical plans

Bootstrap files and task plans are useful while work is active. They should not remain the only place where architectural intent is explained.

## Generated Adapters And Why They Matter

GAL separates canonical methodology from runtime adapters:

- Canonical definitions live here: workflows, agents, conventions, templates, command templates, scripts.
- Runtime adapters are generated from those definitions for a specific tool or repo.

That separation makes tool migration a maintenance problem instead of a rewrite.

## Why Setup Uses Symlinks Instead Of Copies

Setup-Machine installs GAL into tool directories with symlinks and baked command skill files instead of copying the whole tree.

This preserves three invariants:

- The repo stays the only editable source of truth.
- Multiple machines converge through `git pull` instead of manual re-sync.
- Tool discovery requirements can be satisfied without forking the methodology.

See [docs/installation-topology.md](installation-topology.md) for the concrete runtime layout.

## Permanent vs Temporary Information

Use this split when deciding where to write new information:

- Permanent architecture or operating rules: `README.md`, `docs/`, `workflows/`, `agent/`, `conventions/`, `templates/`, `commands/`, `scripts/`
- Per-repo working context: `<repo>/.dev/project.md`, `<repo>/.dev/state.md`
- Human-readable source plan doc: `docs/plans/<plan-slug>.md` (scope, rationale, requirements)
- AI execution work file: `docs/plans/<plan-slug>.prompt.md` (mutable checklist, execution state, write-back target)

If a plan finishes and still contains knowledge worth keeping, that knowledge belongs in canonical docs, not in the plan file.

## Related Documents

- [docs/installation-topology.md](installation-topology.md)
- [docs/command-dispatch-architecture.md](command-dispatch-architecture.md)
- [docs/per-repo-context.md](per-repo-context.md)
- [docs/skills-migration.md](skills-migration.md)
