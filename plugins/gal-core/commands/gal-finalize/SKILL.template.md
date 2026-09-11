---
name: gal-finalize
description: "GAL finalize ($gal-finalize / /gal finalize / finalize / 完成計劃 / 落地). Lands and closes a VERIFIED plan: full-mode gal finalize-check receipt → top-down review → doc-sync → merge/teardown → bounded state write → hygiene-only receipt → test-first cleanup → lifecycle close → last-good tag. Distinct from $gal-wrap-up (session pause / 暫停). Precondition: all tasks [x] + ORCHESTRATOR goal-backward VERIFIED."
---

# /gal finalize

Land a plan that is **actually done** and close out its lifecycle. `finalize` is a thin orchestrator: it runs a hard full-mode precondition gate, then sequences one top-down four-layer review, run in finalize's own runtime, → doc-sync → (if on a worktree/feature branch) merge-to-main + teardown → bounded `.dev/state.md` write + a distinct post-write hygiene-only receipt → deterministic test-first cleanup → lifecycle close → last-good tag. The only genuinely-new pieces are the **orchestration** of the top-down cross-task review and the merge-to-main step.

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

- **ORCHESTRATOR** — owns end-of-run goal-backward verification (the finalize *precondition*) and lifecycle close (ABSORBED marking + plan-file deletion + `.dev/state.md` close-out row). ORCHESTRATOR is **not** a top-down review owner.
- **STEWARD** (`golem-steward`) — owns documentation-structure: knowledge extraction → `docs/` + `.dev/project.md`.

Sequence 1 runs in the finalize runtime itself — it is not dispatched to AUDITOR or golem-architect.

Goal-backward verification is orchestrator-owned and always runs in-process.

## Step 1 — Precondition Gate (hard-block, no action)

Resolve the active plan the same way `/gal pipeline` does: explicit `@<plan>` / `#file:<plan>` argument first, else the active plan from `.dev/state.md`; if an argument points at a source plan and the paired `.dev/plans/<slug>.prompt.md` exists, use the execution prompt.

**Entry signal, not a pass basis.** finalize's entry condition is the pipeline's terminal receipt `.dev/pipeline/receipts/<plan-scope-key>/pipeline-handback-check.receipt.md` carrying `decision: ready-to-finalize` and `final_authorized: true`. Step 1 reads those two fields and nothing else from it. It does not compare the receipt's `head` to the current HEAD, because Sequence 1 reads the actual branch diff and a docs-only commit landed after handback changes nothing the review would miss. The receipt is an entry signal only, never a pass basis. finalize performs no task-level re-derivation: it does not re-run cross-surface checkbox agreement, task-commit existence, cursor state, executor-log classification, or test-first evaluation for any task.

- **Self-bootstrap first (mandatory when the plan touched `crates/`).** If any of the plan's changes are under `crates/`, **rebuild + reinstall `gal` before checking**, wrapping the rebuild and every install target inside an advisory install lease. Running a gate binary against pre-change code produces a receipt that describes the old binary, not the work being finalized — any `pass` from a stale binary is evidence of nothing. The lease covers every install target: canonical root `~/.gal/plugins/gal/`, `~/.cargo/bin/gal.exe`, Claude plugin-cache copy when present. Rebuild and installation sit inside the lease in this order:

  1. Create the marker's parent `~/.gal/.locks/` (this step may use `-Force` / `-p`).
  2. Acquire the lease by creating `~/.gal/.locks/gal-install/` with the exact commands in the table below and no `-Force` / `-p`.
  3. On "already exists" poll every `5 seconds` up to `900 seconds`, then stop and surface the marker path without removing it.
  4. Run `cargo build --release` and install to all three targets — canonical root, `~/.cargo/bin/gal.exe`, Claude plugin-cache copy when present.
  5. Record the pinned hash while still holding the lease by resolving the executable with the command in the table below and hashing it with SHA-256, storing `gal --version` output beside it as context that never participates in the comparison.
  6. Release the marker on both the success and the error path, reporting the exact path if the release itself fails.
