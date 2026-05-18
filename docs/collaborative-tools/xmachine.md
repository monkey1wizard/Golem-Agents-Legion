# xmachine Collaborative Execution Contract

xmachine is an optional execution tool for GAL that offloads scoped tasks from a primary control node to an SSH-accessible work node. It operates without modifying GAL's control plane, repo-owned state model, or patch-first convergence rules.

The `-WorkNode` parameter accepts an xmachine work-node ID. This ID must be a node alias defined under the top-level `nodes` object in `xmachine.config.json` in the GAL runtime checkout, not necessarily in the target project being worked on. xmachine resolves this alias into an explicit SSH target (e.g., `user@host`) and uses the local SSH client for specific connection settings, such as account, host, port, and key configurations. Readiness is machine-local rather than repo-local: GAL records verified nodes in `~/.gal/xmachine-nodes.json` so other repositories can reuse them. A node is marked as `tooling-ready` after passing SSH, repository, tool, and work-node smoke tests. It becomes fully `readied` only after passing the separate GAL pipeline smoke gate.

## Capabilities

When xmachine is active, GAL can execute tasks through a shared contract across the following implemented paths:

- **Windows Work-Node Dispatch**: Task execution over SSH targeting Windows machines.
- **POSIX-Compatible Detached Execution**: Asynchronous execution on macOS or Linux targets capable of running the bash-based local asynchronous path using Bash, Zellij, and required shell utilities.
- **Standardized Runtime Outputs**: Generation of task-specific artifacts including `status.json`, `summary.md`, `runtime.log`, and `result.patch`.
- **Smoke-Test Integration**: Predefined entry points and a patch-first retrieval workflow for validating node health.

Without xmachine, GAL continues to operate using its standard local control-plane and agent-owned execution paths.

## Core Principles and Invariants

The following principles remain unchanged when using xmachine:

- The `/gal` command family still governs the public control plane.
- Repo-local Markdown files remain the source of truth for durable state.
- Work nodes never perform direct git commits.
- Patch review and convergence are always performed on the main control-plane checkout.
- Specialist ownership remains with the designated GAL agents.

## Preflight Check Model

This tool adheres to the shared preflight model defined in [checking-contract.md](checking-contract.md).

| Status | xmachine Definition | GAL Behavior |
| --- | --- | --- |
| `not-applicable` | The task belongs on the local control plane and does not require offloading. | Proceed with standard local execution. |
| `unavailable` | The current machine or checkout lacks the required runtime primitives. | Fall back to local execution or the documented alternative path. |
| `available-but-needs-init` | The xmachine contract is present, but no work nodes have been verified. | Report the missing setup; do not attempt to auto-bootstrap. |
| `available-but-not-ready` | The contract is present, but the selected node is only `tooling-ready` and has not cleared the final pipeline smoke gate. | Revert to the non-xmachine path or select a different node. |
| `ready` | At least one readied node is available for the requested path. | Route to xmachine when the user specifies a readied work node. |

xmachine is a repo-owned utility rather than a third-party package. `Setup-Tools.ps1` reports contract readiness and cached node states; it does not install xmachine from external sources.

## Machine Prerequisites

- **SSH Access**: Every control node must have an SSH client and permission to access target work nodes.
- **Repository Visibility**: Target work nodes must have SSH enabled and provide access to the repository checkout.
- **Authentication**: You **MUST** configure SSH key-based (passwordless) authentication. xmachine scripts use `BatchMode=yes`; connections requiring interactive passwords will fail.
- **Detached Execution**: Every work node must have Zellij installed, as the xmachine contract standardizes on Zellij for detached execution.
- **POSIX Runtime Tools**: The bash-based local path requires `zellij`, `git`, and `jq`.
- **System PTY Utility**: The bash-based local path expects the POSIX `script` command to be available in the `PATH`. This is a standard system PTY utility rather than an AI tool or repo-specific dependency.
- **AI Tool CLIs**: The selected execution path may require configured AI CLIs (e.g., `gemini`, `copilot`, `claude`, or `codex`), depending on the engine family being used.

### POSIX Smoke-Test Batches

The POSIX work-node smoke path checks readiness in three distinct batches:

