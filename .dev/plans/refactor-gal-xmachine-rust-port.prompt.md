# Plan Prompt: GAL xmachine Subsystem → Rust Native (refactor-gal-xmachine-rust-port)

<!--
Generated from docs/plans/refactor-gal-xmachine-rust-port.md.
Output path: .dev/plans/refactor-gal-xmachine-rust-port.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal

Reimplement the xmachine subsystem (~6028 lines of `.ps1`+`.sh`: SSH remote execution, local/remote task dispatch, pipeline orchestration, result collection) as native Rust with behavioral parity, deleting each ps1/sh pair as parity is reached. Final state: execution orchestration runs through the `gal` binary; `scripts/` keeps **zero xmachine-family ps1/sh**. **This plan depends on `base` (extracted) and de-prefixed `dispatch` from the sister core plan.**

## Requirements

- R-00 Architecture: compose, don't rebuild (protected): `pipeline`/`xmachine` are new crates depending on `base`+`dispatch`; local execution delegates to `dispatch`, remote is a `Transport` SSH+zellij impl. Must NOT rebuild dispatch/routing/stage. `Transport` trait lives in `pipeline`, xmachine implements it.
- R-01 pipeline backend: task-splitting + multi-provider dispatch + multi-stage orchestration, composing `dispatch`, local transport; exposed as `gal pipeline`.
- R-02 task-spec → `pipeline`: port `New-TaskSpec.ps1` task-spec assembly into `pipeline` (not xmachine); the small-context extractor upgrade (multi-line + per-task files) is the spec for this Rust port; golem-auditor's routing change goes to `dispatch::routing`, not task-spec.
- R-03 xmachine remote backend: SSH+zellij `Transport` impl + remote result collection; `gal xmachine {start,pipeline,task}`.
- R-04 Contract surface sync (protected): repoint gal-pipeline SKILL, task-xmachine templates, agents.md references to `gal xmachine`/`gal pipeline`.
- R-05 Preflight (check-only, never provision): before remote invocation, preflight SSH reachable / zellij installed / remote `gal` compatible; fail-loud with fix guidance. GAL never configures SSH, never installs zellij, never scp's gal. Exposed via `base::HealthCheck` for `gal doctor` aggregation.
- R-06 Session records (traceability): remote execution writes session records (host, transport, SSH/zellij session id, time, result path), **extending dispatch's `.dev/executor-logs/`, not a new store**.
- R-07 Cross-machine SSH parity: real cross-machine verification — win→mac-mini (Unix, covers win→linux) + win→win (Win11 laptop).
- R-08 Oracle reparent precondition: `Test-Xmachine`/`test-t022-ssh.sh`/`Test-PipelineTokenBurn` reparented to fixtures/behavioral tests before deletion.
- R-09 Shared-file coordinated deletion: after xmachine functions in `common.{ps1,sh}` move to Rust, whole-file deletion of `common.*`/`gal.{ps1,sh}` happens jointly with the core plan when both are done.
- R-10 Final state zero ps1/sh (xmachine layer): no xmachine-family ps1/sh remain in `scripts/`.

## Approach

**Motivation (same as core plan):** (1) eliminate dual-implementation maintenance (xmachine is the heaviest dual impl, ~6028 lines twice); (2) single binary speeds AI invocation (pipeline/dispatch are AI hot paths; remote execution body = remote `gal` binary, no shell-script fleet on the remote).

**Key relationship (user clarification):** the current `pipeline` already does "split task small + dispatch to other providers" (local). **`xmachine` = the pure upgrade of this pipeline** — same orchestration plus SSH+zellij remote transport for multi-machine, plus SSH/zellij session records for traceability. Layers compose, none rebuilds the layer below.

**Target architecture (DAG `base ← dispatch ← pipeline ← xmachine`):**

