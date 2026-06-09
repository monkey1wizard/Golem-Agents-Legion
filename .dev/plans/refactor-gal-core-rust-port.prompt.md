# Plan Prompt: GAL Core Script Layer → Rust Native (refactor-gal-core-rust-port)

<!--
Generated from docs/plans/refactor-gal-core-rust-port.md.
Output path: .dev/plans/refactor-gal-core-rust-port.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal

Reimplement **all non-xmachine** Bash+PowerShell functionality under `scripts/` as native Rust with behavioral parity, deleting each `.ps1`+`.sh` pair as its Rust replacement reaches parity. Final state: install / setup / MCP / adapter-regen / catalog / release / git-filter / translation all run through the `gal` binary; `scripts/` keeps **zero core-family ps1/sh, no exceptions**. The xmachine remote-execution subsystem is owned by the sister plan `refactor-gal-xmachine-rust-port` (which depends on `base`/`dispatch` extracted here).

## Requirements

- R-00 Architecture decomposition (JIT, not big-bang; protected, architect sign-off): extract `base` (config/mode/paths/install-state/provider-selection + `platform`/`render` modules + `HealthCheck` trait; sink render.rs cross-platform primitives), establish `providers`, de-prefix `gal-cli`/`gal-dispatch`. Split remaining domain crates just-in-time at each port phase, not all up front. Dependency law: `base` has no GAL deps; no cycles; no god-crate; doctor aggregates via `HealthCheck` trait.
- R-01 Shared core → `base`: port `common.{ps1,sh}` install/setup functions (install mode, provider selection, symlink, install state, plugin root) into `base`, reused by install/setup/mcp/adapters.
- R-02 `gal mcp`: port `update-mcp.{ps1,sh}` to `gal mcp` backed by `mcp.rs`, four-provider parity.
- R-03 adapter-regen + Sync-DevContext → `adapters` (protected): converge `update-skills/commands/personalization` + `Sync-DevContext.{ps1,sh}` into a single `adapters` backend (all generate adapter files); expose via `gal sync` (init-time) / `gal update` (incremental), sharing `base::render`.
- R-04 `gal setup` (protected): port `setup-machine.{ps1,sh}` + `setup-tools.{ps1,sh}` into a thin `setup` orchestrator (`gal setup`, `--tools`); register git filter `gal clean`/`gal smudge`.
- R-05 Install family parity + deletion: `install-gal-plugins`/`build-core-plugin`/`build-provider-plugins`/`provider-plugin` four-provider orchestration + render parity, then paired deletion.
- R-06 Misc Rust-ification: `gal clean`/`gal smudge` (`vcs`), `gal uninstall` parity, init-repo, catalog parsing, release packaging into `gal release`, translation freshness — each ported then paired-deleted.
- R-07 `gal` entry core subcommands: port core subcommands of `gal.{ps1,sh}` into `cli`; physical deletion of the entry file is deferred to the cross-plan end-gate.
- R-08 Oracle reparent precondition: tests that use a live script as oracle are reparented to fixtures/behavioral tests before deletion.
- R-09 Cross-platform correctness: Windows junction / Unix symlink / permissions aligned across three platforms; isolated-home verification.
- R-10 Real-machine acceptance (inherited from prior plan, hard gate): after clean `gal install`, mac-mini (normal packaged-source) + Windows (normal + dev) both doctor green, skill surface loads, canonical root complete.
- R-11 doctor covers new surfaces: each domain implements `HealthCheck`; `gal doctor` aggregates and extends to mcp/setup/sync/filter; fail-loud on gaps.
- R-12 Final state zero ps1/sh (hard, no exception): no core-family ps1/sh remain in `scripts/`.

## Approach

**Motivation (governs all scope/trade-offs):** (1) eliminate dual-implementation maintenance cost — every function currently exists twice (`.ps1`+`.sh`); single Rust = write once, cross-platform via `cfg!`; rule: anything dual-implemented goes to Rust, no "in scope?" debate. (2) single binary speeds AI invocation — compiled `gal` starts instantly vs spawning bash/pwsh + sourcing shared libs.

