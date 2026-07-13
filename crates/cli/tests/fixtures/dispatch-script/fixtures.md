# dispatch-script parity fixtures

Frozen oracle for the `gal dispatch-script` Rust port (`refactor-gal-dispatch-script-rust-port`).
Captured from the live `scripts/gal.ps1 dispatch …` output on 2026-06-12 (commit `8ecf7db`)
before any shell change (architect condition C-2 reparent discipline).

## Caller inventory (architect C-1)

Live `gal dispatch …` forms the contract surface + chat procedures actually invoke:

| Form | Block kind | Fixture |
| --- | --- | --- |
| `dispatch <golem>` (non-pipeline, non-utility) | golem **consult** | `consult-golem.txt` |
| `dispatch <utility-golem>` (`golem-debugger`) | golem **utility** | `utility-golem.txt` |
| `dispatch golem-{implementer,tester,auditor} --pipeline-phase <p> --task-scope T-NNN` | golem **bound** + pipeline-phase metadata | `bound-implement.txt`, `bound-test.txt`, `bound-audit.txt` |
| `dispatch init` | subcommand intent | `intent-init.txt` |
| `dispatch research` | subcommand intent | `intent-research.txt` |
| `dispatch deep-research` | subcommand intent | `intent-deep-research.txt` |
| `dispatch pipeline` | subcommand intent | `intent-pipeline.txt` |
| `dispatch golem-X --pipeline-phase … --task-scope …` with routing present | headless **OFFLOAD** | captured for the OFFLOAD path (needs `config.json#executorRouting` fixture) |

**Retired (xmachine, `refactor-ssh-transport-replace-xmachine`):** `dispatch <golem>/<intent> … xmachine <node>` and its `golem-xmachine.txt`/`intent-research-xmachine.txt` fixtures are removed — the xmachine remote-execution shorthand is replaced by the SSH dispatch lane on `config.json#executorRouting` (`sshTarget`/`remoteWorkdir`), which needs no dispatch-script intent of its own.

**Dead branches (NOT ported — architect OE-01 / OQ-002):** bare-`gal dispatch` with no intent/golem → state auto-detect `suggest` blocks (`gal.ps1` L1056–1087). The `/gal` no-args path is handled in chat (`/gal-whats-next`), not via shell dispatch — confirmed caller-less. These are deleted from the shell, not ported.

## Parity definition — D-001 (field-set, not byte)

`scripts/gal.ps1` `Write-Dispatch([hashtable]$Fields)` (L404-412) iterates `$Fields.Keys`,
which is **non-deterministic** in PowerShell even when the caller builds an `[ordered]@{}` (the
`[hashtable]` parameter type coerces away the order). Observed: the same case emits the same
key→value set in a different line order on different runs / cases.

Therefore parity is defined as **field-set equality**: the Rust emitter must produce the exact
same set of `KEY: value` pairs (and the `--- GAL DISPATCH ---` / `--- END DISPATCH ---` framing),
order-insensitive. The Rust port emits a **deterministic** (stable, e.g. sorted or insertion)
order — an improvement that consumers don't care about (the chat procedures parse `KEY: value`
lines, not positions).

Each fixture below stores the canonical field set **sorted by key**, one `KEY: value` per line,
framed by the markers. Parity tests sort the emitter's field lines and compare to the fixture.

## State-derived fields (architect C-4)

`PIPELINE_CONTEXT_MODE`, `WORKFLOW_STATE`, and `PIPELINE_CONTEXT_FILES` are computed by the shell
`Get-PipelineDispatchMetadata` / `Get-StateContext` helpers from the repo's `.dev/state.md` +
`.dev/project.md` at dispatch time:

- phase `implement` → `PIPELINE_CONTEXT_MODE: full`, plus `WORKFLOW_STATE` and a `PIPELINE_CONTEXT_FILES` list.
- phases `test`/`audit` → `PIPELINE_CONTEXT_MODE: delta` (no files list, no `WORKFLOW_STATE`).

`PIPELINE_CONTEXT_FILES` holds absolute paths; the fixture normalizes the repo root to `<REPO>`.
`WORKFLOW_STATE` reflects the live state read (captured value: `IDLE`). This logic is ported and
its parity test controls/normalizes the state inputs.
