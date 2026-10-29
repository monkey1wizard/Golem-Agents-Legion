# Token Budget

For chat and documents, prioritize accuracy, then clarity, then brevity.
Compression must not remove actors, conditions, negation, causality, scope, or
uncertainty. Use complete relationships at every response stage, including
progress and handbacks. See [writing-quality.md](writing-quality.md).

Rules for managing context window and token consumption across all agents.

## Bounded Format Recovery

This section is the canonical contract for catalogued format recovery during
planning, execution, and finalize. It repairs supported presentation defects
only. It never supplies missing facts, approval, task content, evidence, or
gate authority.

### Invocation budget

At the start of the outer workflow invocation, create one stable
`invocation_id` and set the workflow-owned consumed-round counter to `0`.
Carry both through nested gate reruns, phase loops, and host-close entries.
Never reset them inside the invocation. At most two rounds may be consumed
across all document and phase-payload repair in one invocation. The counter
is separate from the receipt's `repair_round` field, which the runtime binds
to `0`.

A document round is one `gal format-repair` call followed by a fresh rerun of
the original gate. It is successful only when the helper commits and the
rerun passes. A helper refusal, a helper failure, or a failing rerun makes the
round unsuccessful. Advance the counter once per document round, after the
helper exits, whatever the outcome.

A payload round is one write-back whose runtime prints a `FORMAT_REPAIR` line.
Advance the counter from that line. The round is successful only when
validation and placement complete.

### Receipt dispositions

A failed or not-run gate receipt ends with a `## Repair disposition` section.
A bound section carries `repair_context: bound`, `gate`, `mode`,
`gate_attempt_id`, `repair_round`, and one `repair_target` line per bound file
with its raw SHA-256. Each failed or not-run row then reads either
`- <check>: format (repair_id: <repair>, target: <path>)` or
`- <check>: owner (repair_id: none)`. An unbound section, an unknown check, an
operational error, and an execution-envelope failure are always `owner`.
Advisory rows never receive a disposition.

A row is `format` only when the runtime's eligibility probe succeeds on the
exact bound bytes. The probe builds the candidate in memory, proves the
protected records unchanged and the repair idempotent, and reruns the failed
check's existing validator on the candidate. The closed catalog is:

| Gate | Mode | Failed check | Repair | Target |
| --- | --- | --- | --- | --- |
| `planning` | any | `required-sections` | `leading-bom` | source plan |
| `planning` | any | `machine-anchor-parity` | `bullet-shape` | source plan |
| `refining` | any | `id-well-formed` | `bullet-shape` | source plan |
| `refining` | any | `test-plan-coverage`, `owner-acceptance`, `preconditions` | `table-shape` | source plan |
| `prompt` | any | `tasks-heading` | `leading-bom`, then `heading-level` | execution prompt |
| `prompt` | any | `unchecked-task-anchor` | `bullet-shape` | execution prompt |
| `preflight` | `ordinary` | `tasks-well-formed` | `bullet-shape` | execution prompt |
| `preflight` | `ordinary` | `retry-handoff-shape` | `retry-field-shape` | execution prompt |
| `finalize` | `hygiene-only` | `finalize-review-shape` | `table-shape` | execution prompt |

Every other gate, mode, and check stays `owner` on purpose. The `boundary`,
`converge`, `handback`, and quickfix receipts have no catalog entry, because
their failures are scope, convergence, or command results and not
single-target presentation defects. Preflight in `terminal-reverify` mode and
finalize in `full` mode have no entry either, so terminal-reverify stays
read-only and a failed full-mode finalize row follows its owner route.

### Document repair

Inspect the complete failed receipt before acting. Repair only when every
failed or not-run row is `format`, the section is bound, and the budget has a
round left. Any `owner` row refuses the entire set. Do not partially repair
the eligible rows. The helper enforces the same rule and refuses a receipt
that holds any `owner` row.

A successful helper commit makes every `repair_target` hash in that receipt
stale. One failed receipt therefore authorizes exactly one helper call. Take
the first `format` row in receipt order and run:

```text
gal format-repair --target <target> --receipt <failed-receipt> --repair-id <repair> --gate <gate> --mode <mode> --attempt <gate_attempt_id> --round <repair_round>
```

Copy every value from that row and the receipt's repair context. Do not infer
a value from prose or reuse a gate attempt. The helper verifies its executable
envelope, the receipt context, the target hashes, and the declared row. It
reruns the same eligibility probe on the bound bytes and refuses dirty
targets, preexisting staged entries, and concurrently changed bytes. Before
publication it appends one deterministic `Deviation` row when the target has a
`### Deviations` table, and it stamps planning metadata when the target
carries it. A target without that table gets no row, and the helper commit
`docs: repair <gate> formatting` is its audit record. The helper commits the
exact repaired target itself. Do not append, stamp, commit, or remove a lease
before or after it.

Then rerun the original gate in its original mode and require a fresh
receipt. A prior pass never passes the rerun. Any `format` row left in the
fresh receipt starts the next round under the same rules.

Preflight is an eligible gate. A fresh preflight issues a new `run_id`. A
resume-pinned preflight keeps the pinned `run_id`. Keep the active `run_id`
stable during execution and use a distinct `gate_attempt_id` for every gate
attempt. During the task loop the boundary and converge receipts mark every
failure `owner`, so the loop itself never calls the helper.

On a resume, a preflight helper commit moves HEAD past the recorded
`Task Final Commit`. The helper commit changes only the running plan's
execution prompt, which the boundary check treats as a workflow-state path.
After the preflight rerun passes, set `Task Final Commit` to HEAD before any
TEST or AUDIT dispatch, because dispatch requires `Task Final Commit` to equal
HEAD. Do not change `Task Base Commit`. A later `Task Final Commit drift` stop
follows the commit checks in the stop table below.

In finalize, record `git rev-parse HEAD` immediately after the helper exits 0
as the helper commit. Then rerun `gal finalize-check --hygiene-only` with the
same target and `--durable-commit` value. Accept the rerun only when its
receipt reads `mode: hygiene-only` and `overall: pass`, its
`working-tree-clean` row passes, and `git rev-parse HEAD` still equals the
recorded helper commit. The finalize receipt binds no `head` field, so Git is
the commit evidence.

### Payload repair

For a phase-payload write-back, pass
`--format-repair-context '{"invocation_id":"<invocation_id>","round":<next_round>}'`
only to `gal pipeline <prompt>`, where `<next_round>` is the consumed count
plus one. The runtime accepts only rounds `1` and `2`. When both rounds are
consumed, omit the option. Ordinary write-back still runs, and payload repair
is disabled. Never pass the option to executor raw arguments or
`gal format-repair`.

The current write-back owner derives normalized bytes in memory from the
verified immutable receipt snapshot. The only payload repairs are removing one
leading UTF-8 BOM and adding `Evidence:` to a unique existing contiguous
`Command:` / `Result:` pair. It preserves the sealed original, prints
`FORMAT_REPAIR` with the invocation, round, repair, original hash, and derived
hash, and runs the normal validator and placement once under the held prompt
lease. It never calls the document helper or takes a second lease. Several
candidate pairs, a stray `Command:` or `Result:` line, or several `Evidence:`
markers are refused before any round is consumed. That refusal prints no
`FORMAT_REPAIR` line, leaves the counter unchanged, and fails the write-back
with its error. Payload bytes stay untracked evidence, need no `Deviation`
row, and create no commit.

### Stop and next action

Stop when any row is `owner`, when the budget is spent and a gate still fails,
or whenever eligibility, consumption, custody, publication, rollback, or fresh
gate evidence is uncertain. Never start a third round. Keep the last failed
receipt, preserve executor output, and report residual paths. Never restore
executor output, delete unrelated files, undo a successful commit, or claim a
stale pass.

The stop report names the failed check, the receipt path, the row summary or
the helper's `gal format-repair: <reason>` line as evidence, what was verified,
and one exact next action:

| Stop | Exact next action |
| --- | --- |
| `owner` row in a planning, refining, or prompt receipt | The plan owner corrects the content the row summary names in the bound target, then reruns that gate command. |
| `owner` row in a preflight receipt | The plan owner corrects the content the row summary names in the execution prompt, commits it, then reruns `/gal pipeline <prompt>`. |
| `owner` row in a boundary, converge, or handback receipt | Follow the phase protocol and handback rule that gate already defines. |
| `owner` `finalize-review-shape` row | When the summary names the Finalize Review heading, table, cells, or `Review Independence:` line, rerun Sequence 1 with `gal pipeline <prompt> --phase finalize-review`, commit its write-back, then rerun `gal finalize-check --hygiene-only`. When it names the `## Requirements` count, the plan owner corrects the requirement bullets in a remediation commit before that rerun. |
| Other `owner` finalize row | Follow the finalize failure recovery route for its failure class. |
| Helper refusal or failure | The plan owner corrects the defect the helper's reason names in the bound target, then reruns the original gate. |
| Budget spent and a gate still fails | The plan owner corrects the rows the last receipt names, then starts a new invocation of the same workflow. |
| Payload refusal | Report the write-back error. The phase follows its normal retry ceiling. |
| TEST or AUDIT dispatch stops with `Task Final Commit drift` after a resume | Check every commit `c` in `<recorded Task Final Commit>..HEAD`. Each `c` must have the subject `docs: repair preflight formatting`. `git diff --name-only c^ c` must list only the execution prompt `.dev/plans/<slug>.prompt.md`. The commit must add exactly one `Formatting-only repair` `Deviation` row. When every commit meets all three conditions, set `Task Final Commit` to HEAD and dispatch again once. Otherwise stop, record a `Deviation` row, and hand back to the human. |
| Uncertain consumption, custody, or publication | Report the residual paths and the last observed counter. The owner resolves them before any rerun. |

Task test and review failures keep their normal retry ceilings. Root
`BLOCKING`, `Workflow: DONE`, missing content or active-plan rows, owner
acceptance, custody, execution binding, protected paths, and task-retry gates
stay intact. Use the applicable human handback contract when its producer and
reason apply.


---

## Cold Start Priority

When an agent begins a session, load context in this order (stop when sufficient):

1. **`.dev/project.md`** — compressed project summary + indexes
2. **`.dev/state.md`** — active plan index + session continuity
3. **Active plan file** — `.dev/plans/<plan-slug>.prompt.md` (AI execution work file) with `## Status` section
4. **Source docs** — only when `project.md` explicitly references them for the current task
5. **Source code** — only files relevant to the current plan step

Do **not** read `README.md`, `docs/`, or full codebase on cold start. Let `project.md` guide what to load.

## During Work

- **Summarize early**: When a conversation exceeds ~20 exchanges, compress key decisions and findings into the plan's `## Status` or `## Handoff Notes`.
- **Don't repeat context**: If information is already in `project.md` or the plan, reference it — don't paste it into the conversation.
- **One plan at a time**: Focus on the active plan. Don't load unrelated plans.

## File-System Memory Contract

GAL memory is file-owned, repo-visible, and reviewable. The file system is the authoritative memory substrate.

- Do not treat provider-local chat history as authoritative memory. It is advisory only.
- Do not introduce a second core memory layer through Mem0-style middleware, provider memory APIs, vector stores, databases, graphs, or long-running memory daemons.
- Use the smallest file surface that can carry the current fact safely and durably.

### Memory Scopes

