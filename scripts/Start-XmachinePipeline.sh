#!/usr/bin/env bash

set -euo pipefail

RUN_ID=""
WORKTREE=""
PLAN_SNAPSHOT=""
STATE_SNAPSHOT=""
OUTPUT_DIR=""
TIMEOUT_MINUTES=30

while [[ $# -gt 0 ]]; do
    case "$1" in
        --run-id)          RUN_ID="$2"; shift 2 ;;
        --worktree)        WORKTREE="$2"; shift 2 ;;
        --plan-snapshot)   PLAN_SNAPSHOT="$2"; shift 2 ;;
        --state-snapshot)  STATE_SNAPSHOT="$2"; shift 2 ;;
        --output-dir)      OUTPUT_DIR="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

require_arg() {
    local flag_name="$1"
    local flag_value="$2"
    local flag_label="$3"
    if [[ -z "$flag_value" ]]; then
        echo "Missing required arg: $flag_label" >&2
        exit 2
    fi
}

require_arg RUN_ID "$RUN_ID" --run-id
require_arg WORKTREE "$WORKTREE" --worktree
require_arg PLAN_SNAPSHOT "$PLAN_SNAPSHOT" --plan-snapshot
require_arg STATE_SNAPSHOT "$STATE_SNAPSHOT" --state-snapshot
require_arg OUTPUT_DIR "$OUTPUT_DIR" --output-dir

mkdir -p "$OUTPUT_DIR"

STATUS_PATH="$OUTPUT_DIR/pipeline-status.json"
EVENTS_PATH="$OUTPUT_DIR/pipeline-events.jsonl"
TASKS_DIR="$OUTPUT_DIR/tasks"
mkdir -p "$TASKS_DIR"

iso_now() {
    date -u +"%Y-%m-%dT%H:%M:%SZ"
}

append_event() {
    local event_type="$1"
    local details_json="$2"
    jq -cn \
        --arg timestamp "$(iso_now)" \
        --arg type "$event_type" \
        --argjson details "$details_json" \
        '{timestamp:$timestamp,type:$type,details:$details}' >> "$EVENTS_PATH"
}

write_status() {
    local status="$1"
    local phase="$2"
    local tasks_json="$3"
    local message="$4"

    jq -n \
        --arg runId "$RUN_ID" \
        --arg status "$status" \
        --arg phase "$phase" \
        --arg startedAt "$STARTED_AT" \
        --arg updatedAt "$(iso_now)" \
        --arg worktree "$WORKTREE" \
        --arg planSnapshot "$PLAN_SNAPSHOT" \
        --arg stateSnapshot "$STATE_SNAPSHOT" \
        --arg message "$message" \
        --argjson tasks "$tasks_json" \
        '{
            runId:$runId,
            status:$status,
            phase:$phase,
            startedAt:$startedAt,
            updatedAt:$updatedAt,
            worktreePath:$worktree,
            planSnapshotPath:$planSnapshot,
            stateSnapshotPath:$stateSnapshot,
            message:$message,
            tasks:$tasks
        }' > "$STATUS_PATH"
}

if [[ ! -d "$WORKTREE" ]]; then
    echo "Missing worktree: $WORKTREE" >&2
    exit 1
fi

if [[ ! -f "$PLAN_SNAPSHOT" ]]; then
    echo "Missing plan snapshot: $PLAN_SNAPSHOT" >&2
    exit 1
fi

if [[ ! -f "$STATE_SNAPSHOT" ]]; then
    echo "Missing state snapshot: $STATE_SNAPSHOT" >&2
    exit 1
fi

if ! command -v jq >/dev/null 2>&1; then
    echo "Missing required tool: jq" >&2
    exit 1
fi

STARTED_AT="$(iso_now)"

mapfile -t TASK_LINES < <(grep -E '^- \[[ x]\] T-[0-9]{3} ' "$PLAN_SNAPSHOT" || true)

TASKS_JSON='[]'
for task_line in "${TASK_LINES[@]}"; do
    task_id="$(printf '%s' "$task_line" | sed -E 's/^- \[[ x]\] (T-[0-9]{3}).*/\1/')"
    task_done="false"
    if [[ "$task_line" =~ ^-\ \[x\]\  ]]; then
        task_done="true"
    fi

    task_slug="$(printf '%s' "$task_id" | tr '[:upper:]' '[:lower:]')"
    task_dir="$TASKS_DIR/$task_slug"
    mkdir -p "$task_dir"

    task_json="$(jq -cn \
        --arg taskId "$task_id" \
        --arg line "$task_line" \
        --arg taskDir "$task_dir" \
        --argjson complete "$task_done" \
        '{taskId:$taskId,planLine:$line,complete:$complete,status:"queued",taskDir:$taskDir,phases:[]}')"

    TASKS_JSON="$(printf '%s' "$TASKS_JSON" | jq --argjson item "$task_json" '. + [$item]')"
done

append_event "runner-started" '{"message":"persistent xmachine pipeline bootstrap started"}'
append_event "plan-loaded" "$(jq -cn --arg planSnapshot "$PLAN_SNAPSHOT" --arg stateSnapshot "$STATE_SNAPSHOT" --argjson taskCount "${#TASK_LINES[@]}" '{planSnapshot:$planSnapshot,stateSnapshot:$stateSnapshot,taskCount:$taskCount}')"

write_status "initialized" "bootstrap-complete" "$TASKS_JSON" "Persistent xmachine pipeline runner initialized. Task execution loop is the next implementation slice."

append_event "bootstrap-complete" "$(jq -cn --argjson taskCount "${#TASK_LINES[@]}" '{taskCount:$taskCount,message:"pipeline status scaffold created"}')"

exit 0