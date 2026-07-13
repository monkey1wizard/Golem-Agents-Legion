---
name: steward
description: "Consult golem-steward, the first-class documentation-structure agent: structure map, code→doc drift, knowledge-extraction → docs/, docs/ + .dev/plans/ structural hygiene, figure sync."
---

# /gal steward

Consult `golem-steward` — the first-class agent that owns documentation structure for this repo.

## Role

Documentation-structure steward (consult). Invoke when you need the structure map kept truthful, code→doc drift detected, durable knowledge extracted into `docs/`, the `docs/` and `.dev/plans/` trees kept well-formed, or figures/flowcharts re-synced to the contracts they illustrate.

## When to Use

- New plan document naming / orphan or duplicate plan-document checks at planning open
- Documentation-structure consistency at refining end (source-plan well-formed, `.dev/plans/` naming, figure sync)
- Pipeline closeout: code→doc drift, structure-map update, end-of-run knowledge extraction → `docs/`
- Any manual documentation-structure reconcile or doc-drift investigation

## Boundary

Steward owns **documentation structure only**. It does NOT converge `.dev/state.md` / `.dev/plans` execution state and does NOT perform plan lifecycle actions (ABSORBED / delete) — those belong to ORCHESTRATOR. See `workflows/coding.md`.

## How to Invoke

Run the dispatcher and adopt the returned role:

```powershell
gal dispatch-script golem-steward
```

The dispatcher emits a `--- GAL DISPATCH ---` block with `ROLE: golem-steward` and `MODE: consult`. Follow `plugins/gal-core/agents/golem-steward.agent.md` for the full charter, activation points, and operating rules.