- **Record the pinned hash.** With no Rust source touched the orchestrator takes no lease and runs no build, but still resolves and hashes the executable with the commands in the table below before its first gate command, so a pinned hash always exists.

| Step | PowerShell | Bash |
| --- | --- | --- |
| Create the marker's parent (may already exist) | `New-Item -ItemType Directory -Force -Path "$env:USERPROFILE\.gal\.locks"` | `mkdir -p "$HOME/.gal/.locks"` |
| Acquire the lease (never `-Force` / `-p`) | `New-Item -ItemType Directory -Path "$env:USERPROFILE\.gal\.locks\gal-install"` | `mkdir "$HOME/.gal/.locks/gal-install"` |
| Resolve the executable | `(Get-Command -CommandType Application gal \| Select-Object -First 1).Source` | `command -v gal` |
| Hash it | `(Get-FileHash -Algorithm SHA256 <path>).Hash` | `sha256sum <path>`, or `shasum -a 256 <path>` on macOS |

Three traps stated as rules, not left to the reader:
- `-Force` and `-p` are banned on the acquire step only. Both return success when the directory exists, turning the lease into a no-op with no error and no signal. They are required on the parent step, which is not the lock.
- `-CommandType Application` is mandatory in the PowerShell resolve. Bare `Get-Command gal` resolves to PowerShell's `gal` alias for `Get-Alias` and returns an empty `Source`, failing the hash step.
- Hash comparison is case-insensitive. `Get-FileHash` returns uppercase hex, `sha256sum` lowercase; same digest for the same file, so normalise before comparing.

Manual recovery for a stale marker left by a crashed run: an operator confirms no other GAL pipeline or finalize run is active, then removes the marker directory by hand. Always a human action. Never instruct an orchestrator to auto-remove, because auto-removal defeats the lease the first time a slow build is mistaken for a crash.
- **Run the gate binary in full mode:** `gal finalize-check <plan-or-prompt>` (default receipt: gitignored `.dev/pipeline/receipts/<plan-scope-key>/finalize-check.receipt.md`; explicit receipt output is the only permitted write by the checker command) — **never pass `--hygiene-only` here**; the precondition gate requires the complete check set. The receipt's own `mode: full` line is part of the evidence — a `mode: hygiene-only` receipt cannot satisfy this precondition. Full mode emits exactly these rows in this order and no others: `authoritative-command`, `naming-gate`, `sync-idempotency`, `finalize-mode`, `project-source-doc-existence`, `state-bound`, `contract-roster-parity`, `doc-link-resolution`, `working-tree-clean`. No row is per task. `contract-roster-parity` and `doc-link-resolution` appear only when `plugins/gal-core/` exists at the repo root, so a downstream repo sees seven rows.
- **The receipt is the sole pass-basis.** Before trusting any gate command's receipt, re-resolve the executable and re-hash it with the commands in the self-bootstrap paragraph above, and compare case-insensitively against the pinned hash recorded during self-bootstrap. On mismatch, stop, do not trust the gate result that produced it, and report both `gal --version` strings to the human as context. Proceed only when the binary exits zero **and** every check in the receipt is `pass`. **Any `fail` or `not-run` → HARD-BLOCK** — surface the offending check's **real command output** from the receipt (not a paraphrase). Exit 0 is necessary but the per-check states are authoritative; never wave a check through on a recorded claim.

The gate checks **all** of (the binary records mode in check(f); see point 2):

1. **Working tree clean** — `git status --porcelain` is empty.
2. **Mode detected (dual-mode)** — the receipt's `finalize-mode` check (check(f), state always `Pass`, pure informational report) records `worktree` or `already-on-main` based on `git branch --show-current`. Read its `summary` field to determine whether sequence steps 3–4 apply. The `Pass` state is never a gate condition — mode-detect is reporting only.

The gate **never acts on a failed gate**: this is the **no-action-on-failed-gate** rule. It hard-blocks, surfaces the failing receipt row and its real command output, and takes no repair, mutation, merge, teardown, deletion, or re-verification action in the same invocation. Recovery is a single route: when a repo-level row fails, fix the named row, then re-run the gate.

Only when both checks pass does finalize proceed to Step 2.

