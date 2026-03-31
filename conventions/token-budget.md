# Token Budget

Rules for managing context window and token consumption across all agents.

---

## Cold Start Priority

When an agent begins a session, load context in this order (stop when sufficient):

1. **`.dev/project.md`** — compressed project summary + indexes
2. **`.dev/state.md`** — active plan index + session continuity
3. **Active plan file** — `docs/plans/*.prompt.md` with `## Status` section
4. **Canonical docs** — only when `project.md` explicitly references them for the current task
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