| Scope | Authoritative files | Primary writers | Primary readers | Promotion path |
| --- | --- | --- | --- | --- |
| Shared methodology memory | `conventions/`, `workflows/`, `commands/`, `agents/`, `templates/`, durable shared docs | Reviewed source changes | All runtimes, chat agents, and golem specialists | Highest durable tier; change only when the lesson applies across repos |
| **Durable documentation sink** | `README.md` + all of `docs/` | STEWARD at pipeline closeout; explicit doc updates | Humans, golem-steward, all runtimes | Write new verified knowledge here **first** (never into `.dev/project.md` directly). Policy: `docs/architecture.md#documentation-governance`. |
| **Durable index / state** (project.md) | `.dev/project.md` — **compressed index** of the durable sink; not a sink itself | STEWARD re-sync after durable sink is updated; explicit project-state updates | All providers and agents during cold start | Re-index here **after** landing in the durable sink above. Do not write new knowledge directly into `.dev/project.md`; it is a derivative surface. |
| Project session memory | `.dev/state.md` | `/gal wrap-up`, planning commands, control-plane updates | `/gal status`, `/gal whats-next`, chat agents, golem specialists | Summarized from task execution memory at session boundaries |
| Task execution memory | `.dev/plans/<slug>.prompt.md` `## Status`, `### Handoff Notes`, `## Test Results`, `## Review Results`, `## Analyze` | `/plan-to-prompt`, implementer, tester, auditor, debugger, `/gal wrap-up` | Control-plane chat and specialist agents | First write target for new facts, blockers, failures, and local decisions |
| Source planning memory | `.dev/plans/<slug>.md` | `/planning`, `/deep-planning`, `/refining-plan`, planning review lanes | `/plan-to-prompt`, planning review lanes, humans | Planning source-of-truth; not the mutable execution state once a prompt exists |
| Research work reference | `.dev/research/<slug>.md` | `golem-researcher`, `/gal research`, `/gal deep-research` | Research authors during active investigation | Non-durable `.dev/` work file; findings that survive become `docs/` knowledge at finalize. |
| Private or machine-local memory | `config.json`, local Obsidian/private notes | User and local-only agents | Only explicitly local/private workflows | Never required for another provider or machine to resume repo work |
| Generated adapter memory surface | `AGENTS.md`, generated command files | `gal init`, `gal refresh`, `gal dev-apply`, and `gal render-adapters` only | Providers at startup | Derived carrier only; never edit as source of truth |

### Memory Operations

1. **Retrieve** — Read the minimum relevant cold-start stack before acting: `.dev/project.md`, `.dev/state.md`, the active `.dev/plans/<slug>.prompt.md`, then only the source docs or code required by the current step.
2. **Encode** — Write newly confirmed facts, blockers, failures, and task decisions first into task execution memory.
3. **Summarize** — Compress active work into `### Handoff Notes` and `.dev/state.md` at wrap-up, pause, provider switch, or machine switch boundaries.
4. **Promote** — Move a verified recurring lesson upward from task execution memory into project durable memory, then into shared methodology only when it generalizes across repos.
5. **Prune** — Remove or replace stale, disproven, superseded, or orphaned durable lessons so future retrieval stays trustworthy.

### Promotion And Prune Gates

- **Task to project promotion**: require a verified root cause plus a credible recurrence signal inside the current repo. Acceptable evidence includes confirmed debugger, auditor, or tester findings, or repeated manual correction of the same workflow.
- **Project to shared methodology promotion**: require a lesson that changes GAL conventions, workflows, templates, commands, agents, or other reusable source contracts across repos.
- **No guess promotion**: hypotheses, provisional workarounds, and unverified explanations remain in task execution memory and must not be promoted.
- **Prune gate**: remove or replace a durable lesson when later evidence disproves it, a newer rule supersedes it, the owning surface no longer exists, or the workflow changed enough to make the lesson stale.
- **Promotion cost rule**: promotion is manual and event-triggered. No background workers, daemons, or automatic bulk promotion.

### Provider-Memory Harvest (opt-in)

