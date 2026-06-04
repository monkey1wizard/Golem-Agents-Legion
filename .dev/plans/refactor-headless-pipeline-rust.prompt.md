# Plan Prompt: Headless Pipeline Native Rust Bin — Five-Tool Cross-Dispatch + Provider-Native Records + Honest Tests

<!--
Generated from docs/plans/refactor-headless-pipeline-rust.md
Output path: .dev/plans/refactor-headless-pipeline-rust.prompt.md
This is the shared mutable execution work file consumed by control-plane chat, /gal status, /gal whats-next, /gal pipeline, and specialist write-back flows.

Supersedes (already deleted):
- docs/plans/headless-cli-pipeline.md
- docs/plans/headless-cli-pipeline-test.md
Both were built on a false load-bearing assumption (codex/copilot permanently non-dispatchable) that was never empirically tested and has since been disproven.
-->

## Goal

Rewrite the `/gal pipeline` headless dispatch core as a **prebuilt cross-platform Rust binary (bin)**, replacing the existing PowerShell + Bash dispatch/executor script layer. In the same work arc: correct the false codex/copilot exclusion; promote all five tools (claude / codex / copilot / opencode / agy) to first-class Executors; make `executor-routing.json` the **sole authoritative source** for role configuration (fully replacing `model-roles.md`); prove cross-tool mutual dispatch actually occurs via an honest, unforgeable test matrix.

Four conditions must hold simultaneously on completion:

1. **Single config source end-to-end**: bin reads `executor-routing.json` (per-role `executor` + `model`, delivered in Phase 1) as the only source for dispatch and model injection. `model-roles.md` and `model-roles.example.md` are deleted; their cross-model policy prose moves to `workflows/coding.md`.
2. **Exclusion assumption replaced by evidence**: whether each of the five tools can write back in headless mode is determined by spike, not assertion.
3. **Provider-native traceability**: after dispatching to any provider, that execution is viewable in the tool's own native session/history (provider session/job id is captured and recorded; user can resume/inspect in the native tool).
4. **Tests pass only on receipt**: a matrix cell PASS requires the target file actually written back by the secondary tool + `.dev/executor-logs/` terminal state `completed` + a traceable provider session record. Process start / exit-0 only / "orchestrator could have done it" / "--model was accepted" do **not** count.

## Requirements

