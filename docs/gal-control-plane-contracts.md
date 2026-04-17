# GAL Control-Plane Contracts

This document is the canonical current read/write contract for the `/gal` control plane.

It records the stable command surface and ownership boundaries only. Live/manual verification status is tracked separately in [docs/runtime-verification.md](runtime-verification.md).

## Canonical Command Surface

### `/gal init`

| Field | Value |
| --- | --- |
| **User question answered** | How do I bootstrap this repo for GAL? |
| **Reads** | Existing README, docs, codebase structure |
| **Writes** | `.dev/project.md`, `.dev/state.md` |
| **Dispatched via** | `gal.ps1 dispatch init` |
| **Specialist boundary** | Control-plane only — does not start any coding workflow |

### `/gal status`

| Field | Value |
| --- | --- |
| **User question answered** | Where are we right now? |
| **Reads** | `.dev/state.md` Active Plans index, then all active execution plan files (`.dev/plans/<plan-slug>.prompt.md`: `## Status`, `## Open Questions`, `## Tasks`, `## Analyze`, `## Review Results`, `## Test Results`, `### Handoff Notes`) |
| **Writes** | Nothing — read-only projection |
| **Dispatched via** | Direct skill procedure (no script) |
| **Output** | Active plans · Current position · Review & test status · Blockers · Session continuity · Specialist readiness |
| **Specialist boundary** | Projects specialist artifacts and plan progress markers; does not execute specialist operations |

### `/gal whats-next`

| Field | Value |
| --- | --- |
| **User question answered** | What do I do right now? |
| **Reads** | `.dev/state.md` Active Plans index, then active execution plan `.dev/plans/<plan-slug>.prompt.md` `## Status`, `## Open Questions`, `## Tasks`, `## Analyze`, `## Review Results`, `## Test Results`, `### Handoff Notes` |
| **Writes** | Nothing — read-only recommendation |
| **Dispatched via** | Direct skill procedure (no script) |
| **Output** | Current position (one sentence) · Single next action (command or task) · File to open first |
| **Specialist boundary** | Recommends which specialist command to invoke but does not invoke it |
| **Decision inputs** | Plan artifacts, progress markers, test results, review verdicts, blockers, session continuity |

### `/gal wrap-up`

| Field | Value |
| --- | --- |
| **User question answered** | How do I close this session so it can be resumed cleanly? |
| **Reads** | `.dev/state.md`, active plan (full content) |
| **Writes** | Active plan `### Handoff Notes` · `.dev/state.md` `## Session Continuity` |
| **Dispatched via** | Direct skill procedure (no script) |
| **Output** | Summary of what was written · Suggested git commit command |
| **Specialist boundary** | Converges continuity artifacts; does not advance specialist execution itself |

### `/gal research`

| Field | Value |
| --- | --- |
| **User question answered** | I need to investigate something — how do I enter a structured research flow? |
| **Reads** | Current context as provided |
| **Writes** | Research artifacts as directed by golem-researcher |
| **Dispatched via** | `gal.ps1 dispatch research` |
| **Specialist boundary** | Routes to golem-researcher in consult mode |

## Skill Activation Model

Skill activation is driven by **chat intent and runtime routing**, not by a static allowlist in `.dev/project.md`.

The user invokes a skill or lane entry point by name (for example, `/planning`, `/review`, or `/gal golem-architect`), and the installed skill file provides the full procedure.

If capability gating is needed for skills with external dependencies (browser tools, deploy integrations, Obsidian vault access), this should be handled within the skill's own precondition checks, not by a centralized allowlist managed by the user.

## Separator: Control-Plane vs Specialist Commands

The `/gal` commands form the **control plane**: they read and write the repo's canonical state model, project workflow position, manage continuity, and recommend next actions.

Specialist commands and provider-routed review lanes (for example, `/planning`, `/review`, `/qa`, `/ship`, or the engineering review lane) form the **specialist work layer**: they perform planning-stage refinement, implementation, validation, and release operations within the state established by the control plane.

A `/gal` command should never replicate specialist execution logic. A specialist command should always write its results back to the canonical artifacts that the control plane reads.

| Layer | Commands | Writes To |
| --- | --- | --- |
| Control plane | `/gal init`, `/gal status`, `/gal whats-next`, `/gal wrap-up`, `/gal research` | `.dev/state.md`, `### Handoff Notes` |
| Specialist | `/planning`, `/deep-planning`, provider-routed planning review lanes, `/plan-to-prompt`, `/review`, `/qa`, `/ship`, etc. | Source plan doc first, then execution prompt sections, `.dev/state.md` blockers |

## Artifact Model

When this document refers to "active plan file," it means the AI execution work file at `.dev/plans/<plan-slug>.prompt.md`. This is the mutable artifact that carries `## Status`, `## Open Questions`, `## Tasks`, `## Analyze`, `## Review Results`, `## Test Results`, `### Handoff Notes`, and other per-task state sections.

The human-readable source plan doc lives at `docs/plans/<plan-slug>.md`. Both files share the same `plan-slug` as their correlation key. Planning workflows and planning-stage review lanes produce or refine the source plan first; `/plan-to-prompt` materializes the reviewed source plan into the paired execution work file under `.dev/plans/` only when the plan is implementation-ready. The control plane reads the execution work file for state projection after materialization; before materialization, it may inspect the source plan to recommend the next planning-stage action. The control plane does not write to the source plan doc.

If `.dev/state.md` is missing, the repo is uninitialized. If `.dev/state.md` exists but the active plan entry or the plan `## Status` section is malformed, that is a repo-state error, not an init case.

See `docs/per-repo-context.md` for the complete artifact taxonomy.
