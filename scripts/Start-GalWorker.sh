#!/usr/bin/env bash
# Start-GalWorker.sh — bash GAL worker (Mac Mini / Linux)
# Mirrors scripts/Start-GalWorker.ps1 output contract exactly:
#   status.json, summary.md, worker.log, result.patch
# Spawned either directly (Linux) or inside a detached Zellij session
# (Mac Mini always-on lane via Invoke-GalLocalTask.sh).
#
# Usage:
#   Start-GalWorker.sh \
#       --task-id  20260422-abc123 \
#       --worktree /path/to/disposable/worktree \
#       --task-spec /path/to/task.md \
#       --output-dir /tmp/gal-worker/20260422-abc123 \
#       [--timeout-minutes 30] \
#       [--engine gemini]
#
# Output files all written under --output-dir.
# Exit code: 0 on success, non-zero on failure.

set -u
set -o pipefail

# ── Args ──────────────────────────────────────────────────────────────────────
TASK_ID=""
WORKTREE=""
TASK_SPEC=""
OUTPUT_DIR=""
TIMEOUT_MINUTES=30
ENGINE="gemini"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --task-id)         TASK_ID="$2"; shift 2 ;;
        --worktree)        WORKTREE="$2"; shift 2 ;;
        --task-spec)       TASK_SPEC="$2"; shift 2 ;;
        --output-dir)      OUTPUT_DIR="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        --engine)          ENGINE="$2"; shift 2 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

_require() {
    local name="$1"; local val="$2"; local flag="$3"
    if [[ -z "$val" ]]; then
        echo "Missing required arg: $flag" >&2
        exit 2
    fi
}
_require TASK_ID    "$TASK_ID"    --task-id
_require WORKTREE   "$WORKTREE"   --worktree
_require TASK_SPEC  "$TASK_SPEC"  --task-spec
_require OUTPUT_DIR "$OUTPUT_DIR" --output-dir

mkdir -p "$OUTPUT_DIR"
STATUS_PATH="$OUTPUT_DIR/status.json"
SUMMARY_PATH="$OUTPUT_DIR/summary.md"
LOG_PATH="$OUTPUT_DIR/worker.log"
PATCH_PATH="$OUTPUT_DIR/result.patch"

iso_now() { date -u +"%Y-%m-%dT%H:%M:%SZ"; }
STARTED_AT="$(iso_now)"

# ── status.json writer (jq-based, schema mirrors Start-GalWorker.ps1) ─────────
write_status() {
    local status="$1"
    local exit_code="${2:-0}"
    local error_msg="${3:-}"
    local finished_at
    finished_at="$(iso_now)"

    jq -n \
        --arg taskId        "$TASK_ID" \
        --arg status        "$status" \
        --argjson exitCode  "$exit_code" \
        --arg startedAt     "$STARTED_AT" \
        --arg finishedAt    "$finished_at" \
        --arg engine        "$ENGINE-cli" \
        --arg worktreePath  "$WORKTREE" \
        --arg errorMessage  "$error_msg" \
        '{
            taskId:        $taskId,
            status:        $status,
            exitCode:      $exitCode,
            startedAt:     $startedAt,
            finishedAt:    (if $status == "running" then null else $finishedAt end),
            engine:        $engine,
            worktreePath:  $worktreePath,
            errorMessage:  (if $errorMessage == "" then null else $errorMessage end)
        }' > "$STATUS_PATH"
}

write_status "running" 0 ""

# ── Sanity checks ─────────────────────────────────────────────────────────────
if [[ ! -d "$WORKTREE" ]]; then
    write_status "failed" 1 "Worktree path does not exist: $WORKTREE"
    echo "Worker $TASK_ID — FAILED: missing worktree" >&2
    exit 1
fi
if [[ ! -f "$TASK_SPEC" ]]; then
    write_status "failed" 1 "Task spec not found: $TASK_SPEC"
    echo "Worker $TASK_ID — FAILED: missing task spec" >&2
    exit 1
