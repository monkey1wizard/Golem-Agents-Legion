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

Workflow: IMPLEMENT
Step: 1 of 8
Last activity: 2026-06-10 — **T-01 complete** — moved `## Approval` in `plugins/gal-core/templates/plan.md` to directly follow the title, and grep-confirmed the nearby plan consumers remain section-name driven. Run mode: DEGRADED_BUNDLED (focused manual validation + review write-back).
Next step: implement T-02 (Route ① continues with the New-TaskSpec extractor upgrade)
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

- [ ] **T-02 (R-004) — New-TaskSpec.ps1 extractor upgrade (multi-line + per-task files)**
  - File: `scripts/common/New-TaskSpec.ps1` (not protected). Current bug: `$taskGoal` at lines 103-104 uses `Where-Object {... $TaskScope ...} | Select-Object -First 1` (first line only); `## Affected Files` extraction at lines 113-118 takes the whole `## Files to Create or Modify` section.
  - Change: (a) `$taskGoal` extracts the full multi-line block from the `T-NNN` start line to the next `^\s*-\s*\[.?\]\s*T-\d` or `^##` boundary (preserving indented bullets); (b) `## Affected Files` scans backtick-wrapped file paths inside the extracted task block (e.g. `` `path/to/file` ``) and narrows to that task's named files; fall back to the whole `## Files to Create or Modify` only when the task names none. Keep the existing `<5KB` warning check.
  - Acceptance: TP-02, TP-03, TP-11. Add Pester tests covering multi-line/single-line/trailing/single-task boundaries and per-task file narrowing.
  - Conventions: result-pattern N/A (PS); structured-logging; boundary regex must be commented. **Lands before the auditor plan's same-file agentMap change (Route ①→②).**

- [ ] **T-03 (R-002+R-003) — refining-plan self-contained task + budget contract**
  - File: `plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected). Insertion points: Step 3 "Write ## Tasks" (line 42), Step 4 "Write ## Test Plan" (line 53).
  - Change: Step 3 adds the "pointer-style self-contained task" spec — each `T-NNN` carries (a) exact target file path, (b) concrete change, (c) in-place verifiable acceptance, (d) convention/signature/dependency pointers; state explicitly **no embedded full file contents, the executor reads named files itself** (D-1). Add the `<5KB`-class spec budget (rationale = focus + dispatch cost, not window) + the over-budget atomic-split rule.
  - Acceptance: TP-04. This plan's own `## Tasks` (this section) already satisfies the contract as a self-bootstrapping demo.
  - Conventions: markdown-formatting; token-budget convention (`<5KB` rationale must align).

- [ ] **T-04 (R-005) — dispatch crate local executor preflight (three-state)**
  - File: `crates/dispatch/src/dispatch.rs` (existing `is_available(name)` at line 283), `crates/dispatch/src/main.rs` (safety gate at the is_available check). Protected (core dispatch).
  - Change: add `fn executor_readiness(executor: &str) -> Readiness` (three-state enum `Ready`/`Unauthenticated{hint}`/`Unknown`), probing **cheap-first**: env token (e.g. copilot: `GH_TOKEN`/`COPILOT_GITHUB_TOKEN`) → config file → tool status subcommand; **never spawn the full executor to run a spec**. main.rs adds a readiness gate after `is_available` passes: `Unauthenticated` → print fix guidance (copilot: run `/login` or set `GH_TOKEN`) and exit (reason=`executor-unauthenticated-confirmed`); `Unknown` → allow + stderr warning (C1: no over-block); `Ready` → proceed.
  - Acceptance: TP-05, TP-06, TP-11. Known copilot-unauthenticated evidence in `## Approach > Evidence`.
  - Conventions: rust convention; result-pattern; C1 three-state semantics mapped line by line. Isolate each executor's probe in a small helper; unsupported executors default to `Unknown` (allow).

- [ ] **T-05 (R-005) — doctor routed-executor readiness HealthCheck**
  - File: `crates/cli/src/main.rs` (doctor aggregation at lines 258-273, T-031 pattern: `report.findings.extend(...HealthCheck.check())`). Depends on T-04's `executor_readiness`.
  - Change: add a `HealthCheck` (implements `base::health::HealthCheck`) that reads the role→executor map in `~/.gal/config/executor-routing.json`, calls `executor_readiness` for **each named** executor — `Unauthenticated` → warning finding with fix guidance, `Unknown` → info, `Ready` → no finding; missing routing file = a single INFO finding, not ERROR (C2). Extend at the cmd_doctor aggregation site.
  - Acceptance: TP-07, TP-11. `gal doctor` warns on unauthenticated copilot and only checks routed executors.
  - Conventions: rust; follow T-031's existing three HealthCheck aggregation style (McpProjection/Setup/SkillsProjection).

- [ ] **T-06 (R-006) — honest-pass hard-block contract**
  - File: `plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected, stacked on T-03), `plugins/gal-core/commands/gal-pipeline/SKILL.template.md` (protected; current 2d test gate line 391, 2e review line 418).
  - Change: refining-plan acceptance contract adds "each task must name the focused probe + evidence shape (executor-log terminal `completed` + observable write-back pointer)". gal-pipeline test/review gates add a hard-block rule: a dispatched phase reporting PASS but with no matching executor-log terminal evidence → treated as not-passing, routes to existing retry/handoff, does not advance (D-5). C3: hard block applies to dispatched phases; in DEGRADED_BUNDLED manual mode the evidence = reproducible command output inside `## Test Results` (per existing convention), executor-log not required.
  - Acceptance: TP-08. Aligns with honest-test-pass-bar (memory `feedback_honest_test_pass_bar`).
  - Conventions: markdown-formatting. **Note: gal-pipeline SKILL is also restructured by the auditor plan R-002/R-004 — this plan lands the hard-block text first, auditor preserves it on rebase.**

- [ ] **T-07 (R-007) — gal-engine test-filename trap self-scan test**
  - File: `[ADD] crates/gal-engine/tests/<trap-free filename>.rs` (e.g. `test_filename_guard.rs` — itself free of `install`/`setup`/`update`/`patch`).
  - Change: a unit/integration test that lists `*.rs` filenames in `crates/gal-engine/tests/` and asserts no filename (minus `.rs`) contains the `install`/`setup`/`update`/`patch` substrings (case-insensitive); include an asInvoker-exemption note (if the crate later adds a manifest, switch to skip). The trap-word list is a constant inside the test.
  - Acceptance: TP-09, TP-11. Evidence: R-05 `install_family_r05`, R-06 `uninstall_r06` hit it twice (see `## Approach > Evidence`). Also add to the `golem-tester` contract or the refining-plan test-naming guidance: test filenames avoid those four words, and a spawn failure (incl. os error 740) must not be read as exit 0 through a pipe.
  - Conventions: rust; C4 guard = in-crate self-scan test, zero CI infra.

- [ ] **T-08 (R-008) — refactor self-clean contract**
  - File: `plugins/gal-core/commands/refining-plan/SKILL.template.md` (protected, stacked on T-06).
  - Change: add a "self-clean" clause to the task authoring contract: code/tests orphaned by the change (dead helpers, tests referencing deleted files) must be removed by the **same task**; caveats must be updated between phases, never copied stale; the orchestrator/reviewer gate checks for residue.
  - Acceptance: TP-10. Evidence: R-05 dead `build_shared_args`, vacuous AGY test, `link.exe` caveat copied R-05→R-06 (see `## Approach > Evidence`).
  - Conventions: markdown-formatting.

## Deferred Follow-up

_(none)_

## Analyze

_(pending implementation)_

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

## Debug Log

_(none yet)_
