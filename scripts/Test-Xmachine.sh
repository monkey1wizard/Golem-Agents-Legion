#!/usr/bin/env bash
# Test-Xmachine.sh — smoke-test wrapper for the xmachine POSIX work-node path.
#
# Run this on a macOS/Linux work node after connecting over SSH, or directly on
# the work node itself. It stages the committed smoke task, dispatches it
# through Invoke-XmachineLocalTask.sh, and optionally waits for retrieval
# through Get-XmachineLocalResult.sh.

set -euo pipefail

REPO_PATH=""
TASK_SPEC=""
TIMEOUT_MINUTES=30
WAIT=0
KEEP=0
POLL_SECONDS=30
TASK_ID=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --repo-path)       REPO_PATH="$2"; shift 2 ;;
        --task-spec)       TASK_SPEC="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        --wait)            WAIT=1; shift ;;
        --keep)            KEEP=1; shift ;;
        --poll-seconds)    POLL_SECONDS="$2"; shift 2 ;;
        --task-id)         TASK_ID="$2"; shift 2 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "$REPO_PATH" ]]; then
    echo "Usage: Test-Xmachine.sh --repo-path <repo> [--task-spec <file>] [--timeout-minutes N] [--wait] [--keep] [--poll-seconds N] [--task-id ID]" >&2
    exit 2
fi

if [[ ! -d "$REPO_PATH/.git" ]]; then
    echo "Not a git repo: $REPO_PATH" >&2
    exit 2
fi

INVOKE_SCRIPT="$REPO_PATH/scripts/Invoke-XmachineLocalTask.sh"
RETRIEVE_SCRIPT="$REPO_PATH/scripts/Get-XmachineLocalResult.sh"
DEFAULT_TASK_SPEC="$REPO_PATH/templates/task-xmachine-local-smoke.md"

TASK_SPEC="${TASK_SPEC:-$DEFAULT_TASK_SPEC}"

for required in "$INVOKE_SCRIPT" "$RETRIEVE_SCRIPT" "$TASK_SPEC"; do
    if [[ ! -f "$required" ]]; then
        echo "Required file missing: $required" >&2
        exit 2
    fi
done

dispatch_args=(
    --task-spec "$TASK_SPEC"
    --repo-path "$REPO_PATH"
    --timeout-minutes "$TIMEOUT_MINUTES"
)

if [[ -n "$TASK_ID" ]]; then
    dispatch_args+=(--task-id "$TASK_ID")
fi

set +e
dispatch_output="$(bash "$INVOKE_SCRIPT" "${dispatch_args[@]}" 2>&1)"
dispatch_exit=$?
set -e

printf '%s\n' "$dispatch_output"

if [[ $dispatch_exit -ne 0 ]]; then
    exit $dispatch_exit
fi

TASK_ID="$(printf '%s\n' "$dispatch_output" | sed -n 's/^  TaskId:[[:space:]]*//p' | head -n 1)"
OUTPUT_DIR="$(printf '%s\n' "$dispatch_output" | sed -n 's/^  Output:[[:space:]]*//p' | head -n 1)"

if [[ -z "$TASK_ID" || -z "$OUTPUT_DIR" ]]; then
    echo "Could not parse TaskId or Output directory from Invoke-XmachineLocalTask.sh output." >&2
    exit 3
fi

if [[ "$WAIT" -eq 0 ]]; then
    echo
    echo "Smoke task dispatched. Retrieve when ready with:"
    echo "  bash scripts/Get-XmachineLocalResult.sh --task-id $TASK_ID --output-dir $OUTPUT_DIR --wait --repo-path $REPO_PATH"
    exit 0
fi

retrieve_args=(
    --task-id "$TASK_ID"
    --output-dir "$OUTPUT_DIR"
    --wait
    --poll-seconds "$POLL_SECONDS"
    --timeout-minutes "$TIMEOUT_MINUTES"
    --repo-path "$REPO_PATH"
)

if [[ "$KEEP" -eq 1 ]]; then
    retrieve_args+=(--keep)
fi

echo
echo "Waiting for smoke task completion..."
bash "$RETRIEVE_SCRIPT" "${retrieve_args[@]}"