| Layer | crate | Responsibility | Depends |
| --- | --- | --- | --- |
| foundation | `base` | (extracted by core plan R-00) config/mode/paths/platform/provider-selection + `HealthCheck` trait | none |
| primitive | `dispatch` | (exists, de-prefixed) single task→single executor: spawn+write-back+session+5 adapters (the TP-17 mechanism) | base |
| orchestration | `pipeline` | task-split + multi-provider dispatch + multi-stage; `Transport` trait (local impl here); task-spec (New-TaskSpec) lives here | base, dispatch |
| upgrade | `xmachine` | pipeline's remote upgrade: implements `Transport` (SSH+zellij) + remote result collection + session records | base, dispatch, pipeline |
| edge | `cli` | `gal pipeline`/`gal xmachine`/`gal dispatch`; doctor aggregation | pipeline, xmachine, dispatch |

One orchestration, two transports (local / SSH+zellij); xmachine does NOT rebuild pipeline/dispatch/routing/stage. Architecture decisions folded in review: `Transport` trait in `pipeline` (xmachine implements; direction pipeline ← xmachine); session records reuse `.dev/executor-logs/` (no second store, aligns file-memory contract); xmachine implements `base::HealthCheck`; xmachine is a separate crate to isolate SSH/zellij heavy deps and keep pipeline lean for pure-local use.

**User preconditions (user-owned, GAL preflight-only):** SSH (user-configured; GAL cannot handle SSH — hard boundary), zellij (user-installed), remote `gal` (user-installed; preflight version, reject + guide on mismatch, never scp). GAL only detects readiness and fail-louds with guidance.

**Phases:** P0 freeze fixtures; P1 `pipeline` + task-spec (gated on core R-00); P2 `xmachine` remote transport; P3 entry + contract surface (protected); P4 reparent + deletion; P5 cross-plan joint deletion.

## Files to Create or Modify

- [CREATE] (protected, architecture) `crates/pipeline/`: task-split + multi-provider + multi-stage orchestration + `Transport` trait (local impl) + task-spec; composes `dispatch`. Depends base, dispatch.
- [CREATE] (protected, architecture) `crates/xmachine/`: `Transport` SSH+zellij impl + remote result collection + preflight (HealthCheck) + session records. Depends base, dispatch, pipeline.
- [MODIFY] `crates/dispatch/` (de-prefixed by core plan): if needed, widen adapter interface for remote reuse, avoiding re-doing spawn/write-back.
- [MODIFY] `crates/cli/`: `gal pipeline`/`gal xmachine`/`gal dispatch` wiring + xmachine HealthCheck into aggregation.
- [MODIFY] (protected, contract surface) `plugins/gal-core/commands/gal-pipeline/SKILL.template.md`, `plugins/gal-core/templates/task-xmachine-{local,remote}-smoke.md`, `plugins/gal-core/agents/agents.md`.
- [MODIFY] `tests/fixtures/`, `crates/*/tests/*`: xmachine oracle reparent.
- [DELETE on parity] xmachine family: `Start-xMachine.{ps1,sh}`, `Start-XmachinePipeline.{ps1,sh}`, `Invoke-Xmachine{Task,LocalTask,RemoteTask,Pipeline}.{ps1,sh}`, `Get-Xmachine{Local,Remote}Result.*`, `Test-Xmachine.{ps1,sh}`, `Test-PipelineTokenBurn.ps1`, `test-t022-ssh.sh`, `common/New-TaskSpec.ps1`.
- [DELETE on parity, cross-plan end-gate] `common/Common.{ps1,sh}`, `gal.{ps1,sh}` — joint closeout with core plan.

## Test Cases

See `## Test Plan` (TP-01..TP-14 + TP-17). Hard gate: TP-13 cross-machine SSH parity (win→mac + win→win). Inherited reinforcement: TP-17 mac-mini pipeline write-back spike (honest-test-pass-bar).

## Success Criteria

