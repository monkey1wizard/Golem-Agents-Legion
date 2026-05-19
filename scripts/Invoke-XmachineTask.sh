#!/usr/bin/env bash

set -euo pipefail

WORK_NODE=""
TASK_SPEC=""
WORK_REPO_PATH=""
REMOTE_RUNTIME_REPO_PATH=""
WORK_PLATFORM="auto"
TIMEOUT_MINUTES=30
WAIT=0
LOCAL_OUTPUT_DIR=""
KEEP_REMOTE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --work-node) WORK_NODE="$2"; shift 2 ;;
        --task-spec) TASK_SPEC="$2"; shift 2 ;;
        --work-repo-path) WORK_REPO_PATH="$2"; shift 2 ;;
        --remote-runtime-repo-path) REMOTE_RUNTIME_REPO_PATH="$2"; shift 2 ;;
        --work-platform) WORK_PLATFORM="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        --wait) WAIT=1; shift ;;
        --local-output-dir) LOCAL_OUTPUT_DIR="$2"; shift 2 ;;
        --keep-remote) KEEP_REMOTE=1; shift ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "$WORK_NODE" || -z "$TASK_SPEC" ]]; then
    echo "Usage: Invoke-XmachineTask.sh --work-node alias --task-spec file [--work-repo-path path] [--remote-runtime-repo-path path] [--work-platform auto|posix|windows] [--timeout-minutes N] [--wait] [--local-output-dir dir] [--keep-remote]" >&2
    exit 2
fi

for tool in git jq ssh scp; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing required tool: $tool" >&2
        exit 2
    fi
done

if [[ ! -f "$TASK_SPEC" ]]; then
    echo "Task spec not found: $TASK_SPEC" >&2
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
CONFIG_PATH="$REPO_ROOT/xmachine.config.json"

if [[ ! -f "$CONFIG_PATH" ]]; then
    echo "Missing xmachine config '$CONFIG_PATH'." >&2
    exit 1
fi

if ! jq -e --arg node "$WORK_NODE" '.nodes[$node]' "$CONFIG_PATH" >/dev/null 2>&1; then
    echo "Unknown work node alias '$WORK_NODE' in '$CONFIG_PATH'." >&2
    exit 1
fi

resolve_work_platform() {
    local requested="$1"
    local node_target="$2"
    if [[ "$requested" != "auto" ]]; then
        printf '%s\n' "$requested"
        return 0
    fi

    if ssh -o BatchMode=yes "$node_target" "cmd /c ver" >/dev/null 2>&1; then
        printf 'windows\n'
        return 0
    fi

    printf 'posix\n'
}

get_ssh_connection_info() {
    local ssh_target="$1"
    local ssh_config_output
    ssh_config_output="$(ssh -G "$ssh_target" 2>/dev/null)"
    if [[ -z "$ssh_config_output" ]]; then
        echo "Could not resolve SSH target '$ssh_target'." >&2
        exit 1
    fi

    local host user
    host="$(printf '%s\n' "$ssh_config_output" | awk 'tolower($1)=="host" { print $2; exit }')"
    user="$(printf '%s\n' "$ssh_config_output" | awk 'tolower($1)=="user" { print $2; exit }')"
    printf '%s\n%s\n' "$host" "$user"
}

new_task_id() {
    local rand
    rand="$(LC_ALL=C tr -dc 'a-z0-9' </dev/urandom | head -c 6 || true)"
    printf '%s-%s\n' "$(date -u +%Y%m%d)" "$rand"
}

get_repo_context_root() {
    local current
    current="$(pwd)"

    while true; do
        if [[ -f "$current/.dev/state.md" ]]; then
            printf '%s\n' "$current"
            return 0
        fi

        local parent
        parent="$(dirname "$current")"
        if [[ -z "$parent" || "$parent" == "$current" ]]; then
            pwd
            return 0
        fi

        current="$parent"
    done
}