**Provider memory** is defined once here: any memory local to one of the five coding-agent runtimes in `docs/glossary.md` Part D (each runtime's own session or chat history), never GAL's file-owned memory. It is advisory only, per the File-System Memory Contract above; this section is the sole path by which it can ever reach a GAL-owned surface.

- **Default-off, machine-local opt-in.** Controlled by `config.json#memoryHarvest.enabled`. Missing or `false` is a silent no-op — current behavior is unchanged, mirroring the opt-in shape already used for Personal Enhancement (`core-vs-personal.md`).
- **Current-task-context-only intake.** The Harvest contract never lists, opens, globs, parses, writes, edits, or deletes provider-memory paths. It considers only provider-memory content the active runtime has already surfaced in the current task context — presence in a provider's store is never itself consent to read it.
- **Repo anchor and redaction.** A candidate must carry a current-repo anchor verifiable against repo files, and must be paraphrased and redacted. Raw quotes, secrets, absolute or provider paths, cross-project content, and personal content are excluded.
- **Per-candidate human approval.** `/gal wrap-up` is the sole proposal/approval inlet. Rejected or unanswered candidates are never written. An approved candidate lands only in the execution prompt's `### Handoff Notes` under `#### Approved Memory-Harvest Candidates`, marked `provisional` and `advisory`.
- **Honest not-run.** Enabled but no eligible memory surfaced: record `Harvest: not-run — no eligible provider memory surfaced` and continue normal wrap-up. No active execution prompt (no write target for an approved candidate): record `Harvest: not-run — no active execution prompt` and continue normal wrap-up.
- **Promotion evidence unchanged.** Approval and provider recall are never promotion evidence on their own. STEWARD promotes a harvested candidate only after independently proving the same verified-root-cause-plus-credible-recurrence-signal gate as any other task-to-project promotion (above). A gate miss omits the candidate; it never blocks `/gal finalize`.

### Bounded Current-Topic Index (`.dev/project.md`)

This convention is the single authority for `.dev/project.md`'s content shape and size budget. `crates/cli/src/gal/render.rs` and any project-index tooling must read this section, not invent a separate rule.

**Content shape — current-topic, not history.** `## Verified Facts` holds one bullet per fixed topic (currently ten: runtime/layout, lifecycle/control plane, adapters, documentation/structure map, planning/review/gates, dispatch/remote, naming/personalization/Core, install/release/restore, Codex compatibility, research). Each bullet states the topic's **current** state only, with a durable pointer into `README.md` or `docs/`. It is never a per-plan changelog.

**Mutation model — upsert / replace / prune, never append:**

- **Upsert** — when new information changes a topic's current state, that topic's bullet is updated to reflect it.
- **Replace** — a topic bullet is rewritten as a whole; it is never appended to or grown line-by-line.
- **Prune** — superseded, historical, or per-plan narration is deleted from this file, not archived here. Durable value from a finished plan lives in `docs/` (the sink) and reaches this file only as an updated current-topic pointer, never as new prose appended to the index.

**No history sink.** Do not add a new bullet, table row, or paragraph to record that a plan finished. `## Suspected Drift` and `## Documentation Gaps` read exactly `None.` when there is nothing outstanding — never leave a stale entry, a fabricated gap, or a misfiled history note under either heading.

**Size budget — normalized-LF UTF-8, 30,720 B advisory cap.** Measure the whole file after normalizing line endings to LF. Exceeding 30,720 bytes UTF-8 is a hygiene finding reported by checks and render warnings. It does not reject a write or stop a pipeline; use the repair and debt rules below.

### Bounded Session State (`.dev/state.md`)

This convention is the single authority for `.dev/state.md`'s content shape and size budget. Command contracts (`gal-finalize/SKILL.template.md`, `plugins/gal-core/templates/state.md`, `gal finalize-check`'s `state-bound` check) point here; they do not restate the rule body.

**Content shape — live index, not a history sink.** `.dev/state.md` owns the Active Plans / Parked Plans index, Global Decisions, Blockers, Session Continuity / Session Execution Context, a compact `## Recent Close-outs` table, and a compact `## Follow-ups` table. Durable narrative about a finished plan belongs to git history and the durable documentation layer (`README.md` + `docs/`), never to a growing HTML-comment blob in this file.

**`## Recent Close-outs` table.** Columns `Date | Plan | Landing | Result`, newest-first, **at most 2 data rows**. Each row is an evidence pointer only — plan name, finalize date, `Landing` commit, and `Result` (e.g. `ABSORBED`) — never a narrative paragraph.