- `scripts/` has zero xmachine-family ps1/sh.
- `gal pipeline`/`gal xmachine`/`gal dispatch` exist and reach parity with old scripts (vs frozen fixtures).
- Cross-machine SSH: win→mac-mini (Unix) + win→win (Win11) remote execution + result collection == old scripts.
- Preflight fail-louds with guidance when preconditions unmet; GAL never configures SSH/zellij/remote-gal.
- Session records land in `.dev/executor-logs/` (host/transport/session-id), traceable.
- Contract-surface references repointed to `gal xmachine`/`gal pipeline`.
- `cargo test` green and test code no longer spawns any xmachine live `scripts/*.{ps1,sh}`.

## Risks

- SSH remote parity (high): cross-machine behavior hard to isolate-test, environments differ. Mitigation: P0 freeze fixtures; P2 real cross-machine (win→mac + win→win).
- User preconditions unmet (high): SSH/zellij/remote-gal are user responsibility, GAL cannot provision (SSH is a hard boundary). Mitigation: R-05 preflight check-only + fail-loud; document preconditions.
- Cross-plan shared files (high): `common.*`/`gal.{ps1,sh}` shared with core plan. Mitigation: P5 joint end-gate.
- Cross-plan dependency gate (medium): pipeline/xmachine depend on core plan R-00 (base/dispatch). Mitigation: core R-00 precedes P1; sequencing coordination.
- pipeline semantic complexity (medium): token-burn/multi-stage state. Mitigation: P1 lock existing semantics via fixtures before replacing.

## Open Questions

None open — all planning-stage OQs resolved and internalized as Decisions in the source plan: xmachine = pipeline's pure remote upgrade (dispatch→pipeline→xmachine); task-spec belongs to `pipeline` (xmachine out of New-TaskSpec contention; small-context extractor upgrade → pipeline task-spec spec; golem-auditor routing → `dispatch::routing`); cross-machine matrix = win→mac-mini (covers win→linux) + win→win (Win11); SSH/zellij/remote-gal are user preconditions, GAL preflight-only.

## Approval

- Human approval: [approved at 2026-06-09]
- Architect review: APPROVE (direction; 2nd review folded full-architecture findings; 3rd review re-sliced task granularity). Implementation-time `pipeline`/`xmachine` crates + protected contract surface require architect sign-off; depends on core plan R-00 (base/dispatch).
- Additional domain review: not triggered (no customer-facing / business-rule content).

---

## Status

Workflow: DRAFT
Step: 5 of 15
Last activity: 2026-06-11 — architect completed R-01 (T-003 orchestration composing dispatch) + R-02 (T-004 task-spec port); F-2 resolved, F-1 still bound to T-005
Next step: T-005 — scaffold `crates/xmachine` + SSH `Transport` impl; relocate the remote run-planning slice out of `pipeline` (F-1) as part of that step
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0

### Deviations

| Date | Task | Planned | Actual | Reason |
| --- | --- | --- | --- | --- |
| 2026-06-11 | T-003 | Port local task-split + multi-provider dispatch + multi-stage orchestration into `pipeline`, composing `dispatch::spawn_executor` | What landed (`ef5ae36`/`bf41183`) is remote pipeline **path-planning** (`RemotePipelinePaths` with `launcher:"zellij"`, remote `Start-XmachinePipeline.*` runner path, remote temp dirs, `PipelineRunRecord`, dispatched/running state notes, local run-record staging) | Architect review (2026-06-11): scope + layer drift. Two binding corrections below. T-003 reverted to open. |

### Handoff Notes

