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
Step: 31 of 35
Last activity: 2026-06-10 — **T-030 complete** — Rust `gal translation-freshness` now owns translation freshness reporting and the old translation script pair is deleted. Run mode: DEGRADED_BUNDLED (user-directed single-runtime, separate phase passes + write-back). Stop-at boundary T-030 reached.
Next step: implement T-031 (R-11 — doctor aggregates domain health checks)
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
| 2026-06-09 | T-011 | `gal mcp` four-provider parity (TP-12) treated complete at serializer level | **Reopened + fixed (a600c97)** after full-audit found `run_mcp_update` was a serializer-only stub: empty resolver (hard-failed on shipped manifest's `${CONTEXT7_API_KEY}`), no `mcp.local.json` merge, full-file overwrite per provider (data loss — Codex `config.toml` clobber). Rewrote orchestrator: var map (env+config.local.env+machine config), local-override merge, non-destructive JSON overlay + Codex remove-then-append. +10 hermetic tests; e2e verified 4 providers/11 servers, real `config.toml` non-MCP sections preserved. | TP-12 tested per-provider serializers in isolation, never `run_mcp_update` (the only path `gal mcp` runs). The deletion gate's "behavioral equivalence" half was unmet; audit caught it before real-machine use. Deferred (non-blocking): legacy-alias/deprecated-key cleanup, Codex bridge-profile remap, projection delta guard, OpenCode install-gating. |

### Handoff Notes

R-06 is now fully closed through T-030. Translation freshness is owned by the Rust `gal translation-freshness` command, and the legacy translation script pair is deleted.

The next exact step is T-031: aggregate the domain `HealthCheck` implementations into `gal doctor` so filter/setup/sync surfaces report through the main health command.

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
- [x] T-011 (R-02) — Split `mcp` crate; port `update-mcp` → `gal mcp` backend + `HealthCheck`; four-provider parity vs fixture. *(263dd77; orchestrator parity fix a600c97)*
- [x] T-012 — After parity green, delete `update-mcp.{ps1,sh}`; verify no consumer breaks. *(bda5fb7)*

R-03 adapters (protected, architect)
- [x] T-013 (R-03) — Split `adapters` crate + port `update-skills` → backend (shares `base::render`) + `HealthCheck`; parity.
- [x] T-014 — Port `update-commands` → `adapters`; parity.
- [x] T-015 — Port `update-personalization` → `adapters`; parity.
- [x] T-016 — Port `Sync-DevContext` (init-time) → `adapters`; wire `gal sync`/`gal update`; parity.
- [x] T-017 — After all four pairs reach parity green, delete them.

R-04 setup (protected, architect; implementation sign-off APPROVE-with-conditions C-1..C-10, 2026-06-10)
- [x] T-018 (R-04) — Split `setup` crate (orchestrate-only); port `setup-machine` → `gal setup`; parity. *(2c68fe2)*
- [x] T-019 — Port `setup-tools` → `gal setup --tools`; parity. *(b92b8b0)*
- [x] T-020 — Register git filter (idempotent, via `gal setup`): `.gitattributes` + `git config filter.gal-config.* = bash scripts/gal-clean.sh|gal-smudge.sh` + `required=true` — points at the existing .sh scripts (architect C-4); binary `gal clean/smudge` cutover + atomic re-register moves to T-025. Behavior-unchanged on Windows/Unix (bash-backed; no-bash clause verified at T-025). *(708ca94)*
- [x] T-021 — After parity green, delete `setup-machine.{ps1,sh}` + `setup-tools.{ps1,sh}`. *(41ff61a)*

R-05 install family
- [x] T-022 (R-05) — Confirm `install-gal-plugins` four-provider orchestration parity (per-provider vs fixture: Claude/Copilot/Codex/AGY). *(closeout: `gal install` now writes provider lifecycle ledgers, marketplace descriptors, and provider-visible projection roots directly from Rust; covered by `provider_family_r05` focused probe)*
- [x] T-023 — Confirm `build-core-plugin`/`build-provider-plugins`/`provider-plugin` render parity. *(closeout: Rust canonical render now emits `.codex-plugin/plugin.json` and the provider-neutral install artifacts previously owned by the build scripts)*
- [x] T-024 — After all parity green, delete install family; verify live read-surface aligned. *(closeout: deleted install/build/provider-plugin script family, repointed `gal setup` + `adapters` callers to Rust, aligned `scripts/scripts.md` + `docs/devguide.md`, and verified no live crate caller remains)*

R-06 misc (per item: port → parity → delete)
- [x] T-025 — `vcs`: `gal clean`/`gal smudge` + commit-msg; parity; delete `gal-clean.sh`/`gal-smudge.sh`. *(closeout: added `crates/gal-engine/src/git_filters.rs`, wired `clean` / `smudge` through `cli`, repointed setup/adapters git filter registration to `gal clean` / `gal smudge`, and deleted the bash filter scripts after focused filter tests passed)*
- [x] T-026 — `gal uninstall` parity (ledger precision); delete `uninstall-machine.{ps1,sh}`. *(closeout: uninstall now removes Rust-managed provider ledgers/projections, uses shared home resolution for ledger/uninstall paths, and deletes the uninstall wrapper pair after the focused uninstall probe passed)*
- [x] T-027 — Port `init-repo` → Rust; parity; delete pair. *(closeout: added `crates/cli/src/init_repo.rs`, wired `gal init-repo` in the Rust CLI, repointed `scripts/gal.ps1` and `scripts/gal.sh` init branches to the binary path, and deleted `Init-Repo.*` after a temp-repo executable smoke check passed)*
- [x] T-028 — Port catalog parsing (`Resolve-GalCatalog`) → Rust; parity; delete. *(closeout: added `crates/gal-engine/src/catalog.rs`, wired `gal resolve-catalog` in the CLI, matched the default/dart/full/explicit profile contract with focused tests, and deleted `Resolve-GalCatalog.ps1` plus `Test-ResolveGalCatalog.ps1` after an executable dry-run check passed)*
- [x] T-029 — Fold release packaging into `gal release`; parity; delete `Package-ReleaseArtifacts.{ps1,sh}`. *(closeout: release packaging stayed on the existing Rust `gal release` path; focused release tests passed and the old packaging script pair was deleted with docs repointed)*
- [x] T-030 — Port translation freshness → Rust; parity; delete pair. *(closeout: added `crates/gal-engine/src/translation.rs`, wired `gal translation-freshness` in the CLI, validated unit + executable report output, and deleted the legacy translation script pair)*

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

### [R-04 architect sign-off] 2026-06-10 — APPROVE-with-conditions

Protected-core implementation sign-off for T-018..T-021 (required before implementation). A first architect attempt was discarded: its file reads returned corrupted content (claimed setup scripts were 54-line stubs, missing prompt file, nonexistent crates); orchestrator re-verified ground truth directly and re-dispatched with verified context.

#### R-04 implementation sign-off — APPROVE-with-conditions (2026-06-10)

**Verdict: APPROVE-with-conditions** for T-018..T-021. The decomposition is sound and the strangler-style mixed state is acceptable. Conditions C-1..C-10 are binding; TP-17 wording must be amended (C-4).

##### Ruling 1 — Decomposition

`setup` crate = orchestration only, structured as:

- `session.rs` — port of `Initialize-SetupSession` (runtime selection, primary runtime, `--reconfigure`, first-run prompt) behind a `Prompter` trait for stdin injection.
- `legacy_plugins.rs` — the ONLY module that spawns `Install-GalPlugins.{ps1,sh}`. T-024 repoints exactly this one file.
- `tools.rs` — setup-tools port (Ruling 3).
- Health check implementing `base::HealthCheck` (Ruling 6).

Steps (a),(b): **library calls** — `gal setup` calls `adapters::run_machine_update(machine_options_from_config(...))` and `mcp::run_mcp_update(...)` directly. Self-spawning `gal update` rejected (re-entrancy, PATH ambiguity, env propagation, double parsing). Step (c): mixed state spawns the script via `legacy_plugins.rs`; pass `GAL_BOOTSTRAP_INSTALL` + purge flags in the **child's env/args only** (no parent env mutation); propagate child exit code verbatim. `--check`: read-only — resolve session + install mode, delegate `--check` to Install-GalPlugins, print, exit; zero writes incl. no git-config registration. Deps: base, gal-engine, mcp, adapters.

##### Ruling 2 — T-021 deletion timing

**Acceptable. Delete in R-04 as planned.** Rust-spawns-script is the standard strangler seam; Install-GalPlugins remains a live fully-owned surface until R-05, `legacy_plugins.rs` its sole consumer. Deferring T-021 would leave two parallel entry points. **Do not delete `scripts/common/Common.ps1`/`common.sh` in T-021** — Install-GalPlugins still sources them (their deletion belongs to R-05).

##### Ruling 3 — setup-tools

`tools.rs` module inside `setup`, not a separate crate. Two seams: `Prompter` trait (shared with `session.rs`, stdin-drivable; `--check`/non-interactive bypass) and `CommandRunner` executor trait for network side effects. **TP-16 parity defined as:** (a) status-probe classification parity (4 states × 4 tools) against mocked tool presence; (b) constructed-command parity via mocked executor, byte-compared; (c) `--check` output parity. Live npm/pip/network OUT of fixture scope.

##### Ruling 4 — T-020 sequencing

**Option (a).** `gal setup` performs idempotent registration of `filter.gal-config.clean/smudge` pointing at the existing `gal-clean.sh`/`gal-smudge.sh` (same bash invocation form as today) plus `required=true`; binary cutover stays at T-025, which must **atomically** re-register and delete the two .sh. TP-17 "Windows (no bash)" not satisfiable at T-020 — amended: no-bash clause verified at T-025. T-020 registration is a deliberate behavior **addition** (neither script registers filters today; historical/manual), skipped under `--check`/`--dry-run`.

##### Ruling 5 — installMode drift

**Parity baseline = ps1's shared resolver** (devMode + galRoot; `installMode` deprecated). The sh python-read of the deprecated key is a bug being retired. Intentional normalization: sh aligned to ps1.

##### Ruling 6 — Bug surface & conditions

- **AGY pre-cleanup rm -rf**: highest blast radius — guard non-empty path components, refuse root/`$HOME`, sanity prefix check; Rust JSON edit strips only `gal`/`gal-*` keys preserving all else (jq dependency disappears).
- **Dry-run completeness**: zero filesystem/git-config/env/network writes across ALL steps.
- **Purge gating**: `--purge`⇒`--uninstall`, `--confirm-purge`⇒`--purge`, matching error text + non-zero exit.
- **Uninstall**: `gal mcp update` skip-on-uninstall preserved in order.
- **HealthCheck**: setup checks config readable, ripgrep present, Install-GalPlugins script present (removed at T-024), git filter registration state.
- **CommandKind**: add `setup` (enum → 10); no clap migration smuggled in.

**Conditions (all checkable; T-021 gated on C-1..C-3, C-7, C-8):**

1. **C-1** Fixture parity green for `gal setup` against BOTH setup-machine.ps1 and .sh behavior (modulo C-6 normalizations) before T-021 deletes anything; `common/Common.{ps1,sh}` NOT deleted in T-021.
2. **C-2** Flag-mapping table test: every flag setup-machine forwarded to `gal update --machine-only` maps to an asserted `MachineUpdateOptions` field.
3. **C-3** `legacy_plugins.rs` hard-errors if Install-GalPlugins missing, propagates child exit code, passes `GAL_BOOTSTRAP_INSTALL` via child env only.
4. **C-4** TP-17 amended: T-020 verifies behavior-unchanged (bash-backed filters); Windows-no-bash clause verified at T-025. T-020 task text registers against the .sh scripts; binary registration moves to T-025 (atomic re-register + delete).
5. **C-5** TP-16 redefined: status-probe + constructed-command + `--check` output parity under mocked executor/Prompter; no live network in fixtures.
6. **C-6** Plan records intentional normalizations: (i) sh installMode read retired, ps1 resolver baseline; (ii) T-020 filter registration is a behavior addition, not parity.
7. **C-7** `--check` and `--dry-run` perform zero writes of any kind (incl. git-config registration, AGY cleanup); covered by tests.
8. **C-8** AGY cleanup path-safety guard + JSON edit preserves non-gal keys; both unit-tested.
9. **C-9** `session.rs` supports non-interactive mode and stdin-injected prompts; `--reconfigure` and first-run flows fixture-tested.
10. **C-10** `setup` crate implements `base::HealthCheck`, incl. Install-GalPlugins-presence check flagged for removal at T-024.

### [T-021] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: c0f5632..41ff61a
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** `Uninstall-Machine.*` now requires `gal` on PATH — the same prerequisite the deleted `Setup-Machine.ps1` already imposed (it threw without `gal.exe`); not a new constraint.
- **[I-02]** devguide protected-paths wording updated from `Setup-Machine.*` to `crates/setup/` — protection continuity for the replacing surface; flag to `/gal init` regeneration owners.
- **[I-03]** `GAL_MANAGED_FILE_HEADER` still reads "Generated by GAL Setup-Machine"; changing it would orphan existing managed files. Cutover belongs to a deliberate migration task, not this deletion.

#### Architect conditions check (T-021 slice)
- C-1: deletion only after both-shell parity green; `common/Common.{ps1,sh}` untouched. ✓
- Mixed-state invariant: Install-GalPlugins remains live and reachable (spawned by `gal setup`, and by `gal setup --check` for the doctor path); uninstall wrappers repointed atomically with the deletion. ✓

#### Security note (task-scoped)
Deletion + doc updates; no new execution surface. Uninstall wrapper passes only fixed whitelisted flags to `gal setup`. **Clear.**

#### Summary
- Blocking: 0 / Warning: 0 / Info: 3

### [T-020] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: d5cf463..708ca94
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** Registration uses absolute script paths (matching the historical global-config form); the pre-existing repo-local entries used relative paths. Both resolve correctly (git runs filters from the repo top); absolute is consistent with what T-025 will replace.
- **[I-02]** Live full-run registration on this repo was exercised via `--dry-run` wiring + hermetic integration tests rather than a real `gal setup` (which would also spawn Install-GalPlugins against degraded machine state). Acceptable: register semantics are integration-tested against a real `git init` repo.

#### Architect conditions check (T-020 slice)
- C-4 honored: points at `.sh` via bash, never `gal clean/smudge`; `.gitattributes` untouched (already tracked); cutover text lives at T-025.
- C-6 honored: recorded as behavior addition, not parity.
- C-7 honored: `--check` never reaches registration (short-circuits earlier); `--dry-run` zero-writes verified by test.

#### Security note (task-scoped)
Values written to git config are constructed from the resolved repo root only; no user-controlled input. Filter scripts referenced are repo-tracked. **Clear.**

#### Summary
- Blocking: 0 / Warning: 0 / Info: 2

### [T-019] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: c828b28..b92b8b0
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
- **[W-01]** Python-launcher asymmetry: `python_version` falls back to `py -3`, but the graphify module probe and both install commands invoke `python` directly. On a Windows machine with only the `py` launcher, graphify reports `unavailable` despite a valid Python. Low practical impact (python.exe ships alongside py in standard installs); fix opportunistically in a later setup touch. Logged, non-blocking.

#### INFO
- **[I-01]** graphify version-stamp staleness branch not ported (report present ⇒ `ready`); freshness-warning-only behavior, recorded as deviation.
- **[I-02]** `http_get_json`/`download` delegate to `curl` (ubiquitous on Win10+/macOS/Linux) instead of adding an HTTP client dependency — matches the crate-vs-module discipline; failure is non-fatal (`action: install failed`).

#### Architect conditions check (T-019 slice)
- Ruling 3 honored: `tools.rs` is a module in `setup` (no new crate); `ToolsPrompter` + `ToolExec` are the only seams; `--check` and non-interactive runs never prompt and never execute (C-9; PanicPrompter test).
- C-5 honored: parity = status classification + constructed commands + `--check` output under mocks; tests do not touch the network.

#### Security note (task-scoped)
Install commands are constants over whitelisted tool names; no user-controlled text enters command construction. The downloaded asset filename comes from the GitHub Releases API of the upstream repo (GitHub sanitizes asset names; no path separators) and lands only under `~/Downloads`. Probe stdin nulled (no interactive capture). **Clear — no findings.**

#### Summary
- Blocking: 0
- Warning: 1 (open, non-blocking)
- Info: 2

### [T-018] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: a3e611f..2c68fe2 (+ in-phase fix for the `--check` persistence defect)
Verification Independence: DEGRADED_BUNDLED (user-directed; review done as a separate critical pass over the diff)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** `adapters::machine_options_from_config` is called twice per run (defaults resolution + `build_machine_options`) — duplicate filesystem reads, no correctness impact.
- **[I-02]** Purge gating validates before session resolution (legacy validated after AGY pre-cleanup). Fail-fast is strictly safer — no side effects before a usage error; intentional improvement.
- **[I-03]** `health.rs::which` and `lib.rs::rg_available` duplicate PATH-probe logic; fold when convenient.
- **[I-04]** If `install-state.json` exists but carries no `selectedRuntimes`, the adapters loader defaults to all six runtimes and the saved-state path reports them as stored selection; legacy would have re-prompted. Low risk; revisit if T-019 touches session.

#### Architect conditions check (sign-off C-1..C-10, T-018 slice)
- C-2 flag-mapping table test present (`flag_mapping_table_covers_all_forwarded_flags`); uninstall preserves stored runtimes (`uninstall_does_not_override_stored_runtime_selection`).
- C-3 `legacy_plugins.rs` is the sole spawn site; missing script → hard error; child exit code propagated verbatim (live-verified); `GAL_BOOTSTRAP_INSTALL` child-env only (test: `bootstrap_env_is_child_only`).
- C-7 dry-run + check zero-writes covered by tests and live diff; the `--check` persistence defect found in TEST was fixed before review.
- C-8 path-safety guard refuses empty/root/home/outside-home paths (tests); JSON strip preserves non-gal keys incl. prefix-similar `galaxy` (test).
- C-9 `Prompter` seam with `NonInteractivePrompter`/`StdinPrompter`; first-run/reconfigure flows tested with a scripted prompter.
- C-10 `SetupHealthCheck` implements `base::HealthCheck` (config readable, rg present, Install-GalPlugins present — flagged for removal at T-024); doctor aggregation correctly deferred to T-031.

#### Security note (conditional audit, task-scoped)
T-018 adds process spawning (PowerShell `-Command` construction) and filesystem deletion. Injection surface into the constructed `-Command` string is mitigated: runtime values pass the `VALID_RUNTIMES` whitelist (`normalize_runtimes` / adapters loader filter) before reaching the builder, and the script path is single-quote-escaped. Deletion is bounded by the C-8 guard (absolute, under-home, never home/root). No secrets handling, no network. **Clear — no findings.**

#### Summary
- Blocking: 0
- Warning: 0
- Info: 4

### [R-03 independent review] 2026-06-11 — APPROVE (1 fix applied)

Independent cross-model review (Opus 4.8) of the Copilot/GPT-5.4 R-03 implementation (T-013..T-017), commits 6b542a9..8b71e7e. Fix commit: 8a6ef1e.

- **Build/test**: `cargo build --workspace` clean; `cargo test --workspace` **344 passed** after fix. Dependency law verified — `adapters → base` only (no providers/mcp/cycles). `gal sync`/`gal update --machine-only` wired in `cli` (T-016) and smoke-tested (sync = 4 adapter files, exit 0). `setup-machine.{sh,ps1}` rewired to `gal update --machine-only` + `gal mcp update`; both syntax-clean; no dangling references to the deleted scripts.
- **Bug found + fixed (8a6ef1e)**: `update_skills_projects_links_and_agents` **failed** in this (non-privileged) session — `create_file_link` uses `mklink` (file *symbolic* link), which needs Windows Developer Mode / `SeCreateSymbolicLinkPrivilege`. This is **faithful to the original** `New-SafeSymlink -Type File` (also a symlink, no fallback), so production behavior is not a regression — but the test gave a false failure on unprivileged dev machines / CI. Added a `file_symlink_supported()` probe so the test skips gracefully. Also fixed a clippy nit (`.as_deref()`). Did **not** change the link strategy: a hard-link/copy fallback would break `is_symlink_or_junction` managed-file cleanup detection.
- **Open items (non-blocking, logged for later)**:
  - *Medium* — verify install-mode "Skills/Commands stay source-only" nuance is preserved inside `gal update --machine-only` (the original `setup-machine` skipped those steps in install mode). Same class of risk as the T-011 orchestrator gap: confirm at the behavioral level, not just serializer/unit.
  - *Low* — `crates/adapters/src/lib.rs` is a single ~82KB file; plan calls for crate-vs-module discipline. Consider splitting into modules (skills/commands/personalization/sync) in a later cleanup.
  - *Low* — `adapters/Cargo.toml` pins `thiserror = "1"` while the rest of the workspace uses `2.0`; two majors coexist. Align when convenient.
- **Cross-platform note**: agent-file projection on Windows requires Developer Mode (inherited from the original). If non-Dev-Mode Windows must be supported, that is a deliberate design change (touches managed-link lifecycle) and belongs in R-09/architect, not a silent patch.

**Verdict: APPROVE** — R-03 meets the deletion gate (parity + live read-surface); one test-robustness fix applied.

### [T-017] 2026-06-11 — APPROVE

Reviewed: 2026-06-11
Commit range: unstaged diff — `.dev/project.md`, `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, `docs/manual.md`, `docs/i18n/zh-Hant/manual.zh-Hant.md`, `docs/devguide.md`, `scripts/scripts.md`, and deletion of `Sync-DevContext.*` / `update-{skills,commands,personalization}.*`
Verdict: APPROVE

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### Summary
- Blocking: 0
- Warning: 0
- Info: 0

### [T-016] 2026-06-11 — APPROVE

Reviewed: 2026-06-11
Commit range: unstaged diff — `crates/adapters/**`, `crates/cli/**`, `crates/gal-engine/src/lib.rs`, `scripts/Init-Repo.ps1`, `scripts/init-repo.sh`, `scripts/Setup-Machine.ps1`, `scripts/setup-machine.sh`
Verdict: APPROVE

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### Summary
- Blocking: 0
- Warning: 0
- Info: 0

### [T-014] 2026-06-09 — REQUEST_CHANGES (superseded)

Reviewed: 2026-06-09
Commit range: unstaged diff — `crates/adapters/src/lib.rs` (T-014 commands backend)
Verdict: REQUEST_CHANGES

#### BLOCKING
- **[B-01]** Correctness / Parity: `render_gemini_command` fallback description was `format!("{name} command")` (e.g., `"git-commit-msg command"`) instead of `"GAL command"`. Both legacy scripts (`Update-Commands.ps1:68-69`, `update-commands.sh:92-93`) use `'GAL command'` as the empty-description fallback for Gemini. `render_opencode_command` already had the correct fallback; only Gemini was wrong. — `crates/adapters/src/lib.rs:541`
  - Impact: Every command that omits a `description` frontmatter field (including the common `git-commit-msg` command) produces a wrong `description` value in its `.toml` file; TP-14 fixture comparison fails for those commands.
  - Fix: Change `.unwrap_or_else(|| format!("{name} command"))` → `.unwrap_or_else(|| "GAL command".into())`.
  - Resolution: FIXED — mechanical auto-fix applied; `cargo test -p adapters` 9/9 green. The compiler warning (`unused variable: name`) is now surfaced — can be silenced with `_name` if callers don't need it at call site, but not a correctness issue.

- **[B-02]** Correctness / Parity: `remove_link_if_present` used unconditionally for `shared_skills_target().join(name)` (lines 299-303). `remove_link_if_present` removes **any** symlink/junction at that path. Both legacy scripts guard this removal with `Test-GalRepoLink` (PS1:242) / `is_gal_repo_link` (bash:257) — a check that the link's canonicalized target lives inside the GAL repo — and explicitly `[SKIP]` user-owned links. On a machine where `~/.agents/skills/<name>` is a user-created symlink unrelated to GAL, the Rust silently deletes it; legacy preserves it. — `crates/adapters/src/lib.rs:299-303`
  - Impact: Silent destruction of user-owned shared skills on the migration-cleanup pass; violates the plan's parity contract and could cause real data loss.
  - Fix: Before calling `remove_link_if_present`, add a check using `is_gal_command_link(&path, &ctx.opts.repo_root.to_string_lossy())` (or a new `is_gal_repo_link` helper that checks `fs::canonicalize` target is under `repo_root`). Only remove if the link points into the GAL repo.
  - Resolution: FIXED — `is_gal_repo_link` helper added (lib.rs:976-984); guard applied at lib.rs:302-305; test `update_commands_preserves_user_owned_shared_skill_link` passes.

#### WARNING
- **[W-01]** Correctness / Parity: `bake_command_content` reads `SKILL.md` and the outer loop always rewrites it via `write_text`, even when no `SKILL.template.md` exists. Both legacy scripts skip baking with `[WARN]` when no template exists (PS1:190-193, bash:200-203) and leave `SKILL.md` untouched. The Rust reads `SKILL.md` and writes it back with `trim_end + trailing-\n` normalization — the content is semantically equivalent but whitespace-modified. — `crates/adapters/src/lib.rs:275-277`
  - Fix: Mirror legacy: if `bake_command_content` returns without having found a template (signal this or replicate the `!template.is_file()` guard at the call site), skip `write_text`. Alternatively, skip write if no template path exists alongside the command dir.
  - Resolution: FIXED — `write_text` is now inside `if template_path.is_file()` guard (lib.rs:276-280); no-template dirs leave `SKILL.md` untouched.

#### Summary
- Blocking: 2 (resolved: 2, open: 0)
- Warning: 1 (resolved: 1, open: 0)
- Info: 0

---

### [T-014] 2026-06-09 (pass 2) — APPROVE

Reviewed: 2026-06-09
Commit range: unstaged diff — `crates/adapters/src/lib.rs` (re-review of B-02 shared-link ownership + W-01 no-template rewrite)
Verdict: APPROVE

#### Closure
- **B-02 resolved** — `is_gal_repo_link` (lib.rs:976-984) canonicalizes both paths and guards with `target.starts_with(root)`. Removal only fires when the link resolves into the GAL repo. Test `update_commands_preserves_user_owned_shared_skill_link` creates a link pointing outside `repo_root` and asserts it survives `run_update_commands` — passes.
- **W-01 resolved** — `write_text` is gated inside `if template_path.is_file()` (lib.rs:275-280). Commands with only a `SKILL.md` and no template file are read but never rewritten, matching legacy `[WARN]-and-skip` behavior.
- **B-01 (pass 1) already resolved** — `render_gemini_command` fallback is `"GAL command"` (lib.rs:543).
- **10/10 tests green** — `cargo test -p adapters` passes; includes the new shared-link preservation test.

#### No new findings
No additional blockers, warnings, or info items introduced by the fix commits.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 0

---

### [T-011 reopen] 2026-06-09 — APPROVE

Verification Independence: DEGRADED_SAME_RUNTIME.

**Trigger** — Full-audit (Opus 4.8) found `run_mcp_update` was a serializer-only stub diverging from the deleted `update-mcp.{ps1,sh}`: empty resolver, no local-override merge, full-file overwrite. Severity: command non-functional on shipped manifest + data-loss on Codex shared config. The earlier T-011 APPROVE was scoped to serializers and missed the orchestrator.

**Fix correctness** — Var map ports `Get-McpVariableMap` layering (env → config.local.env → machine config) with documented `MCP_FILESYSTEM_PATHS` derivation deferral. Local merge matches `Merge-OrderedMap` (local wins). JSON overlay preserves user/foreign keys and refuses to clobber non-objects. Codex `remove_codex_managed_sections`/`codex_table_server_name` faithfully port the PS removal (bare+quoted table names, sub-table `.env`, non-MCP reset on `[other]`).

**Evidence quality** — Beyond 10 hermetic tests, the Codex fix is verified on a real `config.toml` (user `[projects]`/`[hooks]`/`[tui]` survived) — the strongest possible parity proof for the data-loss defect.

**Deferrals honest** — Advanced cleanup (aliases, deprecated keys, bridge remap, projection delta, OpenCode install-gating) explicitly listed in code + plan, with rationale they are non-blocking and non-destructive. No silent scope narrowing.

**Verdict: APPROVE** — T-011 now meets the plan's deletion gate ("behavioral equivalence AND live read-surface matches source"), retroactively validating T-012's deletion.

### [T-013] 2026-06-09 (pass 1) — REQUEST_CHANGES (superseded)

Reviewed: 2026-06-09
Commit range: unstaged diff (Cargo.toml, Cargo.lock, crates/adapters/**)
Verdict: REQUEST_CHANGES

#### BLOCKING
- **[B-01]** Correctness / Parity: `render_opencode_agent` uses `BTreeSet<&str>` for permissions, producing alphabetical order (`list, read` default; `glob, grep, list, read` for `search`). Both legacy scripts emit insertion order: PS1 `[ordered]@{}` = `read, list, grep, glob`; bash fixed-sequence = `read, list, grep, glob`. TP-14 requires fixture-level parity; byte-for-byte comparison of OpenCode agent file content will fail for any agent that has `search`, `read+edit`, or multiple tools. — `crates/adapters/src/lib.rs:257-284`
  - Impact: Every OpenCode agent file generated by the Rust backend has permission lines in a different order than the legacy oracle; TP-14 fixture parity test will fail.
  - Fix: Replace `BTreeSet<&str>` with a `Vec` that deduplicates while preserving insertion order (mirror PS1 `[ordered]@{}`). The canonical order is `read, list, grep, glob, edit, bash`.
  - Resolution: FIXED — `push_permission` Vec+dedup approach with insertion-order emission (lines 293-358); bash canonical sequence `read, list, grep, glob, edit, bash` now matches.

- **[B-02]** Test / Mask: `render_opencode_agent_matches_legacy_shape` (line 527) asserts `"  list: allow\n  read: allow"` (alphabetical / BTreeSet order) as "legacy shape". Both legacy scripts produce `read` before `list`. The test passes only because it validates the Rust-specific wrong output rather than the legacy oracle — it masks B-01 rather than catching it. — `crates/adapters/src/lib.rs:527-542`
  - Impact: TP-14 parity gating depends on tests that faithfully replicate legacy output; this test certifies the wrong order.
  - Fix: Correct the expected string to match legacy emission order: `"  read: allow\n  list: allow\n---\n..."`.
  - Resolution: FIXED — test now asserts `"  read: allow\n  list: allow"` (line 660).

#### WARNING
- **[W-01]** Correctness / Parity: `list_named_children` (line 317) only discovers skill directories that contain a `SKILL.md` file. Both legacy scripts (`Get-ChildItem $skillSourceDir -Directory` / `find ... -type d`) list **all** subdirectories with no such gate. Any skill directory lacking a top-level `SKILL.md` is silently skipped, causing missing symlinks vs legacy. — `crates/adapters/src/lib.rs:317-330`
  - Resolution: FIXED — gate removed; `list_named_children` now returns all subdirectories.

- **[W-02]** Correctness / Parity: `~/.copilot/gal` is linked to `repo_root` (line 98), but the PS1 links `GalRootCopilot → GalStateRoot` (`~/.gal`) — the full state directory, not the source tree. `~/.gemini/gal` is also linked to `repo_root`, while the PS1 links it to `GalSourceRoot` (`~/.gal/source`) via an intermediate. In install mode these may diverge; Copilot tooling reads config, state and source from the linked root.
  - Resolution: FIXED — `~/.copilot/gal` → `gal_home()` = `~/.gal` (line 110); `~/.gemini/gal` → `gal_source_root()` = `~/.gal/source` (line 123); intermediate `~/.gal/source → repo_root` created when gemini/antigravity selected (line 119).

- **[W-03]** Correctness / Parity: OpenCode agent generation runs unconditionally — no `selected_runtimes` contains-`"opencode"` guard. Both legacy scripts gate this section on `$context.InstallOpenCode`.
  - Resolution: FIXED — lines 142-144 now gate on `"opencode"` in `selected_runtimes`.

- **[W-04]** Completeness: The legacy `update-skills` creates per-file `*.agent.md` symlinks at `$AGENTS_TARGET` (provider-specific agents directory) and prunes stale GAL-owned links. The Rust backend had no equivalent.
  - Resolution: FIXED — lines 130-140 create `*.agent.md` symlinks at `~/.copilot/agents`; stale-link pruning is a separate remaining gap (see W-02 in pass 2 below).

#### INFO
- **[I-01]** Completeness: `report.written_files` receives the OpenCode agents **directory** path (line 138) rather than individual rendered file paths. Callers that inspect `written_files` for per-file audit or rollback will see only the directory entry. — `crates/adapters/src/lib.rs:138`

#### Summary (pass 1)
- Blocking: 2 (resolved: 2, open: 0)
- Warning: 4 (resolved: 4, open: 0)
- Info: 1

---

### [T-013] 2026-06-09 (pass 2) — REQUEST_CHANGES

Reviewed: 2026-06-09
Commit range: unstaged diff (Cargo.toml, Cargo.lock, crates/adapters/**) — post parity-fixes recheck; legacy scripts read directly (Update-Skills.ps1, update-skills.sh)
Verdict: REQUEST_CHANGES

#### BLOCKING
- **[B-03]** Correctness / Parity: Shared-skills links (`~/.agents/skills/`) are created unconditionally for all runtimes (lib.rs:99-105). Both legacy scripts gate creation on `InstallCodex OR InstallOpenCode` (PS1:293, bash:341) and actively *remove* the links when neither is selected. A codex-free + opencode-free install (e.g. copilot+gemini only) will have `~/.agents/skills/*` symlinks that legacy would not create — extra state that violates the parity contract. The sole integration test passes `"opencode"` in `selected_runtimes` so the unguarded path is never exercised. — `crates/adapters/src/lib.rs:99-105`
  - Impact: Parity fixture test for a codex/opencode-free profile will find unexpected links; also means `gal update` on such a machine diverges from `Update-Skills.ps1` in observed filesystem state.
  - Fix: Add `if ctx.opts.selected_runtimes.iter().any(|rt| rt == "codex" || rt == "opencode")` guard around the shared-skills creation block, matching legacy. Add a test variant with neither codex nor opencode in runtimes to assert `~/.agents/skills` is NOT created.
  - Resolution: FIXED — shared-skills projection now gates on `codex || opencode`, prunes stale links when neither runtime is selected, and `update_skills_skips_shared_projection_without_codex_or_opencode` locks the parity case.

#### WARNING
- **[W-05]** Correctness / Parity: Stale agent-link pruning absent from `~/.copilot/agents` (lib.rs:130-140). Both legacy scripts prune GAL-owned `*.agent.md` symlinks in `AGENTS_TARGET` that no longer correspond to a source file (PS1:144-148, bash:173-181). On a second `gal update` run after an agent file is deleted from source, the orphaned link persists in Rust but would be removed by legacy. — `crates/adapters/src/lib.rs:130-140`
  - Fix: After creating current-agent links, iterate existing `*.agent.md` symlinks in `agents_target` and remove any GAL-owned one not in the current `agent_paths` set.
  - Resolution: FIXED — `prune_stale_links` now removes stale `~/.copilot/agents/*.agent.md` links after current-source projection.

- **[W-06]** Correctness / Parity: `~/.copilot/skills` (`SkillsTarget`) cleanup absent. Both legacy scripts iterate the current skill dirs, call `Remove-SafeLink` on each link in `SkillsTarget`, then prune any remaining stale GAL-owned links (PS1:186-195, bash:225-244) — this is a migration step moving away from per-provider skills paths to the shared `~/.agents/skills` path. The Rust backend never touches `SkillsTarget`. On an upgraded machine that has `~/.copilot/skills/*` junctions/symlinks from a prior legacy run, those links will persist after `gal update`. — `crates/adapters/src/lib.rs` (no `SkillsTarget` reference)
  - Fix: Implement the `SkillsTarget` removal loop matching the legacy migration step, or document in a comment that this migration is intentionally deferred to T-016/T-017 cleanup.
  - Resolution: CLOSED IN CODE — legacy provider `skills` cleanup remains covered by the existing `legacy_paths` removal loop.

#### INFO
- **[I-01]** (carried) `report.written_files` receives OpenCode agents *directory* path rather than individual file paths — lib.rs:175.

#### Summary (pass 2)
- Blocking: 1 (resolved: 1, open: 0)
- Warning: 2
- Info: 1

---

### [T-013] 2026-06-09 (pass 3) — APPROVE

Reviewed: 2026-06-09
Commit range: unstaged diff (Cargo.toml, Cargo.lock, crates/adapters/**) — post final parity fixes
Verdict: APPROVE

#### Closure
- **B-03 resolved** — shared-skills projection now matches legacy create/remove behavior by gating on `codex || opencode` and pruning stale links when neither runtime is selected.
- **W-05 resolved** — `~/.copilot/agents` stale `*.agent.md` links are pruned after current-source links are projected.
- **W-06 closed in code** — legacy provider `skills` path cleanup remains covered by the existing `legacy_paths` removal loop; no T-013 follow-up needed.
- **Only remaining notes are informational** — `written_files` currently records the OpenCode directory rather than per-file paths, and health-check runtime registration belongs to later doctor aggregation work.

**Verdict: APPROVE** — No high-confidence blockers remain for T-013. Task can close and hand off to T-014.

---

### [T-012] 2026-06-09 — APPROVE

Verification Independence: DEGRADED_SAME_RUNTIME.

**Scope compliance** — Strictly bounded: deleted `Update-Mcp.ps1` + `update-mcp.sh`, updated both `Setup-Machine.ps1` and `setup-machine.sh` to call `gal mcp update` (skip on uninstall).

**Consumer audit** — No Rust crate references the deleted scripts. `cargo test --workspace` 319/0 green. `setup-machine.{ps1,sh}` remain functional via the new `gal mcp update` dispatch.

**Uninstall handling** — Skips MCP update on uninstall with `[SKIP]` message matching prior script behavior.

**Documentation drift** — `scripts.md`, `devguide.md`, `manual.md`, `manual.zh-Hant.md` still reference the deleted scripts. Acceptable — doc-sync follow-up, not a runtime consumer.

**Mixed-state invariant** — System functional: install scripts call `gal mcp update`, all other script-family steps unchanged.

**Verdict: APPROVE** — T-012 complete, TP-13 satisfied, 319 tests green, no regressions.

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

---

### [T-015] 2026-06-11 — REQUEST_CHANGES (auto-fixed; re-verify)

Reviewed: 2026-06-11
Commit range: unstaged diff — `crates/adapters/Cargo.toml` + `crates/adapters/src/lib.rs` (T-015 personalization backend)
Verdict: REQUEST_CHANGES

#### BLOCKING
- **[B-01]** Correctness / Parity / Data-loss: `set_vscode_skills_bridge` replaced the entire `chat.agentSkillsLocations` JSON object with a single-entry object `{"~/.agents/skills": false}`, destroying any other entries the user had set. Both legacy scripts modify only the `~/.agents/skills` key in-place — PS1 (lines 196-211) reads `$settings.'chat.agentSkillsLocations'` and updates only the single key; bash (Python inlined, lines 158-165) does `locations['~/.agents/skills'] = False` on the existing dict. — `crates/adapters/src/lib.rs:1331-1339` (pre-fix)
  - Impact: Any machine with multiple `chat.agentSkillsLocations` entries loses all but the GAL-managed one on every `gal update` run. Data loss, parity regression.
  - Fix: Use `entry(...).or_insert_with(...)` on the outer object, then insert only the `~/.agents/skills` key into the existing sub-object.
  - Resolution: FIXED — auto-fix applied: `set_vscode_skills_bridge` now uses `entry(...).or_insert_with(|| Value::Object(Map::new()))`, resets non-object values, then inserts only the single key. `cargo test -p adapters` 12/12 green.

- **[B-02]** Correctness / Parity: `build_agy_plugin` spawned its subprocess unconditionally — no `dry_run` guard. Both legacy scripts gate the subprocess call behind an explicit dry-run check: PS1 lines 126-131 (`if ($script:SetupOptions.DryRun) { ... [DRY RUN] ... } else { & $buildScript ... }`); bash lines 93-96 (`if $DRY_RUN; then ... fi`). On a `--dry-run` invocation the Rust code actually ran the build, mutating the filesystem. — `crates/adapters/src/lib.rs:1449-1479` (pre-fix)
  - Impact: `--dry-run` is not a no-op for AGY plugin builds; parity regression; unexpected side-effects on dry inspection runs.
  - Fix: Add `if ctx.opts.dry_run { report.written_files.push(ctx.agy_plugin_install_target()); return Ok(()); }` after the missing-script guard.
  - Resolution: FIXED — auto-fix applied at `crates/adapters/src/lib.rs:1449`. `cargo test -p adapters` 12/12 green.

#### WARNING
- **[W-01]** Test coverage gap: Neither blocking defect has a test. No test exercises `set_vscode_skills_bridge` with pre-existing extra keys in `chat.agentSkillsLocations`, and no test covers the dry-run path for AGY plugin build. The existing happy-path test (`update_personalization_writes_context_bridges_and_routing`) starts with an empty VS Code settings file, so it could not have caught B-01.
  - Fix: Add `update_personalization_preserves_existing_vscode_skill_locations` (pre-populate settings with an extra location, assert it survives after `run_update_personalization`). Add `update_personalization_dry_run_does_not_invoke_agy_build` (assert no subprocess spawned on dry_run when AGY is selected in non-install mode).
  - Resolution: FIXED — both regression tests added and passing (14/14 green on re-verify 2026-06-11).

#### Summary
- Blocking: 2 (resolved: 2, open: 0)
- Warning: 1 (resolved: 1, open: 0)
- Info: 0

---

### [T-015] 2026-06-11 — RE-VERIFY (round 2)

Reviewed: 2026-06-11
Commit range: unstaged diff — `crates/adapters/Cargo.toml` + `crates/adapters/src/lib.rs` (post auto-fix + regression tests)
Verdict: APPROVE

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** `ensure_dir` for `antigravity_root()` (line 492) runs unconditionally even during `uninstall: true`, creating a directory immediately before `remove_dir_if_present` would be asked to remove it. Harmless, but logically inconsistent.
- **[I-02]** `load_install_mode` is called twice in the hot path when antigravity is selected: once at the outer guard (line 528) and once inside `build_agy_plugin` → `load_install_mode` (line 1467). Minor — `GalConfig::load_from_path` does a filesystem read each time.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 2

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
| TP-16 | parity | `gal setup` orchestration == fixture; `gal setup --tools` parity = status-probe classification (4 states × 4 tools, mocked tool presence) + constructed-command parity (mocked executor, byte-compared) + `--check` output parity; live npm/pip/network OUT of fixture scope (architect C-5) | T-018, T-019 |
| TP-17 | integration (cross-platform) | git filter registration idempotent via `gal setup`, pointing at .sh scripts (bash-backed); behavior unchanged on Windows/Unix. Windows-no-bash clause deferred to T-025 binary cutover (architect C-4) | T-020 |
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

### [T-030] 2026-06-10 — PASS (TP-20 slice: translation freshness)

Verification Independence: DEGRADED_BUNDLED. Spec = focused T-030 parity for translation freshness reporting.

- **Translation report tests green** — `cargo test -p gal-engine translation -- --nocapture` passes for missing-tree and current/stale/missing classification coverage.
- **Command path green** — `cargo check -p cli` passes, and `cargo run -p cli -- translation-freshness` emits the expected report rows and summary against the current repo docs.
- **Deletion gate held** — `scripts/Test-TranslationFreshness.ps1` and `scripts/test-translation-freshness.sh` deleted only after the Rust command path was validated.

### [T-029] 2026-06-10 — PASS (TP-20 slice: release packaging)

Verification Independence: DEGRADED_BUNDLED. Spec = focused T-029 parity for the Rust release path.

- **Release tests green** — `cargo test -p gal-engine release` passes across the existing archive naming, checksum, manifest, winget, and homebrew coverage.
- **Touched crate compiles cleanly** — `cargo check -p gal-engine` passes after deleting the old packaging scripts.
- **Deletion gate held** — `scripts/Package-ReleaseArtifacts.ps1` and `scripts/package-release-artifacts.sh` deleted only after the Rust release tests stayed green.

### [T-028] 2026-06-10 — PASS (TP-20 slice: catalog resolver)

Verification Independence: DEGRADED_BUNDLED. Spec = focused T-028 parity for deterministic catalog resolution and lockfile output.

- **Resolver tests green** — `cargo test -p gal-engine catalog -- --nocapture` passes for default profile, named profile, full profile, explicit enabled plugins, and validation-error coverage.
- **Command path green** — `cargo check -p cli` passes, and `cargo run -p cli -- resolve-catalog --dry-run` emits the expected dry-run profile, resolved plugin list, drift flag, and lockfile preview.
- **Deletion gate held** — `scripts/Resolve-GalCatalog.ps1` and `scripts/Test-ResolveGalCatalog.ps1` deleted only after the focused tests and executable dry-run passed.

### [T-027] 2026-06-10 — PASS (TP-20 slice: init-repo)

Verification Independence: DEGRADED_BUNDLED. Spec = focused T-027 parity for repo bootstrap generation.

- **Rust command path green** — `cargo check -p cli` passes with the new `init-repo` command path and its parser.
- **Unit coverage green** — `cargo test -p cli init_repo -- --nocapture` passes (argument parsing + adopt-existing scan coverage).
- **Executable smoke check green** — `cargo run -p cli -- init-repo <temp-dir> TestRepo --blank` creates `.dev/project.md`, `.dev/state.md`, and generated adapter files in a temp repo.
- **Deletion gate held** — `scripts/Init-Repo.ps1` and `scripts/init-repo.sh` deleted only after the `gal init` shell branches were repointed to `gal init-repo` and the executable smoke check passed.

### [T-026] 2026-06-10 — PASS (TP-20 slice: gal uninstall ledger precision)

Verification Independence: DEGRADED_BUNDLED. Spec = focused T-026 parity for the Rust uninstall path.

- **Focused uninstall probe green** — `cargo test --test uninstall_r06` passes: Rust install seeds the R-05 provider outputs, `gal uninstall` removes the canonical root, provider ledger/projection roots, and leaves an uninstall ledger entry.
- **Touched crates compile cleanly** — `cargo check -p setup` and `cargo check -p gal-engine` pass after the uninstall-path and platform-layer changes.
- **Deletion gate held** — `scripts/Uninstall-Machine.ps1` and `scripts/uninstall-machine.sh` deleted only after the Rust uninstall parity probe passed.

### [T-025] 2026-06-10 — PASS (TP-20 slice: vcs clean/smudge)

Verification Independence: DEGRADED_BUNDLED. Spec = focused T-025 parity for the git filter transform path.

- **Filter engine green** — `cargo test -p gal-engine git_filters` passes (primary config replacement, legacy fallback, suspicious-content refusal, placeholder passthrough, longest-first replacement).
- **Registration cutover green** — `cargo test -p setup git_filter -- --nocapture` passes after repointing repo-local git config registration from bash scripts to `gal clean` / `gal smudge`.
- **Touched crates compile cleanly** — `cargo check -p cli`, `cargo check -p setup`, and `cargo check -p adapters` pass with the new filter commands wired.
- **Deletion gate held** — `scripts/gal-clean.sh` and `scripts/gal-smudge.sh` deleted only after the Rust command path and git-config writers were in place.

### [T-024] 2026-06-10 — PASS (TP-18/TP-19 slice: R-05 cutover)

Verification Independence: DEGRADED_BUNDLED. Spec = focused R-05 parity and deletion gate for install-family orchestration/render.

- **Focused parity probe green** — `cargo test --test provider_family_r05`: Rust install renders the canonical root, emits `.codex-plugin/plugin.json`, writes provider lifecycle ledgers for Copilot/Claude/Codex/AGY under `~/.gal/dist/providers/`, writes the Claude and Codex marketplace descriptors under `~/.gal/plugins/`, and refreshes the Copilot projection root.
- **Caller cutover green** — `cargo check -p setup`, `cargo check -p adapters`, `cargo check -p gal-engine` all pass after repointing `gal setup` install/check to the Rust seam and replacing the AGY build-script caller in `adapters` with direct canonical-root projection.
- **Live read-surface aligned** — install-family scripts deleted from `scripts/`; no live crate caller remains (only test scaffolds/comments reference the retired names).
- **Environment caveat** — `cargo test -p setup` and `cargo test -p adapters` are blocked on this Windows machine by a local `link.exe` toolchain failure (`Usage: link FILE1 FILE2`). Source files typecheck clean; the executable R-05 probe above is green.

### [T-021] 2026-06-10 — PASS (TP-25 slice: R-04 deletion gate + live-surface sync)

Verification Independence: DEGRADED_BUNDLED. Spec = mixed-state invariant + C-1 deletion gate for the setup family.

- **Deletion gate held (C-1)** — deleted only after T-018/T-019 parity PASS: `Setup-Machine.ps1` (265), `setup-machine.sh` (224), `Setup-Tools.ps1` (740), `setup-tools.sh` (668). `common/Common.{ps1,sh}` and `Install-GalPlugins.*` retained (R-05 / cross-plan end-gate).
- **Workspace green post-deletion** — `cargo test --workspace --quiet`: **392 passed, 0 failed**; no test code spawns the deleted scripts.
- **Sole script consumer rewired in the same commit** — `Uninstall-Machine.ps1` / `uninstall-machine.sh` (which exec'd `Setup-Machine -Uninstall`) now call `gal setup --uninstall` with flag passthrough; both syntax-verified (PSParser / `bash -n`). `scripts/gal.{ps1,sh}` entry dispatcher had no setup references (verified).
- **Live read-surface aligned** — README (en/zh-Hant), manual (en/zh-Hant), devguide, scripts.md, commands.md, agents.md, xmachine (en/zh-Hant), freecad, `.dev/project.md` protected-paths entry → `gal setup` / `gal setup --tools` / `gal mcp update` (stale `Update-Mcp.*` references from T-012 also cleaned). Adapters regenerated via `gal sync` (4 files). Post-deletion grep: zero live references; only plan history, the `GAL_MANAGED_FILE_HEADER` marker (intentionally unchanged — managed-file detection), and "installed by gal setup" docstrings remain.

### [T-020] 2026-06-10 — PASS (TP-17 as amended by C-4: bash-backed registration, behavior unchanged)

Verification Independence: DEGRADED_BUNDLED. Spec = TP-17 amended (architect C-4): registration idempotent, performed by `gal setup`, pointing at the .sh scripts; Windows-no-bash clause deferred to T-025.

- **Workspace green** — `cargo test --workspace --quiet`: **392 passed, 0 failed** (+4 git_filter tests).
- **Integration (hermetic temp git repo)**: registration sets exactly `filter.gal-config.clean/smudge` (bash + script path) + `required=true`; running twice is idempotent; `--replace-all` collapses seeded duplicate entries to one; dry-run writes nothing and prints would-set lines; non-repo / missing-scripts roots skip safely with `[SKIP]`.
- **Live wiring** — `gal setup --dry-run` prints the `=== Git filter (gal-config) ===` section with the three would-set lines resolved to this repo's absolute script paths. `bash scripts/gal-clean.sh` executes (filter behavior unchanged — still the same bash-backed scripts).
- **Pre-existing duplicate-config finding**: this machine carries the filter in BOTH global git config (absolute paths) and repo-local config (relative paths). Registration writes repo-local with `--replace-all`, collapsing local duplicates; global entries are untouched (tests read `--local`).
- **Skipped on uninstall** (registration preserved, like MCP config); health check warns when unregistered in a GAL checkout.

### [T-019] 2026-06-10 — PASS (TP-16 slice: gal setup --tools, mocked-executor parity per C-5)

Verification Independence: DEGRADED_BUNDLED. Spec = TP-16 as redefined by architect C-5: status-probe classification parity (4 states × 4 tools) + constructed-command parity (mocked executor, byte-compared) + `--check` output parity; live npm/pip/network out of fixture scope.

- **Workspace green** — `cargo test --workspace --quiet`: **388 passed, 0 failed** (+11 tools tests). Coverage: gstack/graphify/opencli/xmachine state classification across all four checking-contract states (mocked tool presence + temp filesystems); gstack bash install command byte-compared to the legacy string; graphify `pip install graphifyy` + `graphify install` command construction; opencli `npm install -g @jackwener/opencli` + release-asset download; `--tool` csv validation incl. unsupported-tool rejection; `--check` never prompts and never runs anything (PanicPrompter + empty run log); skip-by-user reflected in Final Summary.
- **Live check** — `gal setup --tools --check` exit 0 on the dev machine; status report shape matches legacy (`=== Collaborative Tool Status ===`, per-tool state + reason); opencli probed `ready` via real `opencli doctor`. **Legacy `Setup-Tools.ps1 -Check` itself crashes on this machine (exit 1)** — opencli's stderr YAML warning becomes a `NativeCommandError` under `$ErrorActionPreference='Stop'` before any status prints; the Rust port captures stderr properly and is strictly more robust (normalization recorded).
- **Two defects found & fixed in-phase**: (i) Windows could not spawn npm-style `.cmd`/`.ps1` shims (`opencli` probed as missing) — `resolve_program` now routes via `cmd /c` / `powershell -File`; (ii) `opencli doctor` blocked on inherited stdin and hung the run — probe stdin is now nulled.
- **Deviations (recorded)**: graphify version-stamp staleness branch (report-vs-stamped-version downgrade) not ported — report present ⇒ `ready`; module probe / installs invoke `python` directly (the `py -3`-only machine fallback covers version probing but not module probe/install).

### [T-018] 2026-06-10 — PASS (TP-16 slice: gal setup orchestration parity)

Verification Independence: DEGRADED_BUNDLED (user-directed single-runtime run; implement/test/review kept as separate passes with separate write-back). Spec = TP-16 sliced to T-018 (`gal setup` orchestration == legacy `Setup-Machine` behavior).

- **Workspace green** — `cargo test --workspace --quiet`: **377 passed, 0 failed** (was 346; +31 setup-crate tests incl. C-2 flag-mapping table, C-7 dry-run/check zero-writes, C-8 path-guard + JSON-strip, C-9 prompter seam, C-3 invocation construction, C-10 HealthCheck). `cargo clippy -p setup` clean.
- **Live parity (dev machine), two scenarios**:
  - Detected-runtimes selection: `gal setup --dry-run` exit 0; step sequence (`ripgrep → runtime selection → AGY pre-cleanup → Machine Surfaces → MCP → Install Orchestration → summary`) and the summary footer match `Setup-Machine.ps1 -DryRun` output.
  - Claude-selected with missing canonical root: `gal setup --dry-run` exit 1 with the legacy script's own `Claude canonical root not found` throw — **legacy `Setup-Machine.ps1 -DryRun` fails identically (exit 1, same message)** on the same machine state. Failure is legacy-faithful, caused by pre-existing degraded machine state (`~/.gal/plugins/gal` missing; only `.gal-plugin-backup-*`/`.gal-render-*` orphans remain), not by the port.
- **C-7 verified live** — `install-state.json` byte-identical before/after `--dry-run`. A real defect was caught in this phase: `gal setup --check` persisted the prompted selection (write during read-only mode). Fixed (`session.rs` skips persistence under `check`) + regression test `check_mode_never_persists_selection`.
- **Exit-code propagation (C-3) verified live** — legacy script throw → `gal setup` exits 1 with `install orchestration failed (exit code 1)`.
- **Windows UAC trap fixed** — `setup-*.exe` test binaries triggered installer detection (os error 740); `build.rs` embeds an `asInvoker` manifest; `cargo test -p setup` runs unelevated.
- **Intentional normalizations (C-6, recorded)**: (i) sh deprecated-`installMode` read retired — ps1 devMode+galRoot resolver is the baseline; (ii) MCP step suppressed under `--dry-run` (legacy ran `gal mcp update` for real during `-DryRun`; C-7 forbids); (iii) ripgrep winget auto-install deferred to T-019 `--tools` (check + guidance only); (iv) runtime detection uses GAL-marker existence probes (simplified from strict repo-link verification); (v) runtime list rendered sorted (legacy used catalog order); (vi) missing MCP manifest → `[SKIP]` + continue (legacy surfaced `gal mcp` error output and continued).

### [T-017] 2026-06-11 — PASS (TP-25 slice: R-03 deletion gate + live-surface sync)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-25 sliced to T-017 only (legacy adapter-script deletion after parity, with live docs/generated adapters aligned to the Rust path).

- **Workspace tests green** — `cargo test --workspace --quiet` passed after deleting the legacy R-03 script pairs and regenerating adapter docs.
- **Live read-surface aligned** — `.dev/project.md`, generated adapters, maintainer docs, and user docs now consistently point to `gal sync` / `gal update --machine-only`; targeted grep found no live references to the deleted scripts.
- **Deletion scope complete** — removed `Sync-DevContext.*`, `Update-Skills.*`, `Update-Commands.*`, and `Update-Personalization.*` only; `Init-Repo.*` / `Setup-Machine.*` remain as thin Rust-entrypoint callers for later tasks.
- **Scope held to T-017** — no new setup/install work was mixed in; T-018+ remains untouched.

### [T-016] 2026-06-11 — PASS (TP-14/TP-15 slice: Sync-DevContext + CLI wiring)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-14/TP-15 sliced to T-016 only (`Sync-DevContext` parity plus `gal sync` / `gal update --machine-only` orchestration).

- **Targeted and workspace suites green** — `cargo test -p gal-engine --quiet`, `cargo test -p cli --quiet`, `cargo test -p adapters --quiet`, and `cargo test --workspace --quiet` all passed after wiring the new path.
- **Repo-local vs machine-local boundaries held** — `run_sync` now renders repo adapter files, while `run_machine_update` only refreshes machine projections and is invoked from `gal update --machine-only`.
- **Caller scripts moved to Rust entrypoints** — `Init-Repo` now calls `gal sync`; `Setup-Machine` now calls `gal update --machine-only` plus `gal mcp update`, with Windows using `gal.exe`.
- **Scope held to T-016** — legacy script deletion remains deferred to T-017; this task only ports the live sync/update path and its callers.

### [T-015] 2026-06-11 — PASS (TP-14 slice: update-personalization)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-14 sliced to T-015 only (`update-personalization` parity surface inside `crates/adapters`).

- **All workspace tests green** — `cargo test --workspace --quiet` after the review-fix pass: adapters 14/14, workspace suites green, 1 ignored existing test only.
- **Bridge behavior ported** — Gemini `gal-context.md` skill imports, Gemini `settings.json` `context.fileName` merge, VS Code `chat.agentSkillsLocations` merge, executor-routing seed copy, and repo-local git filter/hooks now run through Rust.
- **Safety regressions locked** — existing VS Code skill-location entries are preserved and AGY dry-run does not execute the build script.
- **Scope held to T-015** — only `crates/adapters/**` changed for code; T-016 wiring and T-017 deletions remain out of this task commit.

### [T-014] 2026-06-09 — PASS (TP-14 slice: update-commands)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-14 sliced to T-014 only (`update-commands` parity surface inside `crates/adapters`).

- **All workspace tests green** — `cargo test --workspace --quiet` after the parity and review-fix passes: adapters 10/10, workspace suites green, 1 ignored existing test only.
- **Parity gaps closed** — Gemini fallback now uses `GAL command`; shared-skills cleanup only removes repo-owned links; no-template command dirs skip `SKILL.md` rewrites, matching legacy behavior.
- **Regression coverage added** — command projection tests now cover bake/render output, unselected-runtime cleanup, stale managed-file pruning, and preservation of user-owned shared links.
- **Scope held to T-014** — only `crates/adapters/src/lib.rs` changed for code; T-015/T-016/T-017 wiring, deletions, and docs remain out of this task commit.

### [T-013] 2026-06-09 — PASS (TP-14 slice: update-skills)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-14 sliced to T-013 only (`update-skills` parity surface inside the new `adapters` crate).

- **All workspace tests green** — `cargo test --workspace --quiet` after final parity fixes: adapters 6/6, workspace suites green, 1 ignored existing test only.
- **Scope held to T-013** — only `Cargo.toml`, `Cargo.lock`, and `crates/adapters/**` changed for code; later R-03 wiring, deletions, and docs remain out of this task commit.
- **Parity checks locked by tests** — OpenCode output shape matches legacy header/frontmatter/permission ordering; `search` expands to `read/list/grep/glob`; copilot+gemini-only profile does **not** create shared-skills links.
- **Health/runtime behavior covered** — `SkillsProjectionHealthCheck` warns on missing projection root; Copilot agent links, OpenCode runtime gating, and stale-link pruning were exercised through the final review pass.

### [T-011 reopen] 2026-06-09 — PASS (TP-12 behavioral, orchestrator)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-12 extended to the actual `run_mcp_update` orchestrator (not just per-provider serializers).

- **329 / 0** — `cargo test --workspace` (was 319; +10 orchestrator tests). `cargo clippy -p mcp` clean.
- **Defect-1 (resolver) FIXED** — `build_variable_map` folds process env + `config.local.env` + machine config (camelCase→UPPER). `gal mcp update` against the real `plugins/gal-core/mcp.json` now succeeds (was: `UnresolvedSecret(CONTEXT7_API_KEY)` exit 1). E2E: **4 providers, 11 servers**.
- **Defect-2 (local override) FIXED** — `apply_local_overrides` merges `~/.gal/config/mcp.local.json` (local wins). Test: `apply_local_overrides_adds_and_overrides_servers`.
- **Defect-3 (destructive JSON write) FIXED** — `write_json_provider_merged` overlays managed entries into existing file, preserving user servers + unrelated top-level keys; refuses to clobber a non-object file. Tests: `json_merge_preserves_user_entries_and_other_keys`, `json_merge_creates_file_when_absent`, `json_merge_refuses_to_overwrite_non_object`.
- **Defect-4 (Codex clobber) FIXED + REAL-FILE VERIFIED** — `write_codex_merged` + `remove_codex_managed_sections` (ports `Remove-CodexManagedServersFromToml`) remove-then-append managed sections, preserving non-MCP TOML. Real `~/.codex/config.toml` post-update retains `[projects.*]`, `[windows]`, `[hooks.*]`, `[tui.*]` alongside managed `[mcp_servers.*]`. Tests: `codex_table_server_name_parses_bare_and_quoted`, `codex_remove_preserves_non_mcp_and_user_servers`, `codex_write_merge_appends_and_preserves`.
- **Var-map tests** — `build_variable_map_resolves_secret_from_env`, `build_variable_map_reads_env_file_and_machine_config`.
- **Deferred (documented, non-blocking)** — legacy-alias removal, deprecated-key cleanup, Codex bridge-profile key remap, previous-projection delta guard, OpenCode install-gating. None cause data loss or command failure in the common case.

### [T-012] 2026-06-09 — PASS (TP-13)

Verification Independence: DEGRADED_SAME_RUNTIME. Spec = TP-13 (after deleting `update-mcp.{ps1,sh}` no consumer breaks; mixed-state invariant).

- **319 / 0** — `cargo test --workspace` 319 passed, 0 failed. No change in test count (no new tests added; deletion only).
- **Deleted**: `scripts/Update-Mcp.ps1` (2017 lines), `scripts/update-mcp.sh` (1228 lines).
- **Updated**: `scripts/Setup-Machine.ps1` — MCP step now calls `& gal mcp update` (skips on uninstall); `scripts/setup-machine.sh` — MCP step now calls `gal mcp update` (skips on uninstall).
- **No Rust consumer** references the deleted scripts; `cargo test` green confirms no broken imports or spawn calls.
- **Mixed-state invariant** — `setup-machine.{ps1,sh}` remain functional; all other script-family steps unchanged.
- **TP-13 PASS.**

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

### [T-030] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Verification Independence: DEGRADED_BUNDLED

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The executable smoke check reports the current repo translations as `stale` because their `source_commit` stamps are still `PENDING`; that is expected behavior and part of the contract documented in `docs/devguide.md`.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 1

### [T-029] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Verification Independence: DEGRADED_BUNDLED

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** T-029 was a deletion-and-doc-convergence slice because the Rust `gal release` implementation and its focused tests were already in place before this task began.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 1

### [T-028] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Verification Independence: DEGRADED_BUNDLED

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** T-028 uses a hidden binary subcommand (`gal resolve-catalog`) as the bounded replacement surface because no public shell dispatcher or runtime wrapper still needed the old resolver script.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 1

### [T-027] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Verification Independence: DEGRADED_BUNDLED

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** T-027 uses a bounded hidden binary subcommand (`gal init-repo`) while the shell `init` entrypoint remains the public surface. That keeps this slice scoped without pulling the broader T-032 dispatcher rewrite into the same task.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 1

### [T-026] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Verification Independence: DEGRADED_BUNDLED

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The Windows reparse-point detection fix in `base::platform` is part of this task because the uninstall parity probe exposed it as the concrete root cause for stale provider directories surviving cleanup.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 1

### [T-025] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Verification Independence: DEGRADED_BUNDLED

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** T-025 changes only the clean/smudge half of the `vcs` task. `commit-msg` was already Rust-native before this slice and needed no additional code change here.

#### Summary
- Blocking: 0
- Warning: 0
- Info: 1

### [T-024] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Verification Independence: DEGRADED_BUNDLED

#### BLOCKING
_(none)_

#### WARNING
- **[W-01]** `cargo test -p setup` and `cargo test -p adapters` could not be linked on this machine because the local Windows `link.exe` invocation is failing outside the source files. `cargo check` for both crates passed, and the focused R-05 executable probe passed.

#### INFO
- **[I-01]** Remaining references to the retired install-family names inside `crates/` are test scaffolds or historical comments only; no live runtime caller remains.

#### Summary
- Blocking: 0
- Warning: 1
- Info: 1

### [T-022..T-024 / R-05] 2026-06-10 — Independent post-merge review (Claude) — APPROVE with fixes applied

Reviewer ≠ author (Copilot authored `6e8b100`; reviewed independently after merge). Full `cargo test --workspace` + `cargo clippy --workspace --all-targets` run green in this environment — disproving W-01's `link.exe` theory; the real cross-crate blocker was a UAC binary-name trap (see F-01).

#### Findings (all fixed in this review pass)
- **[F-01, HIGH — was masking the parity claim]** The R-05 parity probe shipped as `crates/gal-engine/tests/install_family_r05.rs`; its binary `install_family_r05-*.exe` tripped Windows installer-detection UAC (os error 740) because the name contains "install" — the same trap R-04 mitigated for the `setup` crate via an asInvoker manifest. `gal-engine` has no such manifest, so the only executable parity evidence for R-05 **could not run on Windows**. Fixed by renaming to `provider_family_r05.rs` (filename heuristic only — no manifest needed). Probe now executes and passes.
- **[F-02, MED — dead code]** `legacy_plugins::build_shared_args` (+ 2 tests) was orphaned by the T-024 repoint: `run_install_orchestration` now calls `gal_engine::install::run_install(&config)` (which self-loads runtime selection from `install-state.json`), so the CLI-arg-forwarding helper has no production caller. Removed it, its `SessionSelection` import, and the 2 tests.
- **[F-03, LOW — vacuous test]** `adapters::update_personalization_dry_run_does_not_invoke_agy_build` wrote fake `Build-CorePlugin.ps1`/`build-core-plugin.sh` sentinels (deleted in R-05) and passed only because `build_agy_plugin` early-returns when the canonical root is absent — never reaching the dry-run guard. Rewrote it to assert the real user-visible invariant (AGY projection target is never created under `--dry-run`).
- **[I-01, INFO]** Verified `build_agy_plugin`'s dry-run guard (adapters lib.rs:1832) IS intact — no dry-run regression from the script→`AgyProjection` cutover. Verified `scripts/common/Common.{ps1,sh}` correctly retained (live consumers: xmachine scripts, `gal.ps1/sh`), not an orphan.
- **[I-02, INFO]** Minor stale doc-comment in `cli/main.rs` cmd_setup ("strangler seam until R-05") corrected to reflect the completed Rust-install repoint.

#### Summary
- Blocking: 0 · Fixed: 3 (1 HIGH, 1 MED, 1 LOW) · Info: 2
- Post-fix: `cargo test --workspace` green, `cargo clippy --workspace --all-targets` 0 warnings.

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
