---
type: ADR
title: Release
description: Records release planning, maintainer-led local publication, public tests before signing, and static Windows runtime decisions.
tags:
  - release
  - ci
  - windows
  - gate
status: stable
---

# Release

## Context

Release targets vary across CLI binaries, cloud services, and package registries. Published release artifacts require clear operational governance over destinations, initiation, and user-facing communications. Private GitHub Actions became unavailable because of an account billing limit, so publication must run from a maintainer machine with public repository write access.

## Decision

Model release workflows as a distinct plan type that executes through standard pipeline and landing mechanisms. Require explicit human approval of stable versions and curated release notes. Enforce static C runtime linkage on Windows builds.

Publish from private `main` by running `packaging/publish-public.sh` locally. The publisher builds a curated public snapshot from a temporary Git index, preserving tracked file modes and blobs while excluding private paths. It pushes a parentless snapshot to public `main` and pushes the approved release tag to the public repository. The snapshot commit records the full private source SHA in a `Source-Commit:` trailer. The private publishing controller is removed. Private repository Actions are disabled after landing.

The public `release.yml` runs hosted tests through `ci.yml` and builds before signing. The private `snapshot` CI job is removed together with the publisher's `--keep-snapshot` option. The `sign` job waits for both `test` and `build`. A hosted test failure therefore prevents signing and channel publication. Public release tags are immutable. A failed release that has already pushed a tag requires a new version.

GitLab is a backup only and runs no release workflow.

## Rejected Alternatives

**Embed release deployment logic directly within the landing stage.** Landing exists to finalize and verify a plan. It should not manage deployment workflows across diverse platforms. Combining deployment into landing would make it difficult to maintain.

**Add a dedicated release phase to the standard pipeline.** A release phase would force standard feature plans to carry unused release state. The pipeline phase sequence remains intentionally fixed and closed.

**Allow automated pipeline stages to trigger production releases.** Every release must trace directly to an intentional operator decision. A maintainer approves the version and initiates local publication.

**Generate public release notes automatically from Git commit messages.** Raw commit logs expose internal shorthand rather than user-centered descriptions. Machine-generated drafts cannot replace curated release communications.

**Validate release notes using separate local scripts and CI checks.** Independent validation scripts drift over time. Local and hosted checks should invoke the same binary gate against identical document sections.

**Link the Windows C runtime dynamically.** Clean target environments and package manager sandboxes have lacked the Microsoft Visual C++ redistributable. Static C runtime linkage removes this external dependency.

**Permit the release agent role to infer deployment destinations automatically.** Misidentifying a target can publish to the wrong environment without a rollback path. The operator remains the authority for deployment destinations.

**Involve the release agent role in active production deployments.** The release role provides planning and architecture advice only. It does not execute production deployments or monitor production infrastructure.

## Consequences

A stable release requires a finalized changelog and explicit maintainer approval before publication. The local publisher creates a public orphan snapshot from the private source commit, so public history does not include private commits. Hosted tests and builds must pass before signing.

The publication harness remains in the exported tree and passes the configured secret scan. Build synthetic secret fixtures at test runtime so matching literals in test source cannot block publication. The integration harness includes its own source in a success fixture and checks the retained blob and mode.

Operational details, including remote configuration, channel credentials, failure handling, and post-release verification, remain in the private maintainer runbooks. Homebrew and WinGet are stable-only channels and remain unverified until a real stable release completes.

### Reproducing cross-platform CI failures

When investigating a release test failure, use the checked-in CI commands on both the private source checkout and the curated public snapshot from the same candidate. Keep Git metadata and tracked file modes in both checkouts, and keep the temporary directory outside them. Record the source and snapshot commit IDs, the test names, and each command's exit code.

Run provider test doubles with the executable format the host expects. If a readiness check needs a credential, use a synthetic value scoped to the test child or restore the prior process environment when the fixture ends. Check output at the layer that owns it. Compare stored UTF-8 bytes when the collector normalizes input, and verify the expected rejection before comparing unchanged log bytes in a path-sensitive test.
