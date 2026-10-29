---
name: doc-sync
description: Maintain the dual-axis NDJSON structure map for docs drift detection and sync. Use when code changes require doc-impact mapping, per-line NDJSON updates, reconcile scans, or live projection rendering from structure-map.ndjson.
---

# Doc Sync Workflow

Use this skill to keep documentation structure aligned with code changes through the tracked NDJSON map in `docs/structure/`.

## What This Skill Owns

- Read and update `docs/structure/structure-map.ndjson` one JSON record per line.
- Validate records against `docs/structure/structure-map.schema.json` when a validator is available.
- Map file-level code changes to code-axis and doc-axis nodes.
- Keep untouched nodes unchanged; update only nodes re-verified in the current run.
- Render human-facing projections on demand without creating tracked projection files.

## Core Inputs

- `docs/structure/structure-map.schema.json`
- `docs/structure/structure-map.ndjson`
- `templates/structure-map.template.ndjson`
- changed-file set from `git diff`
- affected docs under `docs/` and `README.md`

## Operating Rules

### 1. Drift Detection Baseline

- Always start from native `git diff` plus direct file reads.
- Treat graphify and codebase-memory-mcp as advisory-only enhancement lanes.
- If an advisory lane is unavailable, incomplete, or not ready, continue silently at file granularity.

### 2. NDJSON Update Discipline

- Keep one valid JSON object per line.
- Preserve stable key order inside each record.
- Preserve global line order sorted by `axis`, then `id`.
- Replace only the lines for nodes actually affected by the current run.
- Do not rewrite untouched lines just to normalize formatting.

### 3. Node Model

- Code-axis nodes use `axis: "code"` with `kind: dir | module | file`.
- Doc-axis nodes use `axis: "doc"` with `kind: "doc-section"`.
- Code axis detects undocumented code (`syncStatus: "missing"`).
- Doc axis detects stale or drifted narrative (`syncStatus: "drift"`).
- New nodes may start with `lastSyncedRef: null` until first verified sync.

### 4. Freshness Rules

- Only mark a node based on evidence gathered in the current run.
- Leave untouched nodes exactly as they were.
- For modified files, default conservatively to `syncStatus: "drift"` until the affected docs are actually reviewed.
- Never mass-flip nodes to `ok` because a run completed.

### 5. Lifecycle Rules

- Use `git diff -M --name-status` to classify add, modify, delete, and rename.
- Add without doc coverage: create/update code-axis node and mark `missing`.
- Delete: remove the code-axis line; mark affected doc-axis rows `drift` instead of deleting prose.
- Rename: update `id` and `path`, mark the node `drift`, and repair affected doc references.
- Doc heading rename: update `id` anchor and `title`, then mark `drift` until re-verified.

### 6. Projection Rules

- Todo projection: list only nodes where `syncStatus != "ok"`.
- Tree projection: rebuild hierarchy from `path` for code nodes and `anchor`/doc path for doc nodes.
- Projections are ephemeral output only; do not persist them as tracked files.

### 7. Write-Back Targets

- Update the NDJSON map.
- Update only the affected doc sections in `docs/` or `README.md`.
- Do not treat external graph outputs as authoritative state.
- Do not add a second memory layer; repo files remain the only source of truth.

## Diataxis Coverage Audit (pipeline-closeout landing)

At pipeline closeout the code→doc drift audit is **Diataxis-typed**: the durable layer (`README.md` + `docs/`) is assessed across the four documentation types so the knowledge-extraction gate points at a concrete missing type, not a vague "docs are stale". `golem-steward` frames this at its activation point 3; the procedure below is the delegated landing (steward Rule 7 — the charter frames, this skill owns the steps).

The four types and what each answers:

| Type | Answers | Reader | Typical home |
| --- | --- | --- | --- |
| Tutorial | "get me started" — a learning-oriented walkthrough | a newcomer | `README.md` → Get Started section |
| How-to | "help me do X" — a task-oriented recipe | a user with a goal | `docs/setup.md`, `docs/configuration.md`, `docs/workflows.md` |
| Reference | "tell me exactly" — precise, structured facts | a user who knows what they need | `docs/architecture.md`, `docs/glossary.md`, contracts |
| Explanation | "help me understand why" — rationale, trade-offs, design context | a maintainer building a mental model | `docs/architecture.md` and ADRs |

### Audit Steps

For the durable knowledge the just-completed plan produced:

1. **Existence** — for each of the four types, ask whether this change needs a home of that type and whether one exists. A CLI feature usually needs how-to + reference; an architectural decision needs explanation. Flag any type that *should* exist for this change but does not.
2. **Quality** — for each type that exists, check it against its own bar: a how-to is a runnable recipe (not prose about the feature), a reference is precise and complete (no trailing "etc."), an explanation states the *why* and the rejected alternatives, a tutorial runs start to finish.
3. **Placement** — flag content filed under the wrong type (a reference paragraph buried in a tutorial, rationale stranded in a how-to) and move it to the type that matches its reader.
4. **Gate output** — produce a per-type existence + quality verdict. A missing or failing *required* type is a drift finding, routed like any other doc-sync drift (do not mass-pass because a run completed).

This is a documentation-quality lens layered on the drift detection above; it does not replace the NDJSON structure-map discipline.

## Validation Order

1. Parse every edited NDJSON line as JSON.
2. When available, validate each edited line against `docs/structure/structure-map.schema.json`.
3. Confirm line ordering remains `axis` then `id`.
4. Confirm untouched lines were not rewritten.

Best-effort validation is acceptable when no schema-capable validator is available across the current runtime.

## Commit Boundary Guidance

- Keep the implementation task commit scoped to the code or contract change itself.
- If doc-sync later writes NDJSON plus docs during task closeout, use a separate `docs(sync): <task>` commit.
- Standalone reconcile may use `docs(sync): reconcile` or leave staged changes for the user to commit, depending on the active workflow contract.

## Stop Conditions

- Stop and surface the issue if the NDJSON file cannot be parsed.
- Stop and surface the issue if schema validation fails and the failure can be reproduced locally.
- Stop and surface the issue if a required file path or doc anchor cannot be resolved without guessing.
- Do not invent coverage links just to satisfy the map.