wait_for_posix_task_completion() {
    local ssh_target="$1"
    local remote_output_dir="$2"
    local task_id="$3"
    local timeout_minutes="$4"
    local remote_status_path="$remote_output_dir/status.json"
    local deadline=$(( $(date +%s) + timeout_minutes * 60 ))

    echo "Waiting for task $task_id to complete (timeout: ${timeout_minutes}m)..."
    while (( $(date +%s) < deadline )); do
        local raw_status=""
        raw_status="$(ssh -o BatchMode=yes "$ssh_target" "cat '$remote_status_path' 2>/dev/null" 2>/dev/null || true)"
        if [[ -n "$raw_status" ]]; then
            local status
            status="$(printf '%s' "$raw_status" | jq -r '.status // empty' 2>/dev/null || true)"
            if [[ -n "$status" && "$status" != "running" ]]; then
                echo "Task $task_id — status: $status"
                return 0
            fi
        fi

        echo "  Still running... (checking again in 30s)"
        sleep 30
    done

    echo "Timed out waiting for task $task_id. Retrieving partial results." >&2
}

receive_posix_task_artifacts() {
    local ssh_target="$1"
    local task_id="$2"
    local remote_output_dir="$3"
    local remote_project_repo_path="$4"
    local remote_runtime_repo_path="$5"
    local local_output_dir="$6"
    local remote_runtime_stage_path="$7"
    local keep_remote="$8"

    mkdir -p "$local_output_dir"

    echo "Retrieving results for task $task_id..."
    echo "  From: ${ssh_target}:$remote_output_dir"
    echo "  To:   $local_output_dir"

    local output_file remote_path local_path
    for output_file in status.json summary.md runtime.log result.patch; do
        remote_path="$remote_output_dir/$output_file"
        local_path="$local_output_dir/$output_file"
        scp -o BatchMode=yes -q "${ssh_target}:$remote_path" "$local_path" 2>/dev/null || true
    done

    local status_path summary_path patch_path
    status_path="$local_output_dir/status.json"
    summary_path="$local_output_dir/summary.md"
    patch_path="$local_output_dir/result.patch"

    if [[ -f "$status_path" ]]; then
        echo
        echo "─── Task Status ───────────────────────────────────────────"
        jq -r '
            "  Task ID:    " + (.taskId // "?") + "\n" +
            "  Status:     " + (.status // "?") + "\n" +
            "  Exit Code:  " + ((.exitCode // 0) | tostring) + "\n" +
            "  Started:    " + (.startedAt // "?") + "\n" +
            "  Finished:   " + (.finishedAt // "n/a") + "\n" +
            (if .errorMessage then "  Error:      " + .errorMessage + "\n" else "" end)
        ' "$status_path"
        echo "───────────────────────────────────────────────────────────"
    fi

    if [[ -f "$summary_path" ]]; then
        echo
        echo "─── Summary ───────────────────────────────────────────────"
        cat "$summary_path"
        echo "───────────────────────────────────────────────────────────"
    fi

    if [[ -f "$patch_path" ]]; then
        local patch_size
        patch_size="$(wc -c < "$patch_path" | tr -d ' ')"
        echo
        if [[ "$patch_size" -gt 0 ]]; then
            echo "result.patch: $patch_size bytes — review and apply with: git apply '$patch_path'"
        else
            echo "result.patch: empty (read-only task, no file changes)"
        fi
    fi

    if [[ "$keep_remote" -eq 0 ]]; then
        local cleanup_script
        if [[ -n "$remote_runtime_stage_path" ]]; then
            cleanup_script="bash '$remote_runtime_stage_path/scripts/Get-XmachineLocalResult.sh' --task-id '$task_id' --output-dir '$remote_output_dir'"
            if [[ -n "$remote_project_repo_path" ]]; then
                cleanup_script+=" --repo-path '$remote_project_repo_path'"
            fi
            cleanup_script+=" >/dev/null; rm -rf '$remote_runtime_stage_path'"
        else
            cleanup_script="cd '$remote_runtime_repo_path' && bash scripts/Get-XmachineLocalResult.sh --task-id '$task_id' --output-dir '$remote_output_dir'"
            if [[ -n "$remote_project_repo_path" ]]; then
                cleanup_script+=" --repo-path '$remote_project_repo_path'"
            fi
            cleanup_script+=" >/dev/null"
        fi
        ssh -o BatchMode=yes "$ssh_target" "$cleanup_script" 2>/dev/null || true
    fi

    echo
    echo "Results saved to: $local_output_dir"
}

wait_for_windows_task_completion() {
    local ssh_target="$1"
    local remote_output_dir="$2"
    local task_id="$3"
    local timeout_minutes="$4"
    local remote_status_path="$remote_output_dir/status.json"
    local deadline=$(( $(date +%s) + timeout_minutes * 60 ))

    echo "Waiting for task $task_id to complete (timeout: ${timeout_minutes}m)..."
    while (( $(date +%s) < deadline )); do
        local raw_status=""
        raw_status="$(ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"Get-Content -Path '$remote_status_path' -Raw -ErrorAction SilentlyContinue\"" 2>/dev/null || true)"
        if [[ -n "$raw_status" ]]; then
            local status
            status="$(printf '%s' "$raw_status" | jq -r '.status // empty' 2>/dev/null || true)"
            if [[ -n "$status" && "$status" != "running" ]]; then
                echo "Task $task_id — status: $status"
                return 0
            fi
        fi

        echo "  Still running... (checking again in 30s)"
        sleep 30
    done

    echo "Timed out waiting for task $task_id. Retrieving partial results." >&2
}

receive_windows_task_artifacts() {
    local ssh_target="$1"
    local task_id="$2"
    local remote_output_dir="$3"
    local remote_repo_path="$4"
    local local_output_dir="$5"
    local keep_remote="$6"

    mkdir -p "$local_output_dir"

    echo "Retrieving results for task $task_id..."
    echo "  From: ${ssh_target}:$remote_output_dir"
    echo "  To:   $local_output_dir"

    local retrieved=()
    local missing=()
    local output_file remote_path local_path
    for output_file in status.json summary.md runtime.log result.patch; do
        remote_path="$remote_output_dir/$output_file"
        local_path="$local_output_dir/$output_file"
        if scp -o BatchMode=yes -q "${ssh_target}:$remote_path" "$local_path" 2>/dev/null; then
            retrieved+=("$output_file")
        else
            missing+=("$output_file")
        fi
    done

    echo
    echo "Retrieved: ${retrieved[*]:-none}"
    if (( ${#missing[@]} > 0 )); then
        echo "Not found on remote: ${missing[*]}" >&2
    fi

    local status_path summary_path patch_path
    status_path="$local_output_dir/status.json"
    summary_path="$local_output_dir/summary.md"
    patch_path="$local_output_dir/result.patch"

    if [[ -f "$status_path" ]]; then
        echo
        echo "─── Task Status ───────────────────────────────────────────"
        jq -r '
            "  Task ID:    " + (.taskId // "?") + "\n" +
            "  Status:     " + (.status // "?") + "\n" +
            "  Exit Code:  " + ((.exitCode // 0) | tostring) + "\n" +
            "  Started:    " + (.startedAt // "?") + "\n" +
            "  Finished:   " + (.finishedAt // "n/a") + "\n" +
            (if .errorMessage then "  Error:      " + .errorMessage + "\n" else "" end)
        ' "$status_path"
        echo "───────────────────────────────────────────────────────────"
    fi

    if [[ -f "$summary_path" ]]; then
        echo
        echo "─── Summary ───────────────────────────────────────────────"
        cat "$summary_path"
        echo "───────────────────────────────────────────────────────────"
    fi

    if [[ -f "$patch_path" ]]; then
        local patch_size
        patch_size="$(wc -c < "$patch_path" | tr -d ' ')"
        echo
        if [[ "$patch_size" -gt 0 ]]; then
            echo "result.patch: $patch_size bytes — review and apply with: git apply '$patch_path'"
        else
            echo "result.patch: empty (read-only task, no file changes)"
        fi
    fi

    if [[ "$keep_remote" -eq 0 ]]; then
        local wt_path execution_mode cleanup_cmd
        wt_path="$(jq -r '.worktreePath // empty' "$status_path" 2>/dev/null || true)"
        execution_mode="$(jq -r '.executionMode // empty' "$status_path" 2>/dev/null || true)"
        if [[ "$execution_mode" == "repo" && -n "$wt_path" ]]; then
            cleanup_cmd="\$wt = '$wt_path'; if (Test-Path \$wt) { git worktree unlock \"\$wt\" 2>\$null; git worktree remove --force \"\$wt\" 2>&1; Write-Host \"  Worktree removed: \$wt\" } else { Write-Host \"  Worktree already gone.\" }; Remove-Item -Recurse -Force '$remote_output_dir' -ErrorAction SilentlyContinue; Write-Host '  Output dir removed.'"
        elif [[ "$execution_mode" == "repo" && -n "$remote_repo_path" ]]; then
            cleanup_cmd="Set-Location '$remote_repo_path'; \$wt = (git worktree list --porcelain | Select-String '$task_id' | Select-Object -First 1)?.Line?.Split(' ')[1]; if (\$wt) { git worktree unlock \"\$wt\" 2>\$null; git worktree remove --force \"\$wt\" 2>&1; Write-Host \"  Worktree removed: \$wt\" } else { Write-Host \"  Worktree not found (may already be removed)\" }; Remove-Item -Recurse -Force '$remote_output_dir' -ErrorAction SilentlyContinue; Write-Host '  Output dir removed.'"
        else
            cleanup_cmd="Remove-Item -Recurse -Force '$remote_output_dir' -ErrorAction SilentlyContinue; Write-Host '  Output dir removed.'"
        fi
        ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"$cleanup_cmd\"" >/dev/null 2>&1 || true
    fi

    echo
    echo "Results saved to: $local_output_dir"
}

resolved_work_node_target="$(jq -r --arg node "$WORK_NODE" '.nodes[$node].target // empty' "$CONFIG_PATH")"
if [[ -z "$resolved_work_node_target" ]]; then
    echo "Work node '$WORK_NODE' in '$CONFIG_PATH' must define a non-empty target value." >&2
    exit 1
fi

repo_context_root="$(get_repo_context_root)"
repo_mapping_key="$(basename "$repo_context_root")"
configured_repo_path="$(jq -r --arg node "$WORK_NODE" '.nodes[$node].repoPath // empty' "$CONFIG_PATH")"
mapped_project_repo_path="$(jq -r --arg node "$WORK_NODE" --arg repo "$repo_mapping_key" '.nodes[$node].repoMappings[$repo].repoPath // empty' "$CONFIG_PATH")"
mapped_runtime_repo_path="$(jq -r --arg node "$WORK_NODE" --arg repo "$repo_mapping_key" '.nodes[$node].repoMappings[$repo].runtimeRepoPath // empty' "$CONFIG_PATH")"
configured_runtime_repo_path="$(jq -r --arg node "$WORK_NODE" '.nodes[$node].runtimeRepoPath // empty' "$CONFIG_PATH")"

if [[ -n "$WORK_REPO_PATH" ]]; then
    resolved_project_repo_path="$WORK_REPO_PATH"
elif [[ -n "$mapped_project_repo_path" ]]; then
    resolved_project_repo_path="$mapped_project_repo_path"
else
    resolved_project_repo_path=""
fi

if [[ -n "$resolved_project_repo_path" ]]; then
    execution_mode="repo"
else
    execution_mode="execute"
fi

if [[ -n "$REMOTE_RUNTIME_REPO_PATH" ]]; then
    resolved_remote_runtime_repo_path="$REMOTE_RUNTIME_REPO_PATH"
elif [[ -n "$mapped_runtime_repo_path" ]]; then
    resolved_remote_runtime_repo_path="$mapped_runtime_repo_path"
elif [[ -n "$configured_runtime_repo_path" ]]; then
    resolved_remote_runtime_repo_path="$configured_runtime_repo_path"
elif [[ -n "$configured_repo_path" ]]; then
    resolved_remote_runtime_repo_path="$configured_repo_path"
else
    resolved_remote_runtime_repo_path=""
fi

if [[ -z "$resolved_remote_runtime_repo_path" ]]; then
    echo "No GAL runtime path is configured for work node '$WORK_NODE'. Pass --remote-runtime-repo-path or define runtimeRepoPath or repoPath in '$CONFIG_PATH'." >&2
    exit 1
fi

resolved_platform="$(resolve_work_platform "$WORK_PLATFORM" "$resolved_work_node_target")"
task_id="$(new_task_id)"

if [[ "$resolved_platform" == "windows" ]]; then
    mapfile -t ssh_connection < <(get_ssh_connection_info "$resolved_work_node_target")
    remote_host="${ssh_connection[0]:-}"
    remote_user="${ssh_connection[1]:-}"
    if [[ -z "$remote_user" ]]; then
        echo "SSH config for work node '$WORK_NODE' does not expose a User value." >&2
        exit 1
    fi

    remote_args=(--remote-host "$remote_host" --remote-user "$remote_user" --remote-scripts-path "${resolved_remote_runtime_repo_path}\\scripts" --task-spec "$TASK_SPEC" --timeout-minutes "$TIMEOUT_MINUTES")
    if [[ "$execution_mode" == "repo" ]]; then
        remote_args+=(--remote-repo-path "$resolved_project_repo_path")
    fi
    dispatch_output="$(bash "$SCRIPT_DIR/Invoke-XmachineRemoteTask.sh" "${remote_args[@]}" 2>&1)"
    printf '%s\n' "$dispatch_output"

    task_id="$(printf '%s\n' "$dispatch_output" | sed -nE 's/^[[:space:]]*Task ID:[[:space:]]+(.+)$/\1/p' | head -n1 | sed -E 's/[[:space:]]+$//')"
    remote_output_dir="$(printf '%s\n' "$dispatch_output" | sed -nE 's/^[[:space:]]*Remote output:[[:space:]]+(.+)$/\1/p' | head -n1 | sed -E 's/[[:space:]]+$//')"
    if [[ -z "$task_id" || -z "$remote_output_dir" ]]; then
        echo "Could not parse task metadata from Invoke-XmachineRemoteTask.sh output." >&2
        exit 1
    fi

    if [[ "$WAIT" -eq 1 ]]; then
        resolved_local_output_dir="${LOCAL_OUTPUT_DIR:-$(pwd)/gal-results/$task_id}"
        wait_for_windows_task_completion "${remote_user}@${remote_host}" "$remote_output_dir" "$task_id" "$(( TIMEOUT_MINUTES > 60 ? TIMEOUT_MINUTES : 60 ))"
        receive_windows_task_artifacts "${remote_user}@${remote_host}" "$task_id" "$remote_output_dir" "$resolved_project_repo_path" "$resolved_local_output_dir" "$KEEP_REMOTE"
    fi

    exit 0
fi

task_spec_name="$(basename "$TASK_SPEC")"
remote_task_spec="/tmp/gal-xmachine-task-$task_id-$task_spec_name"
remote_runtime_stage_path=""
dispatch_runtime_path="$resolved_remote_runtime_repo_path"
if [[ "$execution_mode" == "execute" ]]; then
    remote_runtime_stage_path="/tmp/gal-xmachine-runtime-$task_id"
    dispatch_runtime_path="$remote_runtime_stage_path"
fi

echo "GAL Remote Task: $task_id"
echo "  Machine:  $resolved_work_node_target"
if [[ "$execution_mode" == "repo" ]]; then
    echo "  Project:  $resolved_project_repo_path"
else
    echo "  Project:  (execute mode; temp workspace only)"
fi
echo "  Runtime:  $resolved_remote_runtime_repo_path"
echo "  TaskSpec: $task_spec_name"
echo "  Timeout:  ${TIMEOUT_MINUTES}m"
echo

echo "[1/3] Copying task spec to remote..."
if ! scp -o BatchMode=yes -q "$TASK_SPEC" "${resolved_work_node_target}:$remote_task_spec"; then
    echo "Failed to copy task spec to POSIX work node '$resolved_work_node_target'." >&2
    exit 1
fi

if [[ "$execution_mode" == "execute" ]]; then
    echo "[1/3] Staging execute-mode xmachine runtime scripts..."
    remote_runtime_scripts_path="$remote_runtime_stage_path/scripts"
    if ! ssh -o BatchMode=yes "$resolved_work_node_target" "mkdir -p '$remote_runtime_scripts_path'"; then
        echo "Failed to create staged xmachine runtime directory '$remote_runtime_scripts_path'." >&2
        exit 1
    fi
    for script_name in Invoke-XmachineLocalTask.sh Start-xMachine.sh Get-XmachineLocalResult.sh; do
        if ! scp -o BatchMode=yes -q "$SCRIPT_DIR/$script_name" "${resolved_work_node_target}:$remote_runtime_scripts_path/$script_name"; then
            echo "Failed to stage xmachine runtime script '$script_name' on '$resolved_work_node_target'." >&2
            exit 1
        fi
    done
    if ! ssh -o BatchMode=yes "$resolved_work_node_target" "for f in '$remote_runtime_scripts_path'/*.sh; do tmp=\"\$f.tmp\"; tr -d '\r' < \"\$f\" > \"\$tmp\" && mv \"\$tmp\" \"\$f\" && chmod +x \"\$f\"; done"; then
        echo "Failed to normalize staged xmachine runtime scripts in '$remote_runtime_scripts_path'." >&2
        exit 1
    fi
fi

echo "[2/3] Dispatching task on POSIX work node..."
dispatch_script="cd '$dispatch_runtime_path' && bash scripts/Invoke-XmachineLocalTask.sh --task-spec '$remote_task_spec' --timeout-minutes $TIMEOUT_MINUTES --task-id '$task_id'"
if [[ "$execution_mode" == "repo" ]]; then
    dispatch_script+=" --repo-path '$resolved_project_repo_path'"
fi
dispatch_output="$(ssh -o BatchMode=yes "$resolved_work_node_target" "export PATH=/opt/homebrew/bin:/usr/local/bin:\$HOME/.local/bin:\$PATH; source ~/.zprofile >/dev/null 2>&1 || true; source ~/.zshrc >/dev/null 2>&1 || true; $dispatch_script" 2>&1)"
dispatch_exit=$?
if [[ $dispatch_exit -ne 0 ]]; then
    failed_dispatch_cleanup="rm -f '$remote_task_spec'"
    if [[ -n "$remote_runtime_stage_path" ]]; then
        failed_dispatch_cleanup+="; rm -rf '$remote_runtime_stage_path'"
    fi
    ssh -o BatchMode=yes "$resolved_work_node_target" "$failed_dispatch_cleanup" >/dev/null 2>&1 || true
    echo "Failed to dispatch task on POSIX work node '$resolved_work_node_target'." >&2
    printf '%s\n' "$dispatch_output" >&2
    exit 1
fi

printf '%s\n' "$dispatch_output"
remote_output_dir="$(printf '%s\n' "$dispatch_output" | sed -nE 's/^[[:space:]]*Output:[[:space:]]+(.+)$/\1/p' | head -n1 | sed -E 's/[[:space:]]+$//')"
if [[ -z "$remote_output_dir" ]]; then
    echo "Could not parse the POSIX output directory from dispatch output." >&2
    exit 1
fi

echo "[3/3] Cleaning up staged remote task spec..."
ssh -o BatchMode=yes "$resolved_work_node_target" "rm -f '$remote_task_spec'" >/dev/null 2>&1 || true

echo
echo "Task dispatched successfully."
echo
echo "  Task ID:        $task_id"
echo "  Remote output:  $remote_output_dir"
echo

if [[ "$WAIT" -eq 0 ]]; then
    echo "Retrieve results when done:"
    echo "  scripts/Invoke-XmachineTask.sh --work-node $WORK_NODE --task-spec '$TASK_SPEC' --wait"
    exit 0
fi

resolved_local_output_dir="${LOCAL_OUTPUT_DIR:-$(pwd)/gal-results/$task_id}"
wait_for_posix_task_completion "$resolved_work_node_target" "$remote_output_dir" "$task_id" "$(( TIMEOUT_MINUTES > 60 ? TIMEOUT_MINUTES : 60 ))"
receive_posix_task_artifacts "$resolved_work_node_target" "$task_id" "$remote_output_dir" "$resolved_project_repo_path" "$resolved_remote_runtime_repo_path" "$resolved_local_output_dir" "$remote_runtime_stage_path" "$KEEP_REMOTE"