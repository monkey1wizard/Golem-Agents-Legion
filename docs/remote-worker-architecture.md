# Remote Worker Architecture

GAL execution plane — dispatching text-oriented tasks from the main control PC to a LAN worker node over SSH.

## Node Topology

```
Main PC (Control Plane)              Notebook (LAN Burst Worker)
┌─────────────────────────┐          ┌───────────────────────────┐
│  /gal orchestrator      │          │  Start-GalWorker.ps1      │
│  Invoke-GalRemoteTask   │──SSH──▶  │  Gemini CLI (headless)    │
│  Get-GalRemoteResult    │◀──SCP──  │  git worktree (isolated)  │
└─────────────────────────┘          └───────────────────────────┘
```

MVP scope: Windows PC → Windows notebook only. Mac Mini endpoint is Phase 2.

## Node Roles

| Node | Role | Always-on? | Task Types |
| --- | --- | --- | --- |
| **Main PC** | Control plane — submits tasks, receives results | Yes | All interactive work |
| **Notebook** | Burst worker — headless CLI execution | No — wake on demand | Research, review, repo scan, docs |
| **Mac Mini** | Always-on async endpoint (Phase 2) | Yes | Long-running / overnight tasks |

## Task Contract

### Input: Task Spec

A single Markdown file following the [task template](../templates/task.md):

```
task-{YYYYMMDD}-{random6}.md
```

Placed in the task output directory on the worker, not committed to the repo.

### Execution Isolation

Each task runs in a **linked git worktree**:

```
{repoPath}-worker-{taskId}/
```

Worktree is created before the task and removed after result collection. It is never committed to the repo.

### Output Artifacts

All output lives in an **ephemeral task directory** on the worker:

```
$env:TEMP\gal-worker\{taskId}\
  status.json      — machine-readable status + exit metadata
  summary.md       — human-readable task summary and key findings
  worker.log       — raw stdout/stderr from the Gemini CLI run
  result.patch     — git diff of changes in the worktree (may be empty for read-only tasks)
```

Output artifacts are **not tracked by git** and are stored in temp on the worker until retrieved.

### status.json Schema

```json
{
  "taskId": "20260101-abc123",
  "status": "running | success | failed | timeout",
  "exitCode": 0,
  "startedAt": "ISO8601",
  "finishedAt": "ISO8601",
  "engine": "gemini-cli",
  "errorMessage": null
}
```

## Worktree Lifecycle

```
Invoke-GalRemoteTask                  Start-GalWorker
─────────────────────                 ────────────────
1. Generate task ID
2. Create remote temp dir
3. Copy task spec to remote
4. git worktree add (detached)
5. Invoke Start-GalWorker ──────────▶ 6. Write status.json (running)
                                       7. Run gemini --yolo -p ...
                                       8. Write summary.md
                                       9. Generate result.patch
                                      10. Update status.json (success/failed)
                                      11. git worktree lock (preserve for retrieval)
Get-GalRemoteResult
────────────────────
12. SCP: status.json, summary.md,
         worker.log, result.patch
13. Print status + summary
14. SSH: git worktree remove (cleanup)
```

`git worktree lock` is applied after the worker finishes and before retrieval so the worktree survives a recovery SCP call if the network drops. The control plane removes the lock and prunes the worktree after successful retrieval.

## Transport

- **Protocol**: OpenSSH (`ssh`, `scp`)
- **Auth**: SSH key — password auth is not supported for headless dispatch
- **Remote shell**: PowerShell (`pwsh`) — the worker must have PowerShell 7+
- **Assumption**: The repo is already cloned on the worker at a known path

## Worker Engine

- **Engine**: Gemini CLI (`gemini`)
- **Non-interactive flag**: `--yolo` — skips consent prompts for tool use
- **Output**: `--output-format stream-json` piped to `worker.log`; summary extracted post-run
- **Stdin**: closed — pass all context via the task spec file, not stdin
- **Exit codes** (from Gemini CLI troubleshooting):

| Code | Meaning | Worker Action |
| --- | --- | --- |
| 0 | Success | Write success status |
| 41 | Auth failed | Write failed status — re-auth required on worker |
| 42 | Input error | Write failed status — task spec malformed |
| 44 | Sandbox error | Write failed status — check worker sandbox config |
| 52 | Config error | Write failed status — check worker GEMINI.md / settings |
| 53 | Turn limit | Write failed status — task too large, split it |
| other | Unknown failure | Write failed status with raw exit code |

Gemini CLI auth / consent may still attempt to read stdin in some flows. The worker script closes stdin explicitly (`$null | gemini ...`) and treats exit code 41 as a hard failure requiring human re-auth on the worker before the next task.

## Task Types (MVP Scope)

Tasks suitable for remote dispatch in MVP:

- **Research**: investigate a question, gather findings, write a report to `docs/research/`
- **Review**: review code or a plan file, write findings to a result file
- **Repo scan**: audit the codebase for a specific pattern, produce a findings doc
- **Docs**: rewrite or update documentation files

Tasks **not suitable** for MVP (deferred to Phase 2+):

- Tasks requiring human approval mid-execution
- Tasks producing merge conflicts with main branch
- Long-running tasks exceeding Gemini CLI turn limits
- Tasks requiring secrets not available on the worker

## Failure Modes

| Failure | Detection | Recovery |
| --- | --- | --- |
| SSH connection refused | `Invoke-GalRemoteTask` exits with error | Check network + wake notebook |
| Worker never writes status.json | Timeout in `Get-GalRemoteResult` | SSH to worker, check process, run `Get-GalRemoteResult -Force` |
| Gemini CLI auth failure (exit 41) | `status.json` exitCode 41 | SSH to notebook, run `gemini` interactively to re-auth |
| Turn limit hit (exit 53) | `status.json` exitCode 53 | Split task spec, retry |
| Worktree left dangling | `Get-GalRemoteResult` with `-Cleanup` flag | SSH: `git worktree prune` in repo |
| result.patch missing | Empty or absent file | Task was read-only; no changes expected |

## Out of Scope (MVP)

Not implemented in this version. Do not add these without a separate plan:

- Worker loop / daemon / job queue
- Branch return (patch-first only)
- Mac Mini routing
- Multi-engine abstraction
- Discord-triggered dispatch
- Agent callbacks (ACP / hooks)
- Sandbox isolation beyond git worktree
- Parallel tasks on the same worker
