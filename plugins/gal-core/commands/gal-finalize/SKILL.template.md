---
name: gal-finalize
description: "GAL finalize ($gal-finalize / /gal finalize / finalize / 完成計劃 / 落地). Lands and closes a VERIFIED plan: full-mode gal finalize-check receipt → holistic review → doc-sync → merge/teardown → bounded state write → hygiene-only receipt → lifecycle close → last-good tag. Distinct from $gal-wrap-up (session pause / 暫停). Precondition: all tasks [x] + ORCHESTRATOR goal-backward VERIFIED."
---

# /gal finalize

Land a plan that is **actually done** and close out its lifecycle. `finalize` is a thin orchestrator: it runs a hard full-mode precondition gate, then sequences whole-branch holistic review → doc-sync → (if on a worktree/feature branch) merge-to-main + teardown → bounded `.dev/state.md` write + a distinct post-write hygiene-only receipt → lifecycle close → last-good tag. It holds **zero new authority** — every step delegates an existing owner (the checking triangle: ORCHESTRATOR / AUDITOR / STEWARD, plus golem-architect). The only genuinely-new pieces are the **orchestration** of the holistic cross-task review and the merge-to-main step.

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
- **Run the gate binary in full mode:** `gal finalize-check <plan-or-prompt> --receipt <path>` (default receipt: gitignored `.dev/pipeline/receipts/finalize-check.receipt.md`) — **never pass `--hygiene-only` here**; the precondition gate requires the complete check set. The receipt's own `mode: full` line is part of the evidence — a `mode: hygiene-only` receipt cannot satisfy this precondition. It re-runs the repo's authoritative checks (declared in `.dev/project.md` under `<!-- gal:authoritative-check -->`), commit-existence, three-surface checkbox agreement, cited-test-name existence, adapter-render idempotency (in-process ×2), and the repo-hygiene rows (`project-source-doc-existence`, `state-bound`, plus `contract-roster-parity` + `doc-link-resolution` when `plugins/gal-core/` exists at the repo root). The cited-test-name existence check is **task-scoped and negation-aware**: a plan whose `## Test Plan` table declares only `manual` probes still gets the whole-plan `skipped:` early return, but a mixed plan is scoped per `### [T-NN]` subsection — a subsection is excluded from harvest only when every Test Plan row explicitly covering that task is `manual`, while preamble prose, subsections with no explicit covering row, and any non-`manual`-covered subsection stay harvested (fail-closed residue). Within a harvested subsection, a backticked token preceded by an existence-negation marker (`no`, `never`, `not found`, `does not exist`, and their zh-TW equivalents, among others) inside its 40-character prefix window is checked in the inverted direction — resolving nowhere is a pass — so evidence asserting a symbol no longer exists is never treated as a hallucinated citation; a positive citation elsewhere for the same token still wins. The receipt summary always discloses all four counts — `resolved`, `missing`, `skipped-manual-task`, `negative-cited` — so nothing is silently skipped.
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
- **`no-receipt` / `timeout` are never PASS.** A dispatched phase whose terminal state is `no-receipt`, `timeout`, or `disconnected-partial` is **not** a passing task on its own, regardless of any prose "PASS" line. Exit 0 alone is not a pass either (the three-condition threshold still applies). The receipt's `executor-log-scan` check (check(ii) / check(g)) provides mechanized evidence — it is now **per-checked-task**, not a whole-root scan: for every `[x]` task in the target plan/prompt, it first requires **bound convergence** (the same three v1 checks `gal pipeline-converge-check` runs — three-surface agreement, task-commit existence, cursor-cleared — all `Pass`) before trusting any dispatch-evidence classification for that task. Once convergence holds, each required phase (implement/test/audit) classifies into exactly one of the three outcome literals defined in the `gal-pipeline` contract's Execution Outcome Taxonomy — `in-conversation` (no attempt log at all), `dispatch-offload` (latest attempt terminated `completed`), or `recovered-in-conversation` (latest attempt terminated non-`completed` but bound convergence still holds). An unterminated `started` marker always fails regardless of convergence. **Mixed-mode counts**: a passing check(g) reports counts across all checked tasks' phases by outcome (e.g. "8 dispatch-offload, 3 in-conversation, 1 recovered-in-conversation"), not a bare PASS/FAIL — a human reading the finalize report can see how much of the run was dispatched vs. handled in-conversation vs. recovered. **Recovery stays visible**: `recovered-in-conversation` is always counted and reported separately — it is never folded into `dispatch-offload`'s count, since a human reviewing before landing must be able to tell a clean dispatch from a broken one that recovered. **Fail-closed on any bad evidence**: a checked task whose evidence is missing, malformed, unbound (convergence does not hold), unterminated (`started`), or foreign (excluded by the plan-scoped log directory design, so a same-task-ID log from another plan can never satisfy this task) fails the whole check(g) closed, naming the specific task and reason — never a generic "some log somewhere failed." **Zero checked tasks → `NotRun`** (nothing to verify yet; this replaces the old log-file-count framing — the check is now keyed on checked-task count, not raw log-file presence). The binary's check is the authority; prose claims do not override it.
- **Cited test names exist.** Test names referenced as evidence must actually exist in the tree — this reuses the `gal finalize-check` cited-test-existence check (its receipt is the authority; a hallucinated test name is a GAP, never waved through on the prose claim).