fi

if ! command -v "$ENGINE" >/dev/null 2>&1; then
    write_status "failed" 1 "Engine '$ENGINE' not on PATH"
    echo "Worker $TASK_ID — FAILED: engine '$ENGINE' missing" >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    write_status "failed" 1 "jq not on PATH (required for status.json + stream-json parsing)"
    exit 1
fi

# ── Build prompt (read task spec verbatim) ────────────────────────────────────
TASK_PROMPT="$(cat "$TASK_SPEC")"

# ── Spawn engine inside worktree, with a portable watchdog timeout ────────────
TIMEOUT_SECONDS=$(( TIMEOUT_MINUTES * 60 ))

(
    cd "$WORKTREE" || exit 1
    "$ENGINE" --yolo -p "$TASK_PROMPT" --output-format stream-json
) </dev/null >"$LOG_PATH" 2>&1 &
ENGINE_PID=$!

(
    sleep "$TIMEOUT_SECONDS"
    if kill -0 "$ENGINE_PID" 2>/dev/null; then
        echo "" >>"$LOG_PATH"
        echo "TIMEOUT: $ENGINE CLI exceeded ${TIMEOUT_MINUTES}m — task killed" >>"$LOG_PATH"
        # Kill the whole process group of the engine
        kill -TERM "-$ENGINE_PID" 2>/dev/null || kill -TERM "$ENGINE_PID" 2>/dev/null
        sleep 2
        kill -KILL "-$ENGINE_PID" 2>/dev/null || kill -KILL "$ENGINE_PID" 2>/dev/null
    fi
) &
WATCHDOG_PID=$!

(
    while kill -0 "$ENGINE_PID" 2>/dev/null; do
        if grep -qE 'MODEL_CAPACITY_EXHAUSTED|No capacity available for model' "$LOG_PATH" 2>/dev/null; then
            echo "" >>"$LOG_PATH"
            echo "CAPACITY_EXHAUSTED: Gemini model capacity unavailable — task killed early" >>"$LOG_PATH"
            kill -TERM "-$ENGINE_PID" 2>/dev/null || kill -TERM "$ENGINE_PID" 2>/dev/null
            sleep 2
            kill -KILL "-$ENGINE_PID" 2>/dev/null || kill -KILL "$ENGINE_PID" 2>/dev/null
            break
        fi
        sleep 5
    done
) &
CAPACITY_WATCHDOG_PID=$!

wait "$ENGINE_PID"
ENGINE_EXIT=$?

# Reap the watchdog if engine finished first
kill "$WATCHDOG_PID" 2>/dev/null || true
wait "$WATCHDOG_PID" 2>/dev/null || true
kill "$CAPACITY_WATCHDOG_PID" 2>/dev/null || true
wait "$CAPACITY_WATCHDOG_PID" 2>/dev/null || true

# Detect timeout (engine killed by watchdog)
TIMED_OUT=0
if grep -q "^TIMEOUT: $ENGINE CLI exceeded" "$LOG_PATH" 2>/dev/null; then
    TIMED_OUT=1
fi

CAPACITY_EXHAUSTED=0
if grep -q "^CAPACITY_EXHAUSTED: Gemini model capacity unavailable" "$LOG_PATH" 2>/dev/null; then
    CAPACITY_EXHAUSTED=1
fi

# ── Generate result patch ─────────────────────────────────────────────────────
if ! git -C "$WORKTREE" diff HEAD > "$PATCH_PATH" 2>>"$LOG_PATH"; then
    echo "Failed to generate patch (see worker.log)" > "$PATCH_PATH"
fi

