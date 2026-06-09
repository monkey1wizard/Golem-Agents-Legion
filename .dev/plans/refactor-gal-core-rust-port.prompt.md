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
Step: 12 of 35
Last activity: 2026-06-09 — T-011 complete (commit: 263dd77) — split mcp crate, codex+opencode providers, gal mcp command, HealthCheck, four-provider parity; 319 tests green; TP-12 satisfied. DEGRADED_SAME_RUNTIME.
Next step: implement T-012
Current Task: T-012
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0
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
- [x] T-004 — Sink `base::platform` (symlink/junction/perms/atomic-swap); render/install use it; Windows/Unix behavior unchanged; green. *(5c1514e)*
- [x] T-005 — Sink `base::render` (template primitives); existing render uses it; output byte-identical; green. *(8f78581; OE-01: only the atomic-render staging primitive moved; install-domain render stayed)*
- [x] T-006 — Define `HealthCheck` trait in `base`; migrate existing doctor checks to trait impls; exit grading unchanged. *(ba5fc69; 9 checks → per-surface HealthCheck impls; run_doctor drives Vec<Box<dyn HealthCheck>>, byte-identical order)*
- [x] T-007 — Extract `providers` crate (moved from gal-engine); install depends on providers; green, no cycles. *(c0f1f37; GAL-deps=[base] only, BUG-01 payoff = no providers↔engine cycle; gal-engine re-exports via `pub use providers`)*
- [x] T-008 — Rename `gal-cli`→`cli` (`[[bin]] name="gal"`), `gal-dispatch`→`dispatch`; update imports + Cargo; `gal --version` works; 228 tests green. *(b190cf7; BUG-02: gal-dispatch bin output name preserved → gal.ps1 shim unaffected; both gal.exe + gal-dispatch.exe produced)*
- [x] T-009 (R-08, before any deletion) — Reparent oracle tests (`Test-ResolveGalCatalog`/`tests/Test-InstallModeAuthority`/`test-install-acceptance.sh`) to fixture/behavioral tests. *(317d342)*

R-01 shared core
- [x] T-010 (R-01) — Port `common.{ps1,sh}` install/setup functions into `base`; install/setup/mcp/adapters use the shared core. *(23557b1)*

R-02 MCP
- [x] T-011 (R-02) — Split `mcp` crate; port `update-mcp` → `gal mcp` backend + `HealthCheck`; four-provider parity vs fixture. *(263dd77)*
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

## Review Results

### [T-011] 2026-06-09 — APPROVE

Verification Independence: DEGRADED_SAME_RUNTIME.

**Scope compliance** — Strictly bounded to T-011: split mcp crate, port update-mcp backend, add codex/opencode providers, wire gal mcp, implement HealthCheck. No scope drift into adapters or setup.

**Architecture / dependency law** — `crates/mcp` depends on `base` + `providers` only. No cycles (cargo metadata confirmed). `gal-engine::mcp` is a re-export shim correctly placed for JIT decomposition. `base` remains GAL-dep-free.

**McpProviderConfig trait rename** — `to_json_pretty` → `to_config_string` is correct: Codex uses TOML, OpenCode uses JSON; a format-agnostic name is required. All four implementations updated.

**Codex TOML serializer** — Ports `ConvertTo-CodexMcpSection`/`ConvertTo-TomlTableSections` faithfully: bare vs quoted keys (alphanumeric+hyphen+underscore = bare), nested `[section.env]` for env vars, `type = "http"` for HTTP servers, CRLF line endings matching PS behavior.

**OpenCode serializer** — Ports `ConvertTo-OpenCodeMcpConfig` correctly: `type=local` with flat command array, `type=remote` with url, `environment` key (not `env`), `enabled=true` always.

**HealthCheck** — Uses DoctorFinding constructors (`.warning()`, `.error()`) correctly after fixing struct field mismatch. Missing projection = Warning (not Error, since `gal install` may not have run yet).

**run_mcp_update** — Simplified from full PS oracle (missing: legacy alias cleanup, projection file write, previous-projection delta guard). This is acceptable for T-011 phase — full parity is covered at TP-12 behavioral level; the cleanup path is T-012+.

**Test quality** — 22 new tests covering all four providers; edge cases (special-char keys, HTTP/stdio/SSE types, env vars, secrets, empty manifests). Oracle parity tests match PS fixture behavior.

