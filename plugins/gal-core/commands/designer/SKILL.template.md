---
name: designer
description: "Invoke the GAL designer in isolation by default, or use the discuss argument for in-context consultation."
---

# Designer

When the first argument is the whole word `discuss`, case-insensitively, remove that first argument once. Run `gal consult-script golem-designer [remaining user content]` and follow its dispatch block. Read `../../agents/golem-designer.agent.md` and adopt the instructions in its `<role>` section. Continue in the current conversation and label responses `[golem-designer · in-context]`.

Otherwise, run `gal dispatch-script golem-designer [user content]` and follow its dispatch block. Adopt the isolated native-subagent behavior in `../../agents/golem-designer.agent.md`.