**Zero ps1/sh is hard, no exception:** bootstrap obtains the binary via pkg-manager/Releases/cargo; git filter becomes `gal clean`/`gal smudge`; shell completion is `gal completions` output (not source); remote execution body is the binary; toolchain install is `gal setup` calling the system pkg-manager. A `curl|sh` convenience installer (if provided) is a `gal release` artifact, not tracked source.

**Governing principles:** parity = behavioral equivalence AND live read-surface matches source (not "compiles"); deletion hard-gate = fixture-parity green (fixtures frozen in P0 from working scripts); monolithic scripts delete only when ALL live consumers reach parity; mixed-state invariant (no phase boundary leaves a consumer calling a deleted/half-ported surface; system fully functional at each phase end); oracle-parity tests reparented to `tests/fixtures/` snapshot or behavioral tests before deletion.

**Target Rust architecture (no-prefix workspace; foundation/capability/orchestrator/edge):** `gal-engine` (6577-line god-crate, "engine" appears 0× = junk name) is retired. No-prefix internal crates (rust-analyzer convention); foundation cannot be `core` (reserved) → `base`; names anchored to domain vocab (`provider` 54× → `providers`); binary stays `gal` (`cli` package + `[[bin]] name="gal"`).

| Layer | crate | Responsibility | Depends |
| --- | --- | --- | --- |
| foundation | `base` | config/mode/paths/install-state/provider-selection; `platform` module (symlink/junction/perms/atomic-swap); `render` module (template primitives); `HealthCheck` trait | none (no GAL crate) |
| capability | `providers` | Provider trait + claude/copilot/codex/agy projection | base |
| capability | `install` | canonical_root render + bin exposure + orphan cleanup + uninstall ledger | base, providers |
| capability | `mcp` | MCP config | base |
| capability | `adapters` | adapter/skill/command/personalization regen (Sync-DevContext + Update-*), shares `base::render` | base |
| capability | `release` | release packaging | base |
| capability | `vcs` | git filter (clean/smudge) + commit-msg | base |
| orchestrator | `setup` | machine setup: orchestrate-only (install+mcp+adapters + git-filter registration), no domain logic | base, install, mcp, adapters, vcs |
| edge | `cli` | thin entry, noun-grouped subcommands, `[[bin]] name="gal"`; aggregates each domain's `HealthCheck` into `gal doctor` | base + each domain |

Dependency law: `base` depends on no GAL crate; domains depend only on `base` (+ install on providers); no cycles; no god-crate. Architecture decisions folded in 2nd/3rd architect review: doctor dependency inversion (HealthCheck trait in base; cli aggregates trait objects), render primitive shared (install & adapters both via `base::render`), platform self-contained (`base::platform`), setup must stay thin, crate-vs-module discipline (crate only with isolation justification; `vcs`/`release` default to modules pending architect).

**Phases (each: parity → reparent → delete):** P0 freeze fixtures + clean reinstall + real-machine baseline; P0.5 architecture decomposition (JIT: base+providers+de-prefix, verify 228 tests green); P1 shared core → base; P2 `gal mcp`; P3 regen+sync (protected); P4 `gal setup` (protected); P5 install family deletion; P6 misc; P7 entry core subcommands; P8 real-machine acceptance + cleanup verify.

**Already done (inherited from feat-gal-rust-native-install, committed):** install/render/doctor/mode/claude-skill projection/bin exposure/orphan cleanup/cross-platform/pkg-manager convergence/oracle reparent — commits a192df7..bcc755a (228 tests green). **Not yet done (this plan takes over):** real-machine TP-02/TP-03 never run; local `gal install` torn down (`~/.gal/plugins/` empty, skill surface dangling, `gal doctor` exit 1) — needs reinstall + reverify.

## Files to Create or Modify

