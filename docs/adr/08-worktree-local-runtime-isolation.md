---
type: ADR
title: Worktree-Local Runtime Isolation
description: Defines immutable source-worktree runtimes and preserves package-manager ownership of shared GAL installations.
tags:
  - cli
  - runtime
  - worktree
  - isolation
status: stable
---

# Worktree-Local Runtime Isolation

## Context

Source worktrees need to build and validate GAL without changing the executable, projections, or gate validity observed by another worktree or a downstream repository. A mutable shared executable and a machine-global build lease couple otherwise independent worktrees. Downstream repositories still require the package-manager-installed executable.

## Decision

Use two execution lanes. A source worktree builds and runs a private immutable generation below its canonical worktree at `target/gal-pipeline/`. Cargo writes to `target/gal-pipeline/cargo-target/`. Published executables use their SHA-256 address under `target/gal-pipeline/bin/<sha256>/gal[.exe]`. A generation is revalidated and reused when its binary inputs are unchanged. A new generation is built and bound only at a safe new segment. Existing generations are never replaced in place.

Name the lanes `source-worktree` and `downstream-installed`. `CoordinatorState.execution_binding` is the only durable binding authority. Trusted receipts carry the deterministic digest of that binding. Before a trusted action, validate the binding, executable path and hash, and receipt digest. Missing or mismatched binding evidence stops the action. A safe-segment rebind points to a new immutable generation and retains earlier executable bytes.

The installed `gal` remains the stable guarded-entry shim. For a source worktree, every trust-bearing command hands off to the bound private executable before handler entry or trusted gate-result writing. The worktree is the nearest ancestor of the start directory that is a GAL source checkout, else the git top-level directory, else the start directory. A command started in any subdirectory therefore uses the root's binding. Bindings and receipts record executable and root paths in one normalized form, without the Windows verbatim prefix `\\?\` and with `/` separators. Each guarded attempt binds the executable's canonical path and SHA-256. Consumers rehash the executable and recompute the binding digest before trusting evidence. Coordinator-scoped receipts must match the current coordinator binding and fail closed when that state is missing, malformed, stale, or mismatched.

Downstream repositories continue to use the package-manager-installed GAL. Package managers alone install and promote the shared binary. Source builds, tests, pipeline runs, and finalize runs do not write shared binary, plugin, skill, cache, installation, or projection targets. Explicit acceptance roots must be private to the worktree or owned by a fixture.

Private-build writers serialize only within one canonical worktree. Safe rebinds retain prior immutable generations for earlier evidence. A rebuild or rebind is allowed only at a safe new segment with no active attempt, unconsumed checkpoint, or unrecovered projection journal.

## Consequences

Source-worktree gates use an immutable, hash-addressed executable, while downstream repositories retain the installed runtime. Receipt identity and scope are checked again at consumption. A shared installation is changed only through the package manager. Earlier immutable executable bytes remain available to validate evidence bound to them.

## Relationship to Earlier Decisions

ADR 03 remains authoritative for package-manager ownership, binary content hashes, and canonical machine-configuration resolution. This ADR supersedes its machine-global build-lease decision for source-worktree builds and its shared-binary assumption for source-worktree pipeline gates. The lease decision remains applicable only where a machine-global lifecycle operation still requires it; source-worktree private builds do not use that lease.