- **`Landing` semantic**: the **durable-layer commit hash** — the same commit STEWARD provides as the pre-deletion gate before plan files are removed, and the commit referenced by `refs/gal/last-good`. It is used instead of the close-out commit because it is the only landing anchor that already exists at the moment Sequence 5 writes the row; the close-out commit itself does not yet exist at that point.
- **Row limit**: when a 3rd close-out lands, the oldest row is dropped — upsert/replace, never append-forever (mirrors the `.dev/project.md` mutation model above).

**`## Follow-ups` table.** Columns `Date | Origin | Finding | Route`, newest-first, **at most 5 data rows**. `Origin` is the finalized plan's slug. `Finding` is one line describing the deferred item, with any literal `|` character escaped so it cannot break the table row. `Route` is a one-line pointer to where the finding should land (an existing plan, a future plan, or a durable doc), not the resolution itself.

- **Exit rule**: a row leaves the table only when it is promoted into a plan's task list. A row never leaves by age alone, so an unpromoted finding can outlive several close-out cycles.
- **Row limit**: when a 6th row would land, the oldest row is dropped instead, and that eviction is visible in the same commit's diff — upsert/replace, never append-forever (mirrors the `.dev/project.md` mutation model and the `## Recent Close-outs` row limit above). A `Hygiene debt:` row is removed after a successful re-check confirms that its finding is repaired.
- **Mode asymmetry**: `gal finalize-check --full` tolerates an absent `## Follow-ups` section and reports it as the legacy shape rather than failing. `gal finalize-check --hygiene-only` requires the `## Follow-ups` header to be present. Both modes pass a `## Follow-ups` section that has the header but zero data rows.

**Legacy predicate (forbidden after migration).** A "legacy close-out comment" is any HTML comment whose opening matches `<!-- YYYY-MM-DD:` **and** whose body contains both the literal substrings `finalized on main` and `ABSORBED`. This exact predicate — not a word count, not a byte count — is what a bounded `.dev/state.md` must contain zero of. A structural comment that merely *discusses* those words (for example, explaining this predicate) does not match and is unaffected.

**Size budget — normalized CRLF→LF, 16,384 B advisory cap.** Measure the whole file after normalizing CRLF to LF. Exceeding 16,384 bytes UTF-8 is a hygiene finding, not a write rejection or pipeline stop. It is reported by `state-bound` as advisory and handled under the rules below.

**Ownership.** `gal-finalize/SKILL.template.md`'s post-write hygiene step and `gal finalize-check`'s `state-bound` check enforce this shape; they reference this section rather than restating it. `plugins/gal-core/templates/state.md` seeds the empty `## Recent Close-outs` and `## Follow-ups` tables so new repos start bounded.

### .dev Findings: Hygiene and Integrity

This section is the sole rule body for classifying and handling `.dev` findings. A hygiene finding is a repairable condition in one of the two GAL-owned bounded indexes: `.dev/project.md` or `.dev/state.md`. The 30,720 B and 16,384 B limits above are advisory caps. A finding in either index does not by itself fail a gate or become an owner question.

**Automatic hygiene repair boundary.** Before `pipeline-preflight` and before the finalize full-mode gate, normalize the bounded indexes under this section's hygiene rules. Those automatic repairs are limited to `.dev/project.md` and `.dev/state.md`, preserve unrelated sections and content, and re-run the same check once. They do not repair a failed gate after that gate has run. Separately, the bounded format-recovery contract may repair a `.dev/project.md` or `.dev/state.md` target only when a fresh gate receipt explicitly identifies an eligible catalogued format defect for that exact target and every failing disposition is eligible. The catalog repair sequence and its shared invocation budget govern that case. A failed or not-run pipeline preflight may use catalog repair for eligible receipt dispositions, then must pass a fresh preflight before execution.

