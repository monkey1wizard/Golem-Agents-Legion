#!/usr/bin/env bash

set -euo pipefail

REMOTE_HOST=""
REMOTE_USER=""
REMOTE_REPO_PATH=""
TASK_SPEC=""
REMOTE_SCRIPTS_PATH=""
TIMEOUT_MINUTES=30

while [[ $# -gt 0 ]]; do
    case "$1" in
        --remote-host) REMOTE_HOST="$2"; shift 2 ;;
        --remote-user) REMOTE_USER="$2"; shift 2 ;;
        --remote-repo-path) REMOTE_REPO_PATH="$2"; shift 2 ;;
        --task-spec) TASK_SPEC="$2"; shift 2 ;;
        --remote-scripts-path) REMOTE_SCRIPTS_PATH="$2"; shift 2 ;;
        --timeout-minutes) TIMEOUT_MINUTES="$2"; shift 2 ;;
        *) echo "Unknown arg: $1" >&2; exit 2 ;;
    esac
done

if [[ -z "$REMOTE_HOST" || -z "$REMOTE_USER" || -z "$TASK_SPEC" ]]; then
    echo "Usage: Invoke-XmachineRemoteTask.sh --remote-host host --remote-user user [--remote-repo-path path] --task-spec file [--remote-scripts-path path] [--timeout-minutes N]" >&2
    exit 2
fi

if [[ ! -f "$TASK_SPEC" ]]; then
    echo "Task spec not found: $TASK_SPEC" >&2
    exit 1
fi

for tool in ssh scp; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "Missing required tool: $tool" >&2
        exit 2
    fi
done

task_spec_name="$(basename "$TASK_SPEC")"
rand="$(LC_ALL=C tr -dc 'a-z0-9' </dev/urandom | head -c 6 || true)"
task_id="$(date -u +%Y%m%d)-$rand"
ssh_target="${REMOTE_USER}@${REMOTE_HOST}"

remote_temp="C:\Windows\Temp\gal-xmachine\task-$task_id"
remote_spec="${remote_temp}\${task_spec_name}"
remote_worktree="${remote_temp}\workspace"

if [[ -n "$REMOTE_REPO_PATH" ]]; then
    remote_worktree="${REMOTE_REPO_PATH}-xmachine-$task_id"
fi

if [[ -z "$REMOTE_SCRIPTS_PATH" ]]; then
    if [[ -z "$REMOTE_REPO_PATH" ]]; then
        echo "Remote scripts path is required when --remote-repo-path is omitted." >&2
        exit 2
    fi
    REMOTE_SCRIPTS_PATH="${REMOTE_REPO_PATH}\scripts"
fi
remote_start_script="${REMOTE_SCRIPTS_PATH}\Start-xMachine.ps1"

echo "GAL Remote Task: $task_id"
echo "  Machine:  $ssh_target"
if [[ -n "$REMOTE_REPO_PATH" ]]; then
    echo "  Repo:     $REMOTE_REPO_PATH"
else
    echo "  Mode:     execute"
fi
echo "  TaskSpec: $task_spec_name"
echo "  Timeout:  ${TIMEOUT_MINUTES}m"
echo

echo "[1/4] Creating remote temp directory..."
if ! ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"New-Item -ItemType Directory -Force -Path '$remote_temp' | Out-Null\""; then
    echo "Failed to create remote temp directory." >&2
    exit 1
fi

echo "[2/4] Copying task spec to remote..."
if ! scp -o BatchMode=yes -q "$TASK_SPEC" "${ssh_target}:$remote_temp/"; then
    echo "Failed to copy task spec to remote via SCP." >&2
    exit 1
fi

echo "[3/4] Preparing remote workspace..."
if [[ -n "$REMOTE_REPO_PATH" ]]; then
    if ! ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"Set-Location '$REMOTE_REPO_PATH'; git worktree add --detach '$remote_worktree' HEAD\""; then
        echo "Failed to create git worktree on remote." >&2
        exit 1
    fi
elif ! ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"New-Item -ItemType Directory -Force -Path '$remote_worktree' | Out-Null\""; then
    echo "Failed to create remote execute workspace." >&2
    exit 1
fi

echo "[4/4] Starting remote task run..."
if ! ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"Start-Process pwsh -ArgumentList '-NoProfile','-File','$remote_start_script','-TaskId','$task_id','-WorktreePath','$remote_worktree','-TaskSpec','$remote_spec','-OutputDir','$remote_temp','-TimeoutMinutes','$TIMEOUT_MINUTES' -WindowStyle Hidden\""; then
    if [[ -n "$REMOTE_REPO_PATH" ]]; then
        ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"git -C '$REMOTE_REPO_PATH' worktree remove --force '$remote_worktree' 2>&1\"" >/dev/null 2>&1 || true
    else
        ssh -o BatchMode=yes "$ssh_target" "pwsh -NoProfile -Command \"Remove-Item -Recurse -Force '$remote_temp' -ErrorAction SilentlyContinue\"" >/dev/null 2>&1 || true
    fi
    echo "Failed to start the remote task process." >&2
    exit 1
fi

echo
echo "Task dispatched successfully."
echo
echo "  Task ID:        $task_id"
echo "  Remote output:  $remote_temp"
echo "  Workspace:      $remote_worktree"