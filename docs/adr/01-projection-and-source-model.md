---
type: ADR
title: Projection, Source Model, and Adapters
description: Records the overturned assumptions and rejected approaches for the canonical root, personal-layer merge, source resolution, and adapter roots.
tags:
  - projection
  - canonical-root
  - personal-layer
  - adapters
status: stable
---

# Projection, Source Model, and Adapters

## Context

GAL must support four AI runtimes that each impose distinct package format requirements. Users can supply custom skills and MCP servers alongside core workflow contracts. These two layers must merge into a single deterministically reproducible output. Because merging and cleanup interact directly with user-owned files, `gal` must also reliably discover its source contracts in both development source checkouts and packaged production installations.

## Decision

Maintain a single canonical root per machine, using core-first additive overlays. Require explicit ownership evidence before deleting files during cleanup, resolve the source root at the `.git` boundary, and consolidate repository-local adapters into one compact root file (`AGENTS.md`).

## Rejected Alternatives

**Maintain parallel plugin roots for each runtime.** Early designs assumed GitHub Copilot required a dedicated schema manifest at the plugin root. Real-world testing disproved this assumption. Copilot accepts `.claude-plugin/plugin.json` and, when `$schema` is declared, parses `com.github.copilot/` and `mcp.json`. Antigravity discovers content by directory structure and ignores manifest arrays. When this core requirement was disproved, the multi-root approach was retired.

**Use directory junctions in a skill directory shared with third-party tools.** This approach was initially default. Empirical testing revealed that third-party maintenance utilities routinely deleted directory junctions while leaving regular physical files intact. The failure stemmed from the link-based projection strategy rather than any specific third-party tool. While a physical file might become outdated, an outdated file can be refreshed. Unintended file deletion cannot be recovered automatically.

**Control personal-layer activation through a configuration flag.** Configuration flags easily drift from actual filesystem state. If a user deletes the personal directory without updating the flag, the system reports an active personal layer that does not exist. Inspecting directory existence directly eliminates this synchronization error.

**Declare directory layout through configuration mode keys.** Previous implementations used configuration keys such as `GalMode`, `devMode`, and `galRoot`. The runtime environment already provides this context. A source repository contains a `.git` root boundary, whereas an installed binary resides alongside a standardized packaged directory layout. Requiring users to configure their layout manually introduces a failure point where incorrect settings prevent contract discovery.

**Infer ownership of obsolete directories based solely on path names.** Other developer tools might share or reuse directory paths. Deleting paths based strictly on directory names risks destroying external user data. Automated cleanup now runs only when the target file contains an exact GAL generation header on its first line.

**Trigger cleanup based on observations from a single execution run.** In a multi-source architecture, pruning files that were not referenced during a single run can inadvertently delete assets owned by other configured sources. Cleanup routines must verify that an asset's source attribution is absent from the entire configured source set before deleting it.

**Embed complete convention documents directly into repository adapters.** Inlining convention text bloats initial context window consumption across all agent sessions and tightly couples CLI template rendering to the projection engine.

## Consequences

Supporting a new runtime requires adding a namespace directory under the existing canonical root rather than establishing a separate root. An adapter pointer can expand into a full conditional layer only after its runtime-specific loading mechanism has been validated on physical hardware. Obsolete directories lacking explicit ownership markers are retained for manual operator inspection.
