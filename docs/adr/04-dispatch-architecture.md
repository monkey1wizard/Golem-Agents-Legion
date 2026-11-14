---
type: ADR
title: Dispatch Architecture
description: Records rejected approaches and observed corruption in contract delivery, process containment, role boundaries, remote execution, and phase write-back.
tags:
  - dispatch
  - containment
  - isolation
  - remote
  - write-back
status: stable
---

# Dispatch Architecture

## Context

The dispatch engine must address four core operational challenges simultaneously. First, remote executors cannot access filesystem paths that exist only on the control node. Second, uncontained child processes can escape reclamation after execution timeouts. Third, background executor sessions can ingest repository-level instructions and inadvertently act as secondary orchestrators. Fourth, returning execution results must never corrupt the active prompt. All four defensive protections must be established before dispatch begins.

## Decision

Embed the task specification directly into self-contained contract bytes. Enclose child processes in containment groups before launching execution. Enforce role boundaries through two independent structural mechanisms. Run cross-machine execution through synchronous SSH connections. Apply a two-stage write-back flow where executors return only execution receipts while the control node validates and places all updates. Flag every degraded state explicitly.

## Rejected Alternatives

**Deliver contracts through remote file staging, resolver traits, contract hashes, or CLI flags.** Each of these approaches introduces secondary failure states, including partial uploads and incomplete cleanups. Directly embedding contract bytes requires only a single parsing step on receipt. While this increases individual phase payload size by roughly 6 to 14 KB, the predictability outweighs the payload overhead.

**Position role boundary blocks after the primary task objective.** Prompt ordering heavily influences model behavior. Placing isolation boundaries after the primary objective allows earlier instructions to override boundary constraints. The boundary declaration must precede the task objective.

**Restrict unauthorized subcommands using an explicit enumeration list.** Static deny lists become obsolete whenever new subcommands are introduced. The contract instead enforces a blanket prohibition against all subcommands.

**Rely exclusively on boundary prose to prevent instruction ingestion.** Host executor CLIs routinely ingest project-level instruction files before parsing the task specification. Prompt prose cannot counteract context that has already been loaded. Hard runtime invocation flags must enforce isolation alongside textual boundaries.

**Attempt process tree cleanup only after a timeout occurs.** By the time a timeout triggers, child processes have often detached or spawned independently. Executors must launch inside process containers (such as POSIX process groups or Windows job objects) that support atomic group termination from the start.

**Wait indefinitely for cleanup routines to terminate.** A single hanging child process can stall the orchestrator indefinitely. All cleanup routines enforce strict bounds, and unconfirmed processes return an explicit degraded status rather than blocking.

**Manage remote sessions using terminal multiplexers and asynchronous polling.** A previous implementation relied on tmux and screen sessions with polling loops. This introduced complex state machines, timeout races, and orphan process management without improving execution reliability.

**Reset the remote repository before applying incoming diffs.** If diff application fails after an immediate reset, the original diagnostic evidence is wiped out. Preserving the dirty state until diff validation succeeds prevents unrecoverable evidence loss.

**Allow headless executors to write updates directly to prompt files.** Empirical tests produced duplicated Markdown headers and corrupted table structures. The orchestrator now controls prompt placement directly to guarantee structural consistency.

**Rely solely on Git status flags to detect zero-delivery dispatches.** If a file was already modified prior to dispatch, its status flag remains unchanged, causing legitimate updates to be misclassified as empty deliveries. The engine now inspects both Git status flags and cryptographic content digests.

**Allow degraded or fallback executions to report successful completion silently.** If an executor that degraded or failed to launch reports a clean pass, downstream verification gates operate on false premises.

**Infer underlying failure causes from indirect timeout observations.** Attributing a timeout to specific conditions (such as interactive prompt blocking) claims more certainty than monitoring can verify. Similarly, logging a clean state when a probe is inconclusive falsely portrays an unprotected executor as secure. Inconclusive probes are recorded explicitly as inconclusive.

## Consequences

Task specifications intentionally exceed the standard 5 KB threshold, typically measuring between 6 and 14 KB. Two legacy timeout tokens are retained solely for backward log compatibility without asserting causality. Remote execution supports only POSIX environments and handles one synchronous task at a time without reconnection persistence. Remote work directories must be dedicated checkouts because post-apply cleanups are intentionally destructive.
