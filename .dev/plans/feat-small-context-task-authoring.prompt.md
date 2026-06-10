# Plan Prompt: Small-Context Models Reliably Complete a Single Task — Self-Contained Spec + Reachable Dispatch + Verifiable Quality Gates

<!--
Generated from docs/plans/feat-small-context-task-authoring.md.
Output path: .dev/plans/feat-small-context-task-authoring.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal

Let small/cheap models (claude-haiku, gpt-5.4-mini, gemini-3.5-flash) correctly and verifiably complete a single task **from one task spec alone** when dispatched by `/gal pipeline`. Four dimensions:

1. **Template readability**: move `## Approval` to the top of `templates/plan.md`.
2. **Self-contained tasks (core)**: `/refining-plan`'s `## Tasks` become self-contained, budget-bounded execution units; the dispatch layer (`New-TaskSpec.ps1`) extracts the full self-contained content.
3. **Reachable dispatch**: `gal-dispatch` preflights the routed local executor's auth/headless capability; on failure it fails loud with a fix hint, and `gal doctor` surfaces it.
4. **Verifiable quality gates**: the task acceptance contract requires real probe evidence (executor-log terminal state + observable write-back); the pipeline **hard-blocks** a PASS without evidence, guards against the test-filename UAC trap, and requires refactor self-cleanup.

Done when:
- A newly generated plan has `## Approval` at the top.
- Each task `/refining-plan` produces is self-contained and budget-bounded; `New-TaskSpec.ps1` extracts the full multi-line task block + that task's own files.
- An unauthenticated / non-headless routed executor → `gal-dispatch` fails loud with explicit fix steps and `gal doctor` warns; no more silent `disconnected-partial`.
- A PASS without evidence is treated as not-passing (hard block); the `gal-engine` test-filename trap is caught by a guard.

## Requirements

- [ ] **R-001 — Approval to top**: move `## Approval` in `templates/plan.md` to after the title and before `## Goal`; commands that parse plans by section name (`/refining-plan`, `/plan-to-prompt`, `/deep-planning`, `New-TaskSpec.ps1`) are unaffected.
- [ ] **R-002 — Self-contained task authoring contract (pointer-style)**: each `T-NNN` from `/refining-plan` must carry (a) exact target file path, (b) concrete change, (c) in-place verifiable acceptance, (d) needed convention/signature/dependency pointers. No embedded full file contents (D-1).
- [ ] **R-003 — Spec budget and splitting**: each task targets a `<5KB`-class instruction/pointer budget; over-budget tasks split into atomic tasks (D-1).
- [ ] **R-004 — Dispatch layer extracts self-contained content**: `New-TaskSpec.ps1` extracts the full multi-line `T-NNN` block (to the next `T-NNN`/section boundary); `## Affected Files` narrows to that task's named files (falls back to the whole section only when none are named). Rust bin unchanged (D-2).
- [ ] **R-005 — Local executor dispatch preflight (fail-loud)**: the `dispatch` crate verifies the routed executor is **authenticated/headless-ready** (not merely on PATH) before offload; "definitely unauthenticated" → fail loud with the tool's fix steps (e.g. copilot: `/login` or `GH_TOKEN`); "indeterminate" → allow + warn (must not over-block on unreliable probing, see C1). Also expose routed-executor readiness as a `base::HealthCheck` aggregated into `gal doctor` (D-3/D-4).
- [ ] **R-006 — Honest-pass acceptance contract (hard block)**: `/refining-plan` task acceptance must name the focused probe and evidence shape (executor-log terminal `completed` + observable write-back pointer); `/gal pipeline` treats a PASS without evidence as not-passing and routes to retry/handoff (D-5).
- [ ] **R-007 — Test-trap guard**: add a self-scanning unit test inside `gal-engine`: `crates/gal-engine/tests/` filenames must not contain the `install`/`setup`/`update`/`patch` substrings (or pass when the crate already has an asInvoker manifest); sync the test-naming guidance into `/refining-plan`. Note in the tester contract that a spawn failure (incl. 740) must not be read as exit 0 through a pipe.
- [ ] **R-008 — Refactor self-clean contract**: add to the `/refining-plan` task contract: code/tests orphaned by the change (dead helpers, tests referencing deleted files) are cleaned by the same task; caveats must be updated, never copied stale across phases; the reviewer gate checks for this.

## Approach

### Decisions (settled; replace former OQ-002..OQ-006)