- 2026-06-11: T-001 froze the initial xmachine control-node baselines under `tests/fixtures/xmachine/`: Windows `Test-Xmachine.ps1` no-work-node failure, `common/New-TaskSpec.ps1` missing-TaskScope failure, and the current Windows bash-wrapper unavailability surface. Commit: `b952a1c`.
- 2026-06-11: `scripts/Invoke-XmachinePipeline.ps1` did not yield a stable no-argument output in this environment, so it was intentionally excluded from the first fixture freeze rather than treated as a trustworthy oracle.
- Core R-00 is already landed per `.dev/state.md`, so T-002 is no longer blocked by the base/dispatch extraction. Protected-core/contract architect sign-off still applies to T-002..T-009/T-011/T-012. Preflight remains check-only — never configure SSH, never install zellij, never scp gal. Session records must extend `.dev/executor-logs/`, not a new store. Cross-machine parity still needs the user's Win11 laptop SSH setup plus mac-mini.
- 2026-06-11: T-002 created the new `crates/pipeline/` workspace member and established the dependency-correct skeleton: `Transport` trait owned by `pipeline`, `LocalTransport` marker in `pipeline`, and phase reuse through `dispatch::stage::Phase` rather than rebuilding stage semantics. Commit: `ccfad02`.
- 2026-06-11: T-003 started by porting the legacy xmachine pipeline run-planning slice into `crates/pipeline/src/lib.rs`: remote path planning, dispatched-state note strings, and the dispatched run-record model now live in `pipeline` with Windows/POSIX parity tests. Commit: `ef5ae36`.
- 2026-06-11 (architect review): T-003 reverted to open. The landed slice is **remote-transport planning** (zellij launcher, remote runner path, remote temp/session, run-record), which by the DAG + dep-isolation decision belongs in `crates/xmachine` (T-005), not `pipeline` (F-1). T-003's real deliverable — local task-split + multi-provider + multi-stage orchestration composing `dispatch::spawn_executor` — is still missing; `LocalTransport::dispatch` currently fabricates a receipt and never calls into `dispatch` (F-2). Constants themselves are parity-correct vs `Invoke-XmachinePipeline.{ps1,sh}` and worth keeping as test vectors when the remote slice moves to xmachine. Code left in place (no hard revert); the relocation is bound to T-005 and the orchestration core is bound to a T-003 redo. Source plan checkbox realigned to `[ ]`.
- 2026-06-11 (architect implemented R-01 + R-02, F-2 resolved):
  - **R-01 / T-003** — `crates/pipeline/src/orchestration.rs`. The `Transport` trait + `LocalTransport` now genuinely compose `dispatch`: `resolve_plan(task, phase, routing, workdir, spec)` maps phase→role (`Phase::role`), looks the role up in `RoutingTable`, and asks `dispatch::adapters::get_adapter` to build the invocation (multi-provider; stdin vs `-p` CliFlag handled). `LocalTransport::run` builds a `dispatch::SpawnConfig` and calls `dispatch::spawn_executor` — no rebuild of spawn/write-back/routing/stage. `default_stage_plan()` = implement→test→audit (multi-stage; verifier is a separate end pass). The old toy `PipelineRequest`/`TransportReceipt`/toy `LocalTransport` in lib.rs were replaced.
  - **R-02 / T-004** — `crates/pipeline/src/task_spec.rs`. Faithful port of `New-TaskSpec.ps1`: multi-line task-block extraction (incl. indented sub-bullets, stops at next top-level `T-NNN`), per-task affected-file convergence (backtick paths in the block, case-insensitive dedup, matched to the Files section, fallback to the full section), per-phase write-back (`## Test Results`/`## Review Results`/`## Analyze`) + agent-contract maps, and spec assembly with the 5 KB size signal. Pure/deterministic (git/clock/convention passed in).
  - **F-1 still open**: the remote run-planning slice remains in `lib.rs`, bound to relocate into `crates/xmachine` at T-005.
  - Verification: `cargo test -p pipeline` 22 passed; `cargo clippy -p pipeline -- -D warnings` clean; `cargo test --workspace` 447 passed / 1 ignored. CODER=architect here → independent AUDITOR/TESTER pass still owed per CODER≠AUDITOR.

## Tasks

P0
- [x] T-001 (P0) — Freeze xmachine/pipeline parity fixtures; capture SSH behavior baseline.

