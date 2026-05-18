#!/usr/bin/env bash

set -euo pipefail

WORK_NODE=""
PLAN_PATH=""
WORK_REPO_PATH=""
REMOTE_RUNTIME_REPO_PATH=""
TIMEOUT_MINUTES=30
KEEP_REMOTE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --work-node) WORK_NODE="$2"; shift 2 ;;
        --plan-path) PLAN_PATH="$2"; shift 2 ;;
        --work-repo-path) WORK_REPO_PATH="$2"; shift 2 ;;
        --remote-runtime-repo-path) REMOTE_RUNTIME_REPO_PATH="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        --keep-remote) KEEP_REMOTE=1; shift ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "$WORK_NODE" ]]; then
    echo "Usage: Invoke-XmachinePipeline.sh --work-node <alias> [--plan-path file] [--work-repo-path path] [--remote-runtime-repo-path path] [--timeout-minutes N] [--keep-remote]" >&2
    exit 2
fi

for tool in git jq ssh scp perl; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing required tool: $tool" >&2
        exit 2
    fi
done

get_repo_root() {
    local script_dir
    script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    cd "$script_dir/.." && pwd
}

get_repo_context_root() {
    local current parent
    current="$(pwd)"

    while true; do
        if [[ -f "$current/.dev/state.md" ]]; then
            printf '%s\n' "$current"
            return 0
        fi

        parent="$(dirname "$current")"
        if [[ -z "$parent" || "$parent" == "$current" ]]; then
            echo "Could not locate a target project root containing .dev/state.md from '$(pwd)'" >&2
            exit 1
        fi

        current="$parent"
    done
}

unwrap_markdown_code() {
    local value="$1"
    value="${value#\`}"
    value="${value%\`}"
    printf '%s' "$value"
}