- [ ] **Rust core reads routing**: Rust dispatcher reads `~/.gal/config/executor-routing.json`, parses `role → { executor, model }`; missing file / missing role / malformed JSON all degrade safely (non-fatal, not silently successful).
- [ ] **Default safety gate**: bin does NOT dispatch to another provider by default; only offloads when that provider is successfully configured (routing has the role + adapter available + write-back spike passed). Unconfigured / unverified providers must not be auto-attempted.
- [ ] **Stage→role mapping in native layer**: implement→CODER, test→TESTER, review→REVIEWER, verify→VERIFIER lives in Rust core, no longer only in SKILL prose or PS.
- [ ] **Real dispatch + write-back verification**: Rust core spawns the secondary CLI (spec fed via stdin or tool-prescribed method), recovers on timeout via process-tree kill, and only marks completion by **reading the target file content**; must not rely on exit code alone.
- [ ] **Five tools as real Executors**: claude / codex / copilot / opencode / agy each have a native Rust adapter, can receive a spec, execute headlessly, and write results in-place. Exact headless invocation flags locked by spike.
- [ ] **Spike-first blocking gate**: before any adapter ships production code or any matrix cell is allowed to PASS, a spike must prove that tool's headless mode truly writes back to a file (not "CLI accepted --model"). Spike-unverifiable tools are marked env-unverifiable; those tools must not receive a PASS in the matrix.
- [ ] **Honest test threshold (hard requirement)**: cell PASS = target file written by secondary tool AND `.dev/executor-logs/` terminal state `completed`. Anything less is recorded as ⬜ unexecuted or ❌. "CLI accepts --model" and "headless write-back verified" are distinct claims; conflating them is not allowed.
- [ ] **Executor and orchestrator capabilities verified separately**: (a) executor write-back capability; (b) orchestrator can autonomously drive `/gal pipeline` with routing JSON deciding dispatch. Neither may impersonate the other.
- [ ] **Prebuilt bin, zero Rust dependency**: user needs no Rust toolchain; bin is a single cross-platform implementation replacing PowerShell and Bash dispatch/executor script layer.
- [ ] **Graceful degradation in two layers**: (a) bin present but no routing / no matching executor / cannot dispatch → bin itself outputs `--- GAL DISPATCH ---` text dispatch; (b) bin completely absent → thin shim outputs minimal text dispatch. Both layers backward-compatible, degradation observable via `Dispatch:` marker.
- [ ] **Observable footprint preserved**: every dispatch leaves a durable record in `.dev/executor-logs/` (stdout/stderr, start/end/duration, exit, terminal-state classification, actual model injected); terminal-state vocabulary: `completed`/`no-receipt`/`timeout`/`disconnected-partial`/`unavailable`.
- [ ] **Commit boundary unchanged**: secondary tools must not cross the commit boundary; orchestrator owns commits. Existing safety model (retry ceiling, protected-path escalation, conditional security audit, bypass-permission warning) unaffected.
- [ ] **Provider-native traceability**: every dispatch captures the provider's native session/job id into executor-log header and `Dispatch:` marker; user can inspect the execution in the native tool (e.g. `codex resume <id>`). Dispatch modes that discard native history are prohibited.
- [ ] **`executor-routing.json` fully replaces `model-roles.md`**: JSON is the single source for role→{executor, model} mapping (machine-readable + human reference); policy prose (CODER≠TESTER etc.) moves to `workflows/coding.md`; `model-roles.md` and `model-roles.example.md` deleted; all references updated or regenerated by sync; no dead links may remain.
- [ ] **Copilot orchestrator/executor asymmetry**: copilot as orchestrator = **VSCode Copilot Chat**; copilot as executor = **copilot CLI**. Matrix must reflect this asymmetry.
- [ ] **Old plans deleted after merge**: after necessary information is confirmed merged into this plan, `headless-cli-pipeline.md` and `headless-cli-pipeline-test.md` are deleted (not just marked). ✅ Done (git rm'd 2026-06-04).

## Approach

### Phase 0 — Spike Blocking Gate (zero production code, verify first)

**Step 0**: For each of the five tools, prove two things: (1) can the tool receive a spec headlessly and write the result back to a specified file (not merely "CLI responded"); (2) does that execution leave a native session/job id that can be retrieved and used to view/resume the record in the native tool.

- **claude**: `claude -p --model <m> --dangerously-skip-permissions`, spec via stdin; token write-back check; capture session id (resumable via `--resume`).
- **codex**: `codex exec` (+ `-m`, auto-approve/sandbox flags); token write-back; capture session id (`codex resume <id>`). Refer to `codex-plugin-cc` background-job + session-id pattern. **This cell disproves the old exclusion.**
- **copilot**: `copilot -C <workdir> --model <m> --allow-all -p "<spec>"`; token write-back; capture session. Prior TC-04 observation, formally re-proven.
- **opencode**: `opencode run` / `opencode -m <provider/model>` (with `default_agent=build`); token write-back; capture session.
- **agy**: per Antigravity CLI docs; token write-back + session capture (user confirms currently available).

Outcome per tool: `{ write-back passed?, session id retrievable? }` or `env-unverifiable`. Any tool that fails write-back spike: its adapter and matrix cells must not be marked PASS.

### Phase 1 — Rust Bin Core (structural change, architecture APPROVED)

**Step 1 — crate skeleton + routing + stage→role**: New `rust/gal-dispatch` crate; routing JSON parser (safe degradation); stage→role map; CLI entry point (`--phase`/`--task`/`--workdir`/`--timeout`).

**Step 2 — spawn + stdin + process-tree timeout + durable log**: spawn secondary CLI; feed spec via stdin; timeout via process-tree kill (prevents node/subprocess orphans); capture stdout/stderr; write log header (timestamps, duration, executor, task-phase, git branch/HEAD, exit, actual model, terminal state). Five terminal-state classifications.

**Step 3 — write-back verification + degradation (bin owns degradation output)**: read target file content to confirm write-back before marking `completed`; partial write always fails and degrades. Bin present but cannot dispatch → bin outputs `--- GAL DISPATCH ---` with `Dispatch:` marker. Bin absent → thin shim in `gal.ps1` / `gal.sh` outputs minimal text dispatch. PS/Bash no longer duplicate dispatch logic.

**Step 4 — five native Rust executor adapters (with session id capture)**: per spike-locked invocation flags; existence check; timeout; model flag injection; authority = in-place file write (stdout is signal only); **capture provider session/job id into log header and `Dispatch:` marker**.

**Step 5 (T-009)** — `gal.ps1` and `gal.sh` dispatch branch reduced to thin shim calling bin.

**Step 6 (T-010)** — Update `commands/gal-pipeline/SKILL.template.md` (protected path).

**Step 7 (T-011)** — After bin confirmed working, delete replaced dispatch scripts.

### Phase 2 — Honest 5×4 Test Matrix (after Phase 0 + Phase 1)

**Step 5 (T-012)**: Configure routing per test case; confirm `-WorkDir` consistency; verify log header matches current environment.

**Step 6 (T-013)**: Execute 20-cell matrix. User opens each tool as orchestrator in turn, runs `/gal pipeline`, lets routing JSON decide dispatch — no hardcoded dispatch instructions. Three-condition PASS per cell. Re-audit prior TC-02/04/05 under the three-condition bar.

Test rotation order (user-driven): claude → codex → copilot (VSCode Chat as orchestrator) → opencode → agy.
Verification is 3-layer: (1) dispatch visible in chat; (2) executor-log `completed` + receipt file confirmed; (3) **user opens target tool's native UI and confirms the execution record appears / is resumable**. Absence of record in native UI = FAIL.

### Phase 3 — `model-roles.md` → `executor-routing.json` Full Replacement (decoupled from bin/tests)

**Step 7 (T-014)**: Migrate cross-model policy prose (CODER≠TESTER, REVIEWER tier ≥ CODER, planning review rules) into `workflows/coding.md` (protected path). Add human-reference section to `executor-routing.json`.

**Step 8 (T-015)**: Delete `model-roles.md` and `model-roles.example.md`. Update all 33 references (stop seeding in `Update-Personalization.*`, update generated-adapter segments in `Sync-DevContext.*`, `.gitignore`, docs). Grep to verify zero dead links.

**Step 9 (T-016)**: Close-out verification.

## Files to Create or Modify

- `[NEW] rust/gal-dispatch/Cargo.toml` + `src/` — Rust dispatch core source (exact crate path refined during implementation)
- `[NEW] prebuilt bin` — distributed with GAL; user needs no Rust toolchain; build/signing/seeding path refined during implementation
- `[MODIFY] scripts/gal.ps1`, `[MODIFY/REMOVE] scripts/gal.sh` — dispatch branch reduced to thin shim calling bin; shim outputs minimal text dispatch when bin absent
- `[KEEP] executor-routing.json` / `executor-routing.example.json` — Phase 1 delivered; promoted to sole role-config source replacing model-roles
- `[REMOVE/REDUCE] scripts/executors/*` dispatch layer — replaced by bin's native Rust adapters; codex and agy paths added; existing ps1 dispatch logic consolidated into bin
- `[MODIFY] commands/gal-pipeline/SKILL.template.md` **(protected path)** — update Headless Executor Dispatch section: bin core, five tools, provider session traceability, honest three-condition threshold, bin/shim degradation
- `[DELETE] model-roles.md`, `[DELETE] model-roles.example.md` — replaced by `executor-routing.json`; policy prose moves to `workflows/coding.md`
- `[MODIFY] workflows/coding.md` **(protected path)** — absorb cross-model policy prose from model-roles
- `[MODIFY] .gitignore`, `scripts/Update-Personalization.*`, `scripts/Sync-DevContext.*` (incl. .sh; **Sync/Setup are protected paths**) — stop seeding/generating model-roles; source from routing JSON
- `[DELETE] docs/plans/headless-cli-pipeline.md`, `[DELETE] docs/plans/headless-cli-pipeline-test.md` — ✅ Done (git rm'd 2026-06-04)

## Test Cases

- [ ] Five-tool spike: each tool has a definite `{ write-back passed, session id retrievable }` or `env-unverifiable` conclusion
- [ ] Rust core: example routing parses correctly; missing/malformed file degrades safely
- [ ] Dispatch: echo mock → `completed`; timeout → `timeout` with no orphan processes; non-zero exit → `disconnected-partial`; missing CLI → `unavailable`
- [ ] Write-back verification: partial write / missing write always fails and degrades
- [ ] Provider traceability: every dispatch's log header + `Dispatch:` marker includes a resumable session/job id
- [ ] Bin/shim integration: bin+routing → native dispatch; no routing → bin text dispatch; no bin → shim minimal text dispatch
- [ ] model-roles replacement: repo has no valid references to `model-roles.md`; policy prose in `workflows/coding.md`
- [ ] Matrix cells: PASS only when receipt + log `completed` + traceable session; (a)/(b)/(c) layered records

## Success Criteria

- [ ] Rust dispatch core reads the same `executor-routing.json`; completes the full chain spawn→timeout recovery→write-back verification→degradation; behavior recoverable from `.dev/executor-logs/` terminal state.
- [ ] claude / codex / copilot / opencode / agy each have at least one cell with observable dispatch + receipt (log `completed`) + traceable provider session; codex/copilot exclusion assumption overturned by evidence.
- [ ] On machines without routing or bin, `/gal pipeline` falls back to text dispatch with no behavioral regression.
- [ ] **5×4 = 20 cells** fully executed and back-filled in `## Test Matrix`; each cell layered records (a) executor write-back, (b) orchestrator autonomously drives pipeline, (c) provider-native traceability; unreachable cells recorded as `env-unverifiable` (valid output).
- [ ] Old plans deleted and confirmed; repo has no valid references to `model-roles.md` / `model-roles.example.md`.

## Risks

- **Bin cross-platform build and distribution**: must precompile, sign, and seed to `~/.gal` for each platform; aligns with `plugin-bin-migration.md` candidate E. Mitigation: prebuilt bin (no Rust required); bin-absent → thin shim outputs minimal text dispatch; no regression on bin-absent machines.
- **Touching protected paths and released script surface**: modifying `SKILL.template.md`, `gal.ps1`, executor adapters. Mitigation: architecture APPROVE'd; minimize changes; preserve OFFLOAD and degradation semantics.
- **Unstable CLI headless flags / sandbox blocking write-back**: codex `exec` sandbox, per-tool permission flags may block file writes. Mitigation: Step 0 spike verifies before writing any adapter; unverifiable tools do not receive PASS.
- **Orchestrator pipeline-driving capability unproven**: whether Copilot / OpenCode / codex can load the GAL skill and drive `/gal pipeline` is untested. Mitigation: this is itself a test target; result recorded as `env-unverifiable` if not achievable; not a blocker.
- **False-assumption recurrence**: this plan exists to correct exactly that error. Mitigation: honest three-condition threshold is a hard requirement; spike is a blocking gate; (a)/(b)/(c) are kept separate.
- **model-roles deletion blast radius**: 33 file references including protected paths (`workflows/coding.md`, `Sync-DevContext`) and generated adapters; missed updates leave dead links or lose cross-model guardrails (CODER≠TESTER). Mitigation: Phase 3 is decoupled and executed as a separate batch; policy prose explicitly placed in `workflows/coding.md`; grep verifies zero residual references.
- **Provider-native traceability not guaranteed for every tool**: some tools' headless/background mode may not retain a resumable session or make the id extractable. Mitigation: Step 0 spike confirms per-tool; non-traceable tools do not receive a full PASS.
- **Scope coupling risk**: bin rewrite + five tools + exclusion correction + model-roles migration + provider records + plan deletions all in one plan. Mitigation: phased (0 spike → 1 bin → 2 tests → 3 doc migration); Phase 3 decoupled from Phase 1/2.
- **ps1/bash incremental deletion regression**: deleting ps1/bash before bin fully covers them breaks functionality. Mitigation: delete only after bin confirmed working; retain anything bin cannot replace; run smoke before and after each deletion.

## Open Questions

None — all planning questions resolved. Decisions are embedded in Goal, Scope, Requirements, Approach, and Review Results. Historical discussion recoverable from this file's git history.

## Approval

- Human approval: **direction approved** (2026-06-04 — locked decisions: Rust bin as vehicle, five tools included pending spike, honest test threshold, `executor-routing.json` fully replaces `model-roles.md`, merge-then-delete old plans, provider-native traceability, copilot orchestrator=VSCode/executor=CLI)
- Architect review: **APPROVE** (2026-06-04 — all planning questions resolved, no remaining blockers; see Architecture Review below)
- Additional domain review: not requested

---

## Status

```
Workflow: VERIFY
Step: 16 of 16
Last activity: 2026-06-04 — T-015 COMPLETE (58a65bf; 29 files updated; model-roles.md deleted; grep 0 residual)
Next step: T-016 — 收尾驗證：舊計畫已移除、state.md 指向新 prompt、repo 無殘留引用
Current Task: T-016
Task Base Commit: 58a65bf
Task Final Commit: —
Test Retry Count: 0
Review Retry Count: 0
Verification Independence: DEGRADED_SAME_RUNTIME
```

### Deviations

| # | Task | Deviation | Impact | Resolution |
| --- | --- | --- | --- | --- |
| — | — | — | — | — |

### Handoff Notes

#### Spike Results — T-001 (RESOLVED 2026-06-04)

- Status: RESOLVED
- claude: write-back ✅ | session_id ✅ `f5c9f450-c44d-41ba-9e7e-e0bfcb19e105` — requires `--output-format json`; field: `session_id`; resumable via `claude --resume f5c9f450-c44d-41ba-9e7e-e0bfcb19e105`
- opencode: write-back ✅ | session_id ✅ `ses_16d664ca2ffeTFVT8ZOljYFfrq` — requires `--format json`; field: `sessionID` in stream events; resumable via `opencode run --session ses_16d664ca2ffeTFVT8ZOljYFfrq`
- copilot CLI: write-back ✅ | session_id ✅ `0f3e7af9-6cd2-4853-bf41-9e5a7a9b6df9` — requires `--output-format json`; field: `sessionId` in terminal `result` object; resumable via `copilot --resume=0f3e7af9-6cd2-4853-bf41-9e5a7a9b6df9`
- codex: write-back ✅ | session_id ✅ `019e92a3-afbb-7af1-9d42-5bd8f5b017ef` — requires `--json`; field: `thread_id` in `thread.started` event; resumable via `codex exec resume 019e92a3-afbb-7af1-9d42-5bd8f5b017ef`; flags: `--json -s workspace-write` (no dangerous flags needed) — **舊排除假設推翻**
- agy: write-back ✅ | session_id ✅ `10e5606f-f97a-4713-ac4f-d2e29ea5ab00` — id NOT in stdout; stored in `~/.agy/brain/<uuid>/`; adapter must scan brain dir for newest entry post-run; resumable via `agy --conversation 10e5606f-f97a-4713-ac4f-d2e29ea5ab00`
- Adapter implementation note: JSON output mode required for claude/opencode/copilot/codex; agy requires post-run brain dir scan. Field names: claude=`session_id`, opencode=`sessionID`, copilot=`result.sessionId`, codex=`thread_id`, agy=newest `~/.agy/brain/` subdir
- **Phase 0 COMPLETE 2026-06-04 — all five tools: write-back ✅, session id ✅. Spike blocking gate passed. Phase 1 (Rust bin) may begin.**
- Constraint discovered: `--dangerously-skip-permissions` (claude/opencode) and `--allow-all` (copilot) are blocked by Claude Code auto-mode classifier; all Phase 0 spikes must be run manually by user in their own terminal

Phase 0 spike must complete before any Phase 1 adapter work starts. The spike for claude/opencode/copilot is a re-proof of write-back (known to work) plus the new requirement: capturing a resumable provider session id. codex (T-002) and agy (T-003) are the genuinely unknown spikes.

Prior evidence in repo (from the old test plan, carried forward per merge-retention list):
- `.dev/tc-02-receipt.txt` — Copilot CLI → OpenCode write-back (session traceability not yet confirmed)
- `tc-04a-receipt.txt` — Claude → Copilot write-back (via `copilot.ps1` adapter)
- `tc-05-receipt.txt` — Claude → OpenCode write-back
- `.dev/executor-logs/20260604-120618-*-opencode.log`

These three cells (old TC-02/04/05) satisfy condition (a) only. Conditions (b) orchestrator drives pipeline and (c) provider-native traceability remain unverified for all three — they will be re-audited as part of T-013.

Old plan deletion: `headless-cli-pipeline.md` and `headless-cli-pipeline-test.md` already `git rm`'d (staged, not yet committed as of prompt generation).

## Tasks

### Phase 0 — Spike Blocking Gate (zero production code, verify first)

- [x] T-001 — Prove write-back + native session id capture/resumability for claude, opencode, and copilot (write-back previously observed; this spike formally re-proves and adds session traceability requirement). *(2026-06-04: all three PASS — see Handoff Notes)*
- [x] T-002 — codex headless spike: `codex exec` (+ `-m` + sandbox/auto-approve flags), token write-back, session id capture (`codex resume <id>`); refer to `codex-plugin-cc` background-job + session-id pattern. **Key cell for overturning old exclusion.** *(2026-06-04: PASS — thread_id in --json output; -s workspace-write sufficient)*
- [x] T-003 — agy headless spike: per Antigravity CLI official docs, test write-back + session capture; mark `env-unverifiable` if not confirmable. *(2026-06-04: PASS — write-back confirmed; conversation_id=10e5606f-f97a-4713-ac4f-d2e29ea5ab00; id NOT in stdout — stored in ~/.agy/brain/, retrieve newest dir post-run)*

### Phase 1 — Rust Bin Core

- [x] T-004 — Create `rust/gal-dispatch` crate: routing JSON parser (`role→{executor,model}`, safe degradation) + stage→role map + CLI entry point (`--phase/--task/--workdir/--timeout`). *(77fef32; crates/gal-dispatch/ — workspace path deviation noted)*
- [x] T-005 — Spawn secondary CLI + feed spec via stdin + process-tree timeout recovery + durable executor log (header + five terminal-state classifications: `completed`/`no-receipt`/`timeout`/`disconnected-partial`/`unavailable`). *(9a9371c; 31/31 tests pass)*
- [x] T-006 — Write-back verification (read target file content to judge `completed`; partial write always fails) + capture provider session/job id into log header and `Dispatch:` marker. *(56548fe; 9 new TP-007 tests; 40/40 pass)*
- [x] T-007 — Default safety gate: offload only when provider is successfully configured (routing has role + adapter available + spike passed); otherwise inline; bin itself outputs `--- GAL DISPATCH ---` degradation text dispatch. *(b102793)*
- [x] T-008 — Five **pure Rust** executor adapters (claude / codex / opencode / copilot / agy): per spike-locked flags; existence check; timeout; model injection; in-place write as authority (stdout is signal only); capture session id. **`codex` adapter is new.** *(b102793)*
- [x] T-009 — Reduce `scripts/gal.ps1` and `scripts/gal.sh` dispatch branch to thin shim calling bin; bin absent → shim outputs minimal text dispatch; no duplicated dispatch/degradation logic. *(406abac; gal.sh had no executor block; gal.ps1 pipeline command updated)*
- [x] T-010 — Update `commands/gal-pipeline/SKILL.template.md` **(protected path)**: bin core, five tools, provider traceability, honest three-condition threshold, bin/shim degradation, commit boundary, bypass-permission warning. *(caf353d)*
- [x] T-011 — After bin confirmed working: delete replaced dispatch scripts (`scripts/executors/*` dispatch portion, `Invoke-Executor.ps1`); retain any scripts bin cannot replace; run existing smoke before and after each deletion. *(0d1e056; 7 scripts deleted; 40/40 smoke pass)*

### Phase 2 — 5×4 Cross-Dispatch Test Matrix

- [x] T-012 — Test pre-conditions: configure `~/.gal/config/executor-routing.json` per test case; pass `-WorkDir` matching current working directory; verify log header git branch/HEAD matches current environment. *(5d38b80; session_id log header fixed; 46 tests pass; routing model update needed before T-013)*
- [ ] T-013 — Execute 5×4=20-cell cross-dispatch matrix; back-fill `## Test Matrix`; three-condition PASS per cell (receipt + log `completed` + traceable session); record (a) executor write-back, (b) orchestrator autonomously drives pipeline, (c) provider-native traceability as separate layers; re-audit prior TC-02/04/05 under three-condition bar.

### Phase 3 — `model-roles.md` → `executor-routing.json` Replacement (decoupled)

- [x] T-014 — Migrate cross-model policy prose (CODER≠TESTER, tier rules) into `workflows/coding.md` **(protected path)**; add human-reference section to `executor-routing.json` to promote it to sole role-config source. *(d7a8a5c)*
- [x] T-015 — Delete `model-roles.md` and `model-roles.example.md`; update all 33 references (stop seeding in `Update-Personalization.*`, update generated-adapter segments in `Sync-DevContext.*`, `.gitignore`, docs); grep verifies zero residual dead links. *(58a65bf)*
- [x] T-016 — Close-out: confirm old plans removed, `.dev/state.md` points to this prompt, repo has no residual references. *(pending commit)*

> **Deferred**: Bash channel deferred (limited by Bash host availability); because bin runs natively cross-platform, *nix dispatch is covered by bin; only `gal.sh` thin shim (T-009) needs handling; no separate Bash adapters. Bash host smoke tests deferred until a Bash host is available.

## Deferred Follow-up

- Full project-wide ps1/bash → Rust bin replacement (beyond the dispatch/executor slice delivered here): per project policy, each script replaced only after bin confirmed working; scripts bin cannot replace are retained. Aligned with `plugin-bin-migration.md` candidate E.
- Remote SSH cross-execution-mode abstraction: deferred to future Rust native layer work.
- `gal.ps1` control-plane non-dispatch sections (status / whats-next etc.) full native replacement: deferred to `plugin-bin-migration.md` candidate E.
- Bash host validation smoke tests: deferred until a Bash host is available.

## Analyze

### [T-012] 2026-06-04 — APPROVE

Verdict: **APPROVE**

T-012 changes are minimal and correct:
- `extract_session_id_generic`: JSON-first extraction is layered safely before UUID scan; no behavioral change for tools already handled by T-008 adapters (those override in main.rs); the log file now records the correct session_id for JSON-stdout tools
- 6 new unit tests covering all 5 tool stdout formats + empty
- 46/46 tests pass; no regressions

No BLOCKING findings.

## Test Plan

| ID | Type | Description | Covers |
| --- | --- | --- | --- |
| TP-001 | manual | claude / opencode / copilot: headless write-back already known; re-prove + capture native session id / resumable. | T-001 |
| TP-002 | manual | codex `codex exec` (+`-m`+sandbox/approve flags): token write-back + session id (`codex resume`); refer to `codex-plugin-cc`. | T-002 |
| TP-003 | manual | agy: per Antigravity CLI docs, test headless write-back + session capture; mark env-unverifiable if not confirmable. | T-003 |
| TP-004 | unit | Rust parses example `executor-routing.json` → correct `role→{executor,model}`; missing file / missing role / malformed JSON degrades safely without panic. | T-004 |
| TP-005 | unit | Stage→role map: implement→CODER, test→TESTER, review→REVIEWER, verify→VERIFIER. | T-004 |
| TP-006 | integration | echo mock→`completed`; timeout mock→`timeout` with no orphan processes; non-zero exit→`disconnected-partial`; missing CLI→`unavailable`; log header complete. | T-005 |
| TP-007 | integration | Write-back verification: partial write / missing write always fails and degrades; log header + `Dispatch:` includes provider session id. | T-006 |
| TP-008 | integration | Safety gate: unconfigured provider is not auto-attempted; no routing / no matching executor → bin itself outputs text dispatch. | T-007 |
| TP-009 | integration | Five pure Rust adapters: CLI installed → write-back + session id captured; CLI absent → unavailable signal. | T-008 |
| TP-010 | integration | gal shim: bin+routing → native dispatch; no routing → bin text dispatch; no bin → shim minimal text dispatch; *nix/Windows consistent. | T-009 |
| TP-011 | manual | SKILL static audit: bin core, five tools, provider traceability, honest three conditions, bin/shim degradation, commit boundary, bypass warning; protected-path change marked. | T-010 |
| TP-012 | integration | Delete replaced dispatch ps1 → existing smoke passes; scripts bin cannot replace still present. | T-011 |
| TP-013 | manual | Test pre-conditions: routing configured per case, `-WorkDir` consistent, log header git branch/HEAD matches current environment. | T-012 |
| TP-014 | manual | 5×4=20 matrix fully back-filled; each cell PASS requires receipt + log `completed` + traceable session (3-layer verification); (a)/(b)/(c) layered records; re-audit TC-02/04/05. | T-013 |
| TP-015 | manual | `workflows/coding.md` retains cross-model policy semantics (CODER≠TESTER etc.); `executor-routing.json` has human-reference section. | T-014 |
| TP-016 | integration | grep finds zero valid references to `model-roles.md` / `model-roles.example.md`; `gal init`/sync-generated adapters have no dead links to deleted files. | T-015 |
| TP-017 | manual | Old plans removed, `.dev/state.md` points to new prompt, repo has no residual references. | T-016 |

## Test Results

### [T-012] 2026-06-04

**TP-013: Test pre-conditions — routing + WorkDir + log header**

Verification run: `.\target\debug\gal-dispatch.exe --phase implement --task T-012-smoke --workdir .` (spec via stdin)

| Check | Result |
|-------|--------|
| `~/.gal/config/executor-routing.json` exists | ✅ PASS |
| Routing JSON is valid (CODER → claude) | ✅ PASS |
| Log header: `git_branch: main` | ✅ PASS (matches current env) |
| Log header: `git_head: 56548fe` | ✅ PASS (matches current HEAD) |
| Log header: all required fields present | ✅ PASS |
| `-WorkDir` = `C:\Code\Golem-Agents-Legion` | ✅ PASS (shim uses Get-RepoContextRoot) |
| session_id in log header (fixed T-006 gap) | ✅ FIXED (commit 5d38b80) |

**Pre-conditions for T-013 (user action required):**
- ⚠️ `executor-routing.json` model `claude-2-opus` is invalid (404). Update to `claude-sonnet-4-6` or valid model before running T-013 matrix cells.
- For full 5×4 matrix, configure routing entries for CODER/TESTER/REVIEWER/VERIFIER pointing to different tools per cell.

**Verdict: PASS** — routing file valid, log header correct, session_id extraction fixed.

### 5×4 Cross-Dispatch Matrix

> **PASS requires all three conditions**: (1) receipt file written back by secondary tool; (2) `.dev/executor-logs/` terminal state `completed`; (3) **user opens target tool's native UI and confirms the execution record appears / is resumable** (a record not visible in the native UI = FAIL regardless of conditions 1–2).
>
> copilot as orchestrator = **VSCode Copilot Chat**; copilot as executor = **copilot CLI**.
> Status: ✅ PASS / ❌ FAIL / ⬜ Not executed.

| TC | Orchestrator | Executor | Expected | Actual | Evidence (receipt / log / session id) | Status |
| --- | --- | --- | --- | --- | --- | --- |
| TC-01 | claude | codex | Offload | | | ⬜ |
| TC-02 | claude | copilot (CLI) | Offload | | | ⬜ |
| TC-03 | claude | opencode | Offload | | | ⬜ |
| TC-04 | claude | agy | Offload | | | ⬜ |
| TC-05 | codex | claude | Offload | | | ⬜ |
| TC-06 | codex | copilot (CLI) | Offload | | | ⬜ |
| TC-07 | codex | opencode | Offload | | | ⬜ |
| TC-08 | codex | agy | Offload | | | ⬜ |
| TC-09 | copilot (VSCode) | claude | Offload | | | ⬜ |
| TC-10 | copilot (VSCode) | codex | Offload | | | ⬜ |
| TC-11 | copilot (VSCode) | opencode | Offload | | | ⬜ |
| TC-12 | copilot (VSCode) | agy | Offload | | | ⬜ |
| TC-13 | opencode | claude | Offload | | | ⬜ |
| TC-14 | opencode | codex | Offload | | | ⬜ |
| TC-15 | opencode | copilot (CLI) | Offload | | | ⬜ |
| TC-16 | opencode | agy | Offload | | | ⬜ |
| TC-17 | agy | claude | Offload | | | ⬜ |
| TC-18 | agy | codex | Offload | | | ⬜ |
| TC-19 | agy | copilot (CLI) | Offload | | | ⬜ |
| TC-20 | agy | opencode | Offload | | | ⬜ |

## Review Results

### Architecture Review

**Verdict: APPROVE** *(2026-06-04, deep-planning architecture review)*

Direction is controlled and phased (Phase 0 spike blocking gate → Phase 1 bin → Phase 2 tests → Phase 3 doc migration). The prior load-bearing defect (codex/copilot exclusion as untested assumption) is eliminated by the spike-first design and the `codex-plugin-cc` prior art. All flags addressed.

**2026-06-04 supplement**: matrix fixed at **5×4 = 20 cells**, user fills in `## Test Results` matrix; adapters are pure Rust, old ps1/bash deleted only after bin confirmed — **project-wide ps1/bash→Rust is the standard policy**; this plan delivers the dispatch slice; all planning questions resolved, no remaining blockers.

#### Trade-off Summary

| Decision | Benefit | Cost | Verdict |
| --- | --- | --- | --- |
| Rust prebuilt bin replaces ps1/bash | Single cross-platform implementation; eliminates Bash parallel channel; paves way for candidate E | Cross-platform build/signing/distribution complexity | OK (shim degradation when bin absent) |
| `executor-routing.json` fully replaces `model-roles.md` | Single source for role config; eliminates dual-source drift | 33-file migration; policy prose needs separate home | OK (Phase 3 decoupled; policy in coding.md) |
| Five tools as real executors (incl. agy/codex) | Overturns false exclusion; complete cross-dispatch matrix | Per-tool flag/sandbox risk | OK (spike gates each) |
| Provider-native traceability as hard requirement | User can view dispatch records in native tool | Constrains invocation mode; requires session id extraction | OK (codex-plugin-cc proves feasibility) |
| Merge-then-delete old plans (not just mark) | Eliminates dead plans; single source of truth | Must confirm info merged before deleting | OK (merge-retention list in source plan header) |

#### Over-engineering Flags

- **[OE-01] Adopted**: do not build a cross-model policy rules engine (tier ≥, distinct enforcement) in the bin. Only 5 roles; rules are human judgment; `workflows/coding.md` already states them. JSON carries the mapping; policy stays as prose.

#### Bug Surface

- **[BUG-01] Medium — addressed in T-014/T-015**: model-roles deletion with missed references → dead links or lost cross-model guardrail (CODER≠TESTER). Fix: Phase 3 grep verifies zero residual references; policy explicitly placed in `workflows/coding.md`.
- **[BUG-02] Medium — addressed in T-009 / Requirements**: bin is the degradation output owner; no one degrades when bin is absent. Fix: thin shim in `gal.ps1`/`gal.sh` outputs minimal text dispatch when bin absent.
- **[BUG-03] Low — addressed in T-001–003 spike**: provider headless mode may not retain a resumable session. Fix: Step 0 spike confirms per-tool; non-traceable tools do not receive a full PASS.

#### What's Good

- **Spike-first blocking gate** eliminates "assumption masquerading as verification" at the root.
- **Honest three-condition threshold** (receipt + log completed + traceable session) is unforgeable.
- **Phased execution + Phase 3 decoupled** controls the model-roles migration blast radius.
- **Reference to `codex-plugin-cc`** avoids reinventing background-job / session-extraction patterns.

### Business Review

Not requested.

### Design Review

Not requested (no customer-facing UI).

### Engineering Review

**Verdict: CLEAR** *(2026-06-04)*

Buildability confirmed, dependency chain acyclic:

- **Phase 0 (blocking gate)**: T-001 (claude/opencode/copilot write-back + session re-prove), T-002 (codex new spike, key exclusion-overturn cell), T-003 (agy write-back unproven) — all independent, zero production code; blocking gate for all adapters and tests.
- **Phase 1**: T-004 (crate + routing + stage→role + CLI entry) independent; T-005 (spawn/stdin/timeout/log) depends on T-004; T-006 (write-back verification + session id capture) depends on T-005 + spike; T-007 (safety gate + bin degradation output) depends on T-004 + T-006; T-008 (five pure Rust adapters) depends on spike + T-004; T-009 (gal.ps1/gal.sh shim → call bin) depends on T-004–T-008; T-010 (SKILL protected-path update) depends on T-009 integration semantics; T-011 (delete old dispatch ps1 after confirmed) depends on T-009 + T-010.
- **Phase 2**: T-012 (test pre-conditions) depends on T-008 + T-009; T-013 (5×4=20 matrix + re-audit) depends on T-012 + spike passed.
- **Phase 3 (decoupled)**: T-014 (policy prose → coding.md) independent; T-015 (delete model-roles + 33 reference updates) depends on T-014; T-016 (close-out verification) depends on T-015.

Protected-path reminder: T-010 (`commands/gal-pipeline/SKILL.template.md`), T-014 (`workflows/coding.md`), T-015 (`Sync-DevContext`/`Update-Personalization`) touch protected paths; architecture APPROVE'd and each is an independent task; implementation must minimize changes and not expand to other content.

Blocking-gate discipline: if any of T-001–003 fails (write-back or session not traceable), that tool's adapter (T-008) and matrix cells (T-013) must not be marked PASS. ps1 deletion (T-011) only after bin confirmed. Bash deferred (Bash host availability limitation); *nix dispatch covered by bin natively; only `gal.sh` thin shim (T-009) needed; no separate Bash adapters.

<!-- ENG_REVIEW: CLEAR -->

## Debug Log

_Pending — populated by golem-debugger if issues arise during implementation._
