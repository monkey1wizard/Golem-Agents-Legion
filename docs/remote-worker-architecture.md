# Remote Worker Architecture

GAL execution plane — dispatching text-oriented tasks from the main control PC to a LAN worker node over SSH.

## Node Topology

```text
Main PC (Control Plane)              Notebook (LAN Burst Worker)
┌─────────────────────────┐          ┌───────────────────────────┐
│  /gal orchestrator      │          │  Start-GalWorker.ps1      │
│  Invoke-GalRemoteTask   │──SSH──▶  │  Gemini CLI (headless)    │
│  Get-GalRemoteResult    │◀──SCP──  │  git worktree (isolated)  │
└─────────────────────────┘          └───────────────────────────┘
```

Current validated path remains Windows PC → Windows notebook. The Mac Mini async endpoint keeps the same task/result contract, but it will use a bash worker adapter (`Start-GalWorker.sh`) instead of the Windows PowerShell worker.

## Node Roles

| Node | Role | Always-on? | Task Types |
| --- | --- | --- | --- |
| **Main PC** | Control plane — submits tasks, receives results | Yes | All interactive work |
| **Notebook** | Burst worker — headless CLI execution | No — wake on demand | Research, review, repo scan, docs |
| **Mac Mini** | Always-on async endpoint (Phase 2) | Yes | Long-running / overnight tasks, remote human-triggered bounded tasks via Discord / Telegram intake |

## Ownership Model

The execution plane enforces two worktree classes and three state layers to prevent
split-brain state across the multi-machine setup.

### Worktree Classes

**Primary Feature Worktree** — the worktree on Main PC where active development,
review, and state convergence happen. This is the only writer of `.dev/state.md`,
`.dev/project.md`, and `docs/plans/<plan-slug>.prompt.md`.

**Disposable Remote Worker Worktree** — a linked worktree created by
`Invoke-GalRemoteTask.ps1` on the worker node for a single bounded task. It produces
temp artifacts and optionally canonical-path outputs (via `result.patch`), but it does
not own canonical state.

### State Layers And Write Rules

| Layer | Artifacts | Primary Feature Worktree | Disposable Remote Worker Worktree |
| --- | --- | --- | --- |
| Repo-level canonical | `.dev/project.md`, `.dev/state.md` | Can write (sparingly) | No — never |
| Plan-level execution | `docs/plans/<plan-slug>.prompt.md` | Yes — primary writer | No by default; patch-first only if explicitly permitted |
| Remote runtime (ephemeral) | `status.json`, `summary.md`, `worker.log`, `result.patch` | No | Yes — sole owner, never committed to repo |
| Durable outputs | `docs/research/`, `docs/qa-reports/`, etc. | Yes | Yes — via result.patch; Main PC reviews before applying |

### State Convergence Flow

After a remote task completes, the Main PC must close the loop:

1. Retrieve artifacts with `Get-GalRemoteResult.ps1`
2. Read `summary.md` and review `result.patch`
3. Apply the patch if appropriate: `git apply result.patch`
4. Update the active plan prompt (`## Status`, `## Analyze`, `## Review Results`, etc.)
5. Update `.dev/state.md` only if repo-level blockers, active-plan index, or session continuity changed

## Task Contract

### Input: Task Spec

A single Markdown file following the [task template](../templates/task.md):

```sh
task-{YYYYMMDD}-{random6}.md
```

Placed in the task output directory on the worker, not committed to the repo.

### Execution Isolation

Each task runs in a **linked git worktree**:

```sh
{repoPath}-worker-{taskId}/
```

Worktree is created before the task and removed after result collection. It is never committed to the repo.

### Output Artifacts

All output lives in an **ephemeral task directory** on the worker:

```sh
Windows: $env:TEMP\gal-worker\{taskId}\
macOS:   /tmp/gal-worker/{taskId}/
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

```text
Invoke-GalRemoteTask                  Start-GalWorker(.ps1 | .sh)
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
- **Remote shell**: endpoint-specific
  - Windows burst worker: PowerShell (`pwsh`)
  - Mac Mini async endpoint: `bash` (optionally launched from login shell)
- **Assumption**: The repo is already cloned on the worker at a known path

## Endpoint Profiles

Control plane routing should resolve an endpoint profile before dispatch. The minimum fields are:

| Field | Purpose |
| --- | --- |
| `os` | `windows` or `macos` |
| `shell` | `pwsh` or `bash` |
| `tempRoot` | Where task artifacts live on that endpoint |
| `workerEntry` | `Start-GalWorker.ps1` or `Start-GalWorker.sh` |
| `repoPath` | Absolute path to the repo clone on that endpoint |
| `timeoutPolicy` | Default timeout for that endpoint class |

The endpoint profile is responsible for shell/path selection only. It must not create a second task contract.

## Async Intake Adapters

Mac Mini can also host remote human-facing intake adapters such as Discord or Telegram when the user is away from the main workstation.

Rules:

- Discord / Telegram are intake channels only, not alternate execution planes.
- They must translate user requests into bounded task specs or queue items that still pass through the same control-plane policy.
- They must not write canonical state directly.
- They must not bypass endpoint selection, patch review, or state convergence rules.

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

Mac Mini may also have Copilot CLI, VS Code, and Codex CLI installed, but those tools are not part of the automated worker contract. For remote execution, Gemini CLI remains the only canonical headless engine. Apple Silicon local inference is a separate lane and can prefer MLX-LM without changing the remote task/result contract.

Suggested LOCAL lane split on the Mac Mini:

- Gemma 4: general summarization, classification, pre-processing, and low-risk background drafting
- Breeze 2: Traditional Chinese / Taiwan-specific wording, note cleanup, tagging assistance, and private-text organization

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
- Direct Discord / Telegram-triggered execution that bypasses the control plane
- Agent callbacks (ACP / hooks)
- Sandbox isolation beyond git worktree
- Parallel tasks on the same worker
