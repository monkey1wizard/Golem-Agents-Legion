# Plan Prompt: GAL Core Script Layer → Rust Native (refactor-gal-core-rust-port)

<!--
Generated from docs/plans/refactor-gal-core-rust-port.md (refreshed 2026-06-09 after deep-planning CORR-01).
Output path: .dev/plans/refactor-gal-core-rust-port.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal

Reimplement **all non-xmachine** Bash+PowerShell functionality under `scripts/` as native Rust with behavioral parity, deleting each `.ps1`+`.sh` pair as its Rust replacement reaches parity. Final state: install / setup / MCP / adapter-regen / catalog / release / git-filter / translation all run through the `gal` binary; `scripts/` keeps **zero core-family ps1/sh, no exceptions**. The xmachine remote-execution subsystem is owned by the sister plan `refactor-gal-xmachine-rust-port` (which depends on `base`/`dispatch` extracted here).

## Requirements

- R-00 Architecture decomposition (JIT, not big-bang; protected, architect sign-off): extract `base` (config/mode/paths/install-state/provider-selection + `platform`/`render` modules + `HealthCheck` trait; sink render.rs cross-platform primitives), establish `providers`, de-prefix `gal-cli`/`gal-dispatch`. Split remaining domain crates just-in-time. Dependency law: `base` has no GAL deps; no cycles; no god-crate; doctor aggregates via `HealthCheck` trait.
- R-01 Shared core → `base`: port `common.{ps1,sh}` install/setup functions into `base`, reused by install/setup/mcp/adapters.
- R-02 `gal mcp`: port `update-mcp.{ps1,sh}` to `gal mcp` backed by `mcp.rs`, four-provider parity.
- R-03 adapter-regen + Sync-DevContext → `adapters` (protected): converge `update-skills/commands/personalization` + `Sync-DevContext.{ps1,sh}` into a single `adapters` backend; expose via `gal sync`/`gal update`, sharing `base::render`.
- R-04 `gal setup` (protected): port `setup-machine.{ps1,sh}` + `setup-tools.{ps1,sh}` into a thin `setup` orchestrator; register git filter `gal clean`/`gal smudge`.
- R-05 Install family parity + deletion: four-provider orchestration + render parity, then paired deletion.
- R-06 Misc Rust-ification: `gal clean`/`gal smudge` (`vcs`), `gal uninstall` parity, init-repo, catalog parsing, release packaging into `gal release`, translation freshness — each ported then paired-deleted.
- R-07 `gal` entry core subcommands: port into `cli`; physical deletion of the entry file deferred to the cross-plan end-gate.
- R-08 Oracle reparent precondition: tests using a live script as oracle are reparented to fixtures/behavioral tests before deletion.
- R-09 Cross-platform correctness: Windows junction / Unix symlink / permissions aligned; isolated-home verification.
- R-10 Real-machine acceptance = pure end-user install of a release artifact (hard gate; CORR-01): real machines (mac-mini, Windows normal) install the **prebuilt artifact produced by the R-13 release pipeline** as a **pure end-user** (brew / GitHub Releases / scp the release file). **Never install rust, build, or ad-hoc cross-compile on a test machine.** Verify doctor green, skill surface loads, canonical root complete, packaged-source self-resolves. Prerequisite = R-13 artifact exists.
- R-11 doctor covers new surfaces: each domain implements `HealthCheck`; `gal doctor` aggregates + extends to mcp/setup/sync/filter; fail-loud.
- R-12 Final state zero ps1/sh (hard, no exception): no core-family ps1/sh remain in `scripts/`.
- R-13 Cross-platform release-artifact pipeline (new; prerequisite for R-10): a **CI matrix (macOS/Linux/Windows runners, e.g. GitHub Actions)** produces prebuilt `gal` artifacts per target (macOS-arm64 at minimum) + packaged source (FHS/flat), published to GitHub Releases / brew tap. End-user and real-machine tests install **only these artifacts**, never build on the target. `gal release` must not produce only the current platform. **Governance: never install a toolchain or build on an end-user / test machine.**