R-01/R-02 pipeline (protected, architect; prerequisite = core R-00)
- [x] T-002 — Scaffold `pipeline` crate: define `Transport` trait + local impl composing `dispatch` (no rebuild); `cargo test` green.
- [x] T-003 — Port task-split + multi-provider dispatch + multi-stage orchestration → `pipeline`; parity. (R-01: `orchestration.rs` composes `dispatch` — routing→adapter→`spawn_executor`; F-2 resolved.)
- [x] T-004 — Port task-spec (`New-TaskSpec`, absorbing the small-context multi-line extraction + per-task file convergence spec) → `pipeline`; parity. (R-02: `task_spec.rs`.)

R-03/R-05/R-06 xmachine (protected, architect)
- [ ] T-005 — Scaffold `xmachine` crate + SSH `Transport` impl; remote-execution parity.
- [ ] T-006 — zellij session multiplexing integration; disconnect/reconnect behavior parity.
- [ ] T-007 — Remote result collection; parity.
- [ ] T-008 (R-05) — Preflight (`HealthCheck`: SSH/zellij/remote-gal, check-only, never provision, fail-loud with guidance).
- [ ] T-009 (R-06) — Session records extend `.dev/executor-logs/` (host/transport/session-id/time/result-path), no new store.

reparent / entry / contract
- [ ] T-010 (R-08, before deletion) — Reparent `Test-Xmachine`/`test-t022-ssh.sh`/`Test-PipelineTokenBurn` to fixture/behavioral tests.
- [ ] T-011 (R-04) — `cli` wiring: `gal pipeline`/`gal xmachine`/`gal dispatch` + xmachine `HealthCheck` into doctor aggregation.
- [ ] T-012 (R-04, protected, architect) — Repoint contract surface (gal-pipeline SKILL/task-xmachine templates/agents.md) to the Rust binary.

parity / deletion / closeout
- [ ] T-013 (R-07, hard gate) — Cross-machine SSH parity: win→mac-mini (covers win→linux) + win→win (Win11 laptop); align to fixtures; run TP-17 write-back spike opportunistically.
- [ ] T-014 (R-10) — After parity green, delete xmachine family ps1/sh + `common/New-TaskSpec.ps1`.
- [ ] T-015 (R-09, cross-plan end-gate) — Jointly with core plan, delete `common/Common.{ps1,sh}` + `gal.{ps1,sh}` (only when both plans are done).

## Deferred Follow-up

- SSH connection reuse/pooling evaluation (PERF-X1) — performance, not a gate.

## Analyze

- 2026-06-11: fixture freeze created `tests/fixtures/xmachine/README.md` plus three Windows control-node baseline artifacts under `tests/fixtures/xmachine/windows-control-node/`.
- 2026-06-11: T-002 scaffold created `crates/pipeline/Cargo.toml` and `crates/pipeline/src/lib.rs`, with local transport tests proving phase reuse from `dispatch`.
- 2026-06-11: T-003 planning slice added `WorkPlatform`, `RemotePipelinePaths`, `PipelineRunContext`, `PipelineRunRecord`, plus state-note helpers to `crates/pipeline/src/lib.rs`.

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | integration | fixture freeze: xmachine/pipeline current observable output/side-effects reproducible; SSH behavior baseline | T-001 |
| TP-02 | unit | `pipeline` crate skeleton + `Transport` trait + local impl composing `dispatch` (no rebuild); `cargo test` green | T-002 |
| TP-03 | parity | `pipeline` task-split + multi-provider dispatch + multi-stage orchestration output == fixture | T-003 |
| TP-04 | parity | task-spec (`New-TaskSpec`) assembly == fixture; includes small-context multi-line extraction + per-task file convergence | T-004 |
| TP-05 | integration | `xmachine` SSH `Transport` impl remote execution == fixture | T-005 |
| TP-06 | integration | zellij session multiplexing correct; disconnect/reconnect behavior == fixture | T-006 |
| TP-07 | integration | remote result collection == fixture | T-007 |
| TP-08 | unit | preflight: SSH unreachable / zellij missing / remote gal version mismatch each fail-loud with guidance; GAL has no scp/install side-effect (never provision) | T-008 |
| TP-09 | unit | session records land in `.dev/executor-logs/` (host/transport/session-id/time/result-path), no new store | T-009 |
| TP-10 | unit | after T-010, `cargo test` no longer spawns `Test-Xmachine`/`test-t022-ssh.sh`/`Test-PipelineTokenBurn`; reads fixtures | T-010 |
| TP-11 | integration | `cli`: `gal pipeline`/`gal xmachine`/`gal dispatch` callable; xmachine `HealthCheck` into `gal doctor` aggregation | T-011 |
| TP-12 | integration | contract surface (SKILL/templates/agents.md) repointed to `gal xmachine`/`gal pipeline`; `/gal pipeline` runs through Rust | T-012 |
| TP-13 | integration (hard gate) | cross-machine SSH parity: win→mac-mini (Unix, covers win→linux) + win→win (Win11 laptop) remote execution+collection == old scripts | T-013 |
| TP-14 | manual | `scripts/` xmachine family zero ps1/sh (grep); `cargo test` green and no xmachine live-script spawn | T-014, T-015 |
| TP-17 | integration (redo; reinforcement, non-gate) | mac-mini `/gal pipeline` write-back spike: verify 3 executors (claude/opencode/copilot) real headless write-back — target file written back AND `.dev/executor-logs/` terminal `completed` AND resumable native session id. codex (budget)/agy (no install path) marked ⬜ not-run + reason, not PASS. | T-013 |

