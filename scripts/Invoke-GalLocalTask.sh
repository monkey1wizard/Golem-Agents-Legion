#!/usr/bin/env bash
# Invoke-GalLocalTask.sh — Mac Mini always-on local async dispatcher.
#
# Hosts each task in a detached Zellij session named "gal-task-<taskId>" so the
# worker survives:
#   - VS Code Remote-SSH disconnect
#   - SSH session drop
#   - Win11 main PC shutdown (because the lane runs entirely on Mac Mini)
#
# Companion to Invoke-GalRemoteTask.ps1 (which dispatches over SSH to a Win
# burst worker). Same task/result contract: status.json, summary.md, worker.log,
# result.patch under <output-dir>.
#
# Usage:
#   Invoke-GalLocalTask.sh \
#       --task-spec /path/to/task.md \
#       --repo-path /Users/tzylee/Code/Golem-Agents-Legion \
#       [--timeout-minutes 30] \
#       [--task-id 20260422-abc123]   # auto-generated if omitted
#
# Prints the generated TaskId, OutputDir, and Zellij session name on success.

set -euo pipefail

# ── Args ──────────────────────────────────────────────────────────────────────
TASK_SPEC=""
REPO_PATH=""
TIMEOUT_MINUTES=30
TASK_ID=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --task-spec)       TASK_SPEC="$2"; shift 2 ;;
        --repo-path)       REPO_PATH="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        --task-id)         TASK_ID="$2"; shift 2 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "$TASK_SPEC" || -z "$REPO_PATH" ]]; then
    echo "Usage: Invoke-GalLocalTask.sh --task-spec <file> --repo-path <repo> [--timeout-minutes N] [--task-id ID]" >&2
    exit 2
fi
if [[ ! -f "$TASK_SPEC" ]]; then
    echo "Task spec not found: $TASK_SPEC" >&2
    exit 2
fi
if [[ ! -d "$REPO_PATH/.git" ]]; then
    echo "Not a git repo: $REPO_PATH" >&2
    exit 2
fi

for tool in zellij git jq script; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing required tool: $tool" >&2
        exit 2
    fi
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKER_SCRIPT="$SCRIPT_DIR/Start-GalWorker.sh"
if [[ ! -x "$WORKER_SCRIPT" ]]; then
    chmod +x "$WORKER_SCRIPT" 2>/dev/null || true
fi
if [[ ! -f "$WORKER_SCRIPT" ]]; then
    echo "Worker script missing: $WORKER_SCRIPT" >&2
    exit 2
fi

# ── Generate task id ──────────────────────────────────────────────────────────
if [[ -z "$TASK_ID" ]]; then
    rand="$(LC_ALL=C tr -dc 'a-z0-9' </dev/urandom | head -c 6 || true)"
    TASK_ID="$(date -u +%Y%m%d)-$rand"
fi

OUTPUT_DIR="${TMPDIR:-/tmp}/gal-worker/$TASK_ID"
WORKTREE="${REPO_PATH%/}-worker-$TASK_ID"
SESSION="gal-task-$TASK_ID"
TASK_SPEC_LOCAL="$OUTPUT_DIR/task.md"

mkdir -p "$OUTPUT_DIR"
cp "$TASK_SPEC" "$TASK_SPEC_LOCAL"

# ── Create disposable worktree, bypassing the gal-config smudge filter ────────
# The smudge filter calls scripts/gal-smudge.sh, which doesn't exist yet during
# checkout (race condition); -c overrides the filter for this command only.
echo "Creating disposable worktree: $WORKTREE"
if ! git -C "$REPO_PATH" \
        -c filter.gal-config.smudge=cat \
        -c filter.gal-config.clean=cat \
        worktree add --detach "$WORKTREE" HEAD 2>&1; then
    echo "Failed to create worktree at $WORKTREE" >&2
    exit 3
fi

# ── Spawn worker inside a detached Zellij session ─────────────────────────────
# Pattern (verified on macOS 26 / zellij 0.44.1):
#   script -q /dev/null zellij attach --create-background <name>
# `script` provides the pty zellij requires; `--create-background` keeps the
# session detached so this dispatcher returns immediately.
#
# Then `zellij --session <name> run -- ...` launches the worker as a new pane
# inside that session. The pane survives terminal close, SSH drop, and Win11
# shutdown.
echo "Spawning detached Zellij session: $SESSION"
script -q /dev/null zellij attach --create-background "$SESSION" \
    </dev/null >/dev/null 2>&1 &
SPAWN_PID=$!

# Wait briefly for the session to register (zellij list-sessions output
# contains ANSI color codes, so use a plain substring grep)
session_exists() {
    zellij list-sessions 2>/dev/null | grep -q -F "$SESSION"
}

for _ in 1 2 3 4 5 6 7 8 9 10; do
    if session_exists; then
        break
    fi
    sleep 0.3
done

if ! session_exists; then
    echo "Failed to start Zellij session $SESSION" >&2
    kill "$SPAWN_PID" 2>/dev/null || true
    git -C "$REPO_PATH" worktree remove --force "$WORKTREE" 2>/dev/null || true
    exit 4
fi

# Launch the worker as a pane inside the detached session
zellij --session "$SESSION" run --close-on-exit -- \
    bash "$WORKER_SCRIPT" \
        --task-id        "$TASK_ID" \
        --worktree       "$WORKTREE" \
        --task-spec      "$TASK_SPEC_LOCAL" \
        --output-dir     "$OUTPUT_DIR" \
        --timeout-minutes "$TIMEOUT_MINUTES" \
        --engine         gemini \
    >/dev/null 2>&1

# ── Print dispatch metadata ───────────────────────────────────────────────────
cat <<EOF

Dispatched local async task.
  TaskId:    $TASK_ID
  Session:   $SESSION
  Worktree:  $WORKTREE
  Output:    $OUTPUT_DIR

Inspect live:
  zellij attach $SESSION              # detach with Ctrl-p, d
Retrieve when done:
  scripts/Get-GalLocalResult.sh --task-id $TASK_ID --output-dir $OUTPUT_DIR
EOF
