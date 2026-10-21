# Usage Accounting

This convention is the single source for how GAL records and reports the consumption of dispatched work and the shared account capacity behind it. Command and workflow sources link here. They do not restate these rules in full. When a source disagrees with this file, this file controls, and the source must be corrected.

## Scope and Authority

- Usage accounting records what each runtime reports. It never changes a dispatch outcome, a receipt, or an acceptance decision.
- The scope covers the orchestrator session, each dispatched attempt, and each child agent that a runtime reports.
- Evidence is stored under `gal/usage/v1/` in the Git common directory. Pipeline cleanup and linked-worktree teardown leave that store in place.

## Automatic Capture

GAL captures usage without an operator command. The shared capture helper runs after each terminal path of a standalone, local, SSH, or host-subagent attempt. A retry or resume creates a new attempt that keeps its session lineage.

Capture failure leaves the execution outcome, the retry policy, and the receipt unchanged. The report then shows partial or unavailable accounting, with the reason.

## Explicit Session Binding

Automatic capture binds the orchestrator session only when preflight finds one unambiguous identity. When no identity is found, the report shows the identity as unavailable. GAL does not auto-select a conversation.

Explicit collection reads a binding file for the run. The file names the host session and the run identity, and it carries no counters. A binding whose host session differs from the run identity is refused as a rebind conflict before any source is read. The rate file is optional. Without one, rate-derived estimates stay unknown.

```text
gal usage collect --run <run-id> --binding <path> [--rates <path>]
```

## Final-Turn Cutoff

Every report states its observation cutoff. A live report is an as-of snapshot. The orchestrator's final turn can become observable after the attempt ends. A later explicit collection captures that delayed evidence as a new snapshot. The earlier snapshot does not change.

## Failed and Resumed Attempts

Failed, timed-out, canceled, retried, and resumed attempts keep their collected usage. Known usage stays known even when the final usage event is missing. That case is marked partial. Malformed data is never read as zero.

For example, a failed attempt that consumes 100 units and a successful retry that consumes 60 units produce a task total of 160. The outcome of each attempt is a separate decision.

## Factual Records and Assessments

- Collectors append observations and snapshots. A snapshot names its exact observation set, its reconciliation version, and its rate-set digest.
- Publication never replaces an existing record. Identical content replays without change. Different bytes under the same identity fail as a conflict.
- Assessments append classifications only. An assessment names one snapshot and the stable contribution IDs it classifies.
- Assessment input that contains a factual field fails. Factual fields include prices, token counts, totals, quota fields, and unknown fields. The command exits with the usage error code, 64.
- No assessment, report, or review rewrites a factual snapshot, a receipt, or a goal verification result.

```text
gal usage assess --run <run-id> --snapshot <snapshot-id> --input <path>
```

## Rate Bases

Each estimate keeps three separate values: the provider-reported amount, the rate-derived estimate, and the confirmed charge. These values are never added together.

- Rates key on provider, observed model, billing mode, and unit. The configured routing name never replaces the observed model.
- The owner supplies each rate set, which is versioned and digested. GAL does not fetch rates.
- Arithmetic uses exact decimal values. Rounding happens only at display.
- Cached-input and reasoning subsets follow the provider's semantics. They are never counted twice.
- A missing or ambiguous rate yields an unknown estimate.
- Credits, subscription allowances, and currencies are separate units. GAL never converts one into another or into a charge.
- A confirmed charge needs billing evidence in the same scope. A quota change is never a charge.

## Account Quota

Quota describes shared account capacity. It is not task consumption. Quota never adds to a task cost, and no flow derives money or token counts from a change in quota.

### Runtime Rows

| Runtime | Passive source in this build | Control read in this build | Gap behavior |
| --- | --- | --- | --- |
| Codex CLI | None. Stdout carries no quota, so the gap is recorded. | Bounded `codex app-server` RPC read. | A failed read records a gap with its reason. |
| Claude Code | `rate_limit_event` in stream output, when present. | None. | A window missing from the events is unknown, not zero. |
| GitHub Copilot CLI | None. The usage block carries no quota snapshot, so the gap is recorded. | None. | The gap is pinned to the tested CLI 1.0.98. |
| Antigravity CLI | Structured JSON quota in the output, when present. | None. | Headless token usage is not account quota. |
| OpenCode | None. The gap is recorded. | None. The gap is recorded. | Provider identity is shown with the gap. |