- [CREATE] (protected, architecture) `crates/base/` (foundation + platform/render modules + HealthCheck trait), `crates/providers/`.
- [CREATE/MOVE] (protected, architecture) `gal-engine` **retired**, JIT-split into `crates/install/`, `crates/mcp/`, `crates/adapters/`, `crates/release/`, `crates/vcs/`; render primitives sunk into `base::render`.
- [CREATE] `crates/setup/` (orchestrator, thin).
- [RENAME] `gal-cli`→`crates/cli` (`[[bin]] name="gal"`), `gal-dispatch`→`crates/dispatch`.
- [MODIFY] `crates/cli/`: noun-grouped subcommand wiring + doctor `HealthCheck` aggregation.
- [MODIFY] (protected, contract surface) `plugins/gal-core/commands/gal/SKILL.template.md`, `gal-init/SKILL.template.md`: references point to the Rust binary.
- [MODIFY] `.gitattributes` + `gal setup` writes `git config filter.gal-config.clean = gal clean`.
- [MODIFY] `tests/fixtures/`, `crates/*/tests/*`: oracle reparent.
- [DELETE on parity] install family, MCP, regen+sync (incl. `Sync-DevContext.*`), setup (`setup-machine.*`/`setup-tools.*`/`uninstall-machine.*`/`init-repo.*`/`Resolve-GalCatalog.ps1`), misc (`gal-clean.sh`/`gal-smudge.sh`/`Package-ReleaseArtifacts.*`/`Test-TranslationFreshness.*`/`Test-ResolveGalCatalog.ps1`/`tests/Test-InstallModeAuthority.ps1`/`test-install-acceptance.sh`).
- [DELETE on parity, cross-plan end-gate] `common/Common.{ps1,sh}`, `gal.{ps1,sh}` — shared with xmachine; both plans must be done before whole-file deletion.

## Test Cases

See `## Test Plan` (TP-01..TP-25) for the full matrix aligned to T-NNN. Hard gates: TP-02 (mac-mini normal), TP-03 (Windows dual-mode), TP-23 (scripts/ core-family empty + no live-script spawn).

## Success Criteria

- `scripts/` core family has zero ps1/sh — hard, no exception.
- `gal mcp`/`gal setup`/`gal sync`/`gal clean`/`gal smudge` exist and reach parity with old scripts (vs frozen fixtures).
- Four-provider install, adapter generation, MCP config, machine setup all run through Rust; live read-surface matches source.
- git filter becomes `gal clean`/`gal smudge`; personalizable-file smudge/clean behavior unchanged.
- mac-mini (normal) + Windows (normal+dev) real-machine `gal install` doctor green, skill surface loads, canonical root complete.
- `cargo test` green and test code no longer spawns any core live `scripts/*.{ps1,sh}`.
- Each domain implements `HealthCheck`; `gal doctor` aggregates; fail-loud on gaps.

## Risks

- Large scope + protected paths (high): gal-engine core, Setup-Machine/Sync-DevContext (protected, reverses KEEP). Mitigation: R-00/R-03/R-04/R-05 via architect; CODER≠REVIEWER; record KEEP reversal in Key Decisions.
- Cross-plan shared files (high): `common.*`/`gal.{ps1,sh}` shared with xmachine. Mitigation: only move out core functions/subcommands; physical deletion at cross-plan end-gate.
- Real-machine acceptance slipping again (medium): prior plan marked done without real-machine verification. Mitigation: P0 reinstall+reverify; P8 three-platform hard gate.
- Bash/PS behavioral divergence (medium): dual impls may already disagree. Mitigation: P0 diff both outputs, pick + record the baseline.
- Adjacent active plans (medium): `refactor-golem-auditor`/`feat-small-context`/`fix-install-followups`. Mitigation: sequencing coordination.

## Open Questions

None open — all planning-stage OQs resolved and internalized as Decisions in the source plan (zero-ps1/sh no exception; Sync-DevContext + update-* unified into `adapters`; setup-tools folded into `gal setup`; dispatch owned by sister plan; no external direct callers; no bootstrap script).

## Approval

- Human approval: [approved at 2026-06-09]
- Architect review: APPROVE (direction; REVISE folded across 2nd review; task-granularity re-sliced in 3rd review). Implementation-time R-00/R-03/R-04/R-05 are protected-core changes requiring architect sign-off.
- Additional domain review: not triggered (no customer-facing / business-rule content).

---

## Status

