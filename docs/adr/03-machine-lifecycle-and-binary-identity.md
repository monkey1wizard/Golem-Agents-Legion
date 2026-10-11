---
type: ADR
title: Machine Lifecycle and Binary Identity
description: Records removed approaches and observed failures in installation, rebuilding, binary identification, and configuration-path resolution.
tags:
  - cli
  - lifecycle
  - bootstrap
  - configuration
status: stable
---

# Machine Lifecycle and Binary Identity

## Context

Binary installation, local rebuilding, identity verification, and configuration path resolution operate at the machine layer and closely interact. Concurrent processes can write to the same binary destination simultaneously. Furthermore, the integrity of every gate receipt depends on guaranteeing that the executing binary matches the precise version intended by the orchestrator.

## Decision

This decision remains authoritative for package-manager ownership of shared installation, cryptographic binary identity, and canonical machine-configuration resolution. ADR 08 supersedes only the machine-global advisory lease for source-worktree builds and the assumption that source-worktree pipeline gates execute a mutable shared binary. This ADR remains in force for downstream installed runtimes and the decisions listed above.

Partition CLI subcommands into workflow, lifecycle, and internal layers, while delegating binary installation entirely to package managers. This ADR originally selected a single machine-global advisory lease for local builds and installation. ADR 08 supersedes that lease for source-worktree private builds. Authenticate binary identity using cryptographic content hashes. Resolve machine configuration strictly through a single canonical function.

## Rejected Alternatives

**Retain `install`, `sync`, and `setup` CLI commands.** All three commands previously existed as installation entry points. Because they invoked the projection engine directly, they embedded machine-level package management inside the `gal` binary instead of delegating that responsibility to native package managers.

**Enumerate all subcommands statically in an architectural decision record.** A previous iteration of this ADR listed 26 subcommands, which became obsolete when the codebase grew to 30. Static documentation rosters drift over time. The command array defined in Rust source code serves as the sole authoritative registry.

**Use `gal --version` strings to verify binary identity.** Standard version strings encode only a short Git commit hash and a dirty flag. Empirical testing proved that two concurrent worktrees checked out at the same commit output identical version strings despite executing different binary bytes. Because concurrent worktrees are standard practice in this repository, version strings cannot establish true binary identity. Content hashes are required.

**Acquire file leases by creating directories with an overwrite or force flag.** A forced directory creation succeeds even when the target directory already exists. This turns mutual exclusion into a silent no-op, allowing two competing processes to assume they both acquired the lock.

**Automatically clean up stale lease markers left by crashed processes.** An initial cold compilation on a heavily loaded system can exceed standard lease timeouts. Automated pruning would misclassify a slow build as a crashed process, releasing the lock while compilation is still running. Both situations halt execution and alert the operator because only a human can reliably distinguish a slow build from a crash.

**Unify home directory resolution between the internal path helper and external library.** On Windows, these implementations diverge intentionally: the external library queries Windows Known Folders, whereas the internal helper checks environment variables. Forcing parity breaks established caller assumptions. In previous testing, altering this behavior generated an empty routing table that silently degraded every dispatched agent role into the primary chat session without raising an error.

## Consequences

The two path resolution functions remain intentionally asymmetric, backed by dedicated regression tests. Registering a new subcommand requires updating the authoritative Rust array and its corresponding documentation. The global lease is advisory and does not restrict manual user compilations. Terminated or crashed processes deliberately leave lease markers behind, requiring deliberate manual cleanup.
