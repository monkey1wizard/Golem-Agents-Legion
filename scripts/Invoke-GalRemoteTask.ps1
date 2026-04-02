<#
.SYNOPSIS
    Dispatch a task to a remote LAN worker over SSH.

.DESCRIPTION
    Copies a task spec to the remote worker, creates an isolated git worktree,
    and invokes Start-GalWorker.ps1 on the remote machine to run the task headlessly.

    The task ID is printed on completion. Use Get-GalRemoteResult.ps1 to retrieve
    results once the worker finishes.

.PARAMETER RemoteHost
    SSH hostname or IP of the worker node.

.PARAMETER RemoteUser
    SSH username on the worker node.

.PARAMETER RemoteRepoPath
    Absolute path to the repo clone on the worker (e.g. C:\Code\MyRepo).

.PARAMETER TaskSpec
    Path to the local task spec file (Markdown, following templates/task.md).

.PARAMETER RemoteScriptsPath
    Path to the GAL scripts directory on the remote worker.
    Defaults to the same relative path as this script in the remote repo.

.EXAMPLE
    .\Invoke-GalRemoteTask.ps1 `
        -RemoteHost notebook `
        -RemoteUser alice `
        -RemoteRepoPath "C:\Code\Golem-Agents-Legion" `
        -TaskSpec ".\task-research-oauth.md"
#>

param(
    [Parameter(Mandatory)]
    [string]$RemoteHost,

    [Parameter(Mandatory)]
    [string]$RemoteUser,

    [Parameter(Mandatory)]
    [string]$RemoteRepoPath,

    [Parameter(Mandatory)]
    [string]$TaskSpec,

    [string]$RemoteScriptsPath = "",

    [int]$TimeoutMinutes = 30
)

$ErrorActionPreference = "Stop"

# ── Validate local task spec ──────────────────────────────────────────────────
if (-not (Test-Path $TaskSpec)) {
    Write-Error "Task spec not found: $TaskSpec"
    exit 1
}

$taskSpecName = Split-Path -Leaf $TaskSpec

# ── Generate task ID ──────────────────────────────────────────────────────────
$datePart = (Get-Date -Format "yyyyMMdd")
$randPart = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 6 | ForEach-Object { [char]$_ })
$taskId = "$datePart-$randPart"

Write-Host "GAL Remote Task: $taskId"
Write-Host "  Worker:   $RemoteUser@$RemoteHost"
Write-Host "  Repo:     $RemoteRepoPath"
Write-Host "  TaskSpec: $taskSpecName"
Write-Host "  Timeout:  ${TimeoutMinutes}m"
Write-Host ""

# ── Derive remote paths ───────────────────────────────────────────────────────
$remoteTemp   = "C:\Windows\Temp\gal-worker\$taskId"
$remoteSpec   = "$remoteTemp\$taskSpecName"
$remoteWorktree = "$RemoteRepoPath-worker-$taskId"

if ($RemoteScriptsPath -eq "") {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    $repoRoot  = Split-Path -Parent $scriptDir
    # Assume same relative layout on remote
    $RemoteScriptsPath = "$RemoteRepoPath\scripts"
}
$remoteStartScript = "$RemoteScriptsPath\Start-GalWorker.ps1"

# ── Create remote temp directory ──────────────────────────────────────────────
Write-Host "[1/4] Creating remote temp directory..."
$mkdirCmd = "New-Item -ItemType Directory -Force -Path '$remoteTemp' | Out-Null"
ssh "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$mkdirCmd`""
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to create remote temp directory."
    exit 1
}

# ── Copy task spec to remote ──────────────────────────────────────────────────
Write-Host "[2/4] Copying task spec to remote..."
scp -q $TaskSpec "${RemoteUser}@${RemoteHost}:$remoteTemp/"
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to copy task spec to remote."
    exit 1
}

# ── Create isolated git worktree on remote ────────────────────────────────────
Write-Host "[3/4] Creating git worktree on remote..."
$worktreeCmd = "Set-Location '$RemoteRepoPath'; git worktree add --detach '$remoteWorktree' HEAD"
ssh "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$worktreeCmd`""
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to create git worktree on remote."
    exit 1
}

# ── Invoke worker script on remote (background, detached) ────────────────────
Write-Host "[4/4] Starting remote worker..."
$workerCmd = "Start-Process pwsh -ArgumentList '-NoProfile','-File','$remoteStartScript','-TaskId','$taskId','-WorktreePath','$remoteWorktree','-TaskSpec','$remoteSpec','-OutputDir','$remoteTemp','-TimeoutMinutes','$TimeoutMinutes' -WindowStyle Hidden"
ssh "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$workerCmd`""
if ($LASTEXITCODE -ne 0) {
    Write-Warning "Failed to start remote worker. Cleaning up orphaned worktree..."
    $cleanupCmd = "git -C '$RemoteRepoPath' worktree remove --force '$remoteWorktree' 2>&1"
    ssh "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$cleanupCmd`"" 2>&1 | Out-Null
    Write-Error "Failed to start remote worker process."
    exit 1
}

# ── Print retrieval instructions ──────────────────────────────────────────────
Write-Host ""
Write-Host "Task dispatched successfully."
Write-Host ""
Write-Host "  Task ID:        $taskId"
Write-Host "  Remote output:  $remoteTemp"
Write-Host "  Worktree:       $remoteWorktree"
Write-Host ""
Write-Host "Retrieve results when done:"
Write-Host "  .\Get-GalRemoteResult.ps1 -RemoteHost $RemoteHost -RemoteUser $RemoteUser -TaskId $taskId -RemoteOutputDir '$remoteTemp' -RemoteRepoPath '$RemoteRepoPath' -Wait"
