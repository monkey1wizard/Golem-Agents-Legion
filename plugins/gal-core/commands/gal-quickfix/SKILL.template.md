---
name: gal-quickfix
description: "GAL quickfix ($gal-quickfix / 小錯誤快速修正 / quick fix / fix the failing checks). Bounded repair lane for a small failing check: one host subagent edits, `gal quickfix-run` measures the change against the scope limits, and the owner confirms the commit and the merge. No plan file, no finalize."
---

# /gal quickfix

Repair a small failing check without a plan. The lane is bounded by file count, changed-line count, and attempt count. A host subagent makes the edit. The `gal` binary measures the result on the real working tree. The owner confirms the commit and the merge.

## Entry Criteria

Use this lane only when all of the following hold:

- One or more of the project check commands in `.dev/project.md` fail on the current working tree.
- The repair is small. It fits within the configured limits: `maxFiles` (default 3), `maxLines` (default 50), and `maxAttempts` (default 2) under the `quickfix` key of `~/.gal/config/config.json`.
- The repair touches no Protected Path and no dependency file (`Cargo.toml`, `Cargo.lock`, `package-lock.json`).
- No `/gal pipeline` run is active in this repo.
- The host is Claude Code (`--host claude`) or Codex (`--host codex`).

Escalate to `/planning` instead when the repair needs a design decision, touches a Protected Path or a dependency file, spans more files or lines than the limits allow, or has already used up its attempts.

## Main-Session No-Edit Rule

The main session never edits a file during an attempt. Only the host subagent edits. The main session runs the `gal quickfix-run` commands, spawns the subagent, relays the report, and asks the owner. If the main session edits a file, the change set is no longer the subagent's work and the receipt does not describe the real repair.

## Steps

1. **Reproduce.** Run the project check commands and confirm that at least one fails. If none fails, report that and stop.

2. **Open an attempt.**

   ```sh
   gal quickfix-run --open --host <claude|codex>
   ```

   On success the command prints `QUICKFIX_ATTEMPT: <attempt> spec=<spec path> model=<model>`. Keep the attempt id, the spec path, and the model. On a refusal it prints `QUICKFIX: <status>` and creates nothing. Handle the status as described in Open Refusals.

3. **Spawn one host subagent.** Use the host's own subagent mechanism, one subagent for the attempt, on exactly the model the open step printed. Do not choose another model. The spawn prompt must say all of the following:
   - Read the spec at the printed spec path and fix only the failing checks it lists.
   - Stay within the file and line limits stated in the spec.
   - Edit files only to fix those checks. Do not touch Protected Paths or dependency files.
   - Do not run `git commit`, `git push`, or any other git command that changes history or the index.
   - Do not edit anything under `.dev/`.
   - Do not run any `gal` subcommand and do not load any workflow skill.
   - Finish with a final report that lists each changed file and what changed in it.

4. **Close the attempt.** After the subagent finishes, run:

   ```sh
   gal quickfix-run --close <attempt> --session <host>:<id>
   ```

   `<host>:<id>` is the subagent session reference, for example `claude:<session id>`. The host must match the host used at open. The command verifies the session and its model, checks that `HEAD` did not move, measures the change between the open-time and close-time working-tree snapshots, runs the quickfix gate on that change, and prints one `QUICKFIX: <status>` line. It then prints one `CHANGED: <path>` line per file in the change set and one `OUTSIDE: <path>` line per working-tree change that is outside the change set and outside `.dev/`. If the evidence for the session is not ready yet, the command fails with an error and the attempt stays open. Wait for the subagent session record, then run the same command again. Handle the status as described in Close Statuses.

   Do not loop on `--close`. If the subagent has ended and its session record still does not appear, or the spawn failed and no subagent ever ran, the attempt cannot be closed. Stop and ask the owner. The owner removes the attempt directory `.dev/pipeline/quickfix/<attempt>` (the attempt id has the form `qf_<number>_<number>`) and leaves any working-tree changes in place to judge. The main session never removes the directory itself. After the owner has removed it, `--open` can run again. If a stale `close.lock` remains from a killed process, the owner removes that lock first.

5. **Report to the owner.** Include the following:
   - The attempt id and the terminal status.
   - The changed files, from the `CHANGED:` lines.
   - The check result. A `completed` status means the gate receipt `.dev/pipeline/quickfix/<attempt>/close.receipt.md` records `overall: pass`. That receipt was measured on the real working tree, not on a copy.
   - The files left uncommitted, from the `OUTSIDE:` lines. These are working-tree changes the attempt does not own. The commit step leaves them alone.
   - The target branch, which is the branch checked out now. The commit lands on `quickfix/<attempt>`.
   - The subagent's own report.

6. **Owner confirmation, then commit.** Ask the owner whether to commit. Run the next command only after a clear yes.

   ```sh
   gal quickfix-run --commit <attempt>
   ```

   The command creates the branch `quickfix/<attempt>`, commits exactly the changed files there, and returns to the original branch. On success it prints `QUICKFIX: committed`, `COMMIT: <commit id>`, `BRANCH: <branch>`, and one `OUTSIDE: <path>` line for each remaining working-tree change outside `.dev/`. Report those lines to the owner.

7. **Owner confirmation, then merge.** Ask the owner whether to merge. Run the next command only after a clear yes.

   ```sh
   gal quickfix-run --merge <attempt>
   ```

   The command merges `quickfix/<attempt>` into the original branch with a plain `git merge`. On success it prints `QUICKFIX: merged` and `BRANCH: <branch>`. The branch stays. A failed merge is aborted and the attempt stays `committed`.

Never run `--commit` or `--merge` without the owner's confirmation for that specific step. A confirmation for one step does not cover the other.

