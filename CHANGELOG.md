# Changelog

All notable changes to GAL are recorded here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

The public repository receives a curated snapshot rather than a per-commit
mirror, so this file — not the auto-generated GitHub release notes — is the
authoritative record of what changed in each release.

## [Unreleased]

## [0.3.1] - 2026-10-09

### Added

- Standalone `architect`, `analyst`, `designer`, and `releaser` commands now support isolated consultation and in-context discussion through an optional first argument `discuss`.
- Finalization can dispatch its plan-level review to the configured AUDITOR executor in a fresh context. The dispatcher validates the review receipt and writes the result to the execution prompt.
- Generated `AGENTS.md` files now include a repository command index with pointers to the tracked command templates.

### Changed

- Normal `/gal pipeline` completion now continues into finalization when the same invocation produces and verifies a `goal-verified` handback receipt with `voluntary_response_authorized: true`. Outstanding owner acceptance still stops finalization. `terminal-reverify` refreshes verification only and does not start finalization.
- Finalization permits an in-process review only when the AUDITOR route is absent and records `Review Independence: DEGRADED_SAME_RUNTIME`. Invalid or incomplete routing blocks the review instead of falling back.
- Planning-group routing settings for direct architect, analyst, designer, and releaser calls affect Codex native-agent projection only. Direct-call output now states this scope.
- **Security:** GAL's Codex executor now uses `--dangerously-bypass-approvals-and-sandbox` for dispatched child processes instead of `workspace-write`. This avoids the observed Windows sandbox setup failure and runs those children without Codex sandbox or approval enforcement.
- Workflow, configuration, projection, and setup documentation now describe the consultation commands and automatic finalization, with corresponding Japanese and Traditional Chinese updates.

### Fixed

- Executor smoke checks now accept `SMOKE_PASS` with or without the required audit receipt heading, while still checking the captured path, file bytes, and SHA-256 digest.
- Homebrew installation smoke checks now require `AGENTS.md` and verify that `gal init` does not generate the retired `CLAUDE.md` adapter.
- Dispatch tests now give the readiness probe more time to start. The Windows executor smoke test that changes `PATH` now takes the shared environment lock.

### Removed

- **Migration:** `/gal discuss <role>` and the Codex `$discuss-<role>` skills are retired. Use the standalone role command with `discuss`, such as `/architect discuss` or `$architect discuss`. `gal refresh` removes recorded GAL-owned legacy discussion skills while preserving unrecorded user directories.

## [0.3.0] - 2026-10-07

### Added

- Source worktrees now build and reuse private, immutable GAL executables under `target/gal-pipeline/`. Guarded commands run through the worktree's executable without replacing the shared installation.
- Trusted gate receipts now identify the executable and execution binding. Receipt consumers verify the executable hash and binding before accepting evidence.
- Repository writing checks now support English, Taiwan Traditional Chinese, and Japanese profiles through a pinned textlint toolchain. Checks protect code and literal tokens, validate project terminology, and produce isolated reports through CLI or MCP transport.

### Changed

- **BREAKING:** `AGENTS.md` is now the sole generated repository adapter root. `gal render-adapters` removes retired `CLAUDE.md` files only when they carry GAL ownership markers and preserves user-owned files.
- **Migration:** Runtime plugin registration now determines which fallback files GAL projects. The retired `pluginMode` settings are ignored and reported by `gal doctor`. Missing or uncertain registration keeps fallback files available.
- Source acceptance with `gal refresh` now requires both `--root` and `--source`. Invalid or shared installation roots are rejected before writes. Package managers retain ownership of shared installations and projections.
- Planning tasks now describe deliverable behavior. Human-only checks belong in `Owner Acceptance`, and owner-supplied environment requirements belong in `Preconditions`. `/plan-to-prompt` preserves both sections in the execution prompt.
- Repository adapters and dispatched task specifications now carry shared writing guidance for factual evidence, meaning preservation, and prose review. Optional checkers supplement that review.
- Agent contracts now require continued work on authorized steps until completion or a required stop, with progress updates that identify the next action.

### Fixed

- Guarded commands started from a repository subdirectory now use the repository root's executable binding. Windows private builds use paths Cargo can accept.
- Pipeline preflight, handback, and finalization receipt checks now reject missing, stale, malformed, or mismatched execution bindings. Legacy boundary and convergence checks remain available without a coordinator.
- Planning checks now reject unresolved placeholders inside commands and stop on task parser errors rather than accepting incomplete task evidence.
- Projection refreshes now use one plugin registration snapshot across skills and commands, recompute plugin ownership on each refresh, and preserve user-owned collisions. `gal doctor` uses the same registration evidence and reports uncertainty.
- Writing checks now preserve prose around protected literals and table cells, reject malformed locale arguments, retain partial repair evidence, and isolate concurrent terminology snapshot generation.

