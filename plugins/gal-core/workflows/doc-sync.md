# Doc Sync Workflow

`golem-steward` owns this workflow and delegates the reusable mechanics to the `doc-sync` skill.

## Purpose

Maintain the tracked dual-axis NDJSON structure map and keep affected docs aligned with meaningful code changes without introducing a second memory layer.

## Authoritative State

- `docs/structure/structure-map.ndjson` is the only tracked structure-map authority.
- `docs/structure/structure-map.schema.json` defines the per-line record contract.
- `templates/structure-map.template.ndjson` is the seed template for new maps.
- Human-facing projections are rendered on demand and are never persisted as tracked files.

## Run Modes

### Incremental

- Trigger: pipeline task closeout or direct bounded invocation after a scoped change.
- Diff range: the current task commit range.
- Goal: update only the nodes and doc sections impacted by the just-completed change.

### Reconcile

- Trigger: direct `golem-steward` invocation outside normal task closeout.
- Diff range: per-node `lastSyncedRef..HEAD`, whole repo.
- Goal: catch pipeline-external and cross-plan accumulated drift.

## Preflight

1. Resolve the changed-file set from native `git diff`.
2. Load the structure-map schema and existing NDJSON map.
3. Resolve advisory lanes through the shared preflight model:
   - For structural-aid routing, start at [structural-retrieval](../../../docs/collaborative-tools/structural-retrieval.md).
   - graphify: use only when the graphify lane is `ready`.
   - codebase-memory-mcp: use only when the MCP lane is `ready`.
4. If advisory lanes are not ready, continue silently with file-level detection.

## Core Flow

1. Detect changed files from `git diff`.
2. Classify path changes with `git diff -M --name-status`.
3. Map affected code-axis and doc-axis nodes.
4. Update only the affected NDJSON lines.
5. Update only the affected sections in `docs/` or `README.md`.
6. Refresh `syncStatus`, `lastSynced`, and `lastSyncedRef` only for nodes actually re-verified.
7. Render ephemeral projections for human review when needed.

## Mapping Rules

### Mandatory Baseline

- Native `git diff` plus file reads are mandatory.
- Advisory tooling may refine impact precision but may not replace the file-level baseline.

### Advisory Lanes

- Structural-aid lane selection lives in [structural-retrieval](../../../docs/collaborative-tools/structural-retrieval.md).
- graphify may extend the affected range through cross-module coupling signals.
- codebase-memory-mcp may refine doc hits for `#symbol` references when indexed and ready.
- Both lanes are optional. Missing tools must not block the workflow, emit install prompts, or cause failure.

## Freshness Rules

- Only nodes re-verified in the current run may change status.
- Untouched nodes keep their previous `syncStatus`, `lastSynced`, and `lastSyncedRef`.
- Modified code defaults conservatively to `drift` until the doc side is actually reviewed.
- The workflow must never mass-flip the map to `ok`.

## Node Lifecycle

- `A`: create or update a code-axis node; if doc coverage is absent, mark `missing`.
- `M`: keep the node, update evidence fields, and mark `drift` until reviewed.
- `D`: remove the code-axis node; mark affected doc-axis nodes `drift` instead of deleting prose.
- `R`: update `id` and `path`, then mark `drift`.
- Doc heading rename: update `id` anchor and `title`, repair references, mark `drift`.

## Projection Output

### Todo Projection

- Include only nodes where `syncStatus != ok`.
- Show `id`, `axis`, `syncStatus`, and linked coverage fields.

### Tree Projection

- Reconstruct the code tree from `path`.
- Reconstruct the doc tree from document path plus anchor.
- Tag each node with `ok`, `missing`, or `drift`.

## Validation

1. Parse each edited NDJSON line as JSON.
2. Validate edited lines against `docs/structure/structure-map.schema.json` when a validator is available.
3. Verify stable ordering: `axis` then `id`.
4. Confirm untouched lines were not rewritten.
5. Confirm only affected doc sections changed.

Validation is best-effort when no schema-capable validator is available in the current runtime.

## Commit Boundary

- The implementation task commit remains scoped to the implementation itself.
- Doc-sync writes to NDJSON plus docs after task completion must go into a separate `docs(sync): <task>` commit.
- Standalone reconcile may use `docs(sync): reconcile` or leave the result staged for the user to commit.
- A no-op sync must not create an empty `docs(sync): ...` commit.
- Doc-sync failure does not block the main task pipeline; it records the failure and stops cleanly.

## Write-Back Scope

- Allowed: structure-map files and affected docs.
- Not allowed: replacing plan/state authority, introducing new memory stores, or treating external graph outputs as source of truth.

## Stop Conditions

- Structure-map file cannot be parsed.
- Schema validation fails and the failure is reproducible locally.
- A required path or doc anchor cannot be resolved without inventing data.
- Protected-path or workflow-scope changes are discovered beyond the approved plan.