Workflow: DRAFT
Step: 0 of 33
Last activity: 2026-06-09 — prompt generated from source plan
Next step: T-001 — freeze parity fixtures (then T-002 reinstall + real-machine baseline)
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0

### Deviations

| Date | Task | Planned | Actual | Reason |
| --- | --- | --- | --- | --- |

### Handoff Notes

Fresh prompt. Cross-plan: this plan's R-00 (T-003..T-009: extract `base`, de-prefix `dispatch`) must land before the sister xmachine plan's T-002 can start. Implementation-time architect sign-off required for protected-core tasks (T-003..T-009, T-013..T-017 adapters, T-018..T-021 setup). Local `gal install` is currently torn down — T-002 must reinstall before any parity work.

## Tasks

P0 — baseline
- [ ] T-001 (P0) — Freeze parity fixtures: capture each core script's current observable output/side-effects into `tests/fixtures/` (deletion oracle baseline).
- [ ] T-002 (P0, hard gate) — Clean reinstall `gal install` to rebuild the local green baseline; run TP-02 (mac-mini normal) + TP-03 (Windows dual-mode) to establish the real baseline for already-done engine surfaces.

R-00 architecture decomposition (protected, architect; step-wise green, JIT)
- [ ] T-003 — Scaffold workspace + extract `base` crate: move config/mode/paths/install-state/provider-selection; update root `Cargo.toml` members; repoint importers; `cargo test` green.
- [ ] T-004 — Sink `base::platform` (symlink/junction/perms/atomic-swap); render/install use it; Windows/Unix behavior unchanged; green.
- [ ] T-005 — Sink `base::render` (template primitives); existing render uses it; output byte-identical; green.
- [ ] T-006 — Define `HealthCheck` trait in `base`; migrate existing doctor checks to trait impls; exit grading unchanged.
- [ ] T-007 — Extract `providers` crate (Provider trait + claude/copilot/codex/agy projection, moved from gal-engine); install depends on providers; green, no cycles.
- [ ] T-008 — Rename `gal-cli`→`cli` (`[[bin]] name="gal"`), `gal-dispatch`→`dispatch`; update all imports + Cargo; `gal --version` works; 228 tests green.
- [ ] T-009 (R-08, before any deletion) — Reparent oracle tests (`Test-ResolveGalCatalog`/`tests/Test-InstallModeAuthority`/`test-install-acceptance.sh`) to fixture/behavioral tests.

R-01 shared core
- [ ] T-010 (R-01) — Port `common.{ps1,sh}` install/setup functions into `base` (install mode/provider selection/symlink/install state/plugin root); install/setup/mcp/adapters use the shared core.

R-02 MCP
- [ ] T-011 (R-02) — Split `mcp` crate; port `update-mcp` → `gal mcp` backend + `HealthCheck`; four-provider parity vs fixture.
- [ ] T-012 — After parity green, delete `update-mcp.{ps1,sh}`; verify no consumer breaks.

R-03 adapters (protected, architect)
- [ ] T-013 (R-03) — Split `adapters` crate + port `update-skills` → backend (shares `base::render`) + `HealthCheck`; parity.
- [ ] T-014 — Port `update-commands` → `adapters`; parity.
- [ ] T-015 — Port `update-personalization` → `adapters`; parity.
- [ ] T-016 — Port `Sync-DevContext` (init-time) → `adapters`; wire `gal sync`/`gal update` CLI; parity.
- [ ] T-017 — After all four pairs reach parity green, delete them (update-skills/commands/personalization + Sync-DevContext).

R-04 setup (protected, architect)
- [ ] T-018 (R-04) — Split `setup` crate (orchestrate-only, no domain logic); port `setup-machine` → `gal setup`; parity.
- [ ] T-019 — Port `setup-tools` → `gal setup --tools`; parity.
- [ ] T-020 — Register git filter: `.gitattributes` + `git config filter.gal-config.* = gal clean/smudge`; Windows/Unix smudge/clean behavior unchanged.
- [ ] T-021 — After parity green, delete `setup-machine.{ps1,sh}` + `setup-tools.{ps1,sh}`.

