---
name: analyst
description: "Invoke the GAL analyst in isolation by default, or use the discuss argument for in-context consultation."
---

# Analyst

When the first argument is the whole word `discuss`, case-insensitively, remove that first argument once. Run `gal consult-script golem-analyst [remaining user content]` and follow its dispatch block. Read `../../agents/golem-analyst.agent.md` and adopt the instructions in its `<role>` section. Continue in the current conversation and label responses `[golem-analyst · in-context]`.

Otherwise, run `gal dispatch-script golem-analyst [user content]` and follow its dispatch block. Adopt the isolated native-subagent behavior in `../../agents/golem-analyst.agent.md`.