## Step 2 — Completion Sequence (1–5)

Review **must** precede merge: unreviewed work never lands.

### Sequence 1 — Top-down zero-trust review (whole branch, once)

This is the one current gap finalize closes: per-task audit passing does **not** guarantee the combined branch is coherent. finalize runs a single **top-down, requirement-by-requirement** review over the whole branch, once, in the finalize runtime itself — **no dispatch, no new agent**.

**Permitted inputs.** The source plan's `## Goal`, `## Requirements`, `## Success Criteria` and `## Test Plan` table (a planning-stage contract declaring each probe's Type, not a recorded result), the branch diff (`git diff main...HEAD`, or `git diff gal-last-good...HEAD` when already on main — Sequence 6 advances the `gal-last-good` tag to the landing commit at every finalize, so that range is everything changed since the previous landing), the working tree, and the live output of the authoritative command run once during this review.

**Forbidden inputs.** Every receipt under `.dev/pipeline/receipts/`, every verdict line in `## Test Results` and `## Review Results`, `## Status`, and the verdict lines of `### Handoff Notes`. The Step 1 entry-signal receipt is read before Sequence 1 begins and is not an input to the review. The reviewer derives the must-haves itself from the permitted inputs rather than reusing the goal record's `must_have_1..N`, which is a forbidden input.

**Four layers, per requirement.**

- **L1 Truths** — the requirement is an observable behaviour: one diff hunk plus one test name that reaches the behaviour through its production caller. When the `## Test Plan` row covering the requirement declares Type `integration` or higher, the cited test must be of that Type — a unit test standing in for an integration oracle is a FAIL at L1. When every `## Test Plan` row covering the requirement declares Type `manual`, there is no test name to cite: the L1 evidence is one diff hunk plus the live output of that manual probe, with no backticked test name in the L1 cell.
- **L2 Files** — what must exist for L1 exists and is not a stub: a path plus the non-stub proof.
- **L3 Wiring** — what must be connected is connected and reachable. Cross-task seams, documentation-versus-code checks, `.dev/project.md` and adapter freshness, and scope drift against `## Requirements` live here.
- **L4 Trust boundaries** — every new input source, file write, command execution, path derivation and external call the branch introduces, listed as its own row with one STRIDE judgment.

The authoritative command runs once and its live output is cited.

**Write-back.** Write into the **execution prompt's** `## Review Results` — never the source plan. `finalize-check`'s `finalize-review-shape` row (hygiene-only mode) resolves whatever path it is given straight to `<slug>.prompt.md` and reads only that file; a Finalize Review written to `.dev/plans/<slug>.md` is invisible to it and the row fails `no ### Finalize Review heading found`, even though the plan-stage reviews (Architecture/Business/Design/Engineering) do live in the source plan — do not follow that precedent here. Add `### Finalize Review <date>` holding a table whose header cells are **exactly** `Requirement | L1 | L2 | L3 | L4` (verbatim column names, checked by exact string match), one data row per `## Requirements` entry, then a findings list where each finding carries a severity and `blocking: yes|no`, then one `Review Independence:` line. Each cell is one of exactly three forms: `PASS — <evidence>`, `FAIL — <evidence>`, or `N/A — <reason>`, with non-empty text after the separator — a bare `PASS`/`FAIL`/`N/A`, or a cell with only whitespace after the separator, is an empty cell. An L1 cell is never `N/A`. A row whose four cells are all `N/A` is invalid. **Every backticked token containing `_` inside an L1 cell must be a real identifier** — a function name, module name, or filename that actually exists under `crates/` — or the row fails citation resolution (skipped when the repo has no `crates/` tree); do not backtick a paraphrase or a made-up name. Any empty cell is a reviewer failure: Sequence 1 is re-run, and no implementer is dispatched. A blocking finding triggers the REVISE stop-line below.

On **REVISE**: stop-line (jidoka) — dispatch `golem-implementer` to fix, then re-review to clean, then commit the fix **on the branch**. Do not proceed to merge with an open REVISE.

### Sequence 2 — doc-sync