## Approach

**Motivation:** (1) eliminate dual-implementation maintenance cost (every function exists twice as `.ps1`+`.sh`); single Rust = write once, cross-platform via `cfg!`. (2) single binary speeds AI invocation. Rule: anything dual-implemented goes to Rust, no exceptions; zero ps1/sh is hard (bootstrap via pkg-manager/Releases/cargo; git filter → `gal clean`/`gal smudge`; completion via `gal completions`; remote body = binary; toolchain install = `gal setup`).

**Governing principles:** parity = behavioral equivalence AND live read-surface matches source (not "compiles"); deletion hard-gate = fixture-parity green (fixtures frozen in P0); monolithic scripts delete only when ALL live consumers reach parity; mixed-state invariant (no phase boundary leaves a consumer calling a deleted/half-ported surface); oracle-parity tests reparented before deletion. **Real-machine validation = pure end-user install of an R-13 prebuilt artifact; build host (dev machine / CI) and end-user machine are strictly separate roles (CORR-01).**

**Target architecture (no-prefix workspace; `gal-engine` retired — "engine" 0 concept uses):**

| Layer | crate | Responsibility | Depends |
| --- | --- | --- | --- |
| foundation | `base` | config/mode/paths/install-state/provider-selection; `platform` (symlink/junction/perms/atomic-swap); `render` (template primitives); `HealthCheck` trait | none |
| capability | `providers` | Provider trait + claude/copilot/codex/agy projection | base |
| capability | `install` | canonical_root render + bin exposure + orphan cleanup + uninstall ledger | base, providers |
| capability | `mcp` | MCP config | base |
| capability | `adapters` | adapter/skill/command/personalization regen (Sync-DevContext + Update-*), shares `base::render` | base |
| capability | `release` | release packaging | base |
| capability | `vcs` | git filter (clean/smudge) + commit-msg | base |
| orchestrator | `setup` | machine setup: orchestrate-only + git-filter registration, no domain logic | base, install, mcp, adapters, vcs |
| edge | `cli` | thin entry, noun-grouped subcommands, `[[bin]] name="gal"`; aggregates each domain's `HealthCheck` into `gal doctor` | base + each domain |

Dependency law: `base` depends on no GAL crate; domains depend only on `base` (+ install on providers); no cycles; no god-crate. Architecture decisions (folded across reviews): doctor dependency inversion (HealthCheck trait in base), render primitive shared (`base::render`), `base::platform` self-contained, setup thin, crate-vs-module discipline.

**Phases:** P0 freeze fixtures + dev-machine baseline (Windows, has rust — dev baseline only; real-machine end-user acceptance NOT here); P0.5 architecture decomposition (JIT: base+providers+de-prefix, verify 228 green); P1 shared core; P2 mcp; P3 regen+sync (protected); P4 setup (protected); P5 install family deletion; P6 misc; P7 entry core subcommands; P7.5 release-artifact pipeline (R-13, prereq for R-10); P8 pure end-user real-machine acceptance (install R-13 artifact, not build) + cleanup verify.

**Already done (inherited from feat-gal-rust-native-install, committed):** install/render/doctor/mode/claude-skill projection/bin/orphan cleanup/cross-platform/pkg-manager convergence/oracle reparent — a192df7..bcc755a (228 tests green).

## Files to Create or Modify