1. **POSIX Core Tools**: `zellij`, `git`, and `jq`
2. **System PTY Utility**: `script`
3. **AI Tool CLIs**: The specific AI engines required for the current task (e.g., `gemini` and `copilot`).

### Readiness Progression

xmachine readiness advances in two cacheable stages:

1. **`tooling-ready`**: The node passed SSH connectivity, repo validation, tool-batch checks, and the work-node smoke path.
2. **`readied`**: The node passed `pipeline-smoke`, verifying that the repo-local GAL entry point can successfully dispatch the pipeline from that node.

Only `readied` nodes are considered `ready` by `Setup-Tools.ps1`.

## SSH Troubleshooting

If task dispatch fails with "Permission denied" or continues to prompt for a password:

1. **Account Verification**: Ensure your connection string specifies the correct remote user (e.g., `user@host`). SSH defaults to your local username if unspecified.
2. **Permissions (Linux/macOS)**: SSH requires strict directory and file permissions. On the work node, run:
    - `chmod 700 ~/.ssh`
    - `chmod 600 ~/.ssh/authorized_keys`
    - `chmod go-w ~` (Ensure the home directory is not group-writable).
3. **Windows Administrators**: If the remote user is in the `Administrators` group on Windows, OpenSSH may ignore `authorized_keys`. To fix this:
    - Add the key to `C:\ProgramData\ssh\administrators_authorized_keys` with restricted ACLs, **OR**
    - Comment out the `Match Group administrators` block in `C:\ProgramData\ssh\sshd_config` and restart the `sshd` service.
4. **Manual Test**: Verify the connection manually with `ssh -o BatchMode=yes user@host`. If this fails, xmachine will also fail.

## Terminology

| Term | Definition |
| --- | --- |
| **Control Node** | The machine where the user invokes GAL and reviews task results. |
| **Work Node** | A user-facing ID that resolves to a specific SSH target for task execution. |
| **Tooling-Ready Node** | A node that has passed basic connectivity and tool checks but has not yet cleared the final pipeline smoke gate. |
| **Readied Node** | A node that has cleared all readiness gates, including `pipeline-smoke`, and is available for offloading tasks. |

## Node Configuration

Define work nodes in the GAL runtime checkout's `xmachine.config.json` to provide stable, memorable IDs for your work nodes.

- The top-level `nodes` object is keyed by work-node alias.
- Each node must define a `target` value.
- Each node should usually define a `repoPath` value so `-WorkRepoPath` can stay optional.
- `runtimeRepoPath` is optional and points at the GAL runtime checkout when it differs from the target project checkout.
- `repoMappings` is optional and lets one work node map multiple target repositories by current repo name.
- `target` can be either a `Host` entry from `.ssh/config` or a direct `user@host` string.
- The `-WorkNode` parameter must match a defined alias.

**Example:**

```json
{
  "nodes": {
    "node-name": {
      "target": "username@mechine-name",
      "repoPath": "/path/to/Golem-Agents-Legion",
      "runtimeRepoPath": "/path/to/Golem-Agents-Legion",
      "repoMappings": {
        "local-ai-tools": {
          "repoPath": "/path/to/local-ai-tools",
          "runtimeRepoPath": "/path/to/Golem-Agents-Legion"
        }
      }
    },
    "mac-mini": {
      "target": "username@username-mac-mini.local",
      "repoPath": "/Users/username/Golem-Agents-Legion",
      "runtimeRepoPath": "/Users/username/Golem-Agents-Legion",
      "repoMappings": {
        "local-ai-tools": {
          "repoPath": "/Users/username/Code/zawip/local-ai-tools",
          "runtimeRepoPath": "/Users/username/Golem-Agents-Legion"
        }
      }
    },
    "win11-pc": {
      "target": "alice@win11-pc",
      "repoPath": "%USERPROFILE%\\Golem-Agents-Legion"
    }
  }
}
```

In this setup, `-WorkNode node-name` resolves to `username@mechine-name` before the SSH check begins.

### Repo Path Precedence

`Test-Xmachine.ps1` resolves the work-node repo path in this order:

1. Explicit `-WorkRepoPath`
2. `repoPath` on the selected node in `xmachine.config.json`
3. The existing machine-local cache in `~/.gal/xmachine-nodes.json`