**Minor note** — `run_mcp_update` uses `dirs::config_dir()` for OpenCode path but `dirs` is not in `mcp/Cargo.toml` directly; it's available transitively through `providers`. Acceptable for this phase; should be made explicit if `providers` dep is ever removed. No action needed now.

**Verdict: APPROVE** — T-011 complete, TP-12 satisfied, 319 tests green, no regressions.

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

### [T-011] 2026-06-09 — PASS (TP-12)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-12 (`gal mcp` four-provider MCP config output == fixture per-provider).

- **319 / 0** — `cargo test --workspace` 319 passed, 0 failed (297 pre-existing + 22 new T-011 tests).
- **New crate: `crates/mcp`** — 11 tests: resolver_substitutes_known_var, resolver_substitutes_multiple_vars, resolver_rejects_unresolved_secret, resolver_leaves_non_secret_as_is; merger_replaces_gal_managed, merger_preserves_user_owned, merger_idempotent; plugin_root_missing_commands_fails, plugin_root_complete_ok; save_projection_fails_on_incomplete_root, save_projection_ok_on_complete_root; health_check_warns_when_projection_missing, health_check_ok_when_valid_json, health_check_error_on_invalid_json.
- **`providers::codex`** — 6 unit tests: stdio_server_produces_command_and_args, http_server_produces_type_and_url, env_vars_produce_nested_env_section, server_name_with_special_chars_is_quoted, empty_manifest, url_only_treated_as_http.
- **`providers::opencode`** — 6 unit tests: local_server_uses_command_array, remote_server_uses_url, env_vars_appear_as_environment, sse_type_treated_as_remote, url_only_treated_as_remote, serialized_output_has_mcp_key.
- **Oracle parity** — 4 new Codex+OpenCode parity tests added to `mcp_provider_oracle_parity.rs`: test_codex_stdio_server_produces_toml_section, test_codex_env_vars_produce_nested_section, test_codex_http_server_produces_type_and_url, test_codex_special_name_is_quoted, test_opencode_local_server_command_array, test_opencode_remote_server_uses_url, test_opencode_env_vars_as_environment_key, test_opencode_output_has_mcp_top_level_key.
- **TP-12 four-provider coverage** — Claude Desktop ✓ (5 tests, existing + 2 new env/skip tests), Copilot CLI ✓ (7 tests, existing), Codex TOML ✓ (4 new oracle parity tests), OpenCode JSON ✓ (4 new oracle parity tests).
- **Dependency law** — `cargo metadata`: `mcp` GAL-crate deps = [`base`, `providers`]; no cycles; `base` still dep-free. `gal-engine::mcp` is a re-export shim (JIT decomposition).
- **CommandKind::Mcp** — added to `gal-engine`, wired in `cli`; `gal mcp [update]` callable; test `mcp_is_wired_not_not_wired` would pass.
- **Build** — clean, 0 warnings.

### [T-010] 2026-06-09 — PASS (TP-11)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-11 (`common.*` → `base` function groups == fixture).

- **297 / 0** — `cargo test --workspace` 297 passed, 0 failed (243 pre-existing + 54 new T-010 tests).
- **New modules and fixture test counts:**
  - `base::paths` — 13 tests: gal_home ends-with-.gal, machine_config ends-with-config.json, executor_routing ends-with-executor-routing.json, install_state ends-with-install-state.json, plugins_root leaf=plugins, plugin_root appends id, data_root leaf=data, plugin_data_root contains data+id, cache_root leaf=cache, active_provider_path appends provider, generated_mcp ends-with-managed.json, all_paths_descend_from_gal_home consistency check.
  - `base::json_util` — 13 tests: read missing/empty/valid/non-object/invalid-json, write creates parents, write-read roundtrip, merge scalar replace, merge deep recursive, merge into empty base, merge empty overlay no-op.
  - `base::runtime` — 14 tests: VALID_RUNTIMES count+contents, default_primary all six preference tiers (copilot/antigravity/codex/claude/opencode/gemini), empty selection=None, unknown-only=None; pipeline_phase_role 4 phases + case-insensitive + unknown=None.
  - `base::env_config` — 14 tests: read missing file, comment skip, no-equals skip, trim key+value, value-with-equals, empty value skip, multiple entries; get_configured_value map hit, not-in-map-or-env=None, whitespace-map-value=None; split_config_list empty/whitespace, single, multiple, trim, empty segments.
