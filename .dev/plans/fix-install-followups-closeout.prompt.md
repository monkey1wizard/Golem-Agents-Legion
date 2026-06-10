# Plan Prompt: Install-Followups Residual Closeout (fix-install-followups-closeout)

<!--
Generated from docs/plans/fix-install-followups-closeout.md.
Output path: .dev/plans/fix-install-followups-closeout.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.
-->

## Goal

Close all residual items in `docs/observations/install-followups.md` that cannot be closed by installation verification alone: complete remaining items that still require real code changes or review, confirm items already closed by the Rust port, then slim down or delete the observation file.

## Requirements

**Fixes (still real, confirmed against current code)**

- R-01 (S-1) secret-guard backstop regex — `crates/providers/src/lib.rs::has_unresolved_secrets` uses anchored `^\$\{([A-Z0-9_]+)\}$` which only catches standalone `${SECRET}`. Extend to detect embedded secrets (e.g. `Bearer ${API_KEY}`). Defense-in-depth (upstream resolver already catches them).
- R-02 (FU-03, expanded) uninstall ledger accuracy + guaranteed write — `crates/gal-engine/src/install.rs::run_uninstall()`: (a) ledger entry hardcodes `providers: []` / `mode: "normal"` — replace with actual values from `Ledger.last`; (b) mid-removal `?` errors skip the ledger write leaving unrecorded partial-removal state — change to best-effort: collect errors, always write ledger (record partial failures), report aggregate error at the end.
- R-03 (S-3) cosign CI trust security re-review — Review and document trust configuration (workflow identity + Rekor) in `.github/workflows/release.yml`; prove with `cosign verify-blob` + Rekor query. **Gated on core plan T-034 (R-13 pipeline) completion, before public release.**
- R-04 (AGY M2) transaction/ledger — `crates/providers/src/agy.rs` three surfaces: add rollback on failure + ledger integration (OE-A deferred item).
- R-05 (MCP M2, reduced) AGY serializer + coverage verification — **Corrected path: `crates/providers` (not `crates/mcp`)**. First do a 5-minute recon: if AGY consumes `.mcp.json`-style config surface, implement `McpProviderConfig` in `crates/providers/src/agy.rs` (or new module) and wire into `crates/mcp` write flow; if not applicable, close with evidence. Also add tests to confirm existing Codex/OpenCode serializer coverage is complete (they exist but were never acceptance-tested).
- R-06 (R5, optional) commit-msg scope injection — `crates/gal-engine/src/commit_msg.rs` + `crates/cli`: auto-add scope prefix based on changed files. Lowest priority.

**Verify-to-close (suspected already fixed by port — CORRECTED)**

- R-V1 (S-2, UPGRADED TO REAL FIX) AGY non-UTF-8 path — **Original recon was wrong.** `crates/providers/src/agy.rs:106` and `:140` still have `to_str().unwrap()` in Windows rmdir junction removal paths; non-UTF-8 path will panic. Fix to non-panic handling (`to_string_lossy` or `Option → AgyError`), add non-UTF-8 path test, then mark RESOLVED in install-followups (with commit ref).
- R-V2 (FU-04) mode.rs dead-path timeout — `crates/base/src/mode.rs:82` already has 2s `recv_timeout`; confirm unit test covers dead-path case (no test exists yet — must add one); mark RESOLVED-BY-PORT (with commit).

**Bookkeeping**

- R-07 orphan plan confirmation — `manage-external-plugins.md`: read and decide (close superseded or continue); `feat-gal-file-memory-strategy`: has prompt but not in Active Plans — confirm true state and align `.dev/state.md`.
- R-08 install-followups final state — after all items closed, slim `docs/observations/install-followups.md` to "all closed + commit refs" or delete entirely.

## Approach

| Phase | Content | Prerequisites |
| --- | --- | --- |
| M1 — Fixes + verify-close | R-01, R-02, R-V1 targeted fixes + unit tests; R-V2 verify-close (add dead-path test) | None (install family already stable) |
| M2 — Features | R-04 AGY transaction/ledger, R-05 AGY serializer + coverage verification | Independent of M1 |
| Review gate | R-03 cosign re-review | Core T-034 complete, before public release |
| Optional | R-06 commit-msg scope | Anytime |
| Bookkeeping | R-07 orphan confirmation, R-08 install-followups final state | After all items |

Mark each item RESOLVED (commit ref) in `install-followups.md` as it is completed.

Out of scope: installation path / cross-platform real-machine verification (→ core R-10/R-13/T-034/T-035); engine architecture rewrite; cosign pipeline implementation (→ core T-034, this plan only does its trust configuration re-review).

## Files to Create or Modify