R-05 install family
- [ ] T-022 (R-05) — Confirm `install-gal-plugins` four-provider orchestration parity (per-provider vs fixture: Claude/Copilot/Codex/AGY).
- [ ] T-023 — Confirm `build-core-plugin`/`build-provider-plugins`/`provider-plugin` render parity.
- [ ] T-024 — After all parity green, delete install family (incl. internal-only scripts); verify live read-surface still aligned.

R-06 misc (per item: port → parity → delete)
- [ ] T-025 — `vcs`: `gal clean`/`gal smudge` + commit-msg; parity; delete `gal-clean.sh`/`gal-smudge.sh`.
- [ ] T-026 — `gal uninstall` parity (ledger precision); delete `uninstall-machine.{ps1,sh}`.
- [ ] T-027 — Port `init-repo` → Rust; parity; delete pair.
- [ ] T-028 — Port catalog parsing (`Resolve-GalCatalog`) → Rust; parity; delete.
- [ ] T-029 — Fold release packaging into `gal release`; parity; delete `Package-ReleaseArtifacts.{ps1,sh}`.
- [ ] T-030 — Port translation freshness → Rust; parity; delete pair.

R-11/R-07/R-10 closeout
- [ ] T-031 (R-11) — `cli` aggregates each domain's `HealthCheck` into `gal doctor`, extending to mcp/setup/sync/filter; fail-loud on gaps.
- [ ] T-032 (R-07) — Port core subcommands of `gal.{ps1,sh}` into `cli` (noun-grouped); update `gal`/`gal-init` SKILL.template references to the binary. Entry-file physical deletion deferred to cross-plan end-gate.
- [ ] T-033 (R-09/R-10/R-12, hard gate) — Three-platform real-machine acceptance; `scripts/` core family emptied (shared `common.*`/`gal.{ps1,sh}` deleted at cross-plan end-gate with sister plan); `cargo test` green and test code no longer spawns core live scripts (grep-verified).

## Deferred Follow-up

- Linux normal-mode real-machine verification when a host is available (via SSH after reparent).
- `curl|sh` convenience installer as a `gal release` artifact (optional, not tracked source).

## Analyze