- [CREATE] (protected) `crates/base/` (foundation + platform/render modules + HealthCheck trait), `crates/providers/`.
- [CREATE/MOVE] (protected) `gal-engine` **retired**, JIT-split into `crates/install/`, `crates/mcp/`, `crates/adapters/`, `crates/release/`, `crates/vcs/`; render primitives → `base::render`.
- [CREATE] `crates/setup/` (orchestrator, thin).
- [RENAME] `gal-cli`→`crates/cli` (`[[bin]] name="gal"`), `gal-dispatch`→`crates/dispatch`.
- [MODIFY] `crates/cli/`: noun-grouped subcommand wiring + doctor `HealthCheck` aggregation.
- [MODIFY] (protected, contract surface) `plugins/gal-core/commands/gal/SKILL.template.md`, `gal-init/SKILL.template.md`: references → Rust binary.
- [CREATE] CI release pipeline (R-13): macOS/Linux/Windows matrix producing prebuilt `gal` + packaged source → Releases/brew. Connects to GitHub-public + Actions (git-publish-strategy private note).
- [MODIFY] `.gitattributes` + `gal setup` writes `git config filter.gal-config.clean = gal clean`.
- [MODIFY] `tests/fixtures/`, `crates/*/tests/*`: oracle reparent.
- [DELETE on parity] install family, MCP, regen+sync (incl. `Sync-DevContext.*`), setup (`setup-machine.*`/`setup-tools.*`/`uninstall-machine.*`/`init-repo.*`/`Resolve-GalCatalog.ps1`), misc (`gal-clean.sh`/`gal-smudge.sh`/`Package-ReleaseArtifacts.*`/`Test-TranslationFreshness.*`/`Test-ResolveGalCatalog.ps1`/`tests/Test-InstallModeAuthority.ps1`/`test-install-acceptance.sh`).
- [DELETE on parity, cross-plan end-gate] `common/Common.{ps1,sh}`, `gal.{ps1,sh}` — shared with xmachine.

## Test Cases

See `## Test Plan` (TP-01..TP-26b). Hard gates: TP-02 (mac-mini end-user artifact install), TP-03 (Windows end-user artifact install), TP-23 (scripts/ core-family empty + no live-script spawn). Real-machine gates require R-13 artifact (TP-26) first.

## Success Criteria

- `scripts/` core family has zero ps1/sh — hard, no exception.
- `gal mcp`/`gal setup`/`gal sync`/`gal clean`/`gal smudge` exist and reach parity (vs frozen fixtures).
- Four-provider install, adapter generation, MCP config, machine setup all run through Rust; live read-surface matches source.
- git filter → `gal clean`/`gal smudge`; smudge/clean behavior unchanged.
- R-13 pipeline produces macOS-arm64 (etc.) prebuilt `gal`; mac-mini + Windows install the artifact as **pure end-user** (no rust/build) → doctor green, skill surface loads, canonical root complete.
- `cargo test` green and test code no longer spawns any core live `scripts/*.{ps1,sh}`.
- Each domain implements `HealthCheck`; `gal doctor` aggregates; fail-loud.

## Risks

- Large scope + protected paths (high): gal-engine core, Setup-Machine/Sync-DevContext (reverses KEEP). Mitigation: R-00/R-03/R-04/R-05 via architect; CODER≠REVIEWER.
- Cross-plan shared files (high): `common.*`/`gal.{ps1,sh}` shared with xmachine. Mitigation: cross-plan end-gate.
- Real-machine methodology (high, CORR-01): test machine must never be a build host. Mitigation: R-13 CI artifact first; T-035 installs prebuilt only; governance principle.
- No macOS artifact / CI not built (medium): real-machine end-user acceptance blocked until R-13 (macOS CI runner); ties to GitHub-public + Actions. Mitigation: T-034 is hard prereq of T-035.
- Bash/PS behavioral divergence (medium): Mitigation: P0 diff both outputs, record baseline.

## Open Questions

None open — all planning-stage OQs resolved and internalized as Decisions in the source plan.

## Approval

- Human approval: [approved at 2026-06-09]
- Architect review: APPROVE (direction; 2nd review = full architecture decomposition; 3rd = task-granularity re-slice; 4th = CORR-01 real-machine methodology). Implementation-time R-00/R-03/R-04/R-05 are protected-core changes requiring architect sign-off.
- Additional domain review: not triggered (no customer-facing / business-rule content).

