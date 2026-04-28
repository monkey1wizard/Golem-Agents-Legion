# xmachine Collaborative Tool Contract

xmachine is GAL's optional execution-plane collaborative tool for offloading bounded work into SSH-reachable machine lanes without changing GAL's control plane, repo-owned state model, or patch-first convergence rules.

Transport is lane-specific, not the definition of the tool itself. Every controlling machine and every controlled machine in an xmachine flow must have SSH available. Every controlled machine must also have Zellij installed so detached execution can use a shared session primitive. The repo currently ships two lane implementations: `remote-windows` and `local-async`.

## What This Module Covers

When xmachine is used, GAL can run bounded execution tasks through a shared execution contract across these currently implemented lanes:

- `remote-windows` for unattended burst work on a remote Windows machine
- `local-async` for detached async execution on a controlled machine that hosts a local checkout
- shared runtime outputs: `status.json`, `summary.md`, runtime log `runtime.log`, and `result.patch`
- smoke-test entrypoints and the patch-first retrieval flow

Without xmachine, GAL still works through its normal local control-plane and agent-owned execution paths.

## What Does Not Change When xmachine Is Used

- `/gal` still owns the public control plane.
- Repo-local Markdown files still own durable state.
- Machine lanes do not commit directly.
- Patch review and convergence still happen on the main control-plane checkout.
- Specialist ownership stays with the same GAL agents.

## Preflight - Shared Checking Model

This tool follows the shared preflight model in [checking-contract.md](checking-contract.md).

| Shared state | xmachine meaning | GAL behavior |
| --- | --- | --- |
| `not-applicable` | The current task should stay on the local control plane and does not need machine offload. | Continue with GAL-native local execution. |
| `unavailable` | This checkout or machine cannot access the lane's required runtime primitives. | Do not pretend machine offload ran; use the local path or the documented fallback lane. |
| `available-but-needs-init` | The xmachine contract exists, but one-time lane setup is still incomplete. | Do not auto-bootstrap during normal execution. Surface the missing prep explicitly. |
| `available-but-not-ready` | The tool exists, but the selected repo, task, or lane is missing the artifacts needed for this run. | Degrade to the non-xmachine path or a different lane. |
| `ready` | The selected lane has the required runtime primitives, task contract, and repo artifacts. | Route into the requested xmachine lane. |

xmachine is repo-owned rather than a third-party package. `Setup-Tools.ps1` therefore reports collaboration readiness and lane prerequisites; it is not expected to clone or install xmachine from an external source.

## Shared Machine Prerequisites

- Every controlling machine must have an SSH client and permission to reach the selected controlled machine.
- Every controlled machine must have SSH enabled and expose the repo checkout that xmachine will use.
- **Mandatory**: You MUST configure SSH Key-based (passwordless) authentication between the controlling and controlled machines. xmachine scripts run in non-interactive mode (`BatchMode=yes`); if a connection requires a password, the task dispatch will fail with an error.
- Every controlled machine must have Zellij installed. The `local-async` lane uses it directly today, and the broader xmachine contract standardizes on it as the detached execution primitive.
- Lane-specific runtimes still apply on top of that baseline. For example, the current bash-based local lane also needs `git`, `jq`, `script`, and the configured engine CLI.

## SSH Troubleshooting

If task dispatch fails with "Permission denied" or continues to ask for a password despite adding keys:

1. **Account Matching**: Ensure your connection string specifies the correct remote user (e.g., `tzylee@host`). By default, SSH may try to use your local Windows/Mac username.
2. **Permission Modes (Linux/macOS)**: SSH requires strict permissions. On the controlled machine, run:
    - `chmod 700 ~/.ssh`
    - `chmod 600 ~/.ssh/authorized_keys`
    - `chmod go-w ~` (Your home directory must not be group-writable)
3. **Windows Administrators**: If the remote user is in the `Administrators` group on Windows, OpenSSH ignores `~/.ssh/authorized_keys` by default. You must either:
    - Add the key to `C:\ProgramData\ssh\administrators_authorized_keys` (with restricted ACLs), OR
    - Comment out the `Match Group administrators` block at the bottom of `C:\ProgramData\ssh\sshd_config` and restart the `sshd` service.
4. **BatchMode Check**: Test your connection manually with `ssh -o BatchMode=yes user@host`. If this fails, xmachine will also fail.

## Lane Model