# ── Extract summary from stream-json log (3-strategy parser) ─────────────────
extract_summary() {
    local current_block=""
    local last_block=""
    local line_text=""

    # Prefer the final contiguous assistant-text block from stream-json output.
    # This avoids capturing intermediate planning chatter and produces a stable
    # human-readable summary for summary.md.
    while IFS= read -r line; do
        line_text="$(printf '%s' "$line" | jq -r '
            if .type == "message" and .role == "assistant" then
                .content // ""
            else
                [
                    (.text // empty),
                    (.content.parts[]?.text // empty),
                    (.candidates[]?.content.parts[]?.text // empty)
                ]
                | map(select(length > 0))
                | join("")
            end
        ' 2>/dev/null || true)"

        line_text="${line_text//$'\r'/}"
        if [[ -n "$line_text" ]]; then
            current_block+="$line_text"
            continue
        fi

        if [[ -n "$current_block" ]]; then
            last_block="$current_block"
            current_block=""
        fi
    done < "$LOG_PATH"

    if [[ -n "$current_block" ]]; then
        last_block="$current_block"
    fi

    if [[ -n "$last_block" ]]; then
        printf '%s' "$last_block"
        return 0
    fi

    # Fallback: last 20 non-empty log lines
    {
        echo "(Could not parse stream-json — raw log tail)"
        echo
        grep -v '^[[:space:]]*$' "$LOG_PATH" | tail -20
    }
}

SUMMARY_BODY="$(extract_summary)"

if [[ "$TIMED_OUT" -eq 1 ]]; then
    SUMMARY_STATUS="timeout"
elif [[ "$CAPACITY_EXHAUSTED" -eq 1 ]]; then
    SUMMARY_STATUS="failed (model capacity exhausted)"
elif [[ "$ENGINE_EXIT" -eq 0 ]]; then
    SUMMARY_STATUS="success"
else
    SUMMARY_STATUS="failed (exit $ENGINE_EXIT)"
fi

cat > "$SUMMARY_PATH" <<EOF
# Task Summary: $TASK_ID

**Status**: $SUMMARY_STATUS

## Output

$SUMMARY_BODY
EOF

# ── Lock worktree (preserve until retrieval; non-fatal) ──────────────────────
git -C "$WORKTREE" worktree lock "$WORKTREE" --reason "gal-worker-$TASK_ID" 2>/dev/null || true

# ── Write final status.json with Gemini exit-code mapping ────────────────────
# (case statement for bash 3 compatibility — no associative arrays on macOS)
engine_exit_message() {
    case "$1" in
        41) echo "Gemini CLI auth failed — re-auth required on worker before next task" ;;
        42) echo "Gemini CLI input error — task spec may be malformed" ;;
        44) echo "Gemini CLI sandbox error — check worker sandbox configuration" ;;
        75) echo "Gemini model capacity exhausted — retry later or change the configured model" ;;
        52) echo "Gemini CLI config error — check GEMINI.md or settings.json on worker" ;;
        53) echo "Gemini CLI turn limit reached — split task into smaller pieces" ;;
        *)  echo "Gemini CLI exited with code $1 — check worker.log" ;;
    esac
}

if [[ "$TIMED_OUT" -eq 1 ]]; then
    write_status "timeout" -1 "Task exceeded ${TIMEOUT_MINUTES}m timeout — split the task or raise --timeout-minutes"
    echo "Worker finished: $TASK_ID — TIMEOUT" >&2
    exit 1
fi

if [[ "$CAPACITY_EXHAUSTED" -eq 1 ]]; then
    write_status "failed" 75 "Gemini model capacity exhausted — retry later or change the configured model"
    echo "Worker finished: $TASK_ID — FAILED (model capacity exhausted)" >&2
    exit 1
fi

if [[ "$ENGINE_EXIT" -eq 0 ]]; then
    write_status "success" 0 ""
    echo "Worker finished: $TASK_ID — success"
    exit 0
fi

MSG="$(engine_exit_message "$ENGINE_EXIT")"
write_status "failed" "$ENGINE_EXIT" "$MSG"
echo "Worker finished: $TASK_ID — FAILED (exit $ENGINE_EXIT): $MSG" >&2
exit "$ENGINE_EXIT"