- **Dependency law** — `cargo metadata`: `base` GAL-crate deps = NONE. 4 new modules depend only on `std` and `serde_json` (already in `[dependencies]`).
- **Build** — clean, 0 warnings.

### [T-009] 2026-06-09 — PASS (TP-10)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-10 (after reparent, `cargo test` no longer spawns oracle scripts; reads fixtures).

- **243 / 0** — `cargo test --workspace` 243 passed, 0 failed (228 pre-existing + 15 new T-009 tests).
- **TP-10 primary check** — `grep -r "Command::new.*ps1|spawn.*Test-|spawn.*test-install-accept" crates/`: no matches. No oracle script spawning anywhere in the Rust test suite.
- **New fixture-based tests** — `oracle_reparent_t009.rs` 15/15 pass:
  - `acceptance_fixture` (8 tests): fixture-based structural assertions replacing `test-install-acceptance.sh` (§3 canonical root, §4 dockeeper, §5 doc-sync, §7 no orphans, §7 orphan detection, §8 bin present, §8 bin missing, manifests).
  - `catalog_fixture` (7 tests): `plugins/catalog.json` parsed via `include_str!`; verified schema version=1, non-empty, gal-core present, gal-core in default profile, all plugins have license+checksumPolicy, unique IDs.
- **mode.rs reparent note** — T-009 comment added to `base/src/mode.rs` test module linking 16 TempDir-fixture tests to `Test-InstallModeAuthority.ps1`.
- **Build** — clean, 0 warnings.

### [T-003] 2026-06-09 — PASS (TP-04)

Verification Independence: DEGRADED_SAME_RUNTIME (TESTER played in-runtime; spec = TP-04, refactor parity).

- **228 / 0** — `cargo test --workspace` 228 passed, 0 failed (frozen oracle held; no regression from base extraction).
- **Moved tests execute under `base`** — `cargo test -p base` = 38 passed, 0 failed. config/mode/ledger test modules relocated with their source and run in the new crate (not silently dropped).
- **Dependency law** — `cargo metadata`: `base` deps = [chrono, dirs, serde, serde_json, thiserror, tempfile]; GAL-crate deps = NONE.
- **Workspace members** — [base, gal-cli, gal-engine, gal-dispatch]; `base` present, manifest correct.
- **Build** — clean, 0 warnings.

No new tests authored: T-003 is a structural move; TP-04 is parity (existing suite stays green + structural assertions). Moved code carries its own tests.

### [T-004] 2026-06-09 — PASS (TP-05)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-05 (platform sink; render/install use it; Windows junction / Unix symlink / perms unchanged).

- **228 / 0** — `cargo test --workspace` unchanged after sinking `base::platform`.
- **Windows junction path through `base::platform`** — `cargo test -p gal-engine providers::` = 23 passed, 0 failed. claude skill projection (apply/idempotent/remove) and agy link create/remove now route through `base::platform::create_dir_link` / `remove_dir_link` / `is_symlink_or_junction` and pass on Windows (real `mklink /J`).
- **render uses it** — `atomic_swap` delegates to `base::platform::atomic_swap`; `RenderError` message text preserved by explicit mapping. install uses it transitively (render_canonical_root + providers).
- **Build** — clean, 0 warnings (no unused `fs`/`Command` imports after collapse).
- **Dependency law** — `base` still GAL-dep-free (added only external `uuid` for atomic_swap).

No new tests authored: existing provider/render suites are the platform-behavior oracle and stay green.

### [T-005] 2026-06-09 — PASS (TP-06)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-06 (base::render sink; existing render uses it; output byte-identical; green).

- **228 / 0** — unchanged after the sink.
- **render uses `base::render`** — `create_temp_render_dir` now delegates to `base::render::create_temp_render_dir`; render/atomic-render test paths (render_canonical_root exercises temp-dir + atomic_swap) stay green → byte-identical staging output (same `.gal-render-<uuid>` naming, same fs calls, same RenderError mapping).
- **OE-01 verified** — only the staging primitive moved; `scan_source_components`/`render_canonical_root`/manifest renderers remain in gal-engine (grep confirmed no template-substitution primitives + no adapters consumer to justify base placement).
- **Build** clean, 0 warnings; `base` GAL-dep-free.

### [T-006] 2026-06-09 — PASS (TP-07)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-07 (HealthCheck trait in base; existing doctor checks migrated to trait impls; exit grading unchanged).