(empty — populated by reviewer/debugger during execution)

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | integration | fixture freeze: each core script's `tests/fixtures/` reproduces current observable output/side-effects (deletion oracle) | T-001 |
| TP-02 | integration (hard gate) | mac-mini (Unix) pure normal-mode clean install: packaged-source self-resolves, `~/.claude/skills/gal` symlink + doc-sync loads, canonical root correct, no orphans, doctor green | T-002, T-033 |
| TP-03 | integration (hard gate) | Windows dual-mode: normal (packaged-source+junction) → dev (galRoot repo-root), both modes canonical root + skill surface + doctor green | T-002, T-033 |
| TP-04 | unit | after extracting `base`, 228 tests green; `base` depends on no GAL crate; workspace `Cargo.toml` members correct | T-003 |
| TP-05 | unit | after `base::platform` sink, render/install use it, 228 tests green; Windows junction / Unix symlink / perms unchanged | T-004 |
| TP-06 | unit | after `base::render` sink, existing render uses it, 228 tests green, output byte-identical | T-005 |
| TP-07 | unit | `HealthCheck` trait in `base`; existing doctor checks migrated to trait impls; doctor exit grading unchanged | T-006 |
| TP-08 | unit | after extracting `providers`, 228 tests green; install depends on providers; no cycles | T-007 |
| TP-09 | unit | after renaming `cli`/`dispatch`, 228 tests green; `gal --version` works; no external dep name clash | T-008 |
| TP-10 | unit | after T-009 reparent, `cargo test` no longer spawns `Test-ResolveGalCatalog`/`Test-InstallModeAuthority`/`test-install-acceptance`; reads fixtures | T-009 |
| TP-11 | unit | `common.*` → `base` function groups == fixture (install mode, provider selection, symlink, install state, plugin root) | T-010 |
| TP-12 | parity | `gal mcp` four-provider MCP config output == fixture (per-provider assertions) | T-011 |
| TP-13 | unit | after deleting `update-mcp.{ps1,sh}` no consumer breaks; mixed-state invariant (system functional before/after deletion) | T-012 |
| TP-14 | parity | `adapters`: update-skills/commands/personalization/Sync-DevContext each generate == fixture; shares `base::render` with install, no duplicate impl | T-013..T-016 |
| TP-15 | integration | `gal sync` (init-time)/`gal update` (incremental) run through `adapters`, generated adapter files == fixture | T-016 |
| TP-16 | parity | `gal setup` machine-setup orchestration == fixture; `gal setup --tools` toolchain step == fixture | T-018, T-019 |
| TP-17 | integration (cross-platform) | git filter registration: `.gitattributes` + `git config filter.gal-config.* = gal clean/smudge`; Windows (no bash) and Unix both smudge/clean unchanged | T-020 |
| TP-18 | parity | install family per-provider parity: Claude/Copilot/Codex/AGY orchestration + render == fixture | T-022, T-023 |
| TP-19 | unit | after install-family deletion, four-provider live read-surface still aligned (doctor green) | T-024 |
| TP-20 | parity | `vcs` (clean/smudge/commit-msg), `gal uninstall` (ledger precision), init-repo, catalog, release-packaging, translation each == fixture | T-025..T-030 |
| TP-21 | unit | each domain implements `HealthCheck`; `gal doctor` aggregates, returns non-zero naming fix surface for missing mcp/setup/sync/filter, exit 0 on complete install | T-031 |
| TP-22 | integration | `gal` entry core subcommands all run through Rust; `gal`/`gal-init` SKILL.template references point to binary; noun-grouped subcommands callable | T-032 |
| TP-23 | manual | `scripts/` core family zero ps1/sh (grep); `cargo test` green and test code no longer spawns any core live `scripts/*.{ps1,sh}` | T-033 |
| TP-24 | perf | binary cold-start vs script path measurement (validates Motivation #2; not a gate) | T-032 |
| TP-25 | integration | mixed-state invariant spot-check: at mid-migration states (P2/P4) the system is fully functional, no consumer calls a deleted/half-ported surface | T-011..T-030 |

Real-machine order: (1) mac-mini (SSH, pure normal, tests the never-verified packaged-source path) → (2) copy `~/.gal/config/` carrying `*.local.*` → (3) Windows (normal + dev). Linux normal added via SSH when a host is available.

## Test Results

(empty — populated by tester during execution)

## Review Results

### Architecture Review

APPROVE (3rd review, 2026-06-09). Motivation (full Rust + single binary) sound; layered architecture (base/providers/capability/orchestrator/edge), dependency law, engine retirement, fixture-gated deletion all healthy. Folded across reviews: JIT decomposition (not big-bang), crate-vs-module discipline, mixed-state invariant, fixture-parity deletion gate, doctor dependency inversion (HealthCheck trait in base), render primitive shared (`base::render`), `base::platform` self-contained, setup must stay thin. 3rd review re-sliced tasks to commit-grain (GRAN-01) and patched completeness (GRAN-02: workspace Cargo.toml members, per-split green checks, HealthCheck implemented per domain, mixed-state invariant TP, git-filter cross-platform TP). No blocking issues.

### Business Review

Not triggered (no business-rule content).

### Design Review

Not triggered (no customer-facing UI).

### Engineering Review

CLEAR. 33 T-NNN map to R-00..R-12 / P0..P8, each an independently verifiable commit-size unit (R-00 decomposition split into 7 step-wise green steps T-003..T-009; adapters per-script T-013..T-017; install family per-provider parity; R-06 misc per item). 25 TP cover (incl. per-crate-split "still green", per-provider parity, git-filter cross-platform, mixed-state invariant spot-check, inherited real-machine hard gates TP-02/03). Implementation constraints: (1) protected-core architect sign-off for T-003..T-009/T-013..T-017/T-018..T-021; (2) deletion hard-gate = fixture-parity green, T-009 reparent before its covered deletions; (3) mixed-state invariant at every phase boundary, `common.*`/`gal.{ps1,sh}` physical deletion (T-033 tail) at cross-plan joint gate; (4) JIT decomposition — domain crates split at each port task start. CODER≠REVIEWER; reviewer tier ≥ implementer.

## Debug Log

(empty)
