# Remote Execution

Remote (cross-machine) execution is a core GAL capability, not a collaborative tool. When a `config.json#executorRouting` role entry carries `sshTarget` + `remoteWorkdir`, that role's phase dispatch runs over SSH instead of locally — the same resolved invocation (executor + model + args + stdin spec) that would run locally today runs instead as:

```text
ssh -o BatchMode=yes <target> "cd '<remote-workdir>' && '<executor>' '<arg>' ..."
```

The task spec is piped over the held ssh session's stdin; the executor's stdout/stderr/exit status/session-id flow back over the same session. `/gal pipeline` and golem dispatch behave identically whether the resolved route is local or remote — there is no separate remote-execution syntax or activation phrase.

## Configuration

Add `sshTarget` and `remoteWorkdir` to a dispatch role entry under the `pipeline` group of `~/.gal/config/config.json#executorRouting` (remote dispatch applies to dispatch-class roles `CODER`/`TESTER`/`AUDITOR`, which live in `pipeline`):

```json
{
  "executorRouting": {
    "pipeline": {
      "TESTER": {
        "executor": "claude",
        "model": "claude-sonnet-4-6",
        "sshTarget": "user@build-box",
        "remoteWorkdir": "/home/user/gal-remote"
      }
    }
  }
}
```

Both fields are required together — a half-configured entry (only one of the two) loads with a warning and fails loud at dispatch time (`remote-missing-workdir`). A role entry with neither field stays local (default, unchanged).

| Field | Meaning |
| --- | --- |
| `sshTarget` | `user@host` or a configured `ssh` alias for the remote machine. |
| `remoteWorkdir` | Absolute path to a **GAL-dedicated** git checkout on the remote machine (see Safety Boundaries below). |

## Remote Prerequisites

The remote machine needs only what the user sets up directly — GAL provisions none of it:

- **Passwordless SSH** (`BatchMode=yes` — an interactive password/passphrase prompt is treated as unreachable, not retried).
- **The routed agent CLI** (`claude` / `codex` / `agy` / `opencode`) installed and authenticated on the remote machine. `gal` itself is **not** needed on the remote — the composed command runs the raw agent CLI directly.
- **`git`** in `remoteWorkdir`, at the same commit as the control node's checkout, with a clean working tree, before each dispatch (see the pre-run guard below).

## Execution Model