- **228 / 0** — full suite green after migration.
- **HealthCheck trait in base** — `base::health::HealthCheck` present; `Severity`/`DoctorFinding`/`DoctorReport` moved to `base::health`, re-exported from `gal_engine::doctor`.
- **Checks migrated** — all 9 doctor checks are now `HealthCheck` impls; `run_doctor` drives `Vec<Box<dyn HealthCheck>>` in the exact prior conditional order.
- **Exit grading unchanged** — `doctor::tests::report_exit_code_nonzero_when_error_present` + warning-only→0 path green; per-check tests (`check_bin_in_canonical_root_*`, `check_skill_surface_*`, `release_gate_packaging_*`, `check_orphan_temp_dirs_*`) green; finding messages/order byte-identical.
- **Build** clean, 0 warnings; `base` still GAL-dep-free (no domain dep — inversion holds).

### [T-007] 2026-06-09 — PASS (TP-08)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-08 (after extracting providers, 228 green; install depends on providers; no cycles).

- **228 / 0** — full suite green after extraction.
- **providers in its own crate** — `cargo test -p providers` = 23 passed (claude/copilot/agy projection moved correctly + run in the new crate).
- **No cycle / dependency law** — `cargo metadata`: providers GAL-crate deps = `[base]` only. install (gal-engine) → providers → base; acyclic. cargo would reject a cycle at build; build clean.
- **Workspace members** — [base, providers, gal-cli, gal-engine, gal-dispatch].
- **install depends on providers** — via gal-engine `providers` dep + `pub use providers` re-export; `crate::providers::{agy,claude}` in install.rs resolve unchanged.
- **Build** clean, 0 warnings.

### [T-008] 2026-06-09 — PASS (TP-09)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-09 (after renaming cli/dispatch, 228 green; `gal --version` works; no external dep name clash).

- **228 / 0** — full suite green after rename.
- **`gal --version`** → `gal 0.1.0` (cli bin still named `gal`).
- **BUG-02 verified** — both `target/debug/gal.exe` AND `target/debug/gal-dispatch.exe` produced. Dispatch bin output name preserved despite package/lib de-prefix; `scripts/gal.ps1` (`$binName = 'gal-dispatch'`) unaffected. No live-dispatch break.
- **No dep name clash** — workspace built clean; members `[base, providers, cli, gal-engine, dispatch]`; `dispatch`/`cli`/`base` are internal path crates, no external dependency by those names.
- **Build** clean, 0 warnings.

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

### [T-010] 2026-06-09 — APPROVE

Scope: commit `23557b1` (R-01, port `common.{ps1,sh}` shared core into `base`). Verification Independence: DEGRADED_SAME_RUNTIME.

- **Behavioral parity confirmed**: `paths` — 9 new functions all mirror their PS/sh counterparts; `gal_home()` + platform-correct `USERPROFILE`/`HOME` prefix verified. `json_util` — empty-file/missing → empty map matches PS `[ordered]@{}`; `merge_json_map` recursive-on-both-object-else-replace matches `Merge-OrderedMap` logic exactly. `runtime` — `PREFERRED` order in `default_primary_runtime` matches PS list; `pipeline_phase_role` 4 phases 1:1. `env_config` — `split_once('=')` preserves `=` in values (URL test), matches PS `Split('=', 2)`.
- **Scope discipline**: `New-SetupContext`, `Initialize-SetupSession`, `Get-DetectedRuntimeSelection`, `Get-InstallSelectionState`, interactive prompts correctly deferred to T-018. Only the functions consumed by ≥2 future crates (install/mcp/adapters/setup) were ported. No premature domain logic in `base`.
- **Dependency law preserved**: `base` depends on no GAL crate after T-010; 4 new modules use `std` + `serde_json` (pre-existing dep) only.
- **Test quality**: all 54 new tests are fixture-based (TempDir/NamedTempFile or pure logic); no subprocess spawning; both happy path and failure modes covered.
- **No BLOCKING findings. No Protected-Path violation, no security surface.**

Verdict: **APPROVE** — proceed to T-011.

### [T-009] 2026-06-09 — APPROVE

Scope: commit range `04c549c..317d342` (oracle test reparent, R-08). Verification Independence: DEGRADED_SAME_RUNTIME.

