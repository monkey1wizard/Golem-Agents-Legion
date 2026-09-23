---
type: ADR
title: Gates, Evidence, and Enforcement
description: Records rejected alternatives involving claims instead of evidence, compromised role independence, multiple landing-verification channels, and wording in place of executable enforcement.
tags:
  - gates
  - receipts
  - roles
  - enforcement
status: stable
---

# Gates, Evidence, and Enforcement

## Context

Autonomous agent pipelines face three common failure modes: language models self-report success without proof, verification independence breaks down when roles inspect previous agent outputs, and constraints declared purely as prose are deprioritized by attention mechanisms. These failures occur whenever authority is delegated to model assertions rather than verifiable evidence.

## Decision

Delegate read-only validation to mechanized binary checks, assign semantic evaluation to specialized AI roles, and reserve destructive filesystem and Git operations for the orchestrator. Accept signed receipts as the sole criteria for gate passage. Isolate dispatched role outputs from one another. Use a single evidence channel for landing verification. Enforce constraints through executable code rather than prompt phrasing.

## Rejected Alternatives

**Accept self-reported success as proof of gate completion.** Allowing the executing role to validate its own output makes the evaluated agent its own judge. Preventing this conflict of interest is the primary reason the gate system exists.

**Treat unexecuted checks as passed.** An unexecuted check indicates missing validation rather than a clean result. Any check that is skipped or fails to run must exit with a nonzero code and count as an immediate failure.

**Permit free-form decisions and explanations in gate receipts.** Automated tools cannot reliably parse and validate unconstrained natural language. Gates must enforce closed, enumerated vocabularies for all decisions and reason codes.

**Allow downstream roles to inspect earlier agent outputs.** Each agent role serves a distinct evaluative purpose. If a tester reads the implementer's self-assessment, it tests those specific assertions rather than the code itself. If a security auditor inspects test logs beforehand, it inherits the tester's baseline assumptions.

**Permit dispatched worker roles to execute Git commits.** Destructive repository operations require holistic awareness of global project state, whereas dispatched roles operate under intentionally constrained task boundaries.

**Validate task quality only after dispatch.** Dispatching tasks with missing or malformed acceptance criteria wastes execution tokens on unusable outputs. Preflight validation must reject malformed specs before dispatch begins.

**Retry failed tasks by replaying identical prompt inputs.** When objectives, handoff notes, file allowlists, and contracts remain unchanged, re-executing an identical prompt merely consumes tokens reproducing deterministic failures.

**Introduce a secondary freshness mechanism for goal binding.** Maintaining parallel freshness checks introduces race conditions and contradictory states that make landing failures difficult to diagnose.

**Verify execution facts across multiple evidence channels.** Per-task references and executor logs previously functioned as parallel evidence paths. They inevitably drifted out of sync, leaving orchestrators without a clear source of truth.

**Resolve state file merge conflicts using a custom Git merge driver.** Custom merge drivers require machine-local configuration. Any unconfigured developer machine would silently fall back to default Git behavior and fail during merges.

**Split global state into separate per-plan files.** Decomposing state into multiple small files fragments session context and destroys the at-a-glance visibility of `.dev/state.md`.

**Regenerate `.dev/state.md` automatically from plan files.** Rebuilding state dynamically from scratch wipes out the independent editing history and session metadata recorded in the state file.

**Enforce workflow compliance and dispatch logging through prompt prose.** Language models frequently deprioritize instructions buried in system prompts or skill descriptions. Furthermore, models can hallucinate state transitions that never occurred on disk. Executable binary checks enforce compliance deterministically.

**Automatically resolve Git conflict patterns in files beyond `.dev/state.md`.** Broadening automatic conflict resolution forces the tool to guess developer intent across arbitrary source code.

## Consequences

Boundary checks confirm only that modified files match the configured allowlist. They do not verify functional correctness. Goal-verified handback operates as a cooperative protocol rather than a cryptographic guarantee, so it does not prevent tampering by processes with write access. Process crashes and unconfirmed timeouts deliberately leave stale lock leases behind, forcing subsequent runs to fail closed until an operator clears them. Workflow changes that previously relied on prompt tweaks now require Rust code updates, which introduces deliberate development friction.