## [0.2.4] - 2026-09-30

### Added

- `$gal-pipeline` now coordinates guarded entry, implementation, testing, auditing, and convergence. It records task-scoped evidence and pauses for the required goal-backward judgment.
- Codex projections now install a hook that grants guarded pipeline entry only after a verified host checkpoint. `gal doctor` reports whether the projected hook is ready.
- Dispatch records now capture provider session and process identity evidence. Pipeline recovery can resume compatible attempts without treating incomplete or mismatched evidence as successful work.

### Changed

- Pipeline continuation now uses typed actions, bound receipts, retry ceilings, and a durable coordinator. Projection updates recover atomically across their managed output surfaces.
- Codex, Claude, Copilot, and Antigravity projections preserve the shared pipeline contract while rendering Codex-specific hook configuration only for Codex.

### Fixed

- Guarded pipeline checks now reject placeholder provider sessions, unbound evidence-only writebacks, stale grants, contradictory logs, and ordinary no-writeback attempts.
- `gal doctor` now reports actionable plugin-mode drift and static Codex hook configuration issues.
- Executor diagnostics and writeback checks now bind readiness, launch, receipt, and provider-session evidence to the same dispatch attempt.

## [0.2.3] - 2026-09-23

### Added

- Pipeline plans are checked against a task-quality checklist before dispatch. Each task now follows implementation, reconciliation, commit, testing, and audit stages.
- Documentation checks now validate Markdown links and their heading fragments across tracked files.
- Translation freshness checks now compare identifier coverage in both directions between canonical documents and translations.

### Changed

- Pipeline receipts, logs, and replay evidence are organized by plan and task. Finalization removes the plan's temporary evidence after deleting its plan files.
- The core plugin no longer bundles `github-mcp-server`, `microsoftdocs/mcp`, `microsoft/markitdown`, `DeusData/codebase-memory-mcp`, or `upstash/context7`. Users can register MCP servers in `~/.gal/local/mcp.json` or configure them in the host agent. Skills that list `context7` as optional fall back to `fetch`.
- Documentation now organizes workflow and setup guidance by function, with refreshed Japanese and Traditional Chinese translations.

### Removed

- The legacy test-first scaffold phase and its retired pipeline commands have been removed. Plans now specify task quality criteria, and independent testing and auditing evaluate each task's committed changes.

## [0.2.2] - 2026-09-11

### Added

- `gal import-plan` (`/gal import-plan`): converts a completed external plan
  into a pipeline-ready execution prompt, so plans drafted outside GAL's own
  planning flow can enter `$gal-pipeline` without being redrafted from
  scratch.
- `executorRouting.combinations`: a named registry of executor
  `model`/`effort`/`timeoutSecs` combinations. Routing entries can reference
  a combination by name instead of repeating the same executor settings.
- `gal pipeline-preflight` now prints the resolved route line before its
  summary, showing which executor and combination (or literal override) a
  run will use.

### Fixed

- A race during self-bootstrap rebuild and install, reachable when two
  pipeline runs started concurrently, is now closed by an advisory lease
  around the install step.
- `CALL_FAILED` narration now quotes the executor's raw output instead of
  paraphrasing it, closing a blind spot where the actual failure cause was
  silently dropped.
- The boundary check's affected-files lookup now requires an exact path
  match before falling back to substring matching, fixing a false positive
  when one tracked file's path was a substring of another's.
- The tester golem's `spec` mode now verifies its target and receipt before
  confirming, closing a gap where a wrong target could pass unnoticed.
- `gal release`'s Homebrew step now audits the generated formula before
  pushing it to the tap, and no longer writes a redundant version stanza
  into the formula.
- WinGet manifests now include the Golem and Golem Agents Legion package
  entries.

## [0.2.1] - 2026-09-08

### Added

- `gal render-adapters`: regenerate repo-local adapters on an
  already-initialized repo without touching `.dev/project.md` or
  `.dev/state.md`.
- Plugin-mode support: GAL can now defer skill/command distribution to a
  runtime's own plugin mechanism (Claude, Codex, Copilot, Antigravity)
  instead of writing its own copies, with a new `gal doctor` check that
  flags mismatches.
