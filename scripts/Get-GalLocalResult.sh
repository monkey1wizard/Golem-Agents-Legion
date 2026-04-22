#!/usr/bin/env bash
# Get-GalLocalResult.sh — retrieve results from a Mac Mini local async task
# dispatched by Invoke-GalLocalTask.sh.
#
# Same role as Get-GalRemoteResult.ps1 but local: no SCP, just reads files
# from the task output directory, prints status + summary, optionally cleans
# up the worktree and Zellij session.
#
# Usage:
#   Get-GalLocalResult.sh --task-id 20260422-abc123 \
#       --output-dir /tmp/gal-worker/20260422-abc123 \
#       [--wait] [--poll-seconds 30] [--timeout-minutes 60] \
#       [--keep] [--repo-path /path/to/repo]

set -euo pipefail

TASK_ID=""
OUTPUT_DIR=""
WAIT=0
POLL_SECONDS=30
TIMEOUT_MINUTES=60
KEEP=0
REPO_PATH=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --task-id)         TASK_ID="$2"; shift 2 ;;
        --output-dir)      OUTPUT_DIR="$2"; shift 2 ;;
        --wait)            WAIT=1; shift ;;
        --poll-seconds)    POLL_SECONDS="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        --keep)            KEEP=1; shift ;;
        --repo-path)       REPO_PATH="$2"; shift 2 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "$TASK_ID" || -z "$OUTPUT_DIR" ]]; then
    echo "Usage: Get-GalLocalResult.sh --task-id ID --output-dir DIR [--wait] [--keep]" >&2
    exit 2
fi

STATUS_PATH="$OUTPUT_DIR/status.json"
SUMMARY_PATH="$OUTPUT_DIR/summary.md"
PATCH_PATH="$OUTPUT_DIR/result.patch"
SESSION="gal-task-$TASK_ID"

# ── Optional wait loop ────────────────────────────────────────────────────────
if [[ "$WAIT" -eq 1 ]]; then
    echo "Waiting for $TASK_ID to leave 'running' (timeout: ${TIMEOUT_MINUTES}m)..."
    deadline=$(( $(date +%s) + TIMEOUT_MINUTES * 60 ))
    while (( $(date +%s) < deadline )); do
        if [[ -f "$STATUS_PATH" ]]; then
            cur="$(jq -r '.status // "running"' "$STATUS_PATH" 2>/dev/null || echo running)"
            if [[ "$cur" != "running" ]]; then
                echo "Task $TASK_ID — status: $cur"
                break
            fi
        fi
        echo "  Still running... (next check in ${POLL_SECONDS}s)"
        sleep "$POLL_SECONDS"
    done
fi

# ── Print status + summary ────────────────────────────────────────────────────
echo
if [[ -f "$STATUS_PATH" ]]; then
    echo "─── Task Status ───────────────────────────────────────────"
    jq -r '
        "  Task ID:    " + (.taskId // "?")        + "\n" +
        "  Status:     " + (.status // "?")        + "\n" +
        "  Exit Code:  " + ((.exitCode // 0) | tostring) + "\n" +
        "  Started:    " + (.startedAt // "?")     + "\n" +
        "  Finished:   " + (.finishedAt // "n/a")  + "\n" +
        (if .errorMessage then "  Error:      " + .errorMessage + "\n" else "" end)
    ' "$STATUS_PATH"
    echo "───────────────────────────────────────────────────────────"
else
    echo "status.json not found at $STATUS_PATH" >&2
fi

if [[ -f "$SUMMARY_PATH" ]]; then
    echo
    echo "─── Summary ───────────────────────────────────────────────"
    cat "$SUMMARY_PATH"
    echo "───────────────────────────────────────────────────────────"
fi

if [[ -f "$PATCH_PATH" ]]; then
    psize=$(wc -c < "$PATCH_PATH" | tr -d ' ')
    echo
    if [[ "$psize" -gt 0 ]]; then
        echo "result.patch: $psize bytes — review and apply with: git apply '$PATCH_PATH'"
    else
        echo "result.patch: empty (read-only task, no file changes)"
    fi
fi

# ── Cleanup ───────────────────────────────────────────────────────────────────
if [[ "$KEEP" -eq 0 ]]; then
    echo
    echo "Cleaning up..."

    WT_PATH=""
    if [[ -f "$STATUS_PATH" ]]; then
        WT_PATH="$(jq -r '.worktreePath // empty' "$STATUS_PATH" 2>/dev/null || true)"
    fi

    if [[ -n "$WT_PATH" && -d "$WT_PATH" ]]; then
        # Need a real repo path to call `git worktree remove` — derive from worktree
        # parent if --repo-path not given.
        repo="${REPO_PATH:-${WT_PATH%-worker-*}}"
        if [[ -d "$repo/.git" ]]; then
            git -C "$repo" worktree unlock "$WT_PATH" 2>/dev/null || true
            if git -C "$repo" worktree remove --force "$WT_PATH" 2>&1; then
                echo "  Worktree removed: $WT_PATH"
            else
                echo "  Worktree removal failed; manual cleanup may be needed: $WT_PATH" >&2
            fi
        else
            echo "  Could not infer repo for worktree $WT_PATH (pass --repo-path)" >&2
        fi
    fi

    if zellij list-sessions 2>/dev/null | grep -q -F "$SESSION"; then
        zellij delete-session --force "$SESSION" >/dev/null 2>&1 || true
        echo "  Zellij session deleted: $SESSION"
    fi

    rm -rf "$OUTPUT_DIR"
    echo "  Output dir removed: $OUTPUT_DIR"
fi