Any GAP → **STOP** before the holistic review; surface the offending task + evidence shape and redirect to the pipeline (fix-mode / re-run the real phase). The holistic review runs only once the pipeline-integrity gate is clean.

**Legacy unscoped evidence — one-time migration/adjudication, not an automatic re-converge.** Plan-scoped dispatch logging is a breaking change for any GAL-initialized repo carrying pre-existing top-level `.dev/executor-logs/*.log` files that predate plan-scoped directories. Such a file carries no plan-scoping metadata, so tooling cannot safely guess which plan or task produced it — and check(g) never even looks: it only scans the one plan-scoped subdirectory it derives for the target, and a missing directory there is a legitimate zero-evidence state, not an error. A legacy top-level log is therefore **not detected or fail-closed by check(g) at all** — it is simply never read, and the phase it should have evidenced instead classifies silently as `in-conversation` ("no attempt log at all"), indistinguishable from a phase that genuinely never dispatched. finalize does **not** respond to this silent gap by universally re-running `pipeline-converge-check` across every plan as an automatic fix; that would still be tooling guessing at ownership. The correct response is a **one-time, explicit, human-attributed adjudication**: when a repo first adopts plan-scoped logging, the repo owner reviews any surviving top-level legacy logs, manually moves each one into the correct plan-scoped directory (or discards it) based on their own knowledge of which run produced it, and only then re-runs the affected plan's convergence check. This adjudication happens once per repo at adoption time, performed by a human, not by finalize itself on every run.

**A repo's first adoption of plan-scoped logging needs its own one-time bootstrap** — confirming the top-level executor-log baseline is empty (or has been adjudicated per the paragraph above) before scoped logging can be trusted going forward — which is separate from finalize's normal per-landing behavior described here.

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
- **Provider-memory harvest is pointer-only here.** STEWARD may additionally consider candidates already approved in the plan's `### Handoff Notes` → `#### Approved Memory-Harvest Candidates` (see `conventions/token-budget.md` → Provider-Memory Harvest); finalize performs **no** provider acquisition of its own. Approval is never itself promotion evidence — a candidate is promoted only after STEWARD independently proves the same verified-root-cause-plus-credible-recurrence gate as any other extraction; a gate miss omits the candidate without blocking doc-sync or finalize.
- **Post-reindex double render, five-adapter evidence.** After STEWARD's reindex lands (and after `.dev/project.md`'s normalized size is confirmed within the 30,720 B budget — the render write-boundary rejects an oversized index before any adapter write, so a render failure here means the reindex itself needs trimming, not a retry), run the in-process repo-adapter render **twice** and confirm all five generated adapters (`.github/copilot-instructions.md`, `GEMINI.md`, `CLAUDE.md`, `AGENTS.md`, `.agents/rules/gal.md`) are byte-identical across the two runs. Do **not** cite the precondition-gate's earlier double-render as this evidence — that render ran before the reindex and reflects the pre-doc-sync `.dev/project.md`, not the reindexed one. Record the fresh idempotency result inline (or a fresh receipt pointer if the tooling supports one) as the proof that doc-sync produced a stable, drift-free adapter set from the **reindexed** content. A non-idempotent render, or a render rejected on the size budget, → STOP (doc-sync is not done until the post-reindex render is stable).

Doc-sync runs **before** merge so main receives a complete code+docs unit.