| Lane | Normal operator entrypoint | What it is for |
| --- | --- | --- |
| `remote-windows` | Windows PowerShell controller with SSH access to the target machine | True remote burst execution over SSH/SCP to a Windows machine checkout |
| `local-async` | macOS/Linux shell on the local machine checkout | Long-running async execution in detached Zellij sessions on a local machine that can stay alive after the caller disconnects |

### Ownership Boundary

xmachine lanes are disposable execution surfaces.

- The lane worktree owns only temporary runtime output and an optional `result.patch`.
- The control plane owns review, patch application, and any durable repo state updates.
- Lane processes never commit or directly mutate shared plan state.

## Runtime Output Contract

All xmachine lanes emit the same runtime files.

### Output directories

```text
remote-windows: C:\Windows\Temp\gal-xmachine\task-<taskId>
local-async:    /tmp/gal-xmachine/task-<taskId>
```

### Standard files

| File | Purpose |
| --- | --- |
| `status.json` | machine-readable task status, timing, and exit information |
| `summary.md` | human-readable task summary |
| `runtime.log` | raw Gemini CLI stdout/stderr runtime log |
| `result.patch` | disposable-worktree diff against `HEAD` |

### Retrieval model

The lane run finishes first. The control plane then:

1. reads `status.json` and `summary.md`
2. inspects the runtime log `runtime.log` when needed
3. reviews `result.patch`
4. applies the patch only after human or control-plane approval

## Smoke-Test Assets

Use these committed assets instead of inventing ad hoc smoke tasks.

- [../../scripts/Test-Xmachine.ps1](../../scripts/Test-Xmachine.ps1) as the Windows smoke wrapper for the `remote-windows` lane
- [../../scripts/Test-Xmachine.sh](../../scripts/Test-Xmachine.sh) as the macOS/Linux smoke wrapper for the `local-async` lane
- [../../templates/task-xmachine-remote-smoke.md](../../templates/task-xmachine-remote-smoke.md) for the remote Windows lane
- [../../templates/task-xmachine-local-smoke.md](../../templates/task-xmachine-local-smoke.md) for the local async lane
- [../xmachine/examples/task-local-async-smoke.example.md](../xmachine/examples/task-local-async-smoke.example.md) as the committed read-only local async smoke example

## Entry Points

### Remote Windows lane

Dispatch:

```powershell
.\scripts\Invoke-XmachineRemoteTask.ps1 `
    -RemoteHost windows-machine `
    -RemoteUser alice `
    -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
    -TaskSpec ".\some-task.md" `
    -TimeoutMinutes 30
```

Retrieve:

```powershell
.\scripts\Get-XmachineRemoteResult.ps1 `
    -RemoteHost windows-machine `
    -RemoteUser alice `
    -TaskId <TaskId> `
    -RemoteOutputDir "C:\Windows\Temp\gal-xmachine\task-<TaskId>" `
    -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
    -Wait
```

### macOS/Linux local-async lane

After connecting to the controlled machine over SSH, dispatch from that checkout:

```bash
bash scripts/Test-Xmachine.sh \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion \
    --wait \
    --timeout-minutes 30
```

Or invoke the lane directly:

```bash
bash scripts/Invoke-XmachineLocalTask.sh \
    --task-spec ./docs/xmachine/examples/task-local-async-smoke.example.md \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion \
    --timeout-minutes 30
```

Retrieve manually when needed:

```bash
bash scripts/Get-XmachineLocalResult.sh \
    --task-id <TaskId> \
    --output-dir <OutputDir> \
    --wait \
    --repo-path /Users/yourname/Code/Golem-Agents-Legion
```

## Operational Notes

### Session naming

The `local-async` lane uses detached Zellij sessions named `task-<taskId>`.

### Patch handling

If `result.patch` is non-empty, review and apply it from the control plane:

```bash
git apply --stat path/to/result.patch
git apply --check path/to/result.patch
git apply path/to/result.patch
```

### Cleanup

- `Get-XmachineLocalResult.sh` removes the disposable worktree, Zellij session, and output directory unless `--keep` is used.
- `Get-XmachineRemoteResult.ps1` removes the remote disposable worktree and output directory unless `-KeepRemote` is used.

## Read Next

- [checking-contract.md](checking-contract.md) for the shared collaborative-tool preflight model.
- [../../scripts/scripts.md](../../scripts/scripts.md) for the script inventory.
- [../../commands/commands.md](../../commands/commands.md) for the public GAL command surface.
- [../../workflows/coding.md](../../workflows/coding.md) for execution ownership and lifecycle.