resolve_plan_path() {
    local candidate absolute prompt repo_context_root
    candidate="$(unwrap_markdown_code "$1")"
    [[ -n "$candidate" ]] || return 1
    candidate="${candidate//\\//}"
    repo_context_root="$(get_repo_context_root)"

    if [[ "$candidate" = /* || "$candidate" =~ ^[A-Za-z]:/ ]]; then
        absolute="$candidate"
    else
        absolute="$repo_context_root/$candidate"
    fi

    if [[ "$absolute" == *.prompt.md ]]; then
        printf '%s\n' "$absolute"
        return 0
    fi

    if [[ "$absolute" == *.md ]]; then
        local source_plan_root relative_source_plan_path relative_prompt_path dev_prompt
        source_plan_root="$repo_context_root/docs/plans"
        if [[ "$absolute" == "$source_plan_root"/* ]]; then
            relative_source_plan_path="${absolute#"$source_plan_root"/}"
            relative_prompt_path="${relative_source_plan_path%.md}.prompt.md"
            dev_prompt="$repo_context_root/.dev/plans/$relative_prompt_path"
            if [[ -f "$dev_prompt" ]]; then
                printf '%s\n' "$dev_prompt"
                return 0
            fi
        fi

        prompt="${absolute%.md}.prompt.md"
        if [[ -f "$prompt" ]]; then
            printf '%s\n' "$prompt"
            return 0
        fi
    fi

    printf '%s\n' "$absolute"
}

get_active_plan_path() {
    local state_path repo_context_root in_active_plans header_seen line resolved
    repo_context_root="$(get_repo_context_root)"
    state_path="$repo_context_root/.dev/state.md"
    [[ -f "$state_path" ]] || return 0

    in_active_plans=0
    header_seen=0
    while IFS= read -r line; do
        if [[ "$line" =~ ^##[[:space:]]+Active[[:space:]]+Plans ]]; then
            in_active_plans=1
            continue
        fi

        if (( in_active_plans )) && [[ "$line" =~ ^##[[:space:]]+ ]]; then
            break
        fi

        (( in_active_plans )) || continue
        [[ "$line" == \|* ]] || continue
        [[ "$line" =~ ^\|[[:space:]]*--- ]] && continue

        IFS='|' read -r -a raw_cells <<<"$line"
        local cells=()
        local cell
        for cell in "${raw_cells[@]}"; do
            cell="$(printf '%s' "$cell" | sed -E 's/^[[:space:]]+//; s/[[:space:]]+$//')"
            [[ -n "$cell" ]] && cells+=("$cell")
        done

        (( ${#cells[@]} >= 2 )) || continue

        if (( ! header_seen )); then
            if printf '%s\n' "${cells[@]}" | grep -Fxq 'File'; then
                header_seen=1
            fi
            continue
        fi

        for cell in "${cells[@]}"; do
            resolved="$(resolve_plan_path "$cell")" || continue
            if [[ "$resolved" == *.prompt.md || "$resolved" == *.md ]]; then
                printf '%s\n' "$resolved"
                return 0
            fi
        done
    done < "$state_path"
}

get_repo_mapping_key() {
    basename "$(get_repo_context_root)"
}

get_mapped_project_repo_path() {
    local config_path="$1" work_node="$2" repo_key="$3"
    jq -r --arg node "$work_node" --arg repo "$repo_key" '.nodes[$node].repoMappings[$repo].repoPath // .nodes[$node].repoMappings[$repo] // empty' "$config_path"
}

get_mapped_runtime_repo_path() {
    local config_path="$1" work_node="$2" repo_key="$3"
    jq -r --arg node "$work_node" --arg repo "$repo_key" '.nodes[$node].repoMappings[$repo].runtimeRepoPath // empty' "$config_path"
}

new_run_id() {
    local rand
    rand="$(LC_ALL=C tr -dc 'a-z0-9' </dev/urandom | head -c 6 || true)"
    printf '%s-%s\n' "$(date -u +%Y%m%d)" "$rand"
}

get_plan_hash() {
    local path="$1"
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$path" | awk '{print tolower($1)}'
        return 0
    fi

    shasum -a 256 "$path" | awk '{print tolower($1)}'
}

update_state_execution_context() {
    local state_path="$1"
    local dispatched_node="$2"
    local execution_mode="$3"
    local notes="$4"

    DISPATCHED_NODE="$dispatched_node" EXECUTION_MODE="$execution_mode" NOTES_TEXT="$notes" \
        perl -0pi -e 's/^Dispatched node:.*$/Dispatched node: $ENV{DISPATCHED_NODE}/m; s/^Execution mode:.*$/Execution mode: $ENV{EXECUTION_MODE}/m; s/^Notes:.*$/Notes: $ENV{NOTES_TEXT}/m' "$state_path"
}

detect_work_platform() {
    local target="$1"
    if ssh -o BatchMode=yes "$target" "cmd /c ver" >/dev/null 2>&1; then
        printf 'windows\n'
        return 0
    fi

    printf 'posix\n'
}

repo_root="$(get_repo_root)"
repo_context_root="$(get_repo_context_root)"
state_path="$repo_context_root/.dev/state.md"
if [[ -n "$PLAN_PATH" ]]; then
    resolved_plan_path="$(resolve_plan_path "$PLAN_PATH")"
else
    resolved_plan_path="$(get_active_plan_path)"
fi

if [[ -z "$resolved_plan_path" || ! -f "$resolved_plan_path" ]]; then
    echo "Could not resolve the active plan file for xmachine pipeline dispatch. Pass --plan-path explicitly or update .dev/state.md Active Plans." >&2
    exit 1
fi

config_path="$repo_root/xmachine.config.json"
if [[ ! -f "$config_path" ]]; then
    echo "Missing xmachine config '$config_path'." >&2
    exit 1
fi

if ! jq -e --arg node "$WORK_NODE" '.nodes[$node]' "$config_path" >/dev/null 2>&1; then
    echo "Unknown work node alias '$WORK_NODE' in '$config_path'." >&2
    exit 1
fi

resolved_work_node_target="$(jq -r --arg node "$WORK_NODE" '.nodes[$node].target // empty' "$config_path")"
if [[ -z "$resolved_work_node_target" ]]; then
    echo "Work node '$WORK_NODE' in '$config_path' must define a non-empty target value." >&2
    exit 1
fi

repo_mapping_key="$(basename "$repo_context_root")"
mapped_project_repo_path="$(get_mapped_project_repo_path "$config_path" "$WORK_NODE" "$repo_mapping_key")"
mapped_runtime_repo_path="$(get_mapped_runtime_repo_path "$config_path" "$WORK_NODE" "$repo_mapping_key")"
configured_repo_path="$(jq -r --arg node "$WORK_NODE" '.nodes[$node].repoPath // empty' "$config_path")"
resolved_project_repo_path="${WORK_REPO_PATH:-${mapped_project_repo_path:-$configured_repo_path}}"
if [[ -z "$resolved_project_repo_path" ]]; then
    echo "No repo path is configured for work node '$WORK_NODE'. Pass --work-repo-path or define repoMappings.$repo_mapping_key.repoPath or repoPath in '$config_path'." >&2
    exit 1
fi

configured_runtime_repo_path="$(jq -r --arg node "$WORK_NODE" '.nodes[$node].runtimeRepoPath // empty' "$config_path")"
resolved_remote_runtime_repo_path="${REMOTE_RUNTIME_REPO_PATH:-${mapped_runtime_repo_path:-$configured_runtime_repo_path}}"
if [[ -z "$resolved_remote_runtime_repo_path" ]]; then
    resolved_remote_runtime_repo_path="$resolved_project_repo_path"
fi
resolved_platform="$(detect_work_platform "$resolved_work_node_target")"
run_id="$(new_run_id)"
branch_name="$(git -C "$repo_context_root" rev-parse --abbrev-ref HEAD)"
base_commit="$(git -C "$repo_context_root" rev-parse HEAD)"
plan_hash="$(get_plan_hash "$resolved_plan_path")"

run_records_dir="$repo_context_root/.dev/xmachine-runs"
local_staging_dir="$run_records_dir/$run_id"
local_run_record_path="$run_records_dir/$run_id.json"
local_plan_snapshot_path="$local_staging_dir/plan.prompt.md"
local_state_snapshot_path="$local_staging_dir/state.md"

mkdir -p "$local_staging_dir"
cp "$resolved_plan_path" "$local_plan_snapshot_path"
cp "$state_path" "$local_state_snapshot_path"

if [[ "$resolved_platform" == "windows" ]]; then
    remote_output_dir="C:\Windows\Temp\gal-xmachine-pipeline\$run_id"
    remote_plan_snapshot_path="${remote_output_dir}\plan.prompt.md"
    remote_state_snapshot_path="${remote_output_dir}\state.md"
    remote_pipeline_status_path="${remote_output_dir}\pipeline-status.json"
    remote_pipeline_events_path="${remote_output_dir}\pipeline-events.jsonl"
    remote_worktree_path="${resolved_project_repo_path}-xpipeline-$run_id"
    remote_runner_path="${resolved_remote_runtime_repo_path}\scripts\Start-XmachinePipeline.ps1"
    remote_session_name="pipeline-$run_id"
    launcher="start-process"
else
    remote_output_dir="/tmp/gal-xmachine-pipeline/$run_id"
    remote_plan_snapshot_path="$remote_output_dir/plan.prompt.md"
    remote_state_snapshot_path="$remote_output_dir/state.md"
    remote_pipeline_status_path="$remote_output_dir/pipeline-status.json"
    remote_pipeline_events_path="$remote_output_dir/pipeline-events.jsonl"
    remote_worktree_path="${resolved_project_repo_path}-xpipeline-$run_id"
    remote_runner_path="$resolved_remote_runtime_repo_path/scripts/Start-XmachinePipeline.sh"
    remote_session_name="pipeline-$run_id"
    launcher="zellij"
fi

jq -n \
    --arg runId "$run_id" \
    --arg status "dispatched" \
    --arg workNode "$WORK_NODE" \
    --arg target "$resolved_work_node_target" \
    --arg platform "$resolved_platform" \
    --arg targetProjectRoot "$repo_context_root" \
    --arg localPlanPath "$resolved_plan_path" \
    --arg planSnapshotHash "$plan_hash" \
    --arg branch "$branch_name" \
    --arg baseCommit "$base_commit" \
    --arg remoteProjectRepoPath "$resolved_project_repo_path" \
    --arg remoteRuntimeRepoPath "$resolved_remote_runtime_repo_path" \
    --arg remoteOutputDir "$remote_output_dir" \
    --arg remoteWorktreePath "$remote_worktree_path" \
    --arg remotePipelineStatusPath "$remote_pipeline_status_path" \
    --arg remotePipelineEventsPath "$remote_pipeline_events_path" \
    --arg remoteSessionName "$remote_session_name" \
    --arg launcher "$launcher" \
    --arg dispatchTimeUtc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg convergeCommand "/gal status" \
    '{runId:$runId,status:$status,workNode:$workNode,target:$target,platform:$platform,targetProjectRoot:$targetProjectRoot,localPlanPath:$localPlanPath,planSnapshotHash:$planSnapshotHash,branch:$branch,baseCommit:$baseCommit,remoteProjectRepoPath:$remoteProjectRepoPath,remoteRuntimeRepoPath:$remoteRuntimeRepoPath,remoteOutputDir:$remoteOutputDir,remoteWorktreePath:$remoteWorktreePath,remotePipelineStatusPath:$remotePipelineStatusPath,remotePipelineEventsPath:$remotePipelineEventsPath,remoteSessionName:$remoteSessionName,launcher:$launcher,dispatchTimeUtc:$dispatchTimeUtc,convergeCommand:$convergeCommand}' > "$local_run_record_path"

update_state_execution_context "$state_path" "$WORK_NODE" "xmachine-persistent" "Active run: $run_id; plan: $(basename "$resolved_plan_path"); status: dispatched"

echo "GAL Remote Pipeline Run: $run_id"
echo "  Machine:  $resolved_work_node_target"
echo "  Project:  $resolved_project_repo_path"
echo "  Runtime:  $resolved_remote_runtime_repo_path"
echo "  Plan:     $resolved_plan_path"
echo "  Platform: $resolved_platform"
echo "  Branch:   $branch_name"
echo "  Base:     $base_commit"
echo

if [[ "$resolved_platform" == "windows" ]]; then
    echo "[1/5] Creating remote output directory..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "pwsh -NoProfile -Command \"New-Item -ItemType Directory -Force -Path '$remote_output_dir' | Out-Null\""

    echo "[2/5] Copying frozen plan snapshot..."
    scp -o BatchMode=yes -q "$local_plan_snapshot_path" "${resolved_work_node_target}:$remote_plan_snapshot_path"
    scp -o BatchMode=yes -q "$local_state_snapshot_path" "${resolved_work_node_target}:$remote_state_snapshot_path"

    echo "[3/5] Creating remote disposable worktree..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "pwsh -NoProfile -Command \"git -C '$resolved_project_repo_path' -c filter.gal-config.smudge=cat -c filter.gal-config.clean=cat worktree add --detach '$remote_worktree_path' HEAD\""

    echo "[4/5] Verifying remote runner..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "pwsh -NoProfile -Command \"if (-not (Test-Path '$remote_runner_path')) { exit 41 }\""

    echo "[5/5] Starting detached pipeline runner..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "pwsh -NoProfile -Command \"Start-Process pwsh -ArgumentList '-NoProfile','-File','$remote_runner_path','-RunId','$run_id','-WorktreePath','$remote_worktree_path','-PlanSnapshot','$remote_plan_snapshot_path','-StateSnapshot','$remote_state_snapshot_path','-OutputDir','$remote_output_dir','-TimeoutMinutes','$TIMEOUT_MINUTES' -WindowStyle Hidden\""
else
    echo "[1/5] Creating remote output directory..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "mkdir -p '$remote_output_dir'"

    echo "[2/5] Copying frozen plan snapshot..."
    scp -o BatchMode=yes -q "$local_plan_snapshot_path" "${resolved_work_node_target}:$remote_plan_snapshot_path"
    scp -o BatchMode=yes -q "$local_state_snapshot_path" "${resolved_work_node_target}:$remote_state_snapshot_path"

    echo "[3/5] Creating remote disposable worktree..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "git -C '$resolved_project_repo_path' -c filter.gal-config.smudge=cat -c filter.gal-config.clean=cat worktree add --detach '$remote_worktree_path' HEAD"

    echo "[4/5] Verifying remote runner..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "test -f '$remote_runner_path'"

    echo "[5/5] Starting detached pipeline runner..."
    ssh -o BatchMode=yes "$resolved_work_node_target" "set -euo pipefail; mkdir -p '$remote_output_dir'; script -q /dev/null zellij attach --create-background '$remote_session_name' </dev/null >/dev/null 2>&1 & for _ in 1 2 3 4 5 6 7 8 9 10; do if zellij list-sessions 2>/dev/null | grep -q -F '$remote_session_name'; then break; fi; sleep 0.3; done; if ! zellij list-sessions 2>/dev/null | grep -q -F '$remote_session_name'; then exit 41; fi; zellij --session '$remote_session_name' run --close-on-exit -- bash '$remote_runner_path' --run-id '$run_id' --worktree '$remote_worktree_path' --plan-snapshot '$remote_plan_snapshot_path' --state-snapshot '$remote_state_snapshot_path' --output-dir '$remote_output_dir' --timeout-minutes '$TIMEOUT_MINUTES' >/dev/null 2>&1"
fi

jq '.status = "running" | .remoteSessionStartedUtc = "'"$(date -u +%Y-%m-%dT%H:%M:%SZ)"'"' "$local_run_record_path" > "$local_run_record_path.tmp"
mv "$local_run_record_path.tmp" "$local_run_record_path"
update_state_execution_context "$state_path" "$WORK_NODE" "xmachine-persistent" "Active run: $run_id; session: $remote_session_name; status: running"

if (( ! KEEP_REMOTE )); then
    echo
    echo "Remote cleanup is deferred until convergence."
fi

echo
echo "Persistent pipeline dispatched successfully."
echo
echo "  Run ID:        $run_id"
echo "  Session:       $remote_session_name"
echo "  Remote output: $remote_output_dir"
echo "  Run record:    $local_run_record_path"
echo
echo "Next-day recovery entry point:"
echo "  /gal status"