- Codex and GitHub Copilot both gained a proper plugin manifest, so `gal
  refresh` sets them up the same way it already does for Claude.
- `/gal finalize` now runs its review in-process instead of dispatching a
  separate whole-branch audit.

### Changed

- **BREAKING:** `gal init` refuses to run on a repo that's already
  initialized — use `gal render-adapters` instead. The `--force` flag is
  removed; to reset a repo, delete `.dev/project.md` and `.dev/state.md`
  yourself, then run `gal init`.

### Fixed

- `agy` dispatches no longer time out five minutes into a run.
- The version stamp GAL renders is now reproducible across machines.
- `/gal init` routes correctly again after a command-doc typo.

## [0.2.0] - 2026-09-02

### Added

- Tasks marked `Test-first: required` now follow a locked lifecycle: a
  behavior-free scaffold, a locked probe proven red, an implementation
  against that frozen probe, a rerun proving green, and only then an
  audited commit — with dedicated `gal test-first-probe` tooling to run
  and encode those probes.
- `gal pipeline-preflight --terminal-reverify` adds a closed
  re-verification mode so `/gal finalize` can no longer proceed on a
  prompt that only looks finished.
- `agy` (Antigravity CLI) dispatches now honor `--model` and `--effort`.

### Changed

- Dispatched task specs now carry an explicit worker-boundary block, and
  the `codex`/`claude` adapters suppress repository instruction
  injection, so a dispatched worker can no longer read
  orchestrator-only instructions; review independence is now framed as
  dispatch isolation rather than a required vendor/model difference.
- Dispatched `test`/`audit` results now flow through a phase receipt
  that the `gal` binary validates and applies, instead of the executor
  editing the execution prompt directly.
- A task marked `Test-first: not-applicable` now satisfies the
  `implement` phase with either a completed or a no-writeback attempt.
- Snapshot handling and cleanup around test-first tasks are now
  transactional and crash-safe, and goal verification is now bound to
  a terminal-receipt hash.

### Fixed

- `NoWriteback` detection is now content-aware (a SHA-256 over the
  actual bytes, not just git status flags), and no longer misfires as
  `WorkdirEscape` from ordinary activity in sibling worktrees.
- `gal finalize-check` no longer mutates the repository; its
  sync-idempotency row is now read-only.
- A dispatched `implement` that exits 0 without writing anything is now
  caught as a `no-writeback` state instead of counted as completed.
- `/gal finalize`'s branch-audit now recognizes a same-runtime dispatch
  failure and runs a compensating review instead of treating it as a
  successful independent audit.
- Test-first probe parsing no longer breaks on a backtick-wrapped
  `Expected failures:` header or a nested code fence.
- Test-first contracts no longer reject a task whose production and
  test code live in the same file.
- `agy` dispatches now always pass `--add-dir <workdir>`; without it,
  `agy` wrote into its own scratch directory and missed the
  workspace's `AGENTS.md`/`GEMINI.md` instructions while still
  reporting success.
- Home-directory resolution for `~/.gal/config/config.json` is now
  handled by one shared function instead of three diverging copies.
- Machine-local "personal" settings can no longer leak into shared
  adapter output or match a repository they shouldn't apply to.

## [0.1.4] - 2026-08-24

### Added

- `gal doctor` gained an OpenCode projection health check that detects drift
  between the source pipeline skill and the copy projected into OpenCode.
- Pipeline fix-mode rounds are now validated before they run: a fix attempt
  that changes no file is rejected instead of being retried silently.
- `gal pipeline-handback-check` is wired into the CLI to enforce progressive
  handback authority, so repeated handbacks on the same task are capped
  instead of allowed to loop indefinitely.
- OpenCode agent files are now written and pruned individually instead of
  swapping the whole managed-artifact directory, so a partial write can no
  longer wipe agent files that were not part of the change.

### Changed

- Removed the retired Gemini runtime and every reference to it across the
  CLI, docs, and translated manuals — GAL no longer supports Gemini as a
  runtime.
- Removed the unreachable per-host MCP config writer, and removed MCP
  configuration from the releaser role's documented source list — MCP setup
  is no longer part of the releaser contract.
- Pipeline receipts are handled more strictly: partial reads, stale
  receipts, and unconfirmed leases are now rejected instead of accepted, and
  receipts are scoped per plan instead of shared globally.

### Fixed

- Fixed several OpenCode integration and projection issues surfaced by
  `gal doctor`, including stale legacy paths and an incorrect remediation
  message for a missing Claude skill surface.
