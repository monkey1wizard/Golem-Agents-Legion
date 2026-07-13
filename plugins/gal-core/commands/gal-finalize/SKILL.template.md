---
name: gal-finalize
description: "GAL finalize ($gal-finalize / /gal finalize / finalize / 完成計劃 / 落地). Lands and closes a VERIFIED plan: gal finalize-check receipt → holistic review → merge into main → doc-sync → lifecycle close. Distinct from $gal-wrap-up (session pause / 暫停). Precondition: all tasks [x] + ORCHESTRATOR goal-backward VERIFIED."
---

# /gal finalize

Land a plan that is **actually done** and close out its lifecycle. `finalize` is a thin orchestrator: it runs a hard precondition gate, then sequences whole-branch holistic review → (if on a worktree/feature branch) merge-to-main + teardown → doc-sync → lifecycle close. It holds **zero new authority** — every step delegates an existing owner (the checking triangle: ORCHESTRATOR / AUDITOR / STEWARD, plus golem-architect). The only genuinely-new pieces are the **orchestration** of the holistic cross-task review and the merge-to-main step.

This is a follow-procedure skill (like `/gal wrap-up`): follow the procedure below directly, do not run a script.

## When to Use

- After `/gal pipeline` has completed every task and the orchestrator's end-of-run goal-backward verification returned VERIFIED.
- When the user says "finalize this plan", "land it", "close it out", or similar.
- When `/gal wrap-up` reverse-prompts finalize because the active plan is in a completed state.

## Boundary — What finalize Is NOT

`finalize` has sharp edges against the neighbouring commands. Do not let it absorb their jobs:

- **Not `/gal pipeline`** — finalize does **no** build/implement and does **not** own goal-backward verification. It *consumes* a VERIFIED plan as a precondition; it does not produce one. If tasks remain or verification has not passed, finalize hard-blocks and redirects (see the gate).
- **Not `/gal wrap-up`** — wrap-up is a non-destructive session *pause*, callable anytime. finalize is a one-shot, gated, **destructive** completion landing (merge + plan-file deletion). They reverse-prompt each other but never overlap.
- **Not `/planning` / `/deep-planning`** — finalize never defines or rescopes work.
- **No delete / extract / verify authority** — lifecycle close (ABSORBED + plan-file deletion) and knowledge extraction are **delegated** to ORCHESTRATOR and STEWARD. finalize itself deletes nothing, extracts nothing, and re-verifies nothing.

## Delegated Authority (checking triangle)

finalize **delegates, never re-creates**. The owners it calls:

- **ORCHESTRATOR** — owns end-of-run goal-backward verification (the finalize *precondition*) and lifecycle close (ABSORBED marking + plan-file deletion + `.dev/state.md` close-out row). ORCHESTRATOR is **not** a holistic review owner.
- **AUDITOR** (`golem-auditor`) — owns the perf/security axis of holistic review via its existing **standalone branch-audit mode** (whole-branch deep performance + OWASP/STRIDE). Mode unchanged.
- **STEWARD** (`golem-steward`) — owns documentation-structure: knowledge extraction → `docs/` + `.dev/project.md`.
- **golem-architect** — owns the correctness / architecture-fit / conventions / scope-drift axis of holistic review, applied at branch level (a small non-planning-stage close-out extension of its charter; the review owner must be independent of the dispatcher, so the orchestrator does not self-review).

Goal-backward verification is orchestrator-owned and always runs in-process.

## Step 1 — Precondition Gate (hard-block, no action)

Resolve the active plan the same way `/gal pipeline` does: explicit `@<plan>` / `#file:<plan>` argument first, else the active plan from `.dev/state.md`; if an argument points at a source plan and the paired `.dev/plans/<slug>.prompt.md` exists, use the execution prompt.

**Zero-trust basis — the precondition gate is mechanized, not self-reported.** The prompt's recorded `VERIFIED` verdict is an **unverified claim**, never proof; a recorded "tests pass" / "tasks done" line is likewise a claim. finalize re-establishes the facts by running a deterministic binary and trusting only its receipt:

- **Self-bootstrap first (mandatory when the plan touched `crates/`).** If any of the plan's changes are under `crates/`, **rebuild + reinstall `gal` before checking** (`cargo build --release` then install to the canonical root, and the Claude plugin-cache copy if present). Otherwise `finalize-check` would run against pre-change code and its receipt would describe the old binary, not the work being finalized. Skip the rebuild only when the plan touched no Rust.
- **Run the gate binary:** `gal finalize-check <plan-or-prompt> --receipt <path>` (default receipt: gitignored `.dev/pipeline/receipts/finalize-check.receipt.md`). It re-runs the repo's authoritative checks (declared in `.dev/project.md` under `<!-- gal:authoritative-check -->`), commit-existence, three-surface checkbox agreement, cited-test-name existence, and adapter-render idempotency (in-process ×2). The cited-test-name existence check is **plan-type-aware**: when the target plan's `## Test Plan` table declares only `manual` probes (every `Type` cell exactly `manual`), there is no fn-citation contract, so the check vacuously passes with a `skipped:` summary instead of harvesting `## Test Results` prose; a missing / mixed / unknown-`Type` table stays fail-closed and runs the harvest as before.
- **The receipt is the sole pass-basis.** Proceed only when the binary exits zero **and** every check in the receipt is `pass`. **Any `fail` or `not-run` → HARD-BLOCK** — surface the offending check's **real command output** from the receipt (not a paraphrase), and redirect by gap type below. Exit 0 is necessary but the per-check states are authoritative; never wave a check through on a recorded claim.

The gate checks **all** of (the binary mechanizes 1–3 and records mode in check(f); see point 4):

1. **All tasks complete** — every blocking task in the plan's `## Tasks` is `[x]` in **both** the source plan and the execution prompt (binary: three-surface checkbox agreement).
2. **Goal-backward VERIFIED is re-grounded, not trusted** — the prompt's recorded VERIFIED is only the entry signal; the binary's authoritative-command + integrity checks are what actually gate. A recorded VERIFIED with a failing receipt does **not** pass.
3. **Working tree clean** — `git status --porcelain` is empty.
4. **Mode detected (dual-mode)** — the receipt's `finalize-mode` check (check(f), state always `Pass`, pure informational report) records `worktree` or `already-on-main` based on `git branch --show-current`. Read its `summary` field to determine whether sequence steps 3–4 apply. The `Pass` state is never a gate condition — mode-detect is reporting only.

The gate **never acts** — on any miss it hard-blocks and redirects by gap type:

