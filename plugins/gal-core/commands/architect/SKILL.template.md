---
name: architect
description: "Invoke the GAL architect in isolation by default, or use the discuss argument for in-context consultation."
---

# Architect

When the first argument is the whole word `discuss`, case-insensitively, remove that first argument once. Do not combine `discuss` with one-shot route options. Run `gal consult-script golem-architect [remaining user content]` and follow its dispatch block. If dispatch returns `COMMAND: error`, use its recovery action and retry without one-shot route options. Read `../../agents/golem-architect.agent.md` and adopt the instructions in its `<role>` section. Continue in the current conversation and label responses `[golem-architect · in-context]`.

Otherwise, run `gal dispatch-script golem-architect [user content]` and follow its typed action. `COMMAND: advisory-direct` is advisory only. Its output cannot write a plan or set `CLEAR`. A formal planning review is available only when an orchestrator supplies `--review-role architect` against a source snapshot. Follow `COMMAND: formal-review` under the existing architect contract. The orchestrator alone validates the result and applies planning changes.