TP-17 venue: run at core plan TP-02 (mac-mini install) in the same SSH session. honest-test-pass-bar: PASS requires all of (a) target file written back by the secondary tool, (b) executor-log terminal `completed`, (c) resumable native session id; missing any → not PASS.

## Test Results

- 2026-06-11: `list_dir tests/fixtures/xmachine` confirmed the new fixture domain root and `windows-control-node/` subdirectory.
- 2026-06-11: `list_dir tests/fixtures/xmachine/windows-control-node` confirmed the three baseline artifacts.
- 2026-06-11: `get_errors` clean for `tests/fixtures/xmachine/README.md` and all three baseline `.txt` artifacts.
- 2026-06-11: `cargo test -p pipeline` PASS (3 tests, 0 failed).
- 2026-06-11: `cargo test -p pipeline` PASS (7 tests, 0 failed) after the T-003 run-planning port slice.
- 2026-06-11: `cargo test -p pipeline` PASS (22 tests) after R-01 (orchestration) + R-02 (task-spec); `cargo clippy -p pipeline -- -D warnings` clean.
- 2026-06-11: `cargo test --workspace` PASS (447 passed, 1 ignored) — no regression from the pipeline changes.

## Review Results

### Architecture Review — Post-Implementation (T-001..T-003), 2026-06-11

**Verdict: FINDINGS-OPEN → F-2 RESOLVED (2026-06-11), F-1 deferred to T-005.** T-001 (fixture freeze) and T-002 (skeleton) are sound. T-003 had scope + layer drift; the orchestration core (F-2) has since been implemented (R-01 in `orchestration.rs`, T-003/T-004 now done — see Handoff Notes). F-1 (remote slice in the wrong crate) remains open, bound to relocate at T-005.

**What's good (verified):** the ported constants are real parity, cross-checked against `scripts/Invoke-XmachinePipeline.{ps1,sh}` — `C:\Windows\Temp\gal-xmachine-pipeline\$RunId` / `/tmp/gal-xmachine-pipeline/$RunId`, `pipeline-$RunId` session, `.dev/xmachine-runs` staging, `convergeCommand "/gal status"` all match legacy. Skeleton is dependency-correct (`Transport` trait owned by pipeline, `Phase` reused from `dispatch` not rebuilt). 8 tests green, clippy `-D warnings` clean.

