---
type: ADR
title: Scope and Ownership Boundaries
description: Records the responsibilities GAL deliberately does not assume and the Git features deliberately excluded from parallel development.
tags:
  - boundaries
  - scope
  - git
status: stable
---

# Scope and Ownership Boundaries

## Context

Cross-runtime tooling risks architectural instability when it assumes responsibilities managed by external runtimes. Similarly, parallel development workflows lose traceability when Git history is rewritten for cosmetic cleanliness.

## Decision

Manage only artifacts that can be deterministically reconstructed from source contracts. Isolate concurrent development in separate Git worktrees and restrict version control to standard Git operations.

## Rejected Alternatives

**Generate or execute runtime-specific executable plugins.** Introducing executable binary code into the projection scope would force GAL to maintain dedicated execution runtimes for each supported tool. Because open agent standards rely on declarative data, projecting them requires only standard filesystem operations.

**Place host components directly in the canonical plugin root.** Components developed so an external runtime can interface with GAL, such as editor extensions that expose commands in chat interfaces, consume projected contracts rather than being projected themselves. Every asset in the canonical root must be deterministically reproducible from source contracts. Host programs that consume projections do not meet this standard.

**Write directly to host-managed plugin caches or installed-plugin registries.** Modifying state managed by another application introduces unpredictable errors whenever upstream formats change beyond GAL's control.

**Implement marketplace plugins as full software installers.** This pattern introduces a redundant installation state machine outside the core GAL lifecycle. Marketplace packages should provide discoverability and configuration guidance, while native operating system package managers handle binary installation.

**Use squash merges, interactive rebases, `--ff-only`, or `--no-ff`.** All four Git operations are prohibited from automated execution and manual workflows alike. Concurrent tasks run in isolated Git worktrees without history rewriting. Investigating regressions depends on precise commit provenance, and rewriting history destroys that audit trail.

## Consequences

Restricting plugin execution boundaries limits OpenCode capabilities under GAL. Host applications must restart to detect regenerated projections. Retaining explicit merge commits creates a more detailed commit graph, which is an intentional tradeoff for forensic auditability.