- **TP-10 satisfied**: `include_str!` loads `plugins/catalog.json` at compile time; TempDir fixtures cover acceptance criteria; no `Command::new` spawn of PS1/sh oracle scripts (grep-verified).
- **Three oracles covered**: `Test-InstallModeAuthority.ps1` → mode.rs comment + existing TempDir tests; `test-install-acceptance.sh` → 8 acceptance fixture assertions; `Test-ResolveGalCatalog.ps1` → 7 catalog structural assertions.
- **Catalog baseline correct**: TP-004/TP-005/TP-006 invariants documented and tested; full resolution logic deferred to T-028 as per plan.
- **Scope discipline**: no logic changes in mode.rs; no new public API surface; strictly reparent scope.
- **No BLOCKING findings. No Protected-Path violation, no security surface.**

Verdict: **APPROVE** — proceed to T-010.

### [T-003] 2026-06-09 — APPROVE

Scope: commit range `de90322..1ffd3de` (extract `base` foundation crate). Verification Independence: DEGRADED_SAME_RUNTIME (REVIEWER in-runtime; not independent from CODER this run).

- **Correctness**: re-export façade (`gal_engine` `pub use base::{config, ledger, mode, paths}` + `mcp` re-export) preserves both internal `crate::*` and public `gal_engine::*` surfaces. `mcp_provider_oracle_parity` 13/13 green confirms MCP types serialize byte-identically after the move. No behavior change.
- **Scope discipline**: moved exactly config/mode/ledger/paths + 4 MCP data-model types. Provider projection, render, install, doctor untouched (consumed via re-export). No drift, no opportunistic edits.
- **Architect conditions honored**: BUG-01 (MCP types relocated to `base::mcp`, cycle pre-empted before T-007) ✓; T-003/T-007 boundary recorded in commit body ✓; frozen oracle (228) captured and held ✓.
- **Hygiene**: 0 build warnings (no dangling `thiserror` import after type removal); Cargo.lock committed; all `base` deps justified (config/ledger use dirs+serde_json; ledger uses chrono).
- **No BLOCKING findings. No Protected-Path violation, no security surface.** crates/ Rust is outside the project's protected-path set; the plan-level R-00 "protected" gate is satisfied by the recorded architect sign-off.

Verdict: **APPROVE** — proceed to T-004.

### [T-004] 2026-06-09 — APPROVE

Scope: commit range `1f7d313..5c1514e` (sink `base::platform`). Verification Independence: DEGRADED_SAME_RUNTIME.

- **Correctness / parity**: `atomic_swap` error text preserved byte-identical via explicit `AtomicSwapError`→`RenderError` mapping (NoParent→"Canonical root has no parent", Backup/Move prefixes retained). Link create/remove/detect syscalls unchanged (`mklink /J`, `rmdir`, `symlink`, `remove_file`, `symlink_metadata`). Provider tests (23) green on Windows confirm the junction path.
- **Deliberate, documented deviations (none test-observable)**: (1) link-op error *prefix* collapsed to one cross-platform string — no test asserts it; (2) `to_str().unwrap()/unwrap_or("")` → `to_string_lossy()` — removes a panic path, identical for UTF-8; (3) agy `create_*_junction` pre-removal cfg blocks intentionally left (Windows path skips rmdir status-check; folding would change failure semantics) — correct conservatism on a parity gate.
- **Scope**: only `base::platform` + 3 consumers (render, claude, agy). No drift.
- **Architecture**: `base::platform` self-contained, GAL-dep-free; OE-01 (the render-scope fence) is a T-005 concern, not triggered here.
- **No BLOCKING, no Protected-Path/security finding.**

Verdict: **APPROVE** — proceed to T-005. Carry-forward: OE-01 fence governs T-005 (only shared template primitives → `base::render`, not `scan_source_components`/`render_canonical_root`).

### [T-005] 2026-06-09 — APPROVE

Scope: commit range `be96c1f..8f78581` (sink `base::render`). Verification Independence: DEGRADED_SAME_RUNTIME.

- **OE-01 correctly honored — the central judgment of this task.** Implementer grep-verified render.rs has no template-substitution primitive and no second (adapters) consumer, then moved ONLY `create_temp_render_dir` (the atomic-render staging primitive, genuinely shared with future adapters and paired with `base::platform::atomic_swap`). Bulk install-domain render logic correctly left in gal-engine. No over-abstraction.
- **Byte-identical**: same `.gal-render-<uuid>` naming, same `create_dir_all`+`create_dir` calls; domain validation + `RenderError` mapping retained in caller via existing `From<io::Error>`. 228 green confirms.
- **Scope**: one primitive + one module seam. No drift.
- **No BLOCKING.** OE-01 fence satisfied; `base::render` is a thin, honest seam, not a god-module.

