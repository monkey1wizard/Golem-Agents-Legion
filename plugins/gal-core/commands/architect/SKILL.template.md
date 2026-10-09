---
name: architect
description: "Invoke the GAL architect in isolation by default, or use the discuss argument for in-context consultation."
---

# Architect

When the first argument is the whole word `discuss`, case-insensitively, remove that first argument once. Run `gal consult-script golem-architect [remaining user content]` and follow its dispatch block. Read `../../agents/golem-architect.agent.md` and adopt the instructions in its `<role>` section. Continue in the current conversation and label responses `[golem-architect · in-context]`.

Otherwise, run `gal dispatch-script golem-architect [user content]` and follow its dispatch block. Adopt the isolated native-subagent behavior in `../../agents/golem-architect.agent.md`.