---

## Status

Workflow: IMPLEMENT
Step: 4 of 35
Last activity: 2026-06-09 — T-003 complete (commit: 1ffd3de) — `base` crate extracted, BUG-01 MCP types relocated, 228 green. Run bounds: FROM=T-003, STOP_AT=T-008. Verification Independence: DEGRADED_SAME_RUNTIME.
Next step: implement T-004
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0

### Deviations

| Date | Task | Planned | Actual | Reason |
| --- | --- | --- | --- | --- |
| 2026-06-09 | T-001 | Capture fixtures for every core script upfront | Established `tests/fixtures/README.md` capture convention; fixtures captured JIT per domain at each port task (D-001) | Already-Rust surfaces use the existing Rust behavior-contract parity tests as oracle; not-yet-ported scripts captured JIT aligns with JIT decomposition + avoids running side-effecting scripts before a deterministic context exists (approach A, user decision) |
| 2026-06-09 | T-002 | (orig) reinstall + mac-mini/Windows real-machine gates in one task | Re-scoped: T-002 = **dev-machine dev-mode baseline only** (Windows, has rust); done, `gal doctor` exit 0. Real-machine end-user acceptance moved to **T-035** (install R-13 prebuilt artifact). | CORR-01 (user): test machines are pure end-users — never build/install toolchain on them. The earlier "install rust on mac-mini / cross-compile" path was wrong. Real-machine acceptance requires the R-13 release-artifact pipeline first (T-034). |

### Handoff Notes

**CORR-01 (2026-06-09, deep-planning 4th pass):** real-machine validation = pure end-user install of an R-13 prebuilt artifact; NEVER install rust / build / cross-compile on a test machine (mac-mini, Win11). The plan was missing the release-artifact pipeline (R-13) prerequisite. mac-mini has rustup proxies but no toolchain + brew without rust — irrelevant; treat it as an end user.

Cross-plan: this plan's R-00 (T-003..T-009: extract `base`, de-prefix `dispatch`) must land before the sister xmachine plan's T-002. Implementation-time architect sign-off required for protected-core tasks (T-003..T-009, T-013..T-017 adapters, T-018..T-021 setup).

**P0 complete (2026-06-09):** T-001 done (fixtures convention `tests/fixtures/README.md`, JIT capture — D-001). T-002 done (dev-machine dev-mode baseline, `gal doctor` exit 0). Next: T-003 (extract `base`). Real-machine end-user acceptance (T-035) is gated on the R-13 release pipeline (T-034) — ties to GitHub-public + Actions macOS runner.

## Tasks

P0 — baseline
- [x] T-001 (P0) — Parity-fixture convention established at `tests/fixtures/README.md`; per-domain JIT capture (approach A, D-001). Already-Rust surfaces use existing Rust behavior-contract parity tests as oracle.
- [x] T-002 (P0) — Dev-machine (Windows, has rust) dev-mode `gal install` baseline, `gal doctor` exit 0. (Real-machine end-user acceptance is T-035, not here.)

R-00 architecture decomposition (protected, architect; step-wise green, JIT)
- [x] T-003 — Scaffold workspace + extract `base` crate (config/mode/paths/install-state/provider-selection); update root `Cargo.toml` members; repoint importers; `cargo test` green. *(1ffd3de)*
- [ ] T-004 — Sink `base::platform` (symlink/junction/perms/atomic-swap); render/install use it; Windows/Unix behavior unchanged; green.
- [ ] T-005 — Sink `base::render` (template primitives); existing render uses it; output byte-identical; green.
- [ ] T-006 — Define `HealthCheck` trait in `base`; migrate existing doctor checks to trait impls; exit grading unchanged.
- [ ] T-007 — Extract `providers` crate (moved from gal-engine); install depends on providers; green, no cycles.
- [ ] T-008 — Rename `gal-cli`→`cli` (`[[bin]] name="gal"`), `gal-dispatch`→`dispatch`; update imports + Cargo; `gal --version` works; 228 tests green.
- [ ] T-009 (R-08, before any deletion) — Reparent oracle tests (`Test-ResolveGalCatalog`/`tests/Test-InstallModeAuthority`/`test-install-acceptance.sh`) to fixture/behavioral tests.