**Completed repairs and restoration.** Report the repair GAL will perform or performed in every hygiene message. A successful repair or restoration is reported as completed repair, not open debt. If a bounded index is missing or unparseable, restore it from a valid index using `git show :1:<path>` when available, otherwise from `git show HEAD:<path>`, otherwise from the applicable template. For `.dev/project.md`, STEWARD re-indexes the restored content. Report restoration with a warning that uncommitted content may have been lost. Re-check the restored file. Stop only if neither a valid git copy nor a template is available.

**Hygiene debt.** If the re-check still finds a hygiene problem, upsert one `Hygiene debt:` row in `.dev/state.md`, keyed by the running plan slug. Update the existing row instead of duplicating it. Include the debt in handback and finalize reports. Remove the row after a successful re-check confirms repair. Keep the existing visible oldest-row eviction when the Follow-ups row cap is reached.

**Integrity findings.** Plans, prompts, receipts, research records, approval evidence, and root adapters are outside the automatic repair boundary. Findings in those files are integrity findings. Same-place conflicts, unrestorable bounded indexes, and root adapters that remain oversized after one repair and re-render are also integrity findings. Integrity findings stop processing and are reported with the repair or recovery action GAL took or can take. Do not tell the owner to edit `.dev` by hand before retrying.

**Conflict handling.** `gal state-merge` may resolve supported disjoint conflicts in `.dev/state.md` even when other paths are unmerged. A state place is one keyed row or one whole plain region. Leave every other unmerged path untouched. Authority-bearing conflicts and conflicts outside supported index places remain integrity stops. For unresolved places, report each stage's text and the location as `<file>:<first>-<last>`, with its section and row key when present, plus base, ours, and theirs text.

### Cold-Start Usage Rules

- Start with `.dev/project.md`, then `.dev/state.md`, then the active `.dev/plans/<slug>.prompt.md`.
- Use `.dev/plans/<slug>.md` as planning memory, not as mutable execution memory, when a prompt exists.
- Treat generated adapters as runtime carriers of the contract, not as the contract itself.
- Keep secrets, diary content, personal notes, and machine-local paths out of tracked repo memory unless a workflow explicitly routes them to a private local destination.

## Agent Budget Guideline

Each agent's instruction set should consume **≤ 15%** of the available context window. If an agent's prompt grows beyond this, split into smaller focused sections or move reference material to separate files.

## `/gal wrap-up` Context Handoff

Before switching contexts (worktree, branch, session), run `/gal wrap-up` to:

1. Compress key conversation insights into plan `## Status > ### Handoff Notes`
2. Update `state.md` session continuity
3. Commit changes

This prevents the next session from re-deriving context that was already established.

## Knowledge Flow Direction

```text
Ephemeral (high token cost to replay)
    │
    ▼  compress into
Plan ## Status / ## Handoff Notes (medium-term)
    │
    ▼  extract into
docs/ (permanent, low token cost to reference)
    │
    ▼  index into
.dev/project.md (minimal token cost)
```

**Principle**: The more stable the knowledge, the closer it lives to `project.md`. The more transient, the closer it lives to the plan or scratch file.

---

## Token Discipline

These rules apply to all agents and runtimes. Follow them during exploration, implementation, testing, review, and debugging.

### Language Policy By Artifact

Keep language choice aligned to the artifact's owner and reader:

- `.dev/plans/*.prompt.md` stays English-only because execution prompts are machine-readable work files optimized for cross-model stability and token efficiency.
- `.dev/plans/*.md` and `.dev/research/*.md` follow the per-invocation resolution chain: explicit directive, machine-local `planLanguage` (`config.json`), prompt-language auto-detect, then fallback `en`.
- Canonical README and formal `docs/*.md` files without a language infix follow project-level `PROJECT_LANGUAGE`; `<name>.<lang>.md` files are translation copies rather than canonical docs.

#### Non-English Planning: Three-Layer Authority

When `planLanguage != en`, a localized source plan is not the semantic authority — it is a human-facing render. Planning then has three layers (see `workflows/coding.md` → Planning-Language Authority):