- Fixed the `boundary-check` gate incorrectly flagging workflow-state files
  that belong to the plan currently running through the pipeline.
- Fixed the pipeline handback classifier letting resolved handback blocks
  and a pending stop-at mask genuine `human-required` and `retry-ceiling`
  states.
- Fixed a workspace compile break introduced by a new `CommandKind` variant.
- Fixed dispatch and `gal doctor` tests that could read the real user home
  directory instead of an isolated test path.

## [0.1.3] - 2026-07-31

### Added

- `gal release-notes` drafts a CHANGELOG section from a commit range.
- `gal release` requires a curated CHANGELOG section for stable versions.

### Changed

- `/gal finalize` resolves a `.dev/state.md`-only merge conflict automatically.

## [0.1.2] - 2026-07-29

### Added

- `gal planning-check` gained an `approval-shape` row. A plan's `## Approval`
  section must now carry exactly four lines in the fixed order `Human approval`,
  `Architect review`, `Design review`, `Business review`, each written as
  `- <label>: [<token>]` with an optional ` — <reason>` tail, and each token
  drawn from that field's closed vocabulary. Failures name the offending field,
  the expected order, and the closed set, so the message is actionable without
  reading the checker source.
- `gal planning-check` gained a separate `arch-review-consistency` row. It is a
  biconditional: `Architect review: [clear]` holds if and only if a standalone
  `<!-- ARCH_REVIEW: CLEAR -->` marker exists in the plan. The pre-existing
  `arch-review-clear` row only inspects the marker, so it could not see the
  token and the marker drifting apart. This row catches that in both directions.

### Changed

- **Breaking for existing plans.** Both plan templates now ship the four-line
  Approval block. The retired `Additional domain review` line and the spaced
  `[not requested]` spelling no longer validate. Plans written against the old
  three-line shape fail `approval-shape` with a prescriptive migration message.
- `/plan-to-prompt` now gates on the literal line `- Human approval: [approved]`.
  The previous contract accepted `[clear]` and instructed the reader to treat
  equivalent explicit approvals as satisfied. Both readings are removed, so the
  gate no longer depends on model interpretation.
- `/deep-planning` review lanes now write deterministic Approval tokens. The
  architect lane maps approve and block to `[clear]` and `[blocked]`. The design
  and business lanes each name their own write-back targets and map approve,
  block, and untriggered to `[clear]`, `[blocked]`, and `[not-requested]`.
- The operator manual and its Japanese and Traditional Chinese translations now
  quote the exact approval literal and list all four fields with their closed
  vocabularies, replacing the old equals-sign prose.

### Fixed

- `gal finalize-check`'s `durable-layer-commit` row no longer scrapes a hash out
  of the plan's task lines. It is registered in `--hygiene-only` mode and
  verifies exactly the hash the orchestrator supplies through `--durable-commit`.
  An omitted flag reports `NotRun` so the receipt fails closed instead of passing
  vacuously on whatever hash happened to be nearby.
- `gal planning-check` no longer treats a by-design deleted English draft as a
  checker failure. Once `/plan-to-prompt` stamps a real 64-lowercase-hex
  `prompt-hash` together with `equivalence-verdict: EQUIVALENT`, the draft's role
  has ended and its absence is expected, so `localized-metadata` and
  `machine-anchor-parity` pass. Every other combination — pre-prompt
  placeholders, a malformed hash, a mixed hash and verdict pair — keeps a missing
  draft a hard failure. The terminal-state check runs before the self-referential
  `rendered-source-hash` comparison, which previously stranded such plans in a
  permanently failing state with no way to re-stamp them.

## [0.1.1] - 2026-07-22

### Fixed

- The naming gate's full-tree and staged scans are now git-proven and fail
  closed. A scan that cannot establish its file set from git reports failure
  instead of silently passing on an empty set.

## [0.1.0] - 2026-07-13

First public release. The `gal` binary ships the control-plane command surface,
the golem agent roster, the receipt-driven planning and pipeline gates, and
repo-local adapter generation for Claude Code, GitHub Copilot, Gemini, Codex,
and Antigravity CLI.

[0.1.3]: https://github.com/monkey1wizard/Golem-Agents-Legion/releases/tag/v0.1.3
[0.1.2]: https://github.com/monkey1wizard/Golem-Agents-Legion/releases/tag/v0.1.2
[0.1.1]: https://github.com/monkey1wizard/Golem-Agents-Legion/releases/tag/v0.1.1
[0.1.0]: https://github.com/monkey1wizard/Golem-Agents-Legion/releases/tag/v0.1.0
