# Token Budget

Rules for managing context window and token consumption across all agents.

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
| Shared methodology memory | `conventions/`, `workflows/`, `commands/`, `agent/`, `templates/`, durable shared docs | Reviewed source changes | All runtimes, chat agents, and golem specialists | Highest durable tier; change only when the lesson applies across repos |
| **Durable documentation sink** | `README.md` + all of `docs/` | STEWARD at pipeline closeout; explicit doc updates | Humans, golem-steward, all runtimes | Write new verified knowledge here **first** (never into `.dev/project.md` directly). Policy: `docs/devguide.md#documentation-conventions`. |
| **Durable index / state** (project.md) | `.dev/project.md` — **compressed index** of the durable sink; not a sink itself | STEWARD re-sync after durable sink is updated; explicit project-state updates | All providers and agents during cold start | Re-index here **after** landing in the durable sink above. Do not write new knowledge directly into `.dev/project.md`; it is a derivative surface. |
| Project session memory | `.dev/state.md` | `/gal wrap-up`, planning commands, control-plane updates | `/gal status`, `/gal whats-next`, chat agents, golem specialists | Summarized from task execution memory at session boundaries |
| Task execution memory | `.dev/plans/<slug>.prompt.md` `## Status`, `### Handoff Notes`, `## Test Results`, `## Review Results`, `## Analyze` | `/plan-to-prompt`, implementer, tester, auditor, debugger, `/gal wrap-up` | Control-plane chat and specialist agents | First write target for new facts, blockers, failures, and local decisions |
| Source planning memory | `.dev/plans/<slug>.md` | `/planning`, `/deep-planning`, `/refining-plan`, planning review lanes | `/plan-to-prompt`, planning review lanes, humans | Planning source-of-truth; not the mutable execution state once a prompt exists |
| Research work reference | `.dev/research/<slug>.md` | `golem-researcher`, `/gal research`, `/gal deep-research` | Research authors during active investigation | Non-durable `.dev/` work file; findings that survive become `docs/` knowledge at finalize. |
| Private or machine-local memory | `config.json`, local Obsidian/private notes | User and local-only agents | Only explicitly local/private workflows | Never required for another provider or machine to resume repo work |
| Generated adapter memory surface | `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, generated command files | Sync/setup scripts only | Providers at startup | Derived carrier only; never edit as source of truth |

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

### Bounded Current-Topic Index (`.dev/project.md`)

This convention is the single authority for `.dev/project.md`'s content shape and size budget. `crates/cli/src/gal/render.rs` and any project-index tooling must read this section, not invent a separate rule.

**Content shape — current-topic, not history.** `## Verified Facts` holds one bullet per fixed topic (currently ten: runtime/layout, lifecycle/control plane, adapters, documentation/structure map, planning/review/gates, dispatch/remote, naming/personalization/Core, install/release/restore, Codex compatibility, research). Each bullet states the topic's **current** state only, with a durable pointer into `README.md` or `docs/`. It is never a per-plan changelog.

**Mutation model — upsert / replace / prune, never append:**

- **Upsert** — when new information changes a topic's current state, that topic's bullet is updated to reflect it.
- **Replace** — a topic bullet is rewritten as a whole; it is never appended to or grown line-by-line.
- **Prune** — superseded, historical, or per-plan narration is deleted from this file, not archived here. Durable value from a finished plan lives in `docs/` (the sink) and reaches this file only as an updated current-topic pointer, never as new prose appended to the index.

**No history sink.** Do not add a new bullet, table row, or paragraph to record that a plan finished. `## Suspected Drift` and `## Documentation Gaps` read exactly `None.` when there is nothing outstanding — never leave a stale entry, a fabricated gap, or a misfiled history note under either heading.

**Size budget — normalized-LF UTF-8, 30,720 B hard cap.** The whole file, normalized to LF line endings, must not exceed 30,720 bytes UTF-8. This is enforced at the production write boundary (repo-adapter render) before any write: a candidate write at or under the cap passes; over the cap fails with no partial output.

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
- Exclude generated runtime adapters: `.github/copilot-instructions.md`, `CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, and equivalent generated files.
- Read generated files only when the task is explicitly about auditing or fixing their content.

### Directed Exploration

Read only files required by the current task step:

- Start from the plan's `## Tasks` entry and its named files; do not scan the full codebase first.
- If a file is not mentioned in the current task or a directly required dependency, do not load it.
- When a prior read already returned the information needed, stop. Do not repeat the same search with different terms.

### Capability-First Plumbing

Before using any optional capability lane (CLI tool, MCP server, or external API), resolve its status through the shared preflight model in `docs/collaborative-tools/checking-contract.md`:

- If the lane is `not-applicable` or `unavailable`, use the documented fallback without surfacing a tool-install request.
- If the lane is `available-but-needs-init` or `available-but-not-ready`, degrade silently to the documented non-tool path.
- Only route into the capability when status is `ready`.
- For doc-sync specifically, keep `git diff` as the mandatory base; treat `docs/collaborative-tools/graphify.md` and `docs/collaborative-tools/codebase-memory-mcp.md` as advisory-only capability contracts.

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