## Open Refusals

Each refusal prints `QUICKFIX: <status>`, creates no attempt, and exits with an error.

| Status | Meaning | Next action |
| --- | --- | --- |
| `pipeline-active` | A plan pipeline run holds a lock, has an active coordinator attempt, or has a preflight receipt for an unfinished plan | Finish or close the pipeline run first. Do not use this lane alongside it |
| `attempt-open` | An earlier quickfix attempt has no terminal status. The line names its directory | Run `gal quickfix-run --close <attempt> --session <host>:<id>` for that attempt. If a stale `close.lock` remains from a killed process, remove that lock first. If that attempt never had a subagent session record, do not loop on `--close`. Stop and ask the owner to remove the directory `.dev/pipeline/quickfix/<attempt>` named on the line, then run `--open` again |
| `host-subagent-unsupported` | The `--host` value is not `claude` or `codex` | Re-run with a supported host. If neither fits, escalate to `/planning` |
| `no-change` | The working tree has no change against `HEAD` | Nothing to repair in this tree. Report that and stop |
| `checks-pass` | The project check commands already pass | Report that the checks pass and stop |
| `attempt-limit` | The same failing commands at the same `HEAD` already used `maxAttempts` attempts | Stop. Escalate to `/planning` |
| `scope-exceeded` | The current change already breaks a scope check, and the line lists which | Reduce the change by hand with the owner, or escalate to `/planning` |
| `checks-not-run` | A check could not run, for example the project has no authoritative check fence, and the line lists the reason | Fix the check setup first. Then run `--open` again |

## Close Statuses

Each close records one terminal status in the attempt manifest. Only `completed` exits with success. A closed attempt cannot be closed again. A repeated close prints `QUICKFIX: attempt-closed`.

| Status | Meaning | Next action |
| --- | --- | --- |
| `completed` | The gate receipt records `overall: pass` on the real working tree | Report, then ask the owner about `--commit` |
| `session-invalid` | The session reference or its record does not support the attempt, or the attempt manifest is malformed | Do not commit. Report the stderr reason to the owner. Open a new attempt after the owner decides |
| `model-tier-mismatch` | The subagent ran on a different model or effort than the open step printed | Do not commit. Open a new attempt and spawn the subagent on the printed model |
| `head-moved` | `HEAD` changed during the attempt | Do not commit. Find out why `HEAD` moved. Open a new attempt from the new `HEAD` |
| `no-change` | The subagent changed nothing | Report that. Open a new attempt only if the failure still needs a repair |
| `scope-fail` | The change broke a scope check: files, lines, Protected Paths, or dependency files | Do not commit. The owner decides between discarding the change and escalating to `/planning` |
| `check-fail` | The project checks still fail, or did not run, on the changed tree | Do not commit. Report the failing output. Open a new attempt if `maxAttempts` allows, otherwise escalate to `/planning` |

## Commit and Merge Refusals

These statuses print `QUICKFIX: <status>`, exit with an error, and change nothing.

| Status | Command | Meaning | Next action |
| --- | --- | --- | --- |
| `not-completed status=<status>` | `--commit` | The attempt is not `completed`. The line shows the recorded status, or `open` | Handle the recorded status as in Close Statuses. The two values below are not close statuses |
| `not-completed status=committed` | `--commit` | The commit already exists on `quickfix/<attempt>` | Do not commit again. After the owner confirms, go to the merge step |
| `not-completed status=merged` | `--commit` | The attempt is already merged | The attempt is finished. Report that and stop |
| `receipt-not-pass` | `--commit` | The close receipt does not record `overall: pass` | Do not commit. Open a new attempt |
| `head-moved` | `--commit` | `HEAD` is not the commit the attempt started at | Do not commit. Open a new attempt from the new `HEAD` |
| `snapshot-changed` | `--commit` | The working tree changed after the close | Do not commit. Report what changed and open a new attempt |
| `wrong-branch expected=<branch> actual=<branch>` | `--commit`, `--merge` | The checked-out branch is not the branch the attempt started on | Switch back to the expected branch, then run the same command again |
| `not-committed status=<status>` | `--merge` | The attempt is not `committed`. The line shows the recorded status, or `open` | Run `--commit` first when the status is `completed` |
| `merge-in-progress` | `--merge` | A merge is already in progress in this repo | Finish or abort that merge, then run `--merge` again |

An error on stderr from `--commit` or `--merge` means a git step failed. Read the message. It states which branch the repo is on and how to leave it. Report it to the owner and stop.

## Stop Hook

When `quickfix.hook` is `true` in `~/.gal/config/config.json`, the host runs `gal quickfix-hook --host <claude|codex> stop` at the end of each turn. The hook is off by default. It opens an attempt through the same code as `--open` and uses the same refusals. A refusal, a disabled hook, an unchanged working tree, or any error lets the turn end. When an attempt opens, the hook blocks the stop. The block message names the attempt id, the model, and the spec path. It tells the session to spawn a subagent on that model with the spec, wait for it, and then run `gal quickfix-run --close <attempt> --session <subagent session ref>`. It also tells the session not to commit.

On that block, follow steps 3 to 7 above. The no-edit rule, the report, and the owner confirmations before `--commit` and `--merge` apply in full. The hook never commits or merges. Without the hook, the lane works the same through the commands in the steps.

## Escalation

Stop and tell the owner to run `/planning` when any of these holds:

- The repair needs more than the allowed files or lines.
- The repair touches a Protected Path or a dependency file.
- The status is `attempt-limit`, `scope-fail`, or repeated `check-fail`.
- The failure points to a design problem, not a small error.