- **Synchronous**: dispatch already blocks to exit (with timeout) for local execution; the SSH lane is synchronous too — the held ssh session's lifetime is the run's lifetime. There is no detached session, no background poll, no separate "collect the result later" step.
- **Pre-run guard**: before running the agent, GAL checks that the remote checkout's `git rev-parse HEAD` matches the control node's current HEAD and that `git status --porcelain` is empty. A mismatch or dirty tree fails the dispatch loud (`remote-guard-failed`) rather than risking a stale or conflicting base.
- **Session id**: every adapter except `agy` parses the provider-native session id from stdout, which is genuinely forwarded over the ssh session. `agy`'s adapter instead scans the *local* filesystem (`~/.agy/brain/`) for its session id — since that path is meaningless on the remote machine, a remote `agy` dispatch honestly reports `session_id: none` rather than a stale local value.
- **Unsupported delivery**: `copilot` delivers its task spec via a CLI flag (`-p <spec>`), which cannot be safely forwarded through an ssh command line. A remote route configured for `copilot` fails loud (`remote-cliflag-unsupported`) — copilot is local-only in this version, unchanged by the shared routing options below.
- **Shared typed routing options**: a remote route uses the **same** typed `executorRouting` schema as a local route — `executor`, `model`, and the optional `effort` key (see [Headless Executor Routing](manual.md#headless-executor-routing)). The `effort` pre-dispatch check runs **before** the local/remote branch split, so a remote route honors `effort` (mapping it to the executor's native reasoning flag) or rejects it identically (`unsupported-effort` / `invalid-effort`) with no remote-specific behavior. Every executor-required flag stays adapter-owned in Rust (e.g. OpenCode's `--auto --agent build`); the routing schema never carries raw flags. No `sshTarget`/`remoteWorkdir`-only options exist beyond the two remote fields themselves.

## Receipt Handling

For phases that use a write-back receipt (test/audit), the receipt lives on the **remote** filesystem inside `remoteWorkdir`. After a successful remote run, GAL fetches the receipt bytes over a second ssh call (`cat` the workdir-relative receipt path) and writes them to the control node's expected receipt path before verifying it — the same non-empty-file rule local dispatch uses. Fetch failure or an empty result reports `no-receipt`, matching the existing local contract.

An **explicit absolute** `--receipt` path cannot be mapped to a workdir-relative remote path and is rejected loud (`remote-absolute-receipt-unsupported`) — only receipts resolved under the dispatch workdir can be threaded to the remote side.

## File-Return (Safety Boundaries)

For the mutating `implement` phase, after a successful remote run GAL fetches the remote's uncommitted changes as a binary-safe patch (stage everything, diff cached, then unstage — leaving the remote tree byte-identical), applies that patch to the **control-node** workdir, and only then — **only after a successful local `git apply`** — runs a destructive cleanup on the remote checkout (`git reset --hard && git clean -fd`).

**`remoteWorkdir` must be a GAL-dedicated checkout.** The post-apply cleanup is destructive by design: any uncommitted human work sitting in that directory when a mutating dispatch runs will be discarded. Do not point `remoteWorkdir` at a checkout you also use interactively.

If the local `git apply` fails, the remote checkout is **left intact** (never cleaned) so the fetched diff can be inspected or retried, and the dispatch itself reports a failure rather than silently discarding the remote's work.

Commit boundary is unchanged: applied edits are reviewed and committed on the **control node**, exactly like local-lane output. The remote agent never runs `git commit`.

## Remote HEAD Progression (Known Boundary)

The pre-run guard requires the remote checkout to be at the control node's *current* HEAD — but `/gal pipeline` commits on the control node after each task, so the control HEAD advances while the remote checkout stays at whatever HEAD it was last synced to. A second (or later) remote dispatch in the same pipeline run will fail the guard until the remote checkout is re-synced.

This is intentional: GAL never drives the user's git remote, and a fail-loud guard is safer than silently stacking one task's diff on top of another's stale base. In this version, the remote lane is **single-dispatch / manually-re-synced** — an autonomous multi-task remote pipeline that self-advances the remote checkout after each control-node commit is a deferred follow-up, not implemented here.

## Remote Self-Test (`gal doctor --executor-smoke --transport ssh`)

The remote lane has a durable, repeatable self-test that validates coding-agent readiness on a remote machine without running a real pipeline task:

```sh
gal doctor --executor-smoke --transport ssh --ssh-target <ssh-target> --remote-workdir <dedicated-checkout> --strict --json
```

It reuses the local self-test's smoke core, report schema, renderer, run store, and status classification, and drives this **same** remote dispatch branch (guard, unsupported-CliFlag rejection, receipt fetch, terminal-state sync) — the SSH-specific additions are a non-interactive reachability precheck and remote install/auth probes evaluated on the remote machine. `--ssh-target` and `--remote-workdir` are both required; each run persists under gitignored `.dev/executor-smoke/runs/<utc-run-id>/ssh/` (never the root `.dev/executor-logs/`).

Beyond the shared statuses, the remote self-test adds three remote-only classifications so a remote failure is never collapsed into a generic one:

| Status | Meaning |
| --- | --- |
| `SSH_UNREACHABLE` | Control node could not open a non-interactive SSH session to the target (before any executor probe). |
| `REMOTE_GUARD_FAILED` | The remote workdir is missing, unsafe, dirty, or not at the control node's HEAD (the pre-run guard above). |
| `REMOTE_FETCH_FAILED` | The remote process may have run but its receipt could not be fetched back over SSH — distinct from a genuinely missing/empty remote receipt, which stays `NO_RECEIPT`. |

`PASS` still requires terminal `completed` **plus** a fetched, non-empty receipt. Remote `copilot` reports `UNSUPPORTED` (its CLI-flag delivery is not forwardable over SSH). Continuous validation is scheduler-driven (CI / cron / Task Scheduler running the same strict-JSON command); GAL runs no remote daemon and never self-syncs the remote checkout — re-sync it to the control node's HEAD before a scheduled run. Full status meanings and remediation live in `docs/manual.md` → Remote (SSH) Self-Test. In shared examples always use a `<ssh-target>` placeholder, never a literal machine hostname.

## Out of Scope (v1)

- **Windows remote** — the composed remote command assumes a POSIX login shell (macOS/Linux). Windows remote execution is deferred.
- **copilot remote** — local-only, see Execution Model above.
- **Disconnect survival** — if the ssh session drops mid-run, the dispatch fails; there is no reconnect/resume.
- **Autonomous remote HEAD progression** — see Remote HEAD Progression above.
