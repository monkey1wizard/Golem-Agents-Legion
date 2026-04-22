# GAL State

<!-- Per-task state (workflow step, deviations, test and review results) lives in the plan file's ## Status section, not here. This file tracks repo-level concerns only. -->

## Active Plans

| Plan | File | Plan Phase | Last Activity |
| --- | --- | --- | --- |
| infra-lan-worker-topology | docs/plans/infra-lan-worker-topology.prompt.md | IMPLEMENT (3/5) | 2026-04-22 |

## Global Decisions

| Date | Decision | Rationale | Scope |
| --- | --- | --- | --- |
| 2026-04-21 | Validate bootstrap follow-up work on the GAL repo itself | Closes the deferred macOS and first-real-feature checks with a real initialized repo | repo |
| 2026-04-21 | Keep infra-lan worker topology out of roadmap closeout | Remote execution-plane work already has its own plan and remains intentionally deferred | roadmap |
| 2026-04-22 | Mac Mini = workspace host + always-on control plane; Win11 demoted to opportunistic burst worker | Win11 PC may shut down at any time, so control plane must live on the always-on node | infra |
| 2026-04-22 | Adopt Zellij detached named sessions (`gal-task-<taskId>`) as the always-on async lane container on Mac Mini | tmux unavailable; zellij 0.44.1 already installed; `attach --create-background` matches dispatcher needs | infra |
| 2026-04-22 | Worker dispatchers must bypass the `gal-config` smudge filter when calling `git worktree add` | Filter races against `scripts/` checkout in fresh worktrees and aborts the worktree creation | infra |

## Blockers

None.

## Session Continuity

Last session: 2026-04-22 — rewrote infra-lan-worker-topology plan; built and E2E-verified Mac Mini async lane (`Start-GalWorker.sh`, `Invoke-GalLocalTask.sh`, `Get-GalLocalResult.sh`).
Stopped at: P4 (Mac bash worker) and P5 (Mac Mini Zellij always-on lane core) are done; P3 control-plane offload policy and Win11-from-Mac-mini live verify remain open.
Next step: Sync supporting docs (`docs/collaborative-tools/remote-worker.md`, `scripts/scripts.md`, `model-roles.example.md`, `docs/runtime-verification.md`) with the new node-role model, then implement P3 routing in `commands/commands.md`.
Context: Mac Mini is now both workspace host and always-on control plane. Verified primitives are recorded in the plan's "Verified Mac Mini Primitives" section so they can be re-run on demand.
