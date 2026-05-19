#!/usr/bin/env bash
# Invoke-XmachineLocalTask.sh — macOS/Linux local async dispatcher.
#
# Hosts each task in a detached launcher so the
# task run survives:
#   - VS Code Remote-SSH disconnect
#   - SSH session drop
#   - controller disconnect while the controlled machine stays online
#
# Companion to Invoke-XmachineRemoteTask.ps1 (which dispatches over SSH to a Win
# burst machine). Same task/result contract: status.json, summary.md, runtime log
# runtime.log, and result.patch under <output-dir>.
#
# Usage:
#   Invoke-XmachineLocalTask.sh \
#       --task-spec /path/to/task.md \
#       [--repo-path /Users/username/Code/target-repo] \
#       [--timeout-minutes 30] \
#       [--task-id 20260422-abc123]   # auto-generated if omitted
#
# Prefers Zellij when available and falls back to `nohup` when it is not.
# Prints the generated TaskId, OutputDir, and launcher metadata on success.

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

if [[ -z "$TASK_SPEC" ]]; then
    echo "Usage: Invoke-XmachineLocalTask.sh --task-spec <file> [--repo-path <repo>] [--timeout-minutes N] [--task-id ID]" >&2
    exit 2
fi
if [[ ! -f "$TASK_SPEC" ]]; then
    echo "Task spec not found: $TASK_SPEC" >&2
    exit 2
fi
if [[ -n "$REPO_PATH" && ! -d "$REPO_PATH/.git" ]]; then
    echo "Not a git repo: $REPO_PATH" >&2
    exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
    echo "Missing required tool: jq" >&2
    exit 2
fi

if [[ -n "$REPO_PATH" ]] && ! command -v git >/dev/null 2>&1; then
    echo "Missing required tool for repo mode: git" >&2
    exit 2
fi

LAUNCHER=""
if command -v zellij >/dev/null 2>&1 && command -v script >/dev/null 2>&1; then
    LAUNCHER="zellij"
elif command -v nohup >/dev/null 2>&1; then
    LAUNCHER="nohup"
else
    echo "Missing detached launcher. Install zellij with the system 'script' utility, or ensure 'nohup' is available." >&2
    exit 2
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUNTIME_SCRIPT="$SCRIPT_DIR/Start-xMachine.sh"
if [[ ! -x "$RUNTIME_SCRIPT" ]]; then
    chmod +x "$RUNTIME_SCRIPT" 2>/dev/null || true
fi
if [[ ! -f "$RUNTIME_SCRIPT" ]]; then
    echo "Runtime script missing: $RUNTIME_SCRIPT" >&2
    exit 2
fi

# ── Generate task id ──────────────────────────────────────────────────────────
if [[ -z "$TASK_ID" ]]; then
    rand="$(LC_ALL=C tr -dc 'a-z0-9' </dev/urandom | head -c 6 || true)"
    TASK_ID="$(date -u +%Y%m%d)-$rand"
fi

OUTPUT_DIR="${TMPDIR:-/tmp}/gal-xmachine/task-$TASK_ID"
WORKTREE="$OUTPUT_DIR/workspace"
SESSION="task-$TASK_ID"
TASK_SPEC_LOCAL="$OUTPUT_DIR/task.md"

mkdir -p "$OUTPUT_DIR"
cp "$TASK_SPEC" "$TASK_SPEC_LOCAL"

if [[ -n "$REPO_PATH" ]]; then
    # ── Create disposable worktree, bypassing the gal-config smudge filter ────────
    # The smudge filter calls scripts/gal-smudge.sh, which doesn't exist yet during
    # checkout (race condition); -c overrides the filter for this command only.
    WORKTREE="${REPO_PATH%/}-xmachine-$TASK_ID"
    echo "Creating disposable worktree: $WORKTREE"
    if ! git -C "$REPO_PATH" \
            -c filter.gal-config.smudge=cat \
            -c filter.gal-config.clean=cat \
            worktree add --detach "$WORKTREE" HEAD 2>&1; then
        echo "Failed to create worktree at $WORKTREE" >&2
        exit 3
    fi
else
    echo "Creating disposable execute workspace: $WORKTREE"
    mkdir -p "$WORKTREE"
fi

# ── Spawn the task runtime inside a detached launcher ────────────────────────
if [[ "$LAUNCHER" == "zellij" ]]; then
    # Pattern (verified on macOS 26 / zellij 0.44.1):
    #   script -q /dev/null zellij attach --create-background <name>
    # `script` is a system PTY utility from the base POSIX userland; it provides the
    # pty zellij requires here, while `--create-background` keeps the session detached.
    echo "Spawning detached Zellij session: $SESSION"
    script -q /dev/null zellij attach --create-background "$SESSION" \
        </dev/null >/dev/null 2>&1 &
    SPAWN_PID=$!

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
        if [[ -n "$REPO_PATH" ]]; then
            git -C "$REPO_PATH" worktree remove --force "$WORKTREE" 2>/dev/null || true
        else
            rm -rf "$WORKTREE"
        fi
        exit 4
    fi

    zellij --session "$SESSION" run --close-on-exit -- \
        bash "$RUNTIME_SCRIPT" \
            --task-id        "$TASK_ID" \
            --worktree       "$WORKTREE" \
            --task-spec      "$TASK_SPEC_LOCAL" \
            --output-dir     "$OUTPUT_DIR" \
            --timeout-minutes "$TIMEOUT_MINUTES" \
            --engine         gemini \
        >/dev/null 2>&1
else
    echo "Spawning detached nohup process for task: $TASK_ID"
    nohup bash "$RUNTIME_SCRIPT" \
        --task-id "$TASK_ID" \
        --worktree "$WORKTREE" \
        --task-spec "$TASK_SPEC_LOCAL" \
        --output-dir "$OUTPUT_DIR" \
        --timeout-minutes "$TIMEOUT_MINUTES" \
        --engine gemini \
        >/dev/null 2>&1 </dev/null &
    SPAWN_PID=$!
    disown "$SPAWN_PID" 2>/dev/null || true
    sleep 0.2

    if ! kill -0 "$SPAWN_PID" 2>/dev/null; then
        echo "Failed to start detached nohup process for task $TASK_ID" >&2
        if [[ -n "$REPO_PATH" ]]; then
            git -C "$REPO_PATH" worktree remove --force "$WORKTREE" 2>/dev/null || true
        else
            rm -rf "$WORKTREE"
        fi
        exit 4
    fi
fi

# ── Print dispatch metadata ───────────────────────────────────────────────────
if [[ "$LAUNCHER" == "zellij" ]]; then
        live_hint="zellij attach $SESSION              # detach with Ctrl-p, d"
        session_line="  Session:   $SESSION"
else
        live_hint="tail -f $OUTPUT_DIR/runtime.log"
        session_line=""
fi

cat <<EOF

Dispatched local async task.
    TaskId:    $TASK_ID
    Mode:      $( [[ -n "$REPO_PATH" ]] && printf repo || printf execute )
    Launcher:  $LAUNCHER
$session_line
    Workspace: $WORKTREE
    Output:    $OUTPUT_DIR

Inspect live:
    $live_hint
Retrieve when done:
        scripts/Get-XmachineLocalResult.sh --task-id $TASK_ID --output-dir $OUTPUT_DIR
EOF