`Invoke-XmachinePipeline.ps1` resolves the remote project/runtime paths in this order:

1. Explicit `-WorkRepoPath` for the target project checkout
2. `repoMappings.<current-repo>.repoPath` on the selected node
3. `repoPath` on the selected node

Remote GAL runtime path resolves in this order:

1. Explicit `-RemoteRuntimeRepoPath`
2. `repoMappings.<current-repo>.runtimeRepoPath` on the selected node
3. `runtimeRepoPath` on the selected node
4. The resolved remote project repo path

This allows a node to host both a GAL runtime checkout and separate target repo checkouts without forcing every repo to share the same remote path.

## `/gal xmachine ...` Shorthand

`/gal xmachine <node> to do <task-ref>` is a bounded shorthand for a single active-plan task.

- The dispatcher normalizes it to pipeline-style execution with `FROM` and `STOP_AT` set to the same task reference.
- It is intended for active-plan task references such as `TP-007` or `T-003`, not as a generic freeform remote prompt.
- The selected node still needs to be `readied`, and the task must exist in the resolved plan.

## Supported Execution Paths

| Path | Primary Entry Point | Use Case |
| --- | --- | --- |
| **Windows Work Node** | Windows PowerShell control node | Remote burst execution via SSH/SCP to a Windows checkout. |
| **POSIX-Compatible Node** | PowerShell control node or direct shell | Long-running asynchronous execution using Zellij sessions on macOS or Linux. |

### Ownership Boundary

xmachine paths are treated as disposable execution environments:

- The work node owns only temporary runtime outputs and the optional `result.patch`.
- The control node manages review, patch application, and durable state updates.
- Execution processes never directly commit to the repository or modify shared plan state.

### Task Spec Ownership

xmachine does not invent bounded task specs on its own. It only executes a task spec that the control node already prepared and passed through `-TaskSpec`.

- `Invoke-XmachineTask.ps1` requires the provided `-TaskSpec` path to exist before dispatch starts.
- In `/gal pipeline` xmachine mode, the pipeline orchestrator stays on the control node and may generate per-phase bounded task-spec markdown files so it can offload only the current implement, test, review, security, or verifier slice.
- Those pipeline-generated task specs may be staged under the target project's `.dev/` directory with names such as `.dev/xmachine-t001-feature1.md`.
- These files are transient control-plane artifacts for xmachine dispatch, not durable workflow state like `.dev/state.md` or `.dev/plans/<plan-slug>.prompt.md`.
- Seeing a repo-local `.dev/xmachine-*.md` file therefore usually means `pipeline + xmachine mode` prepared a bounded offload task; it does not mean the work node independently created new repo state.

Direct xmachine usage without `/gal pipeline` is also possible, but in that case the caller must still provide the task spec explicitly. xmachine consumes that file; it does not author it.

## Runtime Output Contract

All xmachine paths produce a consistent set of artifacts for each task run. These are execution artifacts, not durable GAL state:

- `status.json`: Machine-readable state for a single task run (status, timing, exit code, etc.).
- `summary.md`: Human-readable summary of the task execution.
- `runtime.log`: Raw execution logs (stdout/stderr).
- `result.patch`: A git diff of changes made in the disposable worktree.

`status.json` is distinct from the repo's `.dev/state.md` and the machine-local cache at `~/.gal/xmachine-nodes.json`:

- `.dev/state.md` tracks repo-level plans and session continuity.
- `~/.gal/xmachine-nodes.json` tracks verified work nodes for the current machine.
- `status.json` only reports the outcome of a specific xmachine task.

### Output Directories

- **Remote Windows**: `C:\Windows\Temp\gal-xmachine\task-<taskId>`
- **Local Async**: `/tmp/gal-xmachine/task-<taskId>`

### Retrieval Workflow

1. The work node completes the task.
2. The control node reads `status.json` for machine-readable status and `summary.md` for the human-readable summary.
3. The user or control node inspects `runtime.log` if errors occurred.
4. The control node reviews and applies `result.patch` upon approval.

## Smoke Test Assets

Use these assets to verify xmachine functionality:

- [../../scripts/Test-Xmachine.ps1](../../scripts/Test-Xmachine.ps1): Control-node wrapper for node verification.
- [../../scripts/Test-Xmachine.sh](../../scripts/Test-Xmachine.sh): POSIX work-node smoke wrapper.
- [../../templates/task-xmachine-remote-smoke.md](../../templates/task-xmachine-remote-smoke.md): Windows remote smoke template.
- [../../templates/task-xmachine-local-smoke.md](../../templates/task-xmachine-local-smoke.md): POSIX local smoke template.

## Entry Points

### Control Node Smoke Test

Verify and cache a work node from a Windows control node:

```powershell
.\scripts\Test-Xmachine.ps1 `
    -WorkNode office-win `
    -WorkRepoPath "C:\Code\Golem-Agents-Legion" `
    -Wait
```

If the selected node defines `repoPath` in `xmachine.config.json`, `-WorkRepoPath` can be omitted.

### Control Node Generic Dispatch

Dispatch a task spec to a configured work node alias from a Windows control node:

```powershell
.\scripts\Invoke-XmachineTask.ps1 `
  -WorkNode node-name `
  -TaskSpec ".\templates\task-xmachine-local-smoke.md" `
  -Wait
```

This wrapper resolves the node alias through the GAL runtime checkout's `xmachine.config.json`, detects the remote platform, dispatches through the appropriate xmachine lane, and retrieves `status.json`, `summary.md`, `runtime.log`, and `result.patch` into a local `gal-results\<TaskId>` directory when `-Wait` is specified.

### Windows Work Node

**Dispatch Task:**

```powershell
.\scripts\Invoke-XmachineRemoteTask.ps1 `
    -RemoteHost windows-machine `
    -RemoteUser alice `
    -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
    -TaskSpec ".\some-task.md" `
    -TimeoutMinutes 30
```

**Retrieve Results:**

```powershell
.\scripts\Get-XmachineRemoteResult.ps1 `
    -RemoteHost windows-machine `
    -RemoteUser alice `
    -TaskId <TaskId> `
    -RemoteOutputDir "C:\Windows\Temp\gal-xmachine\task-<TaskId>" `
    -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
    -Wait
```

### POSIX Work Node

**Dispatch Task (Local Shell):**

```bash
bash scripts/Invoke-XmachineLocalTask.sh \
    --task-spec ./templates/task-xmachine-local-smoke.md \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion \
    --timeout-minutes 30
```

**Retrieve Results:**

```bash
bash scripts/Get-XmachineLocalResult.sh \
    --task-id <TaskId> \
    --output-dir <OutputDir> \
    --wait \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion
```

## Operational Notes

### Session Naming

Detached Zellij sessions are named `task-<taskId>`.

### Patch Handling

Review and apply `result.patch` files from the control node:

```bash
git apply --stat path/to/result.patch
git apply --check path/to/result.patch
git apply path/to/result.patch
```

### Cleanup

- `Get-XmachineLocalResult.sh` removes the disposable worktree, Zellij session, and output directory unless `--keep` is specified.
- `Get-XmachineRemoteResult.ps1` removes remote temporary assets unless `-KeepRemote` is specified.

## Control-Node Consumption

When the control node retrieves xmachine artifacts, read the minimum needed to answer the current question.

**Default consumption order** (stop as soon as the question is answered):

1. **`status.json`** — machine-readable exit code and timing; sufficient to confirm success or failure.
2. **`summary.md`** — human-readable outcome; read this before opening any raw log.
3. **`runtime.log`** — escalate only when `summary.md` does not explain the failure. Read the tail (errors, tracebacks) first; pull earlier context only if the failure cause is still unclear.
4. **`result.patch`** — retrieve for diff review; do not pipe the raw diff into the conversation unless a specific line is in dispute.

**Do not** load the full `runtime.log` into context as a default step. It exists for targeted error diagnosis, not as a progress trace.

## Reference

- [checking-contract.md](checking-contract.md): Shared preflight model.
- [../../scripts/scripts.md](../../scripts/scripts.md): Script inventory.
- [../../commands/commands.md](../../commands/commands.md): Public command surface.
- [../../workflows/coding.md](../../workflows/coding.md): Execution lifecycle and ownership.
