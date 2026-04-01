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
| **Reads** | `.dev/state.md`, all active plan files (`## Status`, `## Review Results`, `## Test Results`, `### Handoff Notes`) |
| **Writes** | Nothing — read-only projection |
| **Dispatched via** | Direct skill procedure (no script) |
| **Output** | Active plans · Current position · Review & test status · Blockers · Session continuity · Specialist readiness |
| **Specialist boundary** | Projects specialist workflow state; does not execute specialist operations |

### `/gal whats-next`

| Field | Value |
| --- | --- |
| **User question answered** | What do I do right now? |
| **Reads** | `.dev/state.md`, active plan `## Status`, `## Review Results`, `## Test Results`, `### Handoff Notes` |
| **Writes** | Nothing — read-only recommendation |
| **Dispatched via** | Direct skill procedure (no script) |
| **Output** | Current position (one sentence) · Single next action (command or task) · File to open first |
| **Specialist boundary** | Recommends which specialist command to invoke but does not invoke it |
| **Decision inputs** | Workflow state, test results, review verdicts, blockers, session continuity |

### `/gal wrap-up`

| Field | Value |
| --- | --- |
| **User question answered** | How do I close this session so it can be resumed cleanly? |
| **Reads** | `.dev/state.md`, active plan (full content) |
| **Writes** | Active plan `### Handoff Notes` · `.dev/state.md` `## Session Continuity` |
| **Dispatched via** | Direct skill procedure (no script) |
| **Output** | Summary of what was written · Suggested git commit command |
| **Specialist boundary** | Converges continuity artifacts; does not advance workflow state |

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

The user invokes a skill by name (e.g., `/plan-eng-review`), and the installed skill file provides the full procedure.

If capability gating is needed for skills with external dependencies (browser tools, deploy integrations, Obsidian vault access), this should be handled within the skill's own precondition checks, not by a centralized allowlist managed by the user.

## Separator: Control-Plane vs Specialist Commands

The `/gal` commands form the **control plane**: they read and write the repo's canonical state model, project workflow position, manage continuity, and recommend next actions.

Specialist commands (e.g., `/review`, `/qa`, `/plan-eng-review`, `/ship`) form the **execution layer**: they perform coding workflow operations within the state established by the control plane.

A `/gal` command should never replicate specialist execution logic. A specialist command should always write its results back to the canonical artifacts that the control plane reads.

| Layer | Commands | Writes To |
| --- | --- | --- |
| Control plane | `/gal init`, `/gal status`, `/gal whats-next`, `/gal wrap-up`, `/gal research` | `.dev/state.md`, `### Handoff Notes` |
| Specialist | `/office-hours`, `/plan-eng-review`, `/review`, `/qa`, `/ship`, etc. | Plan `## Review Results`, `## Test Results`, `## Status`, `.dev/state.md` blockers |

## Artifact Model

When this document refers to "active plan file," it means the AI execution work file at `docs/plans/<plan-slug>.prompt.md`. This is the mutable artifact that carries `## Status`, `## Review Results`, `## Test Results`, `### Handoff Notes`, and other per-task state sections.

The human-readable source plan doc lives at `docs/plans/<plan-slug>.md`. Both files share the same `plan-slug` as their correlation key. The control plane reads the execution work file (`.prompt.md`) for state projection; it does not write to the source plan doc.

See `docs/per-repo-context.md` for the complete artifact taxonomy.
