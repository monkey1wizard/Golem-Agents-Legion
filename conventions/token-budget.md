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
- Exclude generated runtime adapters: `.github/copilot-instructions.md`, `CLAUDE.md`, `GEMINI.md`, `AGENTS.md`, `ANTIGRAVITY.md`, and equivalent generated files.
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