Every runtime row reports its gap. No runtime is left out of the report silently.

### Identity Proof

A quota observation names its runtime, version, provider, billing mode, and an opaque account or workspace reference.

A control read proves identity only when the returned account or connection identity matches the attempt. A matching profile alone does not prove a matching account. When identity is unverified or mismatched, GAL keeps the observation unattributed.

### Native Windows and Units

Keep each native window with its type, duration, unit, and direction. Windows include five-hour, weekly, monthly, and request-rate windows.

- Do not coerce windows into a fixed five-hour and weekly pair.
- Do not assume that the primary window is five hours.
- Do not treat a missing window as zero or unlimited. Unlimited requires an explicit flag or a documented sentinel.
- Claude `utilization` is a fraction from 0 to 1. A statusline `used_percentage` is a value from 0 to 100, and it can be absent.
- Antigravity `remaining_fraction` is remaining capacity, not used capacity. Keep the two meanings apart.

### Bounded Control Reads

A control read uses the same executable, host, profile, provider, and auth environment as the attempt. A remote attempt never inherits the controller's quota.

- The whole probe has a deadline of five seconds. The deadline covers startup, handshake, identity checks, queries, and retries.
- The whole probe has an output limit of one MiB.
- A probe never starts an inference turn. It never sends a prompt, starts a login, redeems a reset, makes a purchase, or writes configuration.
- When a limit is exceeded, GAL stops the query and reaps only the query processes that it started. The passive evidence, the attempt outcome, and the receipt remain.
- Each return queries on its own. GAL does not reuse an earlier in-process observation, and it runs no background polling.

### Freshness and Failed Refresh

- Each observation keeps its provider observation time. The collection time never makes an observation fresh.
- A missing source time leaves freshness unknown.
- A failed refresh keeps the previous values. The record shows the acquisition status as failed and keeps the original observation time.
- A cached read is not a new remote observation.

### Immutable Return References

Each attempt return stores the exact quota reference that it chose. Later queries append new observations. Historical reports use the stored references and never query current quota. A reassessment never changes a stored reference.

### Query Charge Uncertainty

A quota probe may use provider capacity. GAL records the probe method and its duration as metadata. GAL never states that a probe cost nothing, because a protocol trace shows the protocol behavior and does not prove the billing outcome.

## Report Commands

```text
gal usage report --run <run-id> [--snapshot <snapshot-id>] [--assessment <assessment-id>] [--json]
```

- Without `--snapshot`, the report uses the latest valid factual snapshot and only the latest assessment that names it.
- An explicit assessment that names a different snapshot fails with a mismatch diagnostic.
- A report never collects evidence, never refreshes quota, and never rewrites a timestamp.
- Text and JSON show the same states: complete, partial, unavailable, conflict, and integrity failure.
- JSON output puts exactly one document on stdout. Diagnostics go to stderr.
- A report that renders with a partial or unavailable state exits 0. An invalid selection exits 64. A storage or integrity failure exits nonzero. Dispatch exit codes never change to show accounting coverage.

## Cleanup and Finalize

- Pipeline cleanup removes scope artifacts. It leaves `gal/usage/v1/` in place.
- Finalize keeps accounting evidence and leaves it unchanged.

## Goal Verification and Receipts

Goal verification stays in the orchestrator process. No usage command dispatches it. Receipts keep the content that the dispatch and gate code wrote. Accounting never changes a receipt's acceptance result.

## Capability Gaps

- A source that GAL does not support yet reports its gap with a reason.
- An unsupported field stays unknown, even when elapsed time is known.
- The 2026-10-17 capability audit recorded installed CLI versions. It did not run a paid provider request, and it did not run live quota acceptance for every runtime.
- OpenCode was not on the PATH during that audit.

## Sources That Link Here

- [`commands/gal/SKILL.template.md`](../commands/gal/SKILL.template.md)
- [`commands/gal-pipeline/SKILL.template.md`](../commands/gal-pipeline/SKILL.template.md)
- [`commands/gal-finalize/SKILL.template.md`](../commands/gal-finalize/SKILL.template.md)
- [`workflows/coding.md`](../workflows/coding.md)