**F-1 (HIGH — layer/altitude):** remote-transport-specific planning lives in `crates/pipeline` — `RemotePipelinePaths` carries `launcher:"zellij"` and the remote `Start-XmachinePipeline.*` runner path; `remote_pipeline_paths`, `PipelineRunRecord`, and the dispatched/running state notes are all remote-SSH concerns. The plan DAG and the dep-isolation decision (xmachine is a separate crate *specifically* to isolate SSH/zellij and keep `pipeline` lean for pure-local use) put these in `crates/xmachine` (T-005+), not `pipeline`. Leaving them here inverts the architecture and pre-loads the local layer with remote concerns. **Fix (bound to T-005):** relocate the remote run-planning into `crates/xmachine` when that crate is scaffolded; `pipeline` keeps only transport-agnostic orchestration. Not relocated now — creating `crates/xmachine` is T-005's protected-architecture step, out of a T-003 review's scope.

**F-2 (HIGH — scope + honest state):** T-003's actual deliverable (local task-split + multi-provider dispatch + multi-stage orchestration) is absent. `LocalTransport::dispatch` fabricates a `TransportReceipt` and never calls `dispatch::spawn_executor` / `dispatch::adapters::get_adapter`; the only `dispatch` reuse is the `Phase` type. The "compose, don't rebuild — local execution delegates to dispatch" requirement (R-00/R-01) is not yet demonstrated. The source plan had marked T-003 `[x]` while the prompt itself showed it in-progress (Step 3/15). **RESOLVED 2026-06-11:** `orchestration.rs` now composes `dispatch` end-to-end (`resolve_plan` routing→adapter; `LocalTransport::run` → `dispatch::spawn_executor`); T-003/T-004 implemented and re-marked done; source/prompt realigned.

<!-- ARCH_REVIEW: FINDINGS-OPEN -->
<!-- F-1 open (remote slice relocation → T-005); F-2 resolved 2026-06-11 -->

Note: this implementation was authored in the architect/CODER seat; an independent AUDITOR + TESTER pass is still owed (CODER≠AUDITOR) before T-003/T-004 are treated as cross-checked.

### Architecture Review

APPROVE (3rd review, 2026-06-09). Core decision correct: `xmachine` composes `dispatch`+`pipeline` rather than rebuilding; three layers `dispatch→pipeline→xmachine`, altitude clear (xmachine = pipeline's pure remote upgrade). User clarifications folded: pipeline already does split+multi-provider, SSH/zellij are user preconditions, session records kept. Architecture findings folded: `Transport` trait inversion (trait in pipeline, xmachine implements), session records reuse executor-log (no second store), xmachine implements `base::HealthCheck`, xmachine separate crate for SSH/zellij dep isolation. 3rd review re-sliced task granularity (GRAN-X1: T-002/T-003 split into skeleton/orchestration/task-spec; xmachine T-005..T-009 five steps) and patched test completeness (GRAN-X2: zellij reconnect, result collection, preflight no-provision, session-record schema, cli wiring each its own TP). No blocking issues.

### Business Review

Not triggered (no business-rule content).

### Design Review

Not triggered (no customer-facing UI).

### Engineering Review

CLEAR. 15 T-NNN map to R-00..R-10 / P0..P5, each an independently verifiable commit-size unit (pipeline split into crate-skeleton/orchestration-port/task-spec-port; xmachine split into SSH-transport/zellij/result-collection/preflight/session-records). 13 TP cover (incl. cross-machine hard gate win→mac + win→win, inherited TP-17). Implementation constraints: (1) cross-plan gate — T-002 hard-depends on core plan R-00 (base extracted, dispatch de-prefixed); (2) protected-core/contract architect sign-off for pipeline/xmachine crate tasks + T-012; (3) compose, don't rebuild dispatch/routing/stage; `Transport` trait in pipeline; (4) preflight check-only, never provision; session records extend `.dev/executor-logs/`; (5) T-010 reparent before T-014 deletion; `common.*`/`gal.{ps1,sh}` (T-015) at cross-plan joint gate. CODER≠REVIEWER.

## Debug Log

(empty)