R-01 shared core
- [ ] T-010 (R-01) — Port `common.{ps1,sh}` install/setup functions into `base`; install/setup/mcp/adapters use the shared core.

R-02 MCP
- [ ] T-011 (R-02) — Split `mcp` crate; port `update-mcp` → `gal mcp` backend + `HealthCheck`; four-provider parity vs fixture.
- [ ] T-012 — After parity green, delete `update-mcp.{ps1,sh}`; verify no consumer breaks.

R-03 adapters (protected, architect)
- [ ] T-013 (R-03) — Split `adapters` crate + port `update-skills` → backend (shares `base::render`) + `HealthCheck`; parity.
- [ ] T-014 — Port `update-commands` → `adapters`; parity.
- [ ] T-015 — Port `update-personalization` → `adapters`; parity.
- [ ] T-016 — Port `Sync-DevContext` (init-time) → `adapters`; wire `gal sync`/`gal update`; parity.
- [ ] T-017 — After all four pairs reach parity green, delete them.

R-04 setup (protected, architect)
- [ ] T-018 (R-04) — Split `setup` crate (orchestrate-only); port `setup-machine` → `gal setup`; parity.
- [ ] T-019 — Port `setup-tools` → `gal setup --tools`; parity.
- [ ] T-020 — Register git filter: `.gitattributes` + `git config filter.gal-config.* = gal clean/smudge`; Windows/Unix smudge/clean unchanged.
- [ ] T-021 — After parity green, delete `setup-machine.{ps1,sh}` + `setup-tools.{ps1,sh}`.

R-05 install family
- [ ] T-022 (R-05) — Confirm `install-gal-plugins` four-provider orchestration parity (per-provider vs fixture: Claude/Copilot/Codex/AGY).
- [ ] T-023 — Confirm `build-core-plugin`/`build-provider-plugins`/`provider-plugin` render parity.
- [ ] T-024 — After all parity green, delete install family; verify live read-surface aligned.

R-06 misc (per item: port → parity → delete)
- [ ] T-025 — `vcs`: `gal clean`/`gal smudge` + commit-msg; parity; delete `gal-clean.sh`/`gal-smudge.sh`.
- [ ] T-026 — `gal uninstall` parity (ledger precision); delete `uninstall-machine.{ps1,sh}`.
- [ ] T-027 — Port `init-repo` → Rust; parity; delete pair.
- [ ] T-028 — Port catalog parsing (`Resolve-GalCatalog`) → Rust; parity; delete.
- [ ] T-029 — Fold release packaging into `gal release`; parity; delete `Package-ReleaseArtifacts.{ps1,sh}`.
- [ ] T-030 — Port translation freshness → Rust; parity; delete pair.

R-11/R-07 closeout
- [ ] T-031 (R-11) — `cli` aggregates each domain's `HealthCheck` into `gal doctor`, extending to mcp/setup/sync/filter; fail-loud.
- [ ] T-032 (R-07) — Port core subcommands of `gal.{ps1,sh}` into `cli`; update `gal`/`gal-init` SKILL.template references. Entry-file physical deletion deferred to cross-plan end-gate.
- [ ] T-033 (R-09/R-12, hard gate) — `scripts/` core family emptied (shared `common.*`/`gal.{ps1,sh}` at cross-plan end-gate); `cargo test` green and test code no longer spawns core live scripts (grep-verified).

