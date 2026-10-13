---
type: ADR
title: Documentation and State Governance
description: Records rejected approaches to front matter keys, translation freshness, bounded state, personal-content sourcing, and admission criteria for this directory.
tags:
  - documentation
  - metadata
  - bounds
  - personal-layer
  - adr-policy
status: stable
---

# Documentation and State Governance

## Context

Documentation and persistent state suffer from three primary failure modes: unbounded file growth exhausts model context windows, unmanaged metadata decays silently over time, and personal developer preferences leak into shared repositories. In addition, the ADR directory presents a bootstrapping challenge: its own admission criteria must reside in a permanent architectural record rather than transient planning files.

## Decision

Adopt exactly five metadata keys supported by active workflows. Track translation freshness using upstream Git commit hashes rather than status flags. Enforce content-shape limits on state and index files at render-blocking gates. Source personal customization layers strictly from local machine storage and allow repositories to opt out. Anchor non-English plans to explicit technical meaning through three authority layers. Prohibit semicolons across all documentation. Enshrine ADR admission criteria within this document.

## Rejected Alternatives

**Adopt the four trust and lifecycle keys `verified`, `stale_after`, `generated`, and `sources`.** Empirical analysis supports rejecting these keys. In an evaluation across 32 documents, unmanaged date fields diverged from Git commit timestamps in every single instance. All 32 discrepancies were introduced during the initial commit that added the front matter. A date or lifecycle field lacking automated tooling becomes obsolete the moment it is committed.

**Adopt only `verified` or only `stale_after`.** Adopting `stale_after` alone creates expiration dates without a verification process. Adopting `verified` alone creates a verification timestamp without expiration criteria. These two concepts function only as a paired lifecycle contract.

**Use `status` to represent translation freshness.** An earlier version of this system marked translations as `current` or `stale` within the `status` field. This collided directly with canonical document lifecycle values (`draft`, `stable`, `deprecated`). Furthermore, the freshness validation check compared Git commit hashes directly and ignored the `status` value entirely.

**Store workflow state in an external database or cloud service.** State must move seamlessly between local developer tools and remain inspectable through standard Git diffs. External databases provide neither portability nor direct diffability.

**Enforce only raw byte limits on files.** Byte limits fail abruptly once reached. Structuring bounds around content shape (such as maximum table rows or section depths) keeps files bounded throughout normal use.

**Document size limits purely as authoring guidelines.** Unenforced advice is routinely ignored. Bounds must be enforced at render-blocking gates. If a file exceeds its bound, rendering must halt completely rather than producing partial output that masquerades as valid data.

**Allow index files to append entries indefinitely.** Uncapped index logs accumulate historical debris over time. Version history belongs in Git history, whereas index files must reflect only currently active state.

**Require external third-party tools for repository hygiene checks.** If a hygiene gate relies on external software that contributors might not have installed locally, the gate fails to run when needed most.

**Control personal layer injection using configuration flags.** Defaulting to automatic injection is acceptable for private workflows but risks leaking personal configurations into open-source repositories. Each repository must provide an explicit opt-out mechanism.

**Treat non-English prose directly as the authoritative record of technical meaning.** Complex technical nuance in non-English planning documents degrades across multi-turn model sessions. An English semantic draft must anchor technical intent before translation.

**Permit semicolons in documentation.** Semicolons obscure the logical relationship between adjacent clauses and force the reader to infer semantic connections. This restriction applies equally to halfwidth and fullwidth semicolons across all document languages.

**Assign the same base filename to unrelated topic documents.** Reusing identical filenames causes ambiguity during local searches, editor navigation, and automated cross-referencing.

**Add architectural decision records without formal admission criteria.** An earlier iteration of this project accumulated 41 ADRs that remained static throughout their lifecycle before being deleted in a single batch. Without an admission gate, ordinary design documentation gets numbered as an ADR instead of being maintained by its respective topic owner.

### Admission Criteria for This Directory

An ADR must satisfy two mandatory criteria before admission:

1. The rejected alternative must have **been implemented, actively scheduled, or deployed as default behavior**. Hypothetical designs and mere logical opposites of a decision do not qualify.
2. The supporting evidence must be **concrete and nameable**: an exact metric, a reproducible error message, or a disproven operational premise. Vague claims such as "verified" or "tested" do not qualify.

## Consequences

When a file approaches its bound, authors must compress prose rather than raising the limit. Cross-machine synchronization of personal configurations is excluded from the project scope. The admission criteria are permanently anchored here because plans are deleted upon completion, which would otherwise destroy the governance standard.
