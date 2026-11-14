---
name: local-first-search
description: Search local repo-owned material first, then use optional local-notes backends only when they are ready. Activate before answering conceptual questions, proposing solutions, or planning note-adjacent work.
---

# Local-First Search Strategy

## Core Philosophy

**Context first, local first** means: before answering conceptual questions, proposing solutions, or
planning note-adjacent work, search repo-owned material first.

Avoid generic answers that ignore existing local context.

## Search Order

Use this order unless the user explicitly asks for a different source:

1. **codebase**
   - source files
   - comments
   - tests
   - adjacent call sites

2. **docs**
   - `docs/`
   - plan documents
   - workflow and convention docs

3. **repo text artifacts**
   - `.dev/`
   - configuration files
   - notes stored in the repo itself

4. **optional external notes**
   - only after the first three lanes are insufficient
   - only when the `local-notes` optional-capability lane is `ready`

## Optional local-notes Lane

When repo-local retrieval is insufficient, consult the `local-notes` contract in
[optional-capabilities.md](../../conventions/optional-capabilities.md), with operational setup in
[docs/workflows.md](../../../../docs/workflows.md), [docs/configuration.md](../../../../docs/configuration.md), and [docs/integrations.md](../../../../docs/integrations.md).

Rules:

- treat external notes as optional additional evidence, not a Core prerequisite
- do not assume any specific note application, vault shape, diary ritual, or personal taxonomy
- do not assume semantic search exists
- if local-notes is not `ready`, degrade silently to repo-local retrieval only

## Execution Workflow

### Phase 1 — Codebase Search

- search the owning code path first
- prefer exact text or symbol search for known identifiers
- read the minimum surrounding context needed to answer accurately

### Phase 2 — Docs Search

- search durable docs and plan files relevant to the question
- use conventions and workflow docs when the question is about process or contract behavior

### Phase 3 — Repo Artifact Search

- search `.dev/`, repo-local notes, and tracked config surfaces when they carry task context
- keep repo-owned state ahead of any private note system

### Phase 4 — Optional External Notes

- only enter this phase when the first three phases did not provide enough signal
- only use a machine-local backend that satisfies the shared checking contract
- keep external notes behind the portable Core answer, not in front of it

## Retrieval Guidance

- prefer exact local evidence over broad recollection
- prefer the nearest owning abstraction over a broad repo sweep
- cite repo files directly when the answer comes from repo-owned material
- if external notes are used, say they were supplementary rather than Core-local evidence

## Boundaries

- do not require private note stores for normal GAL operation
- do not write note-app-specific assumptions into Core behavior
- do not use personal folder conventions as a default search policy
- do not use external notes to override authoritative repo state

## Cross-Boundary Referencing

- when writing in the repo, reference repo files normally; do not assume note-app-specific link syntax
- when a downstream note backend is used, let that backend's own contract decide how note references
  are represented