- **doc-sync is MANDATORY, never skipped.** **STEWARD** extracts durable knowledge into the **durable documentation layer** (`README.md` + all of `docs/`, excluding `.dev/plans/` and `.dev/research/`) on **every** finalized plan; `.dev/project.md` is then re-synced as a compressed current-topic index (upsert/replace/prune, never a history append — see `conventions/token-budget.md` § Bounded Current-Topic Index) **after** the durable layer is updated. STEWARD's reindex runs **first, before any adapter-render evidence is collected** — the reindex can change `.dev/project.md`'s content, which flows verbatim into every generated adapter, so evidence collected before the reindex is stale evidence of the wrong content. "doc-sync might cause drift" is **not** a reason to skip — the opposite: skipping it is what leaves the adapters stale. Commit the doc changes on the branch.
- **Provider-memory harvest is pointer-only here.** STEWARD may additionally consider candidates already approved in the plan's `### Handoff Notes` → `#### Approved Memory-Harvest Candidates` (see `conventions/token-budget.md` → Provider-Memory Harvest); finalize performs **no** provider acquisition of its own. Approval is never itself promotion evidence — a candidate is promoted only after STEWARD independently proves the same verified-root-cause-plus-credible-recurrence gate as any other extraction; a gate miss omits the candidate without blocking doc-sync or finalize.
- **Post-reindex double render, two-adapter evidence.** After STEWARD's reindex lands (and after `.dev/project.md`'s normalized size is confirmed within the 30,720 B budget — the render write-boundary rejects an oversized index before any adapter write, so a render failure here means the reindex itself needs trimming, not a retry), re-run `gal finalize-check <plan-or-prompt>` in full mode. That command **is** the mechanism — its `sync-idempotency` row runs the in-process repo-adapter render **twice** and confirms both generated adapters (`AGENTS.md`, `CLAUDE.md`) are byte-identical across the two runs. The render is read-only and writes no adapter file. Do **not** hand-roll a render command for this step. Do **not** cite the precondition-gate's earlier candidate render as this evidence — that render ran before the reindex and reflects the pre-doc-sync `.dev/project.md`, not the reindexed one. Before trusting any gate command's receipt, re-resolve the executable and re-hash it with the commands in the self-bootstrap paragraph above, and compare case-insensitively against the pinned hash recorded during self-bootstrap. On mismatch, stop, do not trust the gate result that produced it, and report both `gal --version` strings to the human as context. Cite the fresh post-reindex receipt's `sync-idempotency` row as the proof that doc-sync produced a stable, drift-free adapter set from the **reindexed** content. If that row reports `drifted>0`, the on-disk adapters no longer match the reindexed index: regenerate them with `gal render-adapters`, then run `gal finalize-check` again and cite the new row. A non-idempotent render, or a render rejected on the size budget, → STOP (doc-sync is not done until the post-reindex render is stable).

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

### Sequence 5 — Bounded state write, post-write hygiene gate, cleanup, and lifecycle close (delegated)

**Durable-layer commit is the pre-deletion gate.** ORCHESTRATOR may not delete plan files until STEWARD has provided a concrete durable-layer commit hash confirming the extraction landed. A "STEWARD confirmed verbally" or "doc-sync ran" without a commit hash is insufficient — the hash is the evidence.

The receipt's `durable-layer-commit` check (check(iii)) provides existence evidence for a hash the orchestrator supplies explicitly via `--durable-commit` on the Sequence 5 hygiene-only run (see below): summary `exists: <hash>` = hash is reachable (Pass); `missing: <hash>` = git object absent (Fail); `not supplied` = the flag was omitted (NotRun) — which makes the hygiene-only receipt not-all-pass and fires the STOP below. **The binary only asserts existence — it does NOT authorize deletion.** Deletion stays orchestrator-executed after ORCHESTRATOR reads the receipt and confirms the hash evidence independently. The `Pass` state of check(iii) is necessary but not sufficient for deletion: STEWARD must also have signalled and ORCHESTRATOR must verify the hash matches the Seq 2 durable-layer commit before proceeding.

#### Sequence 5 entry — Bounded `.dev/state.md` close-out write