### Sequence 3 — Merge into main (worktree/feature-branch mode only)

Skip entirely when already on main.

- Strategy = **plain `git merge`** — no `--squash`, no `--ff-only`, no rebase. GAL pipelines produce clean atomic per-task commits and isolate concurrent plans by worktree, so each plan is already a clean linear branch: plain merge fast-forwards when linear (clean line) and writes an honest merge commit on real divergence. It never rewrites history and never errors on a clean merge. Every atomic commit is preserved.
- **One deterministic exception: a conflict whose entire unmerged path set is exactly `{.dev/state.md}`.** Run `gal state-merge` (finalize-internal, no CLI flags). It independently re-verifies `MERGE_HEAD` and the exact path set before touching anything — a wrong-precondition or wrong-caller invocation is refused, not silently accepted.
  - **Exit 0 → continue.** The resolver staged a deterministic row-keyed three-way merge of `.dev/state.md`; proceed to the merge commit and Sequence 4.
  - **`STATE_MERGE: unresolved` (nonzero exit) → STOP + Interrupted-Phase handoff.** Every check the resolver runs — parsing, row-keyed merge, invariants, or the verified write itself — either failed before any mutation or was proven to have rolled back exactly. Treat this identically to an ordinary merge conflict: repository state is intact, a human resolves `.dev/state.md` normally.
  - **`STATE_MERGE: rollback-unconfirmed` (nonzero exit) → STOP + Interrupted-Phase handoff, naming possible mutation.** The resolver could not prove its own rollback was exact after a failure past the point of mutation. Record in the handoff that `.dev/state.md`'s worktree and index state may differ from the original conflict and must be inspected before resuming, not assumed inert.
- **Merge conflict outside that one exact case → STOP + Interrupted-Phase handoff (jidoka).** Never auto-resolve any other conflict, or a conflict whose path set includes `.dev/state.md` plus anything else. Write the resume marker (see Step 3) and surface to the human.

### Sequence 4 — Teardown (worktree/feature-branch mode only)

Skip entirely when already on main.

- Teardown = **cross-runtime plain git**: `git worktree remove <path>` then `git branch -d <branch>`. This is deliberately runtime-agnostic — do **not** depend on harness-only worktree tooling (e.g. `ExitWorktree`).

### Sequence 5 — Bounded state write, post-write hygiene gate, and lifecycle close (delegated)

**Durable-layer commit is the pre-deletion gate.** ORCHESTRATOR may not delete plan files until STEWARD has provided a concrete durable-layer commit hash confirming the extraction landed. A "STEWARD confirmed verbally" or "doc-sync ran" without a commit hash is insufficient — the hash is the evidence.

The receipt's `durable-layer-commit` check (check(iii)) provides existence evidence for a hash the orchestrator supplies explicitly via `--durable-commit` on the Sequence 5 hygiene-only run (see below): summary `exists: <hash>` = hash is reachable (Pass); `missing: <hash>` = git object absent (Fail); `not supplied` = the flag was omitted (NotRun) — which makes the hygiene-only receipt not-all-pass and fires the STOP below. **The binary only asserts existence — it does NOT authorize deletion.** Deletion stays orchestrator-executed after ORCHESTRATOR reads the receipt and confirms the hash evidence independently. The `Pass` state of check(iii) is necessary but not sufficient for deletion: STEWARD must also have signalled and ORCHESTRATOR must verify the hash matches the Seq 2 durable-layer commit before proceeding.

#### Sequence 5 entry — Bounded `.dev/state.md` close-out write

After doc-sync (Seq 2) and merge/teardown (Seq 3-4, when applicable), ORCHESTRATOR writes the close-out row into `.dev/state.md`. Authority for the shape: `conventions/token-budget.md` § Bounded Session State — this step does not restate that policy, only performs it:

1. **Remove every exact-predicate legacy comment** — any HTML comment opening `<!-- YYYY-MM-DD:` whose body contains both `finalized on main` and `ABSORBED`. A comment merely discussing those words without the date-prefixed opening is untouched.
2. **Upsert one compact row** into `## Recent Close-outs` (`Date | Plan | Landing | Result`), creating the section when absent (an existing repo migrating for the first time). `Landing` = the durable-layer commit hash from Seq 2 (the same hash passed to `--durable-commit` below, whose `exists: <hash>` the hygiene-only receipt's check(iii) confirms).
3. **Trim to the newest two rows** — when a third row would be added, drop the oldest.

#### Post-write hygiene-only receipt (stop-line before deletion)

Run `gal finalize-check <plan-or-prompt> --hygiene-only --durable-commit <hash> --receipt <path>` (default: gitignored `.dev/pipeline/receipts/finalize-check.hygiene.receipt.md` — a distinct file from the Step 1 full-mode receipt; both are retained as evidence, neither overwrites the other), where `<hash>` is STEWARD's Sequence 2 durable-layer commit. When Sequence 2 produced no separate durable-layer commit (the already-on-main case acknowledged in Sequence 6 below), `<hash>` is `git rev-parse HEAD` instead, still meaning "the last committed change before deletion". This closes the full time-of-check window: repo hygiene drift introduced by the holistic-review fix commits, doc-sync, or the merge itself is caught here, **before** anything is deleted — not just at the Step 1 precondition, which ran before any of those steps.

- **Every applicable row must be `pass`.** `project-source-doc-existence`, `state-bound`, and `durable-layer-commit` always apply; `contract-roster-parity` and `doc-link-resolution` apply only when `plugins/gal-core/` exists at the repo root (this repo). `state-bound` in hygiene-only mode requires the `## Recent Close-outs` section to be present (no legacy-shape tolerance) — the Sequence 5 entry step above must have already run.
- **On fail → STOP.** Keep the plan files. Do not proceed to deletion or the last-good tag. Repair the specific failing row (re-run the Sequence 5 entry step if the state write was incomplete, or fix the specific drift the row names), then re-run the hygiene-only receipt until it passes. This is a genuine stop-line, not a retry-and-ignore: never delete plan files against a failing hygiene-only receipt.

#### Sequence 5 close — Lifecycle close

- **STEWARD** confirms durable knowledge has landed in the durable documentation layer (`README.md` + `docs/`, excl. `.dev/plans/` + `.dev/research/`), **records the durable-layer commit hash** in the report evidence block, and signals ORCHESTRATOR only when the hash is present.
- **ORCHESTRATOR** may delete plan files **only after** (a) receiving the commit hash evidence from STEWARD, passing that hash to `--durable-commit` on the post-write hygiene-only run above, and confirming that receipt's check(iii) reports `exists: <hash>` for that same hash, **and** (b) the post-write hygiene-only receipt above reports `pass` on every applicable row. Then marks the plan `ABSORBED`, deletes `.dev/plans/<slug>.md` + `.dev/plans/<slug>.prompt.md`, and confirms `.dev/state.md` already carries the close-out row written above.
- **Plan-file deletion is the last step** so the resume marker survives its needed window. Never delete a plan that has not passed every prior step, including the durable-layer commit gate and the post-write hygiene-only receipt.

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
Precondition:    finalize-check PASS (mode: full)   (evidence: .dev/pipeline/receipts/finalize-check.receipt.md — all checks pass)
Holistic review: CLEAR                  (evidence: per-file diff verdicts; Review Independence: <full|DEGRADED_SAME_RUNTIME>)
Doc-sync:        done                    (evidence: durable-layer commit <hash>; sync idempotency = pass)
Merge:           done | not-applicable   (evidence: merge commit <hash> | already-on-main)
State write:     done                    (evidence: .dev/state.md ## Recent Close-outs upserted + trimmed to <=2 rows)
Post-write hygiene: PASS (mode: hygiene-only)  (evidence: .dev/pipeline/receipts/finalize-check.hygiene.receipt.md — all applicable rows pass)
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
| Merge conflict, unmerged path set exactly `{.dev/state.md}` | `gal state-merge` — exit 0 continues; `STATE_MERGE: unresolved` STOPs with repository state proven intact; `STATE_MERGE: rollback-unconfirmed` STOPs and names possible mutation requiring inspection |
| Merge conflict (any other path set) | STOP + Interrupted-Phase handoff — human resolves, never auto-resolve |
| BLOCKING security / Protected Path finding in holistic review | STOP immediately — human required |
| Post-write hygiene-only receipt fails any applicable row | STOP — keep plan files, repair the named drift, re-run until pass; never delete against a failing receipt |
| Already on main | Skip merge + teardown — continue to the bounded state write |
| Clean completion | Plan LANDED — ABSORBED + files deleted |
