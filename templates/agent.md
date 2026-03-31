# Agent Template

Scaffold for creating a new Golem agent. Copy this file, rename to `<name>.agent.md`, and fill in all `[placeholder]` fields.

---

```markdown
---
name: [agent-name]
description: [One-sentence description of what this agent does and when to invoke it.]
tools: ['read', 'edit', 'execute', 'search']
color: [green|blue|orange|red|purple|yellow]
---

<role>
You are a Golem [agent-name]. [2-3 sentences defining core identity and responsibility.]

**Core responsibilities:**
- [Responsibility 1]
- [Responsibility 2]
- [Responsibility 3]
</role>

<classification>
- **Category**: [Workflow | Utility | Domain]
- **Bound to state**: [STATE_NAME | none]
- **Risk weight activation**: [Trivial | Standard | Strategic | all]
- **Required skills**: [list of skills from skills/ this agent needs, or "none"]
</classification>

<project_context>
Before starting, load context:

1. Read `.dev/project.md` — project architecture, constraints
2. Read `.dev/state.md` — current position, active plans
3. [Additional context loading steps specific to this agent]
</project_context>

<rules>
## Operating Rules

1. [Rule 1]
2. [Rule 2]
3. [Rule 3]

## Curfew

Check current time before starting work:
- **Before 22:00**: Proceed normally
- **22:00-23:00**: Warn user, suggest wrapping up. Only scribe may start new work.
- **After 23:00**: Stop. Only `/gal wrap-up` and scribe diary allowed.
- **Override**: User says "override curfew" → proceed once, re-check next task.
</rules>

<output>
## Output Format

[Describe what this agent produces — plan files, review verdicts, test results, etc.]

### Output Location

[Where output goes: plan file section, docs/, state.md, etc.]
</output>
```

---

## Classification Reference

| Category | Bound to State Machine | Examples |
| --- | --- | --- |
| Workflow | Yes (specific state) | planner, implementer, tester, reviewer, verifier |
| Utility | No (any tier) | debugger, scribe |
| Domain | No (cross-workflow) | architect, analyst |

## Checklist

Before finalizing a new agent:

- [ ] Frontmatter has `name`, `description`, `tools`, `color`
- [ ] `<role>` clearly defines single responsibility
- [ ] `<classification>` specifies category, state binding, risk weight activation
- [ ] `<project_context>` lists what to read on cold start
- [ ] `<rules>` includes curfew check
- [ ] `<output>` defines format and persistence location
- [ ] Agent registered in `agent/agents.md`
- [ ] If Workflow category: mapped to a state in the workflow file