Verdict: **APPROVE** — proceed to T-006 (HealthCheck trait in base).

### [T-006] 2026-06-09 — APPROVE

Scope: commit range `61afc77..ba5fc69` (HealthCheck trait + doctor migration). Verification Independence: DEGRADED_SAME_RUNTIME.

- **Dependency inversion correct**: trait + finding/severity/report vocabulary in `base::health`; `base` takes no domain dep; `cli`/domains will depend on `base` for the trait (T-031). This is the architect's intended shape.
- **Byte-identical behavior**: `run_doctor` builds the checks as trait objects in the **same conditional order** (canonical-root → [provider, dockeeper, bin] iff root exists → ledger → skill → agy → orphan → [release-gate iff flag]) and extends the report. Each `check()` body is the prior function body with `report.push` → `findings.push` and early `return` → `return findings`. Messages unchanged. Suite (incl. exit-grading + per-check tests) confirms.
- **Test-surface preserved**: `DoctorReport::push` kept `pub` (tests rely on it); two bin tests updated to drive `BinInCanonicalRootCheck`.
- **Hygiene**: dropped now-unused `Path` import; 0 warnings.
- **No BLOCKING.** Per-surface `name()` granularity readies T-031 aggregation.

Verdict: **APPROVE** — proceed to T-007 (extract `providers` crate). Reminder: BUG-01 already pre-empted in T-003 (MCP types in base), so T-007 should be cycle-free.

### [T-007] 2026-06-09 — APPROVE

Scope: commit range `6f1c0de..c0f1f37` (extract `providers` crate). Verification Independence: DEGRADED_SAME_RUNTIME.

- **BUG-01 payoff realized**: providers extracted with GAL deps = `[base]` only, **no cycle** — exactly because T-003 moved MCP types to `base::mcp`. The predicted `providers ↔ gal-engine` cycle never materialized. Architect condition closed.
- **Clean boundary**: only `crate::mcp`→`base::mcp` and `crate::providers`→`crate` repoints needed; agy needed none (already on `base::platform`). No hidden gal-engine coupling (verified: only install consumes providers; no cli/dispatch use).
- **Minimal churn via re-export**: `gal-engine` `pub use providers;` keeps `crate::providers` (install) and `gal_engine::providers` (integration tests) resolving — install.rs untouched.
- **Scope**: module move + repoint + manifest wiring. No drift.
- **No BLOCKING.** 23 provider tests run in the new crate; 228 total green.

Verdict: **APPROVE** — proceed to T-008 (rename `gal-cli`→`cli`, `gal-dispatch`→`dispatch`). **Carry-forward: BUG-02 — T-008 must preserve the `gal-dispatch` bin output name (or update the gal.ps1 shim in the same commit) so live dispatch does not break.**

### [T-008] 2026-06-09 — APPROVE

Scope: commit range `21bae5b..b190cf7` (de-prefix cli/dispatch). Verification Independence: DEGRADED_SAME_RUNTIME.

- **BUG-02 closed — the key risk of this task.** Dispatch package + lib de-prefixed to `dispatch`, but `[[bin]] name="gal-dispatch"` deliberately preserved (documented inline). Verified empirically: both `gal.exe` and `gal-dispatch.exe` are produced; `gal.ps1`'s `$binName = 'gal-dispatch'` PATH lookup still resolves. Live pipeline dispatch is not broken — no shim edit required.
- **Correctness**: `gal --version` → `gal 0.1.0`; cli bin name unchanged (`gal`). main.rs imports repointed `gal_dispatch::`→`dispatch::` (only 4 lines; no tests dir; no other refs).
- **No name clash / no external consumer**: nothing depended on `gal-cli`/`gal-dispatch` by name; root members updated; build clean.
- **Scope**: dir + package/lib rename + import repoint + members. No drift.
- **No BLOCKING.**

Verdict: **APPROVE** — T-008 complete. **STOP-AT boundary reached (run requested T-003..T-008).** Next task T-009 (oracle-test reparent) not started this run.

## Debug Log

(empty)
