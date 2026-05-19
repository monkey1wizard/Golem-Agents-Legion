<#
.SYNOPSIS
    Dispatch a task to a remote LAN machine over SSH.

.DESCRIPTION
    Copies a task spec to the remote machine, creates an isolated git worktree,
    and invokes Start-xMachine.ps1 on the remote machine to run the task headlessly.

    The task ID is printed on completion. Use Get-XmachineRemoteResult.ps1 to retrieve
    results once the task run finishes.

.PARAMETER RemoteHost
    SSH hostname or IP of the remote machine.

.PARAMETER RemoteUser
    SSH username on the remote machine.

.PARAMETER RemoteRepoPath
    Absolute path to the repo clone on the remote machine (e.g. C:\Code\MyRepo).

.PARAMETER TaskSpec
    Path to the local task spec file (Markdown, following templates/task.md).

.PARAMETER RemoteScriptsPath
    Path to the GAL scripts directory on the remote machine.
    Defaults to the same relative path as this script in the remote repo.

.EXAMPLE
    .\Invoke-XmachineRemoteTask.ps1 `
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
Write-Host "  Machine:  $RemoteUser@$RemoteHost"
if ([string]::IsNullOrWhiteSpace($RemoteRepoPath)) {
    Write-Host "  Mode:     execute"
}
else {
    Write-Host "  Repo:     $RemoteRepoPath"
}
Write-Host "  TaskSpec: $taskSpecName"
Write-Host "  Timeout:  ${TimeoutMinutes}m"
Write-Host ""

# ── Derive remote paths ───────────────────────────────────────────────────────
$remoteTemp   = "C:\Windows\Temp\gal-xmachine\task-$taskId"
$remoteSpec   = "$remoteTemp\$taskSpecName"
$remoteWorktree = if ([string]::IsNullOrWhiteSpace($RemoteRepoPath)) { "$remoteTemp\workspace" } else { "$RemoteRepoPath-xmachine-$taskId" }

if ($RemoteScriptsPath -eq "") {
    if ([string]::IsNullOrWhiteSpace($RemoteRepoPath)) {
        throw "RemoteScriptsPath is required when RemoteRepoPath is omitted."
    }
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    # Assume same relative layout on remote
    $RemoteScriptsPath = "$RemoteRepoPath\scripts"
}
$remoteStartScript = "$RemoteScriptsPath\Start-xMachine.ps1"

# ── Create remote temp directory ──────────────────────────────────────────────
Write-Host "[1/4] Creating remote temp directory..."
$mkdirCmd = "New-Item -ItemType Directory -Force -Path '$remoteTemp' | Out-Null"
$sshArgs = "-o", "BatchMode=yes", "${RemoteUser}@${RemoteHost}", "pwsh -NoProfile -Command `"$mkdirCmd`""
$errOutput = & ssh @sshArgs 2>&1
if ($LASTEXITCODE -ne 0) {
    if ($errOutput -match "Permission denied|publickey") {
        Write-Error "ERROR: SSH authentication failed. Automated task dispatch requires passwordless key-based login. Please ensure your SSH Public Key is added to the controlled machine and check permissions.`nDetails: $errOutput"
    } else {
        Write-Error "Failed to create remote temp directory.`nDetails: $errOutput"
    }
    exit 1
}

# ── Copy task spec to remote ──────────────────────────────────────────────────
Write-Host "[2/4] Copying task spec to remote..."
scp -o BatchMode=yes -q $TaskSpec "${RemoteUser}@${RemoteHost}:$remoteTemp/"
if ($LASTEXITCODE -ne 0) {
    Write-Error "Failed to copy task spec to remote via SCP."
    exit 1
}

# ── Create isolated remote workspace ─────────────────────────────────────────
Write-Host "[3/4] Preparing remote workspace..."
if ([string]::IsNullOrWhiteSpace($RemoteRepoPath)) {
    $workspaceCmd = "New-Item -ItemType Directory -Force -Path '$remoteWorktree' | Out-Null"
    ssh -o BatchMode=yes "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$workspaceCmd`""
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to create remote execute workspace."
        exit 1
    }
}
else {
    $worktreeCmd = "Set-Location '$RemoteRepoPath'; git worktree add --detach '$remoteWorktree' HEAD"
    ssh -o BatchMode=yes "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$worktreeCmd`""
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Failed to create git worktree on remote."
        exit 1
    }
}

# ── Invoke runtime script on remote (background, detached) ───────────────────
Write-Host "[4/4] Starting remote task run..."
$runtimeCmd = "Start-Process pwsh -ArgumentList '-NoProfile','-File','$remoteStartScript','-TaskId','$taskId','-WorktreePath','$remoteWorktree','-TaskSpec','$remoteSpec','-OutputDir','$remoteTemp','-TimeoutMinutes','$TimeoutMinutes' -WindowStyle Hidden"
ssh -o BatchMode=yes "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$runtimeCmd`""
if ($LASTEXITCODE -ne 0) {
    Write-Warning "Failed to start the remote task run. Cleaning up staged remote workspace..."
    $cleanupCmd = if ([string]::IsNullOrWhiteSpace($RemoteRepoPath)) {
        "Remove-Item -Recurse -Force '$remoteTemp' -ErrorAction SilentlyContinue"
    }
    else {
        "git -C '$RemoteRepoPath' worktree remove --force '$remoteWorktree' 2>&1"
    }
    ssh -o BatchMode=yes "${RemoteUser}@${RemoteHost}" "pwsh -NoProfile -Command `"$cleanupCmd`"" 2>&1 | Out-Null
    Write-Error "Failed to start the remote task process."
    exit 1
}

# ── Print retrieval instructions ──────────────────────────────────────────────
Write-Host ""
Write-Host "Task dispatched successfully."
Write-Host ""
Write-Host "  Task ID:        $taskId"
Write-Host "  Remote output:  $remoteTemp"
Write-Host "  Workspace:      $remoteWorktree"
Write-Host ""
Write-Host "Retrieve results when done:"
Write-Host "  .\Get-XmachineRemoteResult.ps1 -RemoteHost $RemoteHost -RemoteUser $RemoteUser -TaskId $taskId -RemoteOutputDir '$remoteTemp' -RemoteRepoPath '$RemoteRepoPath' -Wait"
