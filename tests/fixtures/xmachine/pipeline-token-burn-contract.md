# Pipeline Token-Burn Dispatch Contract (frozen)

Reparented from `scripts/Test-PipelineTokenBurn.ps1` by T-010 of
`refactor-gal-xmachine-rust-port` (R-08), ahead of the script's deletion at T-014.

This file freezes the **observable dispatch-boundary contract** the token-burn
oracle asserted, so the contract survives independently of the legacy test
script. It is plain frozen memory — it does not embed or depend on the live
`.ps1` source, so T-014 can delete that script without breaking `cargo test`.

## Bounded pipeline dispatch

Invocation:

```
gal dispatch pipeline docs/plans/fix-gal-pipeline-token-burn.md from T-001 stop-at T-001
```

Expected dispatch fields:

| Field | Value | Contract |
| --- | --- | --- |
| PLAN | docs/plans/fix-gal-pipeline-token-burn.md | pipeline dispatch preserves the explicit plan path |
| FROM | T-001 | pipeline dispatch emits the lower task boundary |
| STOP_AT | T-001 | pipeline dispatch emits the upper task boundary |

## Per-phase context mode

| Phase | PIPELINE_CONTEXT_MODE | CONTEXT_CARRY |
| --- | --- | --- |
| implement | full | true |
| test | delta | true |
| audit | delta | true |

The delta phases (test, audit) omit `PIPELINE_CONTEXT_FILES` and `CONVENTION_HINTS`.

## Out of scope here

This contract covers only the pipeline dispatch-boundary surface. The oracle
script also exercised convention-hint filtering and OpenCode startup payload size;
those belong to their own surfaces and are not reparented by this fixture.