- **EN semantic draft** (`.dev/plans/<slug>.en.md`) — the sole planning-stage semantic authority; exists only for non-English `planLanguage`; never a source plan (not in `.dev/state.md` Active Plans, never a pipeline/finalize input).
- **Localized source plan** (`.dev/plans/<slug>.md`) — the human-facing + GAL-tool-visible source/approval/review surface, rendered from the EN draft and carrying a planning-authority metadata block. Machine anchors (headings, paths, IDs, verdict literals, metadata keys) stay English (keep, no rename); only narrative prose follows `planLanguage`.
- **English execution prompt** (`.dev/plans/<slug>.prompt.md`) — post-prompt execution authority, English-only as above.

For `planLanguage` with the `en` prefix there is no draft and no reconcile cost: `.dev/plans/<slug>.md` is itself the EN source plan (single-file fast path). A hand-edit to the localized source is reconciled back into the EN draft before the next planning-stage command continues; after `/plan-to-prompt` the EN draft is deleted, and the equivalence proof lives on as the source plan's inline `prompt-hash` / `equivalence-verdict` metadata fields — there is no separate `.equiv.md` receipt file.

### Source And Prompt Format Split

Planning artifacts have two different readers and must optimize for that split:

- `.dev/plans/*.md` is the source plan: maximize human comprehension. Prefer charts, tables, diagrams, trees, and flow maps when they reduce reread cost.
- `.dev/plans/*.prompt.md` is the execution prompt: maximize AI efficiency. Compress aggressively, remove human-oriented filler, and keep only the machine-readable contract needed for execution.
- Compression in `.prompt.md` is allowed to sacrifice human readability, but it must never break the parser anchors consumed by pipeline/finalize tooling.

#### Compression Lower-Bound (hard)

Every execution prompt must preserve these machine anchors exactly:

- `## Status`
- `Current Task:`
- `## Tasks`
- at least one task line matching `- [ ] T-NN`
- `## Test Results`
- `## Review Results`
- `### Architecture Review`
- `### Business Review`
- `### Design Review`
- `### Engineering Review`

### Generated-Artifact Exclusion

Do not load generated adapters or build outputs into context by default:

- Exclude `bin/`, `obj/`, and other build output directories.
- Exclude generated runtime adapters: `AGENTS.md`, and equivalent generated files.
- Read generated files only when the task is explicitly about auditing or fixing their content.

### Directed Exploration

Read only files required by the current task step:

- Start from the plan's `## Tasks` entry and its named files; do not scan the full codebase first.
- If a file is not mentioned in the current task or a directly required dependency, do not load it.
- When a prior read already returned the information needed, stop. Do not repeat the same search with different terms.

### Capability-First Plumbing

Before using any optional capability lane (CLI tool, MCP server, or external API), resolve its status through the five-state preflight model in [optional-capabilities.md](optional-capabilities.md):

- If the lane is `not-applicable` or `unavailable`, use the documented fallback without surfacing a tool-install request.
- If the lane is `available-but-needs-init` or `available-but-not-ready`, degrade silently to the documented non-tool path.
- Only route into the capability when status is `ready`.
- For doc-sync specifically, keep `git diff` as the mandatory base; treat graphify and codebase-memory-mcp as advisory-only capability contracts (see `docs/integrations.md`).

### Bounded Command Output

Cap command output before it enters the context window:

- Prefer piped filters, `--first N`, `Select-Object -First N`, or `head`/`tail` over raw full dumps.
- When running tests or builds, capture only the summary line and any failing output—not the full pass log.
- When querying file listings or search results, limit to the most relevant matches.

### Failure-Only Evidence

Emit the smallest evidence set that explains the failure:

- For tests: report failing test names and their error messages only. Do not echo passing tests.
- For builds: report the first error and the enclosing file/line range. Do not echo the full build transcript.
- For logs: report the relevant exception, stack trace, and the two lines immediately above it. Do not paste the full log file.
- Preserve raw logs and artifacts as on-disk evidence stores. Retrieve them selectively only when the summary is insufficient to diagnose the problem.