- `[MODIFY] crates/providers/src/lib.rs` (R-01)
- `[MODIFY] crates/providers/src/agy.rs` (R-04 transaction/ledger; R-V1 fix `:106`/`:140` to_str().unwrap())
- `[MODIFY] crates/gal-engine/src/install.rs` (R-02)
- `[MODIFY] crates/providers/src/` (R-05 AGY McpProviderConfig; NOT crates/mcp)
- `[MODIFY] crates/base/src/mode.rs` tests (R-V2, add dead-path unit test)
- `[REVIEW] .github/workflows/release.yml` (R-03, review only)
- `[MODIFY] crates/gal-engine/src/commit_msg.rs` + `crates/cli` (R-06, if done)
- `[MODIFY] docs/observations/install-followups.md`, `.dev/state.md`

## Test Cases

- Embedded `Bearer ${API_KEY}` caught by `has_unresolved_secrets`; standalone form does not regress
- `run_uninstall` ledger records actual providers/mode; partial removal failure still writes ledger (with failure note) before reporting error
- Non-UTF-8 path: AGY junction removal does not panic (R-V1 fix)
- Dead-path `gal_root` determined unusable within ~2s (R-V2)
- AGY serializer output matches its config surface format; Codex/OpenCode existing serializers have test coverage
- cosign re-review has written record (verify-blob + Rekor proof)

## Success Criteria

- R-01/R-02/R-V1 fixed with unit tests; R-V2 closed with evidence.
- R-03 completed and recorded before release (or explicitly gate-marked as not yet reached).
- R-04/R-05 complete with behavioral tests passing.
- R-07 two orphan plan states confirmed, state.md consistent; R-08 install-followups zeroed out.

## Risks

- **Protected core surface**: R-02 changes uninstall error semantics (hard-fail → best-effort + guaranteed ledger write) — behavioral change; must explicitly document new semantics in task spec and add tests. Mitigation: architect has reviewed direction; implementation sign-off per task.
- **R-05 AGY serializer applicability unknown**: AGY's MCP config surface may differ from assumption. Mitigation: 5-minute recon inside T-006; close with evidence if not applicable.
- **R-V1 severity reassessment**: Non-UTF-8 home paths are rare on Windows, but the panic path is real. Mitigation: fix in M1 alongside R-01/R-02 (low change cost).
- **cosign re-review depends on CI environment**: Mitigation: prove with `cosign verify-blob` + Rekor query; "workflow ran" is not accepted.

## Open Questions

*(none carried forward from source plan)*

## Approval

- Human approval: [pending]
- Architect review: **APPROVE** *(2026-06-10 full deep-planning; scope significantly reduced after recon against current code)*
- Additional domain review: [not triggered]

---

## Status

```
Workflow: IMPLEMENT
Step: 0 of 10
Last activity: 2026-06-10 — execution prompt generated from source plan
Next step: Begin T-001 (R-01 has_unresolved_secrets regex backstop)
Current Task: —
Task Base Commit: —
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0
```

### Deviations

| # | Task | Deviation | Impact | Resolution |
| --- | --- | --- | --- | --- |

### Handoff Notes

*(none yet)*

## Tasks

**M1 — Fixes + Verify-Close (ready now, mutually independent)**

- [ ] T-001 — (R-01) In `crates/providers/src/lib.rs::has_unresolved_secrets`, change anchored `^\$\{([A-Z0-9_]+)\}$` to a backstop that also detects embedded-form `${SECRET}` (substring scan, reuse KEY/SECRET/TOKEN/PASSWORD keyword list). Standalone form must not regress.
- [ ] T-002 — (R-02) Rewrite `crates/gal-engine/src/install.rs::run_uninstall`: (a) use `Ledger.last` actual `providers`/`mode` values instead of hardcoded `&[]`/`"normal"`; (b) change mid-removal to best-effort — collect each step's error into `warnings`, always write ledger (record partial failures), report aggregate error only after all steps attempted. No more unrecorded partial-removal state.
- [ ] T-003 — (R-V1, upgraded to real fix) Change `crates/providers/src/agy.rs:106` and `:140` `to_str().unwrap()` to non-panic handling (`to_string_lossy` or `Option → AgyError`); add non-UTF-8 path unit test; mark RESOLVED in install-followups (commit ref).
- [ ] T-004 — (R-V2) Confirm `crates/base/src/mode.rs::is_readable` dead-path timeout has unit test coverage (Windows UNC unreachable branch returns false within ~2s). No test exists — must add one. Mark RESOLVED-BY-PORT (with commit).

**M2 — Features (independent of M1)**

- [ ] T-005 — (R-04) `crates/providers/src/agy.rs` three surfaces (CLI/IDE/GUI-config junction): add transaction rollback + ledger integration. Any surface failure must roll back already-created links (no half-projection state); success/failure recorded in ledger.
- [ ] T-006 — (R-05, path corrected) 5-minute recon: does AGY consume `.mcp.json`-style config surface? If yes: implement `McpProviderConfig` in `crates/providers/src/agy.rs` (or new module) and wire into `crates/mcp` write flow. If not applicable: close with evidence. Separately, add tests confirming existing Codex/OpenCode serializer coverage is complete.

