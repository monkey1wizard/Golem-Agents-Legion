---
name: releaser
description: "Invoke the GAL releaser in isolation by default, or use the discuss argument for in-context consultation."
---

# Releaser

When the first argument is the whole word `discuss`, case-insensitively, remove that first argument once. Run `gal consult-script golem-releaser [remaining user content]` and follow its dispatch block. Read `../../agents/golem-releaser.agent.md` and adopt the instructions in its `<role>` section. Continue in the current conversation and label responses `[golem-releaser · in-context]`.

Otherwise, run `gal dispatch-script golem-releaser [user content]` and follow its dispatch block. Adopt the isolated native-subagent behavior in `../../agents/golem-releaser.agent.md`.