After doc-sync (Seq 2) and merge/teardown (Seq 3-4, when applicable), ORCHESTRATOR writes the close-out row into `.dev/state.md`. Authority for the shape: `conventions/token-budget.md` § Bounded Session State — this step does not restate that policy, only performs it:

1. **Remove every exact-predicate legacy comment** — any HTML comment opening `<!-- YYYY-MM-DD:` whose body contains both `finalized on main` and `ABSORBED`. A comment merely discussing those words without the date-prefixed opening is untouched.
2. **Upsert one compact row** into `## Recent Close-outs` (`Date | Plan | Landing | Result`), creating the section when absent (an existing repo migrating for the first time). `Landing` = the durable-layer commit hash from Seq 2 (the same hash passed to `--durable-commit` below, whose `exists: <hash>` the hygiene-only receipt's check(iii) confirms).
3. **Trim to the newest two rows** — when a third row would be added, drop the oldest.
4. **Upsert every non-blocking finding into `## Follow-ups`** — for every finding recorded under `### Finalize Review <date>` (Sequence 1) with `blocking: no`, upsert one row into `## Follow-ups`, creating the section when absent, newest first. Each row's `Finding` cell holds the finding as one line with any `|` escaped. Trim to the newest 5 rows. Authority for the cap and the exit rule: `conventions/token-budget.md`.

#### Post-write hygiene-only receipt (stop-line before deletion)

Run `gal finalize-check <plan-or-prompt> --hygiene-only --durable-commit <hash>` (default: gitignored `.dev/pipeline/receipts/<plan-scope-key>/finalize-check.hygiene.receipt.md` — a distinct file from the Step 1 full-mode receipt; both are retained as evidence, neither overwrites the other), where `<hash>` is STEWARD's Sequence 2 durable-layer commit. When Sequence 2 produced no separate durable-layer commit (the already-on-main case acknowledged in Sequence 6 below), `<hash>` is `git rev-parse HEAD` instead, still meaning "the last committed change before deletion". This closes the full time-of-check window: repo hygiene drift introduced by the top-down review's fix commits, doc-sync, or the merge itself is caught here, **before** anything is deleted — not just at the Step 1 precondition, which ran before any of those steps. Before trusting any gate command's receipt, re-resolve the executable and re-hash it with the commands in the self-bootstrap paragraph above, and compare case-insensitively against the pinned hash recorded during self-bootstrap. On mismatch, stop, do not trust the gate result that produced it, and report both `gal --version` strings to the human as context.

- **Every applicable row must be `pass`.** The hygiene-only receipt row inventory maps one-to-one to the binary output rows: universal rows `project-source-doc-existence`, `state-bound`, `durable-layer-commit`, and `finalize-review-shape` plus `contract-roster-parity` and `doc-link-resolution` (which apply only when `plugins/gal-core/` exists at the repo root), ending with the final post-write `working-tree-clean` cleanliness row. Both full-mode and hygiene-only mode run `working-tree-clean` as their terminal evaluator row, verifying that `git status --porcelain` is clean after all in-process renders, state writes, or file operations. `state-bound` in hygiene-only mode requires the `## Recent Close-outs` section to be present (no legacy-shape tolerance) — the Sequence 5 entry step above must have already run. `finalize-review-shape` re-checks the `### Finalize Review <date>` table's mechanical shape (row count, cell grammar, the `Review Independence:` line). A failure on this row means Sequence 1's write-back was malformed and must be re-run — it is never a signal to change the branch's code.
- **On fail → STOP.** Keep the plan files. Do not proceed to deletion or the last-good tag. Repair the specific failing row (re-run the Sequence 5 entry step if the state write was incomplete, or fix the specific drift the row names), then re-run the hygiene-only receipt until it passes. This is a genuine stop-line, not a retry-and-ignore: never delete plan files against a failing hygiene-only receipt.

#### Test-first cleanup stop-line (after hygiene)

Only after the top-down review, doc-sync, bounded state write, and hygiene-only receipt have each passed, run the registered deterministic cleanup command:

```bash
gal test-first-cleanup run --plan <slug> --finalized --receipt <cleanup-receipt-path>
```

Read the cleanup receipt and require `overall: pass`. `test-first-cleanup` prevalidates all optional receipt and snapshot roots, entry names, and cleanup-owned identity grammar before making any deletions. On any prevalidation failure, foreign file, link ambiguity, or failed/missing cleanup receipt (`overall: fail`), **STOP** immediately with zero mutation (no files deleted); keep the plan files and no lifecycle step can proceed after a failed cleanup receipt. Cleanup is plan-scoped to the finalized slug and removes only the validated test-first receipt and snapshot roots.

#### Sequence 5 close — Lifecycle close

- **STEWARD** confirms durable knowledge has landed in the durable documentation layer (`README.md` + `docs/`, excl. `.dev/plans/` + `.dev/research/`), **records the durable-layer commit hash** in the report evidence block, and signals ORCHESTRATOR only when the hash is present.
- **ORCHESTRATOR** may delete plan files **only after** (a) receiving the commit hash evidence from STEWARD, passing that hash to `--durable-commit` on the post-write hygiene-only run above, and confirming that receipt's check(iii) reports `exists: <hash>` for that same hash, **and** (b) the post-write hygiene-only receipt above reports `pass` on every applicable row (including the final post-write `working-tree-clean` row), **and** (c) the test-first cleanup receipt reports `overall: pass`. No lifecycle step can proceed after a failed cleanup receipt. Then marks the plan `ABSORBED`, deletes `.dev/plans/<slug>.md` + `.dev/plans/<slug>.prompt.md`, and confirms `.dev/state.md` already carries the close-out row written above.
- **Plan-file deletion is the last step** so the resume marker survives its needed window. Never delete a plan that has not passed every prior step, including the durable-layer commit gate, the post-write hygiene-only receipt, and the test-first cleanup stop-line.

### Sequence 6 — Write `gal-last-good` marker

After all prior sequences succeed (top-down review CLEAR + doc-sync + lifecycle close), advance the lightweight git tag to the landing commit so `gal restore` has a trustworthy known-good baseline:

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
Precondition:    finalize-check PASS (mode: full)   (evidence: .dev/pipeline/receipts/<plan-scope-key>/finalize-check.receipt.md — all checks pass)
Finalize review:  CLEAR                  (evidence: `### Finalize Review <date>` table all-pass; `finalize-review-shape` row pass; Review Independence: <full|DEGRADED_SAME_RUNTIME>)
Doc-sync:        done                    (evidence: durable-layer commit <hash>; sync idempotency = pass)
Merge:           done | not-applicable   (evidence: merge commit <hash> | already-on-main)
State write:     done                    (evidence: .dev/state.md ## Recent Close-outs upserted + trimmed to <=2 rows)
Post-write hygiene: PASS (mode: hygiene-only)  (evidence: .dev/pipeline/receipts/<plan-scope-key>/finalize-check.hygiene.receipt.md — all applicable rows pass)
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
| Precondition gate miss | HARD-BLOCK — fix the named row, then re-run the gate — no action on a failed gate |
| Top-down review returns REVISE | STOP-line — fix on branch, re-review, then continue; never merge with open REVISE |
| Merge conflict, unmerged path set exactly `{.dev/state.md}` | `gal state-merge` — exit 0 continues; `STATE_MERGE: unresolved` STOPs with repository state proven intact; `STATE_MERGE: rollback-unconfirmed` STOPs and names possible mutation requiring inspection |
| Merge conflict (any other path set) | STOP + Interrupted-Phase handoff — human resolves, never auto-resolve |
| BLOCKING security / Protected Path finding in the top-down review | STOP immediately — human required |
| Post-write hygiene-only receipt fails any applicable row | STOP — keep plan files, repair the named drift, re-run until pass; never delete against a failing receipt |
| Test-first cleanup fails or has no passing receipt | STOP — keep plan files, repair or resume the scoped cleanup, and never enter lifecycle close |
| Already on main | Skip merge + teardown — continue to the bounded state write |
| Clean completion | Plan LANDED — ABSORBED + files deleted |