**Review Gate (gated on core T-034)**

- [ ] T-008 — (R-03) Security re-review of `.github/workflows/release.yml` trust configuration (workflow identity + Rekor); prove with `cosign verify-blob` + Rekor query; produce written record. **Prerequisite: core plan T-034 (R-13 pipeline) complete, before public release.**

**Bookkeeping (after all items)**

- [ ] T-009 — (R-07) Adjudicate `docs/plans/manage-external-plugins.md` (close superseded or continue) and `feat-gal-file-memory-strategy` (has prompt but not in `.dev/state.md` Active Plans) — confirm true state, align `.dev/state.md`.
- [ ] T-010 — (R-08) After all items closed, slim `docs/observations/install-followups.md` to "all closed + commit refs" or delete.

## Deferred Follow-up

- T-007 — (R-06, lowest priority) `crates/gal-engine/src/commit_msg.rs` + `crates/cli`: auto-add scope prefix from changed files. Do only if bandwidth allows after M1/M2.

## Analyze

<!-- ANALYZE: NOT-RUN -->

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | unit | Embedded `Bearer ${API_KEY}` caught by `has_unresolved_secrets`; standalone `${API_KEY}` still caught; non-secret `${FOO}` not falsely flagged (no regression) | T-001 |
| TP-002 | unit | `run_uninstall` ledger entry reflects `Ledger.last` actual `providers`/`mode`, not hardcoded values | T-002 |
| TP-003 | unit | Mid-removal failure: ledger still written (with partial-failure note); function ultimately reports aggregate error | T-002 |
| TP-004 | unit | Non-UTF-8 path: AGY junction removal does not panic (returns `Err` or lossy handling) | T-003 |
| TP-005 | unit | `mode::is_readable` returns false for unreachable Windows UNC path within ~2s (dead-path timeout) | T-004 |
| TP-006 | recon+unit | AGY MCP serializer output matches config surface format (if applicable); if not applicable, record evidence closure | T-006 |
| TP-007 | unit | Existing Codex/OpenCode `McpProviderConfig` serializers have test coverage (add if missing) | T-006 |
| TP-008 | unit | AGY three-surface mid-failure rolls back created links, no half-projection state; ledger has record | T-005 |
| TP-009 | unit | commit-msg infers scope prefix from changed files (if T-007 implemented) | T-007 |
| TP-010 | manual | cosign re-review has written record (`cosign verify-blob` + Rekor query proof) | T-008 |
| TP-011 | manual | Two orphan plan states adjudicated, `.dev/state.md` consistent; `install-followups.md` zeroed out | T-009, T-010 |

## Test Results

*(pending implementation)*

## Review Results

### Architecture Review

**Verdict: APPROVE** *(2026-06-10 full deep-planning, post-recon rewrite)*

The plan's core problem was being built on stale premises: coordination target deleted (feat-gal-rust-native-install), four file paths wrong (post-crate-split), two claimed bugs already fixed by port, one claimed missing feature already 2/3 present. Rewritten scope is honest: 2 fixes + 2 verify-closes (one upgraded to real fix) + 2 M2 items + 1 review gate + bookkeeping, all anchored to actual code recon results.

- M1/M2 not split into separate plans (OQ-02 ruling): after recon M1 is only 2 small fixes; splitting is over-process.
- R-02 scope expansion is justified: ledger-skip discovery from R-06 review is same function/same subject as original FU-03; "guaranteed ledger write" semantics (best-effort collect, must record, report last) is the correct error model for a destructive operation.
- R-03 gated on T-034 is correct: cosign trust re-review covers the release pipeline's trust configuration; pipeline itself is in core T-034; gate order holds.
- Verify-close (R-V*) pattern is correct: no item directly crossed off; evidence-based closure — consistent with honest-pass principle.
- **R-V1 correction (engineering review)**: original recon incorrectly claimed `to_str().unwrap()` was gone. `agy.rs:106`/`:140` still present. Upgraded to real fix (T-003). This is a tightening, not a new architectural decision.

### Engineering Review

**Verdict: CLEAR** *(2026-06-10 /refining-plan, verified against current code)*

Implementation contract expanded to T-001..T-010 + TP-001..TP-011. Two recon-table errors corrected (R-V1 upgraded to real fix; R-05 path corrected to `crates/providers`). All plan sections updated to be internally consistent. Protected crate surfaces (providers, gal-engine) reviewed; architect approved direction; implementation sign-off per task.

<!-- ENG_REVIEW: CLEAR -->

### Business Review

*(not triggered)*

### Design Review

*(not triggered)*

## Debug Log

*(empty)*
