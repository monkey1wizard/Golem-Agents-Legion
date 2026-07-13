# local-notes Collaborative Tool

local-notes is GAL's optional collaborative-tool contract for user-owned external note stores.

## What This Module Covers

Use this module when a workflow may optionally read from or write to a user-managed note system
 without making that system a Core prerequisite.

This contract is intentionally minimal:

- optional external notes only
- app-agnostic contract
- local-first access only
- default off
- no PARA, Guide, diary, or owner-specific folder assumptions

GAL does not package, install, or implement note search on behalf of the user. It only routes to
existing tools when the machine-local setup is ready.

## Shared Preflight

All local-notes routing follows the shared five-state model in
[checking-contract.md](checking-contract.md).

Default expectation:

- for most repos and tasks, local-notes is `not-applicable`
- when the user has not opted in through machine-local configuration, local-notes stays effectively
  default-off
- when a backend exists but is not configured or initialized, degrade without surfacing it as a
  Core failure

This file intentionally does not restate the full five-state table.

## Minimal Contract

When local-notes is `ready`, GAL may use it for one or more of these bounded capabilities:

- read note content from a user-owned local note store
- run plain-text or backend-provided search against that store
- write note content only when the selected workflow explicitly allows it

local-notes never implies:

- a required note application
- a required vault structure
- PARA
- Guide resolution
- diary rituals
- semantic search owned by GAL
- cloud synchronization

## Documented Backend Examples

The following are example backend families GAL may reference through machine-local opt-in wiring:

- **Obsidian** via `coddingtonbear/obsidian-local-rest-api` (and related MCP wiring)
- **Logseq** via `ergut/mcp-logseq`
- **Joplin** via `joplin-mcp`
- **generic markdown vaults** via `vault-mcp`
- **generic markdown / CJK-first retrieval** via `SeekLink`

These are examples, not a feature-parity promise. Core only depends on the local-notes contract,
not on any single backend.

## Routing Rules

- Keep `local-first` as the Core retrieval principle for software-engineering work: codebase,
  docs, then repo text first.
- Use local-notes only as an optional additional source after Core-local retrieval.
- Route through machine-local configuration and backend-specific readiness checks.
- If the backend is `unavailable`, `available-but-needs-init`, or `available-but-not-ready`,
  degrade to the normal non-notes path.

## Configuration Boundary

- enablement is machine-local
- default is disabled
- backend choice is machine-local
- private paths and personal folder conventions stay out of tracked Core contracts

Downstream config surfaces such as a future `notes` block may carry this enablement, but this file
defines the contract, not the storage schema.

## Non-Goals

- no GAL-owned semantic search service
- no bundled note backend
- no automatic backend setup
- no required private capture workflow
- no required personal knowledge-management method

## Degrade Behavior

If local-notes is not `ready`, continue with the normal GAL workflow using repo-local material
only. Do not treat missing notes capability as a workflow error.

## Success Signal

The contract is working correctly when a downstream repo with no note backend configured still runs
normally, while a user who opts in can route note access through a ready local backend without
changing Core behavior.
