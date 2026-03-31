# GAL Control-Plane Contracts

**This is an ADR (Architectural Decision Record), not a usage guide.**

It serves two purposes:

1. `## Command Decisions` — records *why* each `/gal` command was kept, renamed, or removed. Read this when you want to understand the historical rationale. Do not treat it as the current command list.
2. `## Canonical Command Surface` — the authoritative read/write contract for each current `/gal` command. This is the section you actually need.

P0 deliverable of `docs/plans/gal-coding-workflow-native.prompt.md`.

## Command Decisions

| Old Command | Decision | Reason |
| --- | --- | --- |
| `/gal init` | **Kept** | Clear intent — bootstraps the repo |
| `/gal status` | **Kept, rewritten** | Name is clear; implementation was not — now a full state projection |
| `/gal next` | **Renamed → `whats-next`** | "Next" is ambiguous; "whats-next" answers a direct user question |
| `/gal pause` | **Renamed → `wrap-up`** | "Pause" implies a temporary suspension; "wrap-up" describes what it actually does |
| `/gal plan` | **Removed** | Planning is specialist work — `/office-hours`, `/autoplan`, `/plan-eng-review` |
| `/gal sync` | **Removed from public surface** | Adapter generation is internal to `/gal init`; exposing it as a user command created a false dependency on static skill allowlists |
| `/gal research` | **Kept** | Clear intent — enters a structured investigation flow |

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

## Legacy Alias Policy

The following commands were renamed or removed and their alias directories have been deleted from the repo:

| Former Command | Replacement |
| --- | --- |
| `/gal next` | `/gal whats-next` |
| `/gal pause` | `/gal wrap-up` |
| `/gal plan` | `/office-hours`, `/autoplan`, `/plan-eng-review` |
| `/gal sync` | Internal to `/gal init` — no public command |

No alias files exist for these. They will not appear in autocomplete.

## Skill Activation Model

Skill activation is driven by **chat intent and runtime routing**, not by a static allowlist in `.dev/project.md`.

The `## Active Skills` section and `gal sync` workflow are no longer the primary mechanism for bringing skills into scope. The user invokes a skill by name (e.g., `/plan-eng-review`), and the installed skill file provides the full procedure.

If capability gating is needed for skills with external dependencies (browser tools, deploy integrations, Obsidian vault access), this should be handled within the skill's own precondition checks, not by a centralized allowlist managed by the user.

## Separator: Control-Plane vs Specialist Commands

The `/gal` commands form the **control plane**: they read and write the repo's canonical state model, project workflow position, manage continuity, and recommend next actions.

Specialist commands (e.g., `/review`, `/qa`, `/plan-eng-review`, `/ship`) form the **execution layer**: they perform coding workflow operations within the state established by the control plane.

A `/gal` command should never replicate specialist execution logic. A specialist command should always write its results back to the canonical artifacts that the control plane reads.

| Layer | Commands | Writes To |
| --- | --- | --- |
| Control plane | `/gal init`, `/gal status`, `/gal whats-next`, `/gal wrap-up`, `/gal research` | `.dev/state.md`, `### Handoff Notes` |
| Specialist | `/office-hours`, `/plan-eng-review`, `/review`, `/qa`, `/ship`, etc. | Plan `## Review Results`, `## Test Results`, `## Status`, `.dev/state.md` blockers |
