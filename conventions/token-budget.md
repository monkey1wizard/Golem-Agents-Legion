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
| Project durable memory | `.dev/project.md`, selected durable repo docs | Planning, verifier, releaser, explicit documentation updates | All providers and agents during cold start | Promote here when a verified lesson is likely to recur in this repo |
| Project session memory | `.dev/state.md` | `/gal wrap-up`, planning commands, control-plane updates | `/gal status`, `/gal whats-next`, chat agents, golem specialists | Summarized from task execution memory at session boundaries |
| Task execution memory | `.dev/plans/<slug>.prompt.md` `## Status`, `### Handoff Notes`, `## Test Results`, `## Review Results`, `## Analyze` | `/plan-to-prompt`, implementer, tester, reviewer, debugger, verifier, `/gal wrap-up` | Control-plane chat and specialist agents | First write target for new facts, blockers, failures, and local decisions |
| Source planning memory | `docs/plans/<slug>.md` | `/planning`, `/deep-planning`, `/refining-plan`, planning review lanes | `/plan-to-prompt`, reviewers, humans | Planning source-of-truth; not the mutable execution state once a prompt exists |
| Private or machine-local memory | `config.local.env`, `mcp.local.json`, `xmachine.config.json`, local Obsidian/private notes | User and local-only agents | Only explicitly local/private workflows | Never required for another provider or machine to resume repo work |
| Generated adapter memory surface | `.github/copilot-instructions.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`, generated command files | Sync/setup scripts only | Providers at startup | Derived carrier only; never edit as source of truth |

### Memory Operations

1. **Retrieve** — Read the minimum relevant cold-start stack before acting: `.dev/project.md`, `.dev/state.md`, the active `.dev/plans/<slug>.prompt.md`, then only the source docs or code required by the current step.
2. **Encode** — Write newly confirmed facts, blockers, failures, and task decisions first into task execution memory.
3. **Summarize** — Compress active work into `### Handoff Notes` and `.dev/state.md` at wrap-up, pause, provider switch, or machine switch boundaries.
4. **Promote** — Move a verified recurring lesson upward from task execution memory into project durable memory, then into shared methodology only when it generalizes across repos.
5. **Prune** — Remove or replace stale, disproven, superseded, or orphaned durable lessons so future retrieval stays trustworthy.

### Promotion And Prune Gates

- **Task to project promotion**: require a verified root cause plus a credible recurrence signal inside the current repo. Acceptable evidence includes confirmed debugger, reviewer, tester, or verifier findings, or repeated manual correction of the same workflow.
- **Project to shared methodology promotion**: require a lesson that changes GAL conventions, workflows, templates, commands, agents, or other reusable source contracts across repos.
- **No guess promotion**: hypotheses, provisional workarounds, and unverified explanations remain in task execution memory and must not be promoted.
- **Prune gate**: remove or replace a durable lesson when later evidence disproves it, a newer rule supersedes it, the owning surface no longer exists, or the workflow changed enough to make the lesson stale.
- **Promotion cost rule**: promotion is manual and event-triggered. No background workers, daemons, or automatic bulk promotion.

### Cold-Start Usage Rules

- Start with `.dev/project.md`, then `.dev/state.md`, then the active `.dev/plans/<slug>.prompt.md`.
- Use `docs/plans/<slug>.md` as planning memory, not as mutable execution memory, when a prompt exists.
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

Before using any optional capability lane (CLI tool, MCP server, xmachine work node, or external API), resolve its status through the shared preflight model in `docs/collaborative-tools/checking-contract.md`:

- If the lane is `not-applicable` or `unavailable`, use the documented fallback without surfacing a tool-install request.
- If the lane is `available-but-needs-init` or `available-but-not-ready`, degrade silently to the documented non-tool path.
- Only route into the capability when status is `ready`.

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