R-13/R-10 pure end-user real-machine acceptance (CORR-01: never build on test machine)
- [ ] T-034 (R-13, prereq of T-035) — Build cross-platform release-artifact pipeline: CI (macOS/Linux/Windows runners, e.g. GitHub Actions matrix) produces prebuilt `gal` per target (macOS-arm64+) + packaged source (FHS/flat), published to GitHub Releases / brew tap. Wire `gal release` to CI; not current-platform-only.
- [ ] T-035 (R-10, hard gate; prereq = T-034) — **Pure end-user real-machine acceptance**: on mac-mini (macOS-arm64, no rust/no repo) and Windows (normal), install the T-034 prebuilt artifact **as an end user** (brew/Releases/scp the release file); verify packaged-source self-resolves, Unix symlink/junction skill surface, doc-sync loads, canonical root complete, no orphans, `gal doctor` green. **Never install rust/build/cross-compile on the test machine.**

## Deferred Follow-up

- Linux normal-mode real-machine end-user verification when a host is available (install R-13 Linux artifact).
- `curl|sh` convenience installer as a `gal release` artifact (optional, not tracked source).

## Analyze

(empty — populated by reviewer/debugger during execution)

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | integration | fixture freeze: each core script's `tests/fixtures/` reproduces current observable output/side-effects | T-001 |
| TP-02 | integration (hard gate) | **mac-mini pure end-user**: no rust/no repo, install T-034 prebuilt macOS-arm64 artifact → packaged-source self-resolves, `~/.claude/skills/gal` Unix symlink + doc-sync loads, canonical root correct, no orphans, doctor green. Never build on the machine. | T-035 |
| TP-03 | integration (hard gate) | **Windows pure end-user**: install T-034 prebuilt artifact (normal, packaged-source + junction) → canonical root + skill surface + doctor green | T-035 |
| TP-04 | unit | after extracting `base`, 228 tests green; `base` depends on no GAL crate; workspace `Cargo.toml` members correct | T-003 |
| TP-05 | unit | after `base::platform` sink, render/install use it, 228 green; Windows junction / Unix symlink / perms unchanged | T-004 |
| TP-06 | unit | after `base::render` sink, existing render uses it, 228 green, output byte-identical | T-005 |
| TP-07 | unit | `HealthCheck` trait in `base`; existing doctor checks migrated; exit grading unchanged | T-006 |
| TP-08 | unit | after extracting `providers`, 228 green; install depends on providers; no cycles | T-007 |
| TP-09 | unit | after renaming `cli`/`dispatch`, 228 green; `gal --version` works; no external dep name clash | T-008 |
| TP-10 | unit | after T-009 reparent, `cargo test` no longer spawns `Test-ResolveGalCatalog`/`Test-InstallModeAuthority`/`test-install-acceptance`; reads fixtures | T-009 |
| TP-11 | unit | `common.*` → `base` function groups == fixture | T-010 |
| TP-12 | parity | `gal mcp` four-provider MCP config output == fixture (per-provider) | T-011 |
| TP-13 | unit | after deleting `update-mcp.{ps1,sh}` no consumer breaks; mixed-state invariant | T-012 |
| TP-14 | parity | `adapters`: update-skills/commands/personalization/Sync-DevContext each generate == fixture; shares `base::render` with install | T-013..T-016 |
| TP-15 | integration | `gal sync`/`gal update` run through `adapters`, generated adapter files == fixture | T-016 |
| TP-16 | parity | `gal setup` orchestration == fixture; `gal setup --tools` == fixture | T-018, T-019 |
| TP-17 | integration (cross-platform) | git filter registration; Windows (no bash) and Unix both smudge/clean unchanged | T-020 |
| TP-18 | parity | install family per-provider parity: Claude/Copilot/Codex/AGY orchestration + render == fixture | T-022, T-023 |
| TP-19 | unit | after install-family deletion, four-provider live read-surface aligned (doctor green) | T-024 |
| TP-20 | parity | `vcs` (clean/smudge/commit-msg), `gal uninstall` (ledger), init-repo, catalog, release-packaging, translation each == fixture | T-025..T-030 |
| TP-21 | unit | each domain implements `HealthCheck`; `gal doctor` aggregates, non-zero naming fix surface for missing mcp/setup/sync/filter, exit 0 complete | T-031 |
| TP-22 | integration | `gal` entry core subcommands run through Rust; SKILL.template references → binary; noun-grouped subcommands callable | T-032 |
| TP-23 | manual | `scripts/` core family zero ps1/sh (grep); `cargo test` green, no core live-script spawn | T-033 |
| TP-24 | perf | binary cold-start vs script path (validates Motivation #2; not a gate) | T-032 |
| TP-25 | integration | mixed-state invariant spot-check: mid-migration states fully functional, no consumer calls a deleted/half-ported surface | T-011..T-030 |
| TP-26 | integration | R-13 pipeline: CI produces prebuilt `gal` per target (macOS-arm64+) + packaged source, installable from Releases/brew | T-034 |
| TP-26b | integration | dev-machine (Windows, has rust) dev-mode `gal install` doctor green (local baseline) | T-002 |

Real-machine end-user order (T-035, prereq T-034): (1) T-034 CI produces macOS-arm64 prebuilt artifact → (2) mac-mini installs it as end-user (brew/Releases/scp; **no rust/no repo/no build**), verified via SSH (packaged-source self-resolve + Unix symlink + doc-sync + doctor green) → (3) Windows normal installs artifact. Linux when a host is available. **Core rule: test machines are pure end-users; never install a toolchain or build on them.**

## Test Results

### [T-003] 2026-06-09 — PASS (TP-04)

Verification Independence: DEGRADED_SAME_RUNTIME (TESTER played in-runtime; spec = TP-04, refactor parity).

- **228 / 0** — `cargo test --workspace` 228 passed, 0 failed (frozen oracle held; no regression from base extraction).
- **Moved tests execute under `base`** — `cargo test -p base` = 38 passed, 0 failed. config/mode/ledger test modules relocated with their source and run in the new crate (not silently dropped).
- **Dependency law** — `cargo metadata`: `base` deps = [chrono, dirs, serde, serde_json, thiserror, tempfile]; GAL-crate deps = NONE.
- **Workspace members** — [base, gal-cli, gal-engine, gal-dispatch]; `base` present, manifest correct.
- **Build** — clean, 0 warnings.

No new tests authored: T-003 is a structural move; TP-04 is parity (existing suite stays green + structural assertions). Moved code carries its own tests.

## Review Results

### Architecture Review

APPROVE (4 review passes, 2026-06-09). Motivation (full Rust + single binary) sound; layered architecture, dependency law, engine retirement, fixture-gated deletion healthy. Folded: JIT decomposition, crate-vs-module discipline, mixed-state invariant, fixture-parity deletion gate, doctor dependency inversion (HealthCheck in base), render primitive shared (`base::render`), `base::platform` self-contained, setup thin. **4th pass CORR-01 (user, fundamental): real-machine validation must be pure end-user install of a prebuilt artifact — never build/install toolchain on a test machine. Missing prerequisite added as R-13 (cross-platform release-artifact pipeline); R-10 rewritten; T-002 re-scoped to dev-machine baseline; T-034 (pipeline) / T-035 (end-user acceptance) added; build-host vs end-user-machine roles strictly separated.** No blocking issues.

#### Implementation-time sign-off — R-00 (T-003..T-008), 2026-06-09 (code-grounded)

Verdict: **APPROVE (direction)** with 3 BLOCKING execution conditions (grounded in real coupling map: `config`/`ledger`/`paths` are leaves; `providers`→`mcp`; platform primitives scattered across render/providers/doctor; `gal-dispatch` engine-independent, bin spawned by `gal.ps1` as `gal-dispatch.exe`; engine 154 `#[test]`, workspace compiles clean).

- **[BUG-01, CRITICAL — fold into T-003]** providers↔engine cycle at T-007: `providers` imports `crate::mcp::{McpManifest, McpServer, Result}`; `mcp` not split until T-011, so `gal-engine::install → providers` + `providers → gal-engine::mcp` = cycle. Fix: move MCP shared **data-model types** (`McpManifest`, `McpServer`, `McpError`/`Result` alias) into `base` during T-003; keep MCP generation logic in engine until T-011. No T-007 before this lands.
- **[BUG-02, HIGH — T-008]** crate rename `gal-dispatch`→`dispatch` changes bin output to `dispatch.exe` while `gal.ps1` spawns `gal-dispatch`; live dispatch breaks. Fix: keep `[[bin]] name="gal-dispatch"` (package renames, bin name preserved) OR update shim path in same commit; verify shim resolves bin before T-008 done.
- **[OE-01, T-005]** `base::render` scope fence: render.rs (1054 lines) is mostly install-domain (`scan_source_components`, `render_canonical_root`) — do NOT sink wholesale into base (rebuilds god-crate). Only shared template-string primitives → `base::render`; `atomic_swap`→`base::platform` (T-004); install render stays install-domain. Define function-level split before coding T-005.
- **Conditions on entry**: write crisp T-003/T-007 boundary (provider *selection/config* → base vs provider *projection* → providers, incl. explicit call on `McpProviderConfig`/`has_unresolved_secrets`); capture exact `cargo test --workspace` pass count as the frozen oracle and gate each task on that number (not engine-only 154).

### Business Review

Not triggered (no business-rule content).

### Design Review

Not triggered (no customer-facing UI).

### Engineering Review

CLEAR (4th pass). 35 T-NNN map to R-00..R-13 / P0..P8, each an independently verifiable commit-size unit. 27 TP cover (per-crate-split "still green", per-provider parity, git-filter cross-platform, mixed-state invariant, R-13 pipeline TP-26, dev baseline TP-26b, end-user real-machine TP-02/03). Implementation constraints: (1) protected-core architect sign-off for T-003..T-009/T-013..T-017/T-018..T-021; (2) deletion hard-gate = fixture-parity green, T-009 reparent before its covered deletions; (3) mixed-state invariant at every phase boundary; `common.*`/`gal.{ps1,sh}` deletion at cross-plan joint gate; (4) JIT decomposition; (5) **CORR-01: real-machine/end-user tests install R-13 prebuilt artifact only; never build on a test machine; T-034 is hard prereq of T-035.** CODER≠REVIEWER; reviewer tier ≥ implementer.

### [T-003] 2026-06-09 — APPROVE

Scope: commit range `de90322..1ffd3de` (extract `base` foundation crate). Verification Independence: DEGRADED_SAME_RUNTIME (REVIEWER in-runtime; not independent from CODER this run).

- **Correctness**: re-export façade (`gal_engine` `pub use base::{config, ledger, mode, paths}` + `mcp` re-export) preserves both internal `crate::*` and public `gal_engine::*` surfaces. `mcp_provider_oracle_parity` 13/13 green confirms MCP types serialize byte-identically after the move. No behavior change.
- **Scope discipline**: moved exactly config/mode/ledger/paths + 4 MCP data-model types. Provider projection, render, install, doctor untouched (consumed via re-export). No drift, no opportunistic edits.
- **Architect conditions honored**: BUG-01 (MCP types relocated to `base::mcp`, cycle pre-empted before T-007) ✓; T-003/T-007 boundary recorded in commit body ✓; frozen oracle (228) captured and held ✓.
- **Hygiene**: 0 build warnings (no dangling `thiserror` import after type removal); Cargo.lock committed; all `base` deps justified (config/ledger use dirs+serde_json; ledger uses chrono).
- **No BLOCKING findings. No Protected-Path violation, no security surface.** crates/ Rust is outside the project's protected-path set; the plan-level R-00 "protected" gate is satisfied by the recorded architect sign-off.

Verdict: **APPROVE** — proceed to T-004.

## Debug Log

(empty)
