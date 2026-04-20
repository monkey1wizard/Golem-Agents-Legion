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
- **Typical activation**: [planning review | implementation | post-implementation review | any]
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

## Working Hours

Resolve working-hours behavior from `conventions/working-hours.md` before starting work.

- If working hours are disabled in local config, proceed normally.
- If working hours are enabled, follow the configured After Hours, Wrap-up Time, and Hard Stop behavior plus the diary-check rules.
- Only the configured after-hours owner may continue the shutdown ritual during the shutdown window.
- If the user says `override working hours` or `override curfew`, allow one invocation and then re-check on the next task.
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

| Category | Activation | Examples |
| --- | --- | --- |
| Pipeline | Specialist workflow or `/gal pipeline` | implementer, tester, reviewer, verifier |
| Utility | No (any tier) | debugger, notewriter |
| Domain | No (cross-workflow) | architect, analyst |

## Checklist

Before finalizing a new agent:

- [ ] Frontmatter has `name`, `description`, `tools`, `color`
- [ ] `<classification>` specifies category, state binding, and typical activation
- [ ] `<role>` clearly defines single responsibility
- [ ] `<project_context>` lists what to read on cold start
- [ ] `<rules>` includes working-hours check
- [ ] `<output>` defines format and persistence location
- [ ] Agent registered in `agent/agents.md`
- [ ] If Workflow category: mapped to a state in the workflow file