| # | Decision | Basis |
| --- | --- | --- |
| D-1 | Spec budget `<5KB`-class; rationale = focus + per-dispatch cost, not window capacity (the three models' windows are 200K/400K/1M). "Self-contained" = embedded instructions + precise pointers, **not** full file contents; the executor reads named files itself | former OQ-002, architecture review 2026-06-05 |
| D-2 | Spec assembly point = `scripts/common/New-TaskSpec.ps1` (called by gal.ps1, produces the spec then feeds stdin); the Rust bin only forwards stdin, it does not assemble | former OQ-003, codebase trace |
| D-3 | Preflight landing = fail-loud inside the `gal-dispatch` bin + a `gal doctor` HealthCheck warning (a+c in parallel; no silent degrade) | former OQ-004, user ruling 2026-06-10 |
| D-4 | This plan lands the minimal **local-executor** preflight in the `dispatch` crate; the xmachine plan extends to remote (SSH/zellij/remote gal belongs to xmachine R-05) | former OQ-005, user ruling 2026-06-10 |
| D-5 | Honest-pass gate = **hard block (BLOCK)**: the pipeline treats any PASS claim lacking executor-log evidence as not-passing, consistent with the honest-test-pass-bar | former OQ-006, user ruling 2026-06-10 |

### Evidence (design basis)

From R-05/R-06/T-031/T-032 reviews (details in the commits and prompt `## Review Results`):
- **Dispatch root cause**: routing sends CODER/TESTER to `copilot`; the local copilot CLI is on PATH but unauthenticated → every dispatch returns `disconnected-partial`, `session_id=none`. `is_available()` only runs `where`, not an auth check; doctor does not warn. → R-005.
- **Spec truncation**: `New-TaskSpec.ps1:103` still uses `Select-Object -First 1` (task first line only); `## Affected Files` dumps the whole plan's file list. → R-004 (former BUG-01/02).
- **Test trap hit twice**: `gal-engine` test binaries whose filename contains the `install` substring trip Windows UAC (os error 740) and cannot run (R-05 `install_family_r05`, R-06 `uninstall_r06`); piped, the nonzero exit is read as 0, silently masking the failure; R-06 then falsely reported PASS. → R-006/R-007.
- **Refactor residue**: dead helper (`build_shared_args`), a vacuous test referencing deleted scripts, a stale caveat copied across phases. → R-008.

### Dimension 1: template Approval to top (R-001, low-risk, independent)
- Files: `[MODIFY] plugins/gal-core/templates/plan.md` (protected).
- Verify: new `/planning` plan has Approval at top; existing section-name parsers unaffected.

### Dimension 2: self-contained task contract + extraction (R-002/003/004, core)
- Files: `[MODIFY] plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected), `[MODIFY] scripts/common/New-TaskSpec.ps1`.
- What: refining-plan adds pointer-style self-containment + budget/splitting; extractor upgraded to multi-line block + per-task file narrowing; keep the `<5KB` check.

### Dimension 3: local executor preflight (R-005)
- Files: `[MODIFY] crates/dispatch/src/dispatch.rs` (per-executor readiness probe alongside `is_available`), `[MODIFY] crates/dispatch/src/main.rs` (gate message), `[MODIFY]` doctor aggregation (`crates/cli/src/main.rs`, following the T-031 pattern).
- What: probe order = cheap-first (env token / config file / CLI status subcommand); never spawn the full executor to probe. Three-state semantics: `ready` allow, `definitely-unauthenticated` fail-loud with guidance, `unknown` allow + warn. Doctor only checks routing-named executors.

### Dimension 4: quality gates (R-006/007/008)
- Files: `[MODIFY] plugins/gal-core/commands/refining-plan/SKILL.template.md`, `[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md` (protected), `[ADD] crates/gal-engine/tests/` filename self-scan test.
- What: acceptance contract = probe + evidence pointers; pipeline hard-blocks PASS without evidence; filename guard is an in-crate unit test (zero CI infra); self-clean clause.

### Scope

In scope: `templates/plan.md` reorder; `refining-plan/SKILL.template.md` contract upgrade (R-002/003/006/007/008); `gal-pipeline/SKILL.template.md` honest-pass gate text; `New-TaskSpec.ps1` extraction (R-004); `crates/dispatch` local-executor preflight + doctor HealthCheck (R-005); `gal-engine` test-filename self-scan test (R-007).

Out of scope: review-model assignment / golem merge (→ `refactor-golem-auditor.md`); full Rust port of pipeline/task-spec and **remote** preflight (→ `refactor-gal-xmachine-rust-port.md` R-02/R-05); `executor-routing.json` schema change; retroactive rewrite of old plans; a Bash equivalent of `New-TaskSpec`; actually authenticating any executor for the user (credentials are the user's).

### Cross-plan coordination

- task-spec self-contained contract (R-002/003/006/007/008) → written into refining-plan/gal-pipeline SKILL as the Rust spec source; xmachine R-02/T-004 absorbs it into the `pipeline` crate task-spec.
- extractor upgrade (R-004) → xmachine T-004 Rust rewrite reuses it; auditor R-006 changes the same-file agentMap (**this plan lands first, auditor rebases**).
- executor preflight (R-005) → xmachine R-05/T-008 extends to remote (SSH/zellij/remote gal) using the same `HealthCheck` pattern.
- honest-pass gate (R-006) → refining-plan contract + gal-pipeline skill hard-block text; xmachine pipeline crate enforces it natively later.

Execution order (full cross-plan route in `.dev/state.md ## Cross-Plan Route`): this plan → auditor → the matching xmachine port.

## Files to Create or Modify

- `[MODIFY] plugins/gal-core/templates/plan.md` (protected) — R-001
- `[MODIFY] plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected) — R-002/003/006/007/008
- `[MODIFY] plugins/gal-core/commands/gal-pipeline/SKILL.template.md` (protected) — R-006 hard block
- `[MODIFY] scripts/common/New-TaskSpec.ps1` — R-004
- `[MODIFY] crates/dispatch/src/dispatch.rs`, `crates/dispatch/src/main.rs` — R-005 preflight
- `[MODIFY] crates/cli/src/main.rs` (or a HealthCheck inside dispatch) — R-005 doctor aggregation
- `[ADD] crates/gal-engine/tests/<self-scan>.rs` — R-007 (filename itself avoids the trap words)

## Test Cases

- [ ] New `/planning` plan has `## Approval` at top; existing section-name parsing unchanged
- [ ] `New-TaskSpec.ps1` extracts a multi-line task fully (multi/single/trailing task boundaries)
- [ ] `## Affected Files` contains only that task's named files; falls back to the whole section when none named
- [ ] Unauthenticated routed executor → dispatch fails loud with the tool's fix steps; doctor warns
- [ ] Indeterminate probe → allow + warn (no over-block)
- [ ] Authenticated → offload succeeds, executor-log terminal `completed`
- [ ] A `gal-engine` test filename containing `install`/`setup`/`update`/`patch` is caught by the self-scan test
- [ ] Pipeline routes a no-evidence PASS to retry/handoff (hard block), does not advance

## Success Criteria

- [ ] Approval status at top is scannable at a glance.
- [ ] Under "one spec only, executor reads named files itself", a small model can complete the task correctly and within budget.
- [ ] Dispatch **either reaches the target or clearly states why it cannot**; no more silent disconnected-partial.
- [ ] False PASS and the test-filename trap are caught at the gate/guard layer, not by after-the-fact human review.

## Risks

- **Protected-path blast radius** (templates/refining-plan/gal-pipeline/dispatch): minimize per file; dimension 1 can go first independently; per-item sign-off at implementation.
- **Extractor regex boundaries**: multi-line boundaries are easy to mis-cut. Mitigation: explicit boundary rules + unit tests.
- **Preflight probe misjudgment**: tools differ widely; over-blocking hurts more than under-blocking. Mitigation: three-state semantics (C1), fail loud only on "definitely unauthenticated".
- **Same-file coordination** (`New-TaskSpec.ps1` with auditor): this plan lands first, auditor rebases (route settled).
- **honest-pass vs spec-budget tension**: evidence = pointers (log path + terminal keyword), not full text.

## Open Questions

None open — OQ-002..OQ-006 were resolved and baked into the Decisions table above.

## Approval

- Human approval: **approved 2026-06-10** (OQ-004/005/006 ruled by the user, see Decisions)
- Architect review: **APPROVE-with-conditions (2026-06-10 full deep-planning, C1..C4, see ## Review Results)** — R-001..R-004 carry the 2026-06-05 original APPROVE; R-005..R-008 reviewed this pass.
- Engineering review: **CLEAR (2026-06-10, see ## Review Results)** — 8 T-NNN + 11 TP.
- Additional domain review: not requested.

---

## Status

Workflow: VERIFY
Step: 8 of 8
Last activity: 2026-06-10 — **Verifier returned GAPS_FOUND** — implementation tasks are complete, but the TP-11 workspace clippy gate still fails, including touched `dispatch` warnings and unrelated existing `base` warnings. Run mode: DEGRADED_BUNDLED (focused manual validation + verifier write-back).
Next step: resolve the clippy gaps, rerun verifier, then hand off to release prep
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0

### Deviations

| Date | Task | Deviation | Rationale |
| --- | --- | --- | --- |
| — | — | — | — |

### Handoff Notes

_(none yet)_

## Tasks

> Each task is self-contained: target file path, concrete change, in-place acceptance, convention pointers. Protected-path tasks (templates/refining-plan/gal-pipeline/dispatch) get per-item sign-off at implementation time.

- [x] **T-01 (R-001) — templates/plan.md Approval to top**
  - File: `plugins/gal-core/templates/plan.md` (protected). Current: `## Approval` at line 51 (after `## Open Questions`, before `## Review Results`); `## Goal` at line 3.
  - Change: move the whole `## Approval` block (3 bullets) to after the `# Plan: [Feature Name]` title and before `## Goal`; keep all other section order.
  - Acceptance: TP-01. New `/planning` plan has Approval at top; commands that parse by section name (`/refining-plan` Step 1/3/4, `/plan-to-prompt`, `New-TaskSpec.ps1`'s `## Files to Create or Modify`/`T-NNN` scans) are position-independent — grep-confirm these parsers use section names (`^## Approval` etc.), not line numbers.
  - Conventions: markdown-formatting skill; the reorder must not change bullet content.

- [x] **T-02 (R-004) — New-TaskSpec.ps1 extractor upgrade (multi-line + per-task files)**
  - File: `scripts/common/New-TaskSpec.ps1` (not protected). Current bug: `$taskGoal` at lines 103-104 uses `Where-Object {... $TaskScope ...} | Select-Object -First 1` (first line only); `## Affected Files` extraction at lines 113-118 takes the whole `## Files to Create or Modify` section.
  - Change: (a) `$taskGoal` extracts the full multi-line block from the `T-NNN` start line to the next `^\s*-\s*\[.?\]\s*T-\d` or `^##` boundary (preserving indented bullets); (b) `## Affected Files` scans backtick-wrapped file paths inside the extracted task block (e.g. `` `path/to/file` ``) and narrows to that task's named files; fall back to the whole `## Files to Create or Modify` only when the task names none. Keep the existing `<5KB` warning check.
  - Acceptance: TP-02, TP-03, TP-11. Add Pester tests covering multi-line/single-line/trailing/single-task boundaries and per-task file narrowing.
  - Conventions: result-pattern N/A (PS); structured-logging; boundary regex must be commented. **Lands before the auditor plan's same-file agentMap change (Route ①→②).**

- [x] **T-03 (R-002+R-003) — refining-plan self-contained task + budget contract**
  - File: `plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected). Insertion points: Step 3 "Write ## Tasks" (line 42), Step 4 "Write ## Test Plan" (line 53).
  - Change: Step 3 adds the "pointer-style self-contained task" spec — each `T-NNN` carries (a) exact target file path, (b) concrete change, (c) in-place verifiable acceptance, (d) convention/signature/dependency pointers; state explicitly **no embedded full file contents, the executor reads named files itself** (D-1). Add the `<5KB`-class spec budget (rationale = focus + dispatch cost, not window) + the over-budget atomic-split rule.
  - Acceptance: TP-04. This plan's own `## Tasks` (this section) already satisfies the contract as a self-bootstrapping demo.
  - Conventions: markdown-formatting; token-budget convention (`<5KB` rationale must align).

- [x] **T-04 (R-005) — dispatch crate local executor preflight (three-state)**
  - File: `crates/dispatch/src/dispatch.rs` (existing `is_available(name)` at line 283), `crates/dispatch/src/main.rs` (safety gate at the is_available check). Protected (core dispatch).
  - Change: add `fn executor_readiness(executor: &str) -> Readiness` (three-state enum `Ready`/`Unauthenticated{hint}`/`Unknown`), probing **cheap-first**: env token (e.g. copilot: `GH_TOKEN`/`COPILOT_GITHUB_TOKEN`) → config file → tool status subcommand; **never spawn the full executor to run a spec**. main.rs adds a readiness gate after `is_available` passes: `Unauthenticated` → print fix guidance (copilot: run `/login` or set `GH_TOKEN`) and exit (reason=`executor-unauthenticated-confirmed`); `Unknown` → allow + stderr warning (C1: no over-block); `Ready` → proceed.
  - Acceptance: TP-05, TP-06, TP-11. Known copilot-unauthenticated evidence in `## Approach > Evidence`.
  - Conventions: rust convention; result-pattern; C1 three-state semantics mapped line by line. Isolate each executor's probe in a small helper; unsupported executors default to `Unknown` (allow).

- [x] **T-05 (R-005) — doctor routed-executor readiness HealthCheck**
  - File: `crates/cli/src/main.rs` (doctor aggregation at lines 258-273, T-031 pattern: `report.findings.extend(...HealthCheck.check())`). Depends on T-04's `executor_readiness`.
  - Change: add a `HealthCheck` (implements `base::health::HealthCheck`) that reads the role→executor map in `~/.gal/config/executor-routing.json`, calls `executor_readiness` for **each named** executor — `Unauthenticated` → warning finding with fix guidance, `Unknown` → info, `Ready` → no finding; missing routing file = a single INFO finding, not ERROR (C2). Extend at the cmd_doctor aggregation site.
  - Acceptance: TP-07, TP-11. `gal doctor` warns on unauthenticated copilot and only checks routed executors.
  - Conventions: rust; follow T-031's existing three HealthCheck aggregation style (McpProjection/Setup/SkillsProjection).

- [x] **T-06 (R-006) — honest-pass hard-block contract**
  - File: `plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected, stacked on T-03), `plugins/gal-core/commands/gal-pipeline/SKILL.template.md` (protected; current 2d test gate line 391, 2e review line 418).
  - Change: refining-plan acceptance contract adds "each task must name the focused probe + evidence shape (executor-log terminal `completed` + observable write-back pointer)". gal-pipeline test/review gates add a hard-block rule: a dispatched phase reporting PASS but with no matching executor-log terminal evidence → treated as not-passing, routes to existing retry/handoff, does not advance (D-5). C3: hard block applies to dispatched phases; in DEGRADED_BUNDLED manual mode the evidence = reproducible command output inside `## Test Results` (per existing convention), executor-log not required.
  - Acceptance: TP-08. Aligns with honest-test-pass-bar (memory `feedback_honest_test_pass_bar`).
  - Conventions: markdown-formatting. **Note: gal-pipeline SKILL is also restructured by the auditor plan R-002/R-004 — this plan lands the hard-block text first, auditor preserves it on rebase.**

- [x] **T-07 (R-007) — gal-engine test-filename trap self-scan test**
  - File: `[ADD] crates/gal-engine/tests/<trap-free filename>.rs` (e.g. `test_filename_guard.rs` — itself free of `install`/`setup`/`update`/`patch`).
  - Change: a unit/integration test that lists `*.rs` filenames in `crates/gal-engine/tests/` and asserts no filename (minus `.rs`) contains the `install`/`setup`/`update`/`patch` substrings (case-insensitive); include an asInvoker-exemption note (if the crate later adds a manifest, switch to skip). The trap-word list is a constant inside the test.
  - Acceptance: TP-09, TP-11. Evidence: R-05 `install_family_r05`, R-06 `uninstall_r06` hit it twice (see `## Approach > Evidence`). Also add to the `golem-tester` contract or the refining-plan test-naming guidance: test filenames avoid those four words, and a spawn failure (incl. os error 740) must not be read as exit 0 through a pipe.
  - Conventions: rust; C4 guard = in-crate self-scan test, zero CI infra.

- [x] **T-08 (R-008) — refactor self-clean contract**
  - File: `plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected, stacked on T-06).
  - Change: add a "self-clean" clause to the task authoring contract: code/tests orphaned by the change (dead helpers, tests referencing deleted files) must be removed by the **same task**; caveats must be updated between phases, never copied stale; the orchestrator/reviewer gate checks for residue.
  - Acceptance: TP-10. Evidence: R-05 dead `build_shared_args`, vacuous AGY test, `link.exe` caveat copied R-05→R-06 (see `## Approach > Evidence`).
  - Conventions: markdown-formatting.

## Deferred Follow-up

_(none)_

## Analyze

### 2026-06-10 — Verifier: GAPS_FOUND

The task loop is complete and R-001..R-008 are implemented in the expected files, but the plan is not yet VERIFIED because TP-11 is currently false.

- **Gap** — `cargo clippy --workspace --all-targets -- -D warnings` fails. The current failures include touched `dispatch` warnings at `crates/dispatch/src/dispatch.rs:179`, `crates/dispatch/src/dispatch.rs:208`, `crates/dispatch/src/dispatch.rs:279`, and `crates/dispatch/src/stage.rs:46`, plus existing `base` warnings at `crates/base/src/mode.rs:133`, `crates/base/src/platform.rs:30`, `crates/base/src/platform.rs:52`, `crates/base/src/runtime.rs:20`, and `crates/base/src/json_util.rs:89`.
- **What verified successfully** — focused executable checks passed for the implemented slices: `Invoke-Pester tests/powershell/New-TaskSpec.Tests.ps1`, `cargo test -p dispatch`, `cargo test -p cli`, and `cargo test -p gal-engine --test filename_guard_r07`.
- **Verifier conclusion** — do not hand off to release prep yet. Resolve the clippy gate, rerun the verifier, and only then treat the plan as ready for release.

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-01 | manual | A new `/planning` plan has `## Approval` after `# Plan:` and before `## Goal`; running `/refining-plan`+`/plan-to-prompt`+`New-TaskSpec.ps1` on an existing plan still parses correctly by section name (position-independent) | T-01 |
| TP-02 | unit (Pester/PS) | `New-TaskSpec.ps1` extracts a multi-line, bulleted task fully up to the next `T-NNN`/`##` boundary; single-task, trailing-task, single-line-task boundaries all correct | T-02 |
| TP-03 | unit (Pester/PS) | `## Affected Files` contains only the task block's named files; falls back to the whole `## Files to Create or Modify` when the task names none | T-02 |
| TP-04 | manual | `refining-plan/SKILL.template.md` Step 3/4 contains the pointer-style self-contained spec (a path / b change / c acceptance / d convention pointers) + the `<5KB` budget + the over-budget split rule; states "no embedded full file contents" | T-03 |
| TP-05 | unit (Rust) | `dispatch`'s per-executor readiness probe returns a fail state for "definitely unauthenticated" and an unknown state for "indeterminate"; the probe does not spawn the full executor (verified via mock/cheap path) | T-04 |
| TP-06 | integration (Rust) | A routed executor confirmed unauthenticated → `gal-dispatch` exits with the tool's fix steps (copilot: `/login`/`GH_TOKEN`); unknown → allow + stderr warning; ready → normal offload | T-04 |
| TP-07 | unit (Rust) | `gal doctor` only produces a readiness finding for executors named in `executor-routing.json`; missing routing file = INFO, not ERROR (C2) | T-05 |
| TP-08 | manual | `refining-plan/SKILL.template.md` acceptance contract requires a focused probe + evidence shape (executor-log terminal `completed` + write-back pointer); `gal-pipeline/SKILL.template.md` hard-blocks a no-evidence PASS to retry/handoff; DEGRADED_BUNDLED evidence = Test Results command output (C3) | T-06 |
| TP-09 | unit (Rust) | `gal-engine` self-scan test: asserts failure for a `crates/gal-engine/tests/*.rs` filename containing `install`/`setup`/`update`/`patch` (the test's own filename avoids the trap words); exempt when the crate has an asInvoker manifest (C4) | T-07 |
| TP-10 | manual | `refining-plan/SKILL.template.md` contains the refactor self-clean clause (orphan code/tests cleaned by the same task, caveats updated not copied stale across phases) + the orchestrator/reviewer gate checkpoint | T-08 |
| TP-11 | manual | `cargo test --workspace` green; `cargo clippy --workspace --all-targets` 0 warnings (R-004/005/007 Rust-change regression) | T-02, T-04, T-05, T-07 |

## Test Results

### [T-01] 2026-06-10 — PASS (TP-01 slice: template Approval order)

Verification Independence: DEGRADED_BUNDLED. Spec = focused T-01 template reorder with parser-safety confirmation.

- **Template change landed** — `plugins/gal-core/templates/plan.md` now places `## Approval` immediately after `# Plan:` and before `## Goal`, with the original bullet content unchanged.
- **Focused validation green** — `get_errors` on the touched template returned no issues after the edit.
- **Section-name parsing still holds** — targeted grep checks against nearby plan consumers showed section-name references for `## Approval` / plan sections and no line-number dependency in the touched slice.

### [T-02] 2026-06-10 — PASS (TP-02/TP-03 slice: task-block extraction + per-task files)

Verification Independence: DEGRADED_BUNDLED. Spec = focused extractor upgrade for `scripts/common/New-TaskSpec.ps1` with real fixture-driven PowerShell validation.

- **Pester boundary coverage green** — `Invoke-Pester -Script tests/powershell/New-TaskSpec.Tests.ps1 -PassThru` passed all 5 cases covering multi-line, trailing, single-line, and fallback extraction paths.
- **Live prompt extraction green** — running `scripts/common/New-TaskSpec.ps1 -TaskScope T-02 -PromptPath ./.dev/plans/feat-small-context-task-authoring.prompt.md -ConventionHints tests/fixtures/README.md` wrote `.dev/task-specs/T-02-implement.md` at 2.01 KB with the expected multi-line task block.
- **Task-scoped affected files now narrow correctly** — named task paths preserve the original `## Files to Create or Modify` annotations when present and fall back to plain path bullets only when no matching file-list line exists.

### [T-03] 2026-06-10 — PASS (TP-04 slice: self-contained task contract)

Verification Independence: DEGRADED_BUNDLED. Spec = focused `refining-plan` contract update for self-contained task authoring and budget guidance.

- **Step 3 contract landed** — the skill now requires exact file paths, concrete changes, local acceptance checks, and convention/dependency pointers for each `T-NNN`.
- **Budget rule landed** — the skill now states that tasks stay within a `<5KB`-class instruction+pointer budget and must split when they cannot remain self-contained within that limit.
- **Step 4 alignment landed** — the test-plan guidance now ties each test row directly to the matching task acceptance so the task spec remains sufficient on its own.

### [T-04] 2026-06-10 — PASS (TP-05/TP-06 slice: dispatch readiness preflight)

Verification Independence: DEGRADED_BUNDLED. Spec = focused dispatch preflight for routed local executors.

- **Dispatch crate tests green** — `cargo test -p dispatch` passed 55 tests, including the new readiness-state coverage for `Ready`, `Unauthenticated`, `Unknown`, and unsupported executors.
- **Unknown path validated live** — running `cargo run -q -p dispatch --bin gal-dispatch` with Copilot routing in the current environment emitted `warning: gal-dispatch: copilot has local state under ~/.copilot... allowing dispatch` before continuing, which matches the C1 no-overblock rule.
- **Fail-loud branch encoded** — the main preflight gate now emits `reason=executor-unauthenticated-confirmed` plus a `/login` or token hint when the readiness probe returns `Unauthenticated`.

### [T-05] 2026-06-10 — PASS (TP-07 slice: routed-executor doctor aggregation)

Verification Independence: DEGRADED_BUNDLED. Spec = focused `gal doctor` aggregation of routed-executor readiness findings.

- **CLI tests green** — `cargo test -p cli` passed 20 tests, including the new missing-routing and indeterminate-routing healthcheck cases.
- **Aggregation path landed** — `cmd_doctor` now extends the report with `RoutedExecutorHealthCheck::from_default().check()` after the existing MCP/setup/skills health checks.
- **C2 semantics preserved** — missing routing and indeterminate executor readiness both surface as non-error warnings, so `gal doctor` stays informational for the non-ready-but-not-confirmed-broken cases.

### [T-06] 2026-06-10 — PASS (TP-08 slice: honest-pass hard block)

Verification Independence: DEGRADED_BUNDLED. Spec = focused contract update for evidence-backed dispatched PASS handling.

- **Refining-plan contract landed** — task authoring guidance now requires the focused probe and evidence shape, including the executor-log `completed` + write-back pointer expectation for dispatched runs.
- **Pipeline hard block landed** — the test and review gates now stop immediately when dispatched PASS or APPROVE results lack the named evidence instead of advancing on an unverified claim.
- **C3 fallback preserved** — both gate bullets explicitly keep `DEGRADED_BUNDLED` evidence on task-scoped `## Test Results` or `## Review Results` write-back instead of imposing executor-log requirements on manual-mode runs.

### [T-07] 2026-06-10 — PASS (TP-09 slice: filename guard)

Verification Independence: DEGRADED_BUNDLED. Spec = focused `gal-engine` self-scan guard for forbidden test filename substrings.

- **Guard test green** — `cargo test -p gal-engine --test filename_guard_r07` passed and confirmed the current `gal-engine/tests` filenames avoid the forbidden UAC-trigger words.
- **Manifest exemption encoded** — the new test skips only when it detects an `asInvoker` manifest marker under the crate root, which preserves the plan’s C4 escape hatch without adding CI-only infrastructure.
- **Authoring guidance aligned** — `refining-plan` now tells task authors to avoid those four words in `crates/gal-engine/tests/*.rs` filenames and to treat Windows error 740 as a real spawn failure, not a piped exit-0 success.

### [T-08] 2026-06-10 — PASS (TP-10 slice: self-clean contract)

Verification Independence: DEGRADED_BUNDLED. Spec = focused contract update for refactor self-clean enforcement.

- **Self-clean clause landed** — `refining-plan` now tells task authors to remove orphaned helpers, stale tests, and copied caveats within the same task instead of deferring them.
- **Gate expectation landed** — the clause explicitly instructs the orchestrator and reviewer to treat leftover residue as a task failure rather than an optional follow-up.
- **Scope held** — the final contract edit stays inside the existing pointer-style task guidance without reopening test-plan or pipeline semantics.

## Review Results

### Architecture Review

**Verdict: APPROVE-with-conditions** *(2026-06-10 full deep-planning; R-001..R-004 carry the 2026-06-05 original review)*

The four dimensions are directionally correct and bounded. R-005..R-008 move the recurring failure modes that "humans caught during review" (silent unauthenticated degrade, false PASS, filename trap, refactor residue) forward to the gate/guard layer, at the right altitudes: contract → SKILL, mechanism → dispatch crate, guard → in-crate test (zero new infra).

**Conditions (binding at implementation):**
- **C1 (R-005) three-state probe semantics**: fail loud only on "definitely unauthenticated"; indeterminate → allow + warn. Over-blocking turns a usable dispatch unusable, which hurts more than under-blocking. The probe must be cheap (env/config/status subcommand) and must never spawn the full executor.
- **C2 (R-005) doctor checks routed executors only**: do not scan all known tools, only those actually named in `executor-routing.json`; a missing routing file = INFO, not ERROR.
- **C3 (R-006) hard-block semantics scoped to dispatched phases**: a dispatched phase's no-evidence PASS = not-passing → retry/handoff; DEGRADED_BUNDLED manual-mode evidence = reproducible command output in Test Results (per existing convention), executor-log not required.
- **C4 (R-007) guard = in-crate self-scan test**: no CI lint infra; one unit test scanning its own tests/ directory filenames suffices, with the trap-word list and asInvoker exemption built in.

**Trade-off Summary**

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Pointer-style self-contained + `<5KB` (D-1) | small spec, low cost, focused | stricter authoring | OK (original review) |
| preflight in bin+doctor (D-3) | dispatch failure becomes visible, fixable | dispatch crate gains a probe surface | OK under C1/C2 |
| local first, remote to xmachine (D-4) | unblocks local immediately, no duplicate impl | two-stage delivery | OK, joins via the same HealthCheck pattern |
| honest-pass hard block (D-5) | a false PASS cannot advance | a little extra spec for evidence | OK, scoped by C3 |
| filename guard = self-scan test | zero infra, runs with cargo test | only guards known trap words | OK, C4 |

**Bug Surface**: extractor boundary (in Test Cases); probe misjudgment (C1 three-state); hard block killing manual mode (C3 excludes it).
**Over-engineering check**: no CI lint, no new store, no auto-auth of executors (credentials are the user's) — all excluded; the minimal surface holds.

### Business Review

Not requested (no customer-facing surface).

### Design Review

Not requested (no customer-facing surface).

### Engineering Review

**Verdict: CLEAR** *(2026-06-10)*

8 T-NNN map to R-001..R-008, each commit-size and independently verifiable. The tasks themselves obey this plan's own R-002 self-contained contract (file:line pointers, concrete change, in-place acceptance, convention pointers, no full text) as a self-bootstrapping demo. Key ordering: T-04 (dispatch probe fn) → T-05 (doctor uses the fn); T-03→T-06→T-08 all edit `refining-plan/SKILL.template.md` and must stack distinct contract subsections in order. Protected paths (templates/refining-plan/gal-pipeline/dispatch) get per-item sign-off at implementation. Architect C1..C4 mapped into each task's acceptance. Cross-plan: T-06 edits `gal-pipeline/SKILL.template.md`, which the auditor plan also restructures — this plan lands first, auditor rebases (Route ①→②).

<!-- ENG_REVIEW: CLEAR -->

### [T-01] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `07e5c7d`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The diff is limited to section movement inside `plugins/gal-core/templates/plan.md`; no bullet content changed.
- **[I-02]** The local parser check matched section-name driven consumers, which is the relevant regression surface for this task.

#### Architect conditions check (T-01 slice)
- Protected-path scope held to the one template file only. ✓
- Existing command behavior remains section-name driven in the nearby validated surfaces. ✓

#### Security note (task-scoped)
Static template reorder only; no executable surface changed. Clear.

#### Summary
- Blocking: 0 / Warning: 0 / Info: 2

### [T-02] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `3280543`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The extractor change is localized to helper functions plus the original task-goal / affected-files slice; no dispatch adapter or routing surface changed.
- **[I-02]** The new Pester fixture uses literal here-strings so the extractor is exercised against real backtick-delimited path syntax instead of a simplified surrogate.

#### Architect conditions check (T-02 slice)
- Boundary handling is explicit and commented at the regex site. ✓
- Same-file sequencing with the auditor plan is preserved; this lands before the later agentMap rebase. ✓

#### Security note (task-scoped)
Task-spec extraction only; no privilege, network, or credential surface changed. Clear.

#### Summary
- Blocking: 0 / Warning: 0 / Info: 2

### [T-03] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `f9f0f0f`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The protected-path change stays inside Step 3/4 authoring guidance and does not widen into the later honest-pass or self-clean clauses.
- **[I-02]** The Step 4 addition keeps the test plan aligned to task acceptance without changing the command's existing scope guard.

#### Architect conditions check (T-03 slice)
- Self-contained task guidance is explicit about pointer-style authoring and the executor reading named files itself. ✓
- The `<5KB` threshold is expressed as a dispatch-focus rule, not a context-window claim. ✓

#### Security note (task-scoped)
Protected-path documentation update only; no runtime execution surface changed. Clear.

#### Summary
- Blocking: 0 / Warning: 0 / Info: 2

### [T-04] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `a605130`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
- **[W-01]** The current Copilot probe can only confirm `Ready` from headless token presence and `Unauthenticated` from missing local state; a secure-store-backed login without a dedicated status command remains `Unknown` by design to satisfy C1.

#### INFO
- **[I-01]** The readiness API is exported from `dispatch.rs`, which keeps the later doctor aggregation task on a reuse path instead of duplicating the Copilot-specific logic.
- **[I-02]** Unsupported executors remain `Unknown` explicitly, so this task does not silently invent readiness claims for tools it cannot probe yet.

#### Architect conditions check (T-04 slice)
- Only confirmed unauthenticated state blocks; local-state ambiguity degrades to warning + allow. ✓
- The probe remains cheap and does not spawn a full task-spec execution just to determine readiness. ✓

#### Security note (task-scoped)
Preflight logic only; it changes early-exit behavior, not the spawned executor permissions or sandbox model. Clear.

#### Summary
- Blocking: 0 / Warning: 1 / Info: 2

### [T-05] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `0fd29c0`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
- **[W-01]** The doctor surface still reports C2 informational states as `WARNING` because `base::health` only exposes `Warning` and `Error` severities today; the important invariant is preserved because these findings do not flip the doctor run to error.

#### INFO
- **[I-01]** The healthcheck deduplicates routed executors before probing, so multiple roles mapped to the same executor do not spam duplicate findings.
- **[I-02]** The implementation reuses `dispatch::dispatch::executor_readiness`, which keeps doctor and dispatcher semantics aligned for the later T-05/T-04 port consumers.

#### Architect conditions check (T-05 slice)
- Doctor checks routed executors only, not every known tool. ✓
- Missing routing stays non-error and is surfaced as an informational warning path, not a hard doctor failure. ✓

#### Security note (task-scoped)
Read-only aggregation only; no new mutation or credential flow was introduced. Clear.

#### Summary
- Blocking: 0 / Warning: 1 / Info: 2

### [T-06] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `e5874a0`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The hard block is inserted directly into the existing test/review stop conditions, so retry and handoff semantics stay on the same control path.
- **[I-02]** `DEGRADED_BUNDLED` evidence remains explicitly exempt from executor-log requirements, which preserves the architect’s C3 scope limit.

#### Architect conditions check (T-06 slice)
- The hard block is scoped to dispatched phases only. ✓
- Manual fallback evidence remains task-scoped command output or write-back, not synthetic executor-log requirements. ✓

#### Security note (task-scoped)
Contract and orchestrator-gate text only; no runtime permission or execution surface changed. Clear.

#### Summary
- Blocking: 0 / Warning: 0 / Info: 2

### [T-07] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `f8f0475`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The guard is fully in-crate and runs with normal Rust tests, so it catches the filename trap without introducing a new lint or CI lane.
- **[I-02]** The exemption remains future-proof but dormant: no current `gal-engine` asInvoker manifest was detected during this task.

#### Architect conditions check (T-07 slice)
- The guard is implemented as a crate-local self-scan test, not external infrastructure. ✓
- The forbidden-word list is explicit and the asInvoker escape hatch is preserved. ✓

#### Security note (task-scoped)
Test-only guard plus authoring guidance; no runtime execution surface changed. Clear.

#### Summary
- Blocking: 0 / Warning: 0 / Info: 2

### [T-08] 2026-06-10 — APPROVE

Reviewed: 2026-06-10
Commit range: working tree review against base `90fa168`
Verification Independence: DEGRADED_BUNDLED (separate critical pass)

#### BLOCKING
_(none)_

#### WARNING
_(none)_

#### INFO
- **[I-01]** The final clause is small but closes the specific residue failures called out in the plan evidence without inventing a new review phase.
- **[I-02]** The self-clean requirement now sits alongside the existing pointer/budget/evidence rules, which keeps all task-authoring constraints in one place for later Rust port consumers.

#### Architect conditions check (T-08 slice)
- Orphaned code/tests and stale caveats are explicitly owned by the same task. ✓
- Residue is elevated to an orchestrator/reviewer failure, not a soft suggestion. ✓

#### Security note (task-scoped)
Contract text only; no executable surface changed. Clear.

#### Summary
- Blocking: 0 / Warning: 0 / Info: 2

## Debug Log

_(none yet)_