- **Genuinely incomplete** (a blocking task is `[ ]` in *both* source plan and prompt) → redirect to **`/gal pipeline`**. Work is not done; finalize does not sneak ahead or proxy-verify.
- **Write-back / convergence gap** (a task is `[ ]` in the source plan but `[x]` in the prompt, i.e. work is done but the source plan was never converged) → redirect to **three-surface convergence** (ORCHESTRATOR converges source plan ↔ execution prompt ↔ `.dev/state.md`), then re-run the gate. Do **not** redo the work.
- **Verification not VERIFIED** → redirect to the orchestrator goal-backward verify pass (or its gap remediation). finalize does not re-verify.
- **Authoritative-check / integrity `fail`** (the binary's receipt has a failing check — e.g. the declared test/clippy command exits non-zero, a cited commit/test name is missing, or the adapter-render is non-idempotent) → HARD-BLOCK, print that check's real command output from the receipt, and redirect to fixing the underlying failure (`/gal pipeline` fix-mode for code; convergence for checkbox disagreement). finalize never lands over a failing receipt.
- **Dirty tree** → tell the user to commit first; if they only want to pause, redirect to **`/gal wrap-up`**.

Only when all four checks pass does finalize proceed to Step 2.

## Step 2 — Completion Sequence (1–5)

Review **must** precede merge: unreviewed work never lands.

### Pre-review — Pipeline-integrity check (before holistic review)

Before the holistic review reads the diff, confirm the **pipeline evidence that produced this branch is honest**. A per-task gate that passed on degraded or mislabelled evidence makes the whole-branch review build on sand. Run this gate over the execution prompt's `## Test Results` / `## Review Results` and the `gal finalize-check` receipt:

- **Integration oracle never unit-substituted.** For every `TP-NN` whose `## Test Plan` Type is `integration` (or higher), the recorded `## Test Results` evidence must be that integration oracle — **a `unit` test standing in for an integration oracle is a substitution → GAP + STOP**, not a pass. Compare each `TP-NN`'s declared Type against the evidence shape actually recorded.
- **`no-receipt` / `timeout` are never PASS.** A dispatched phase whose terminal state is `no-receipt`, `timeout`, or `disconnected-partial` is **not** a passing task, regardless of any prose "PASS" line. Exit 0 alone is not a pass either (the three-condition threshold still applies). Any such state in the per-task history → GAP + STOP. The receipt's `executor-log-scan` check (check(ii)) provides mechanized evidence: any executor log with `terminal_state ≠ completed` → `Fail`; no logs → `NotRun` (non-zero exit). The binary's check is the authority; prose claims do not override it.
- **Cited test names exist.** Test names referenced as evidence must actually exist in the tree — this reuses the `gal finalize-check` cited-test-existence check (its receipt is the authority; a hallucinated test name is a GAP, never waved through on the prose claim).

Any GAP → **STOP** before the holistic review; surface the offending task + evidence shape and redirect to the pipeline (fix-mode / re-run the real phase). The holistic review runs only once the pipeline-integrity gate is clean.

### Sequence 1 — Holistic cross-task review (whole-branch diff)

This is the one current gap finalize closes: per-task audit passing does **not** guarantee the combined branch is coherent. finalize orchestrates a **whole-branch, cross-task** review over the full diff (`git diff main...HEAD` on a feature branch, or the plan's combined commit range on main) by delegating existing owners — **no new agent**:

- **golem-architect lens** (branch-level): correctness, architecture fit, convention compliance, scope drift. Review owner ≠ dispatcher, so the orchestrator does not perform this itself.
- **golem-auditor standalone branch-audit mode** (existing): deep performance + OWASP/STRIDE security; the security axis triggers on trust-boundary or sensitive-surface changes. **Dispatch with `--finalize-branch-audit`** (`/gal auditor --finalize-branch-audit`) so the orchestrated-only gate recognises the finalize branch-audit orchestration context and does not reject the invocation.
- **ORCHESTRATOR is not a review owner here** — it only ran the precondition goal-backward verify and will run the final lifecycle close.

**Review independence — honest when separable, marked when not.** The two review axes must run on a model **independent of the dispatcher / coder**. When the active runtime can route them to a separate model, do so. When it cannot (single-runtime — the orchestrator is also the only available reviewer), do **not** pretend otherwise:

- Mark the review **`Review Independence: DEGRADED_SAME_RUNTIME`** in the holistic-review write-back. Never claim an independent reviewer was dispatched when none was.
- Compensate with **mandatory full-`git diff` per-file judgement**: read the entire `git diff main...HEAD` (or the plan's combined commit range) file-by-file and record a per-file verdict with concrete evidence (what was checked, what was found). A degraded reviewer earns trust only by showing its work.
- **An unread-diff stamp is a gate failure.** A "CLEAR" with no per-file evidence does not pass — treat it exactly as a failed review (back to fix / re-review), never as an approval. The compensating control for lost model-independence is demonstrated diff-reading, not a faster rubber stamp.

On **REVISE**: stop-line (jidoka) — dispatch `golem-implementer` to fix, then re-review to clean, then commit the fix **on the branch**. Do not proceed to merge with an open REVISE.

### Sequence 2 — doc-sync

- **doc-sync is MANDATORY, never skipped.** **STEWARD** extracts durable knowledge into the **durable documentation layer** (`README.md` + all of `docs/`, excluding `.dev/plans/` and `.dev/research/`) on **every** finalized plan; `.dev/project.md` is then re-synced as a compressed current-topic index (upsert/replace/prune, never a history append — see `conventions/token-budget.md` § Bounded Current-Topic Index) **after** the durable layer is updated. STEWARD's reindex runs **first, before any adapter-render evidence is collected** — the reindex can change `.dev/project.md`'s content, which flows verbatim into every generated adapter, so evidence collected before the reindex is stale evidence of the wrong content. "doc-sync might cause drift" is **not** a reason to skip — the opposite: skipping it is what leaves the adapters stale. Commit the doc changes on the branch.
- **Post-reindex double render, five-adapter evidence.** After STEWARD's reindex lands (and after `.dev/project.md`'s normalized size is confirmed within the 30,720 B budget — the render write-boundary rejects an oversized index before any adapter write, so a render failure here means the reindex itself needs trimming, not a retry), run the in-process repo-adapter render **twice** and confirm all five generated adapters (`.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, `AGENTS.md`, `.agents/rules/gal.md`) are byte-identical across the two runs. Do **not** cite the precondition-gate's earlier double-render as this evidence — that render ran before the reindex and reflects the pre-doc-sync `.dev/project.md`, not the reindexed one. Record the fresh idempotency result inline (or a fresh receipt pointer if the tooling supports one) as the proof that doc-sync produced a stable, drift-free adapter set from the **reindexed** content. A non-idempotent render, or a render rejected on the size budget, → STOP (doc-sync is not done until the post-reindex render is stable).

Doc-sync runs **before** merge so main receives a complete code+docs unit.

### Sequence 3 — Merge into main (worktree/feature-branch mode only)

Skip entirely when already on main.

- Strategy = **plain `git merge`** — no `--squash`, no `--ff-only`, no rebase. GAL pipelines produce clean atomic per-task commits and isolate concurrent plans by worktree, so each plan is already a clean linear branch: plain merge fast-forwards when linear (clean line) and writes an honest merge commit on real divergence. It never rewrites history and never errors on a clean merge. Every atomic commit is preserved.
- **Merge conflict → STOP + Interrupted-Phase handoff (jidoka).** Never auto-resolve a conflict. Write the resume marker (see Step 3) and surface to the human.

### Sequence 4 — Teardown (worktree/feature-branch mode only)

Skip entirely when already on main.

- Teardown = **cross-runtime plain git**: `git worktree remove <path>` then `git branch -d <branch>`. This is deliberately runtime-agnostic — do **not** depend on harness-only worktree tooling (e.g. `ExitWorktree`).

### Sequence 5 — Lifecycle close (delegated)

**Durable-layer commit is the pre-deletion gate.** ORCHESTRATOR may not delete plan files until STEWARD has provided a concrete durable-layer commit hash confirming the extraction landed. A "STEWARD confirmed verbally" or "doc-sync ran" without a commit hash is insufficient — the hash is the evidence.

The receipt's `durable-layer-commit` check (check(iii)) provides existence evidence: summary `exists: <hash>` = hash is reachable; `missing: <hash>` = git object absent (failure); `not yet recorded` = no `[x]`-task hash found yet (vacuous Pass, pre-STEWARD). **The binary only asserts existence — it does NOT authorize deletion.** Deletion stays orchestrator-executed after ORCHESTRATOR reads the receipt and confirms the hash evidence independently. The `Pass` state of check(iii) is necessary but not sufficient for deletion: STEWARD must also have signalled and ORCHESTRATOR must verify the hash matches the Seq 2 durable-layer commit before proceeding.

- **STEWARD** confirms durable knowledge has landed in the durable documentation layer (`README.md` + `docs/`, excl. `.dev/plans/` + `.dev/research/`), **records the durable-layer commit hash** in the report evidence block, and signals ORCHESTRATOR only when the hash is present.
- **ORCHESTRATOR** may delete plan files **only after receiving the commit hash evidence** from STEWARD and confirming check(iii) in the receipt reports `exists: <hash>` for that same hash. Then marks the plan `ABSORBED`, deletes `.dev/plans/<slug>.md` + `.dev/plans/<slug>.prompt.md`, and trims `.dev/state.md` to a close-out entry.
- **Plan-file deletion is the last step** so the resume marker survives its needed window. Never delete a plan that has not passed every prior step, including the durable-layer commit gate.

### Sequence 6 — Write `gal-last-good` marker

After all prior sequences succeed (holistic CLEAR + doc-sync + lifecycle close), advance the lightweight git tag to the landing commit so `gal restore` has a trustworthy known-good baseline:

```bash
git tag -f gal-last-good <landing-commit>
```

`<landing-commit>` is the durable-layer commit hash provided by STEWARD in Sequence 5 (the last committed change before plan-file deletion). When already-on-main without a separate durable-layer commit, use `git rev-parse HEAD`. The `-f` flag moves the tag if it already exists; a first-time finalize creates it.

Record the tag in the Step 4 report (see `gal-last-good marker` line). Failure to write the tag is **non-fatal and non-blocking** — log the error and continue; restore will remain fail-closed until the tag is written on a subsequent finalize.

## Step 3 — Idempotent / Resumable (interrupted-phase marker)

finalize is interruptible and resumable without repeating destructive steps. It uses the same interrupted-phase marker shape as `/gal pipeline`, written to the execution prompt `## Status > ### Handoff Notes`:

```markdown
#### Interrupted Phase — finalize / [REVIEW | RELEASE | MERGE | TEARDOWN | CLOSE]

- Status: OPEN | RESOLVED
- Mode: worktree-branch | already-on-main
- Merge state: not-started | merged | not-applicable
- Resume action: <exact next step>
```

Resume rules:

- After a successful merge, set `Merge state: merged`. On resume with `Merge state: merged`, **never re-merge** — continue only teardown / close.
- Because plan-file deletion is the final step, the marker lives in the prompt until close completes; a resume always has a marker to read.
- Clear the marker (`Status: RESOLVED`) only when the phase's own completion condition is met.

## Step 4 — Report

The report is **evidence-aligned**: every status line must be backed by a concrete evidence pointer, and a status word may not claim more than its evidence shows.

- **Each `--- FINALIZE COMPLETE ---` line ↔ an Evidence entry.** No status line stands on its own assertion; it must point at the artifact that proves it (receipt path + check state, commit hash, merge commit, deletion confirmation).
- **No-evidence lines may only say `skipped` / `DEGRADED` / `not-run` — never `done` / `CLEAR` / `LANDED`.** A step you did not actually perform, or could not verify, is reported with one of the honest non-completion words. Reserving `done`/`CLEAR` for evidenced steps is what keeps the report from re-introducing the self-report problem this plan removed.

On clean completion:

```text
--- FINALIZE COMPLETE ---

Plan:            <slug>
Mode:            worktree-branch (merged + torn down) | already-on-main (merge/teardown skipped)
Precondition:    finalize-check PASS    (evidence: .dev/pipeline/receipts/finalize-check.receipt.md — all checks pass)
Holistic review: CLEAR                  (evidence: per-file diff verdicts; Review Independence: <full|DEGRADED_SAME_RUNTIME>)
Doc-sync:        done                    (evidence: durable-layer commit <hash>; sync idempotency = pass)
Merge:           done | not-applicable   (evidence: merge commit <hash> | already-on-main)
Lifecycle:       ABSORBED                (evidence: durable-layer commit <hash> [pre-deletion gate]; plan files deleted; state.md closed out)
gal-last-good:   written | skipped       (evidence: `git tag -f gal-last-good <hash>` succeeded | failed non-fatally)

Overall: LANDED

Evidence:
- <one line per status above, naming the receipt / commit / artifact that proves it>
```

Any line whose evidence is missing must be downgraded to `skipped` / `DEGRADED` / `not-run` with the reason — do not print `done`/`CLEAR`/`LANDED` over an unverified step.

On a stop boundary, mirror the active interrupted-phase / handoff marker so the human can answer immediately: what stopped, what was done, and the exact resume step.

## Stop Conditions Reference

| Condition | Action |
| --- | --- |
| Precondition gate miss (incomplete / convergence gap / not VERIFIED / dirty tree) | HARD-BLOCK + redirect by gap type — do not act |
| Holistic review returns REVISE | STOP-line — fix on branch, re-review, then continue; never merge with open REVISE |
| Merge conflict | STOP + Interrupted-Phase handoff — human resolves, never auto-resolve |
| BLOCKING security / Protected Path finding in holistic review | STOP immediately — human required |
| Already on main | Skip merge + teardown — continue to lifecycle close |
| Clean completion | Plan LANDED — ABSORBED + files deleted |
