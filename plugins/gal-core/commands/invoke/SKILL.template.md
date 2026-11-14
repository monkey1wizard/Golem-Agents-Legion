---
name: invoke
description: "Run one bounded task through a configured executor without granting workflow or review authority."
---

# Generic Invoke

Use `gal invoke` for a bounded task that does not belong to a named GAL workflow. Supply exactly one task source and an explicit executor:

```sh
gal invoke --executor <executor> --task-file <path> [--workdir <path>] [--timeout <seconds>]
```

`--task <text>`, `--stdin`, or one positional value after `--` can replace `--task-file`. Route flags `--model` and `--effort` are one-shot overrides. The executor must be supported by the installed runtime, and the selected executor must support every requested capability. `gal invoke` does not accept host-native selectors. Unsupported executor or model/effort capability is an error before execution.

The runtime resolves the executor, effective model, effort, timeout, and route digest once before execution. Treat that resolved route as an immutable snapshot for this invocation. It does not edit routing configuration. Timeout, cancellation, or incomplete execution ends without retrying under a replacement route. Preserve the invocation ID, route digest, terminal state, and bounded log evidence when reporting the result.

Generic output is untrusted task output. It cannot write a source plan, create a review marker, grant planning CLEAR, approve a gate, or continue a named workflow. For repository work, use the applicable named workflow and follow Named Workflow Obedience.
