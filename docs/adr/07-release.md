---
type: ADR
title: Release
description: Records why release is a distinct plan type, why a person initiates it, and the observed failure of dynamic C runtime linkage on Windows.
tags:
  - release
  - ci
  - windows
  - gate
status: stable
---

# Release

## Context

Release targets vary substantially across CLI binaries, cloud services, and package registries. Published release artifacts require strict operational governance over target destinations, pipeline initiation triggers, and user-facing communications.

## Decision

Model release workflows as a distinct plan type that executes through standard pipeline and landing mechanisms. Initiate release pipelines exclusively from human-pushed Git tags. Mandate human curation of stable release notes. Enforce statically linked C runtime libraries on Windows builds.

## Rejected Alternatives

**Embed release deployment logic directly within the landing stage.** Landing exists to finalize and verify a plan. It should not manage deployment workflows across diverse platforms. Combining deployment into landing transforms it into an unmaintainable monolith.

**Add a dedicated release phase to the standard pipeline.** Adding a release phase forces standard feature plans to carry unused release state. The pipeline phase sequence remains intentionally fixed and closed.

**Allow automated pipeline stages to trigger production releases.** Every release must trace directly to an intentional operator decision. Release workflows launch only when a human operator pushes a signed Git tag.

**Generate public release notes automatically from Git commit messages.** Raw commit logs expose internal engineering shorthand rather than user-centered feature descriptions. Machine-generated drafts cannot substitute for curated release communications.

**Validate release notes using separate local scripts and CI checks.** Independent validation scripts drift over time, allowing CI to pass release notes that fail local validation or vice versa. Both environments must invoke the exact same binary gate against identical document sections.

**Link the Windows C runtime dynamically.** In observed production failures, clean target environments and package manager sandboxes lacked the Microsoft Visual C++ redistributable. The binary compiled cleanly but failed at **runtime** without informative diagnostics. Statically linking the C runtime eliminates this external dependency.

**Permit the release agent role to infer deployment destinations automatically.** Misidentifying a target environment can deploy artifacts to incorrect destinations without a rollback path. The operator must confirm deployment targets explicitly as the sole authority.

**Involve the release agent role in active production deployments.** The release role provides planning and architecture advice only. It does not execute canary rollouts, trigger live deployments, or monitor production infrastructure.

## Consequences

Publishing a stable release requires a finalized changelog prior to publication. Operational deployment procedures, including credential management, remote access configurations, and rollback runbooks, are maintained in private operator guides outside the public repository documentation.
