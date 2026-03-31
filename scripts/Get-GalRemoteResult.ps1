<#
.SYNOPSIS
    Retrieve results from a completed remote GAL task.

.DESCRIPTION
    Fetches status.json, summary.md, worker.log, and result.patch from the remote
    worker's output directory via SCP. Prints the task summary and status.

    After successful retrieval, the remote worktree and output directory are cleaned
    up (unless -KeepRemote is specified).

.PARAMETER RemoteHost
    SSH hostname or IP of the worker node.

.PARAMETER RemoteUser
    SSH username on the worker node.

.PARAMETER TaskId
    The task ID returned by Invoke-GalRemoteTask.

.PARAMETER RemoteOutputDir
    Path to the task output directory on the worker (printed by Invoke-GalRemoteTask).

.PARAMETER LocalOutputDir
    Local directory to write retrieved artifacts. Defaults to .\gal-results\{TaskId}.

.PARAMETER KeepRemote
    If specified, do not clean up the remote worktree and output directory after retrieval.

.PARAMETER Wait
    Poll the remote status.json until the task is no longer "running", then retrieve.
    Default poll interval: 30 seconds.

.EXAMPLE
    .\Get-GalRemoteResult.ps1 `
        -RemoteHost notebook `
        -RemoteUser alice `
        -TaskId "20260101-abc123" `
        -RemoteOutputDir "C:\Windows\Temp\gal-worker\20260101-abc123"
#>

param(
    [Parameter(Mandatory)]
    [string]$RemoteHost,

    [Parameter(Mandatory)]
    [string]$RemoteUser,

    [Parameter(Mandatory)]
    [string]$TaskId,

    [Parameter(Mandatory)]
    [string]$RemoteOutputDir,

    [string]$LocalOutputDir = "",

    [switch]$KeepRemote,

    [switch]$Wait,

    [int]$PollIntervalSeconds = 30,

    [int]$TimeoutMinutes = 60
)

$ErrorActionPreference = "Stop"

if ($LocalOutputDir -eq "") {
    $LocalOutputDir = Join-Path (Get-Location) "gal-results\$TaskId"
}

New-Item -ItemType Directory -Force -Path $LocalOutputDir | Out-Null

$remoteStatusPath = "$RemoteOutputDir/status.json"
$sshTarget = "${RemoteUser}@${RemoteHost}"

# ── Wait for task completion ───────────────────────────────────────────────────
if ($Wait) {
    Write-Host "Waiting for task $TaskId to complete (timeout: ${TimeoutMinutes}m)..."
    $deadline = (Get-Date).AddMinutes($TimeoutMinutes)

    while ((Get-Date) -lt $deadline) {
        $rawStatus = ssh $sshTarget "pwsh -NoProfile -Command `"Get-Content -Path '$remoteStatusPath' -Raw -ErrorAction SilentlyContinue`"" 2>$null
        if ($rawStatus) {
            try {
                $statusObj = $rawStatus | ConvertFrom-Json
                if ($statusObj.status -ne "running") {
                    Write-Host "Task $TaskId — status: $($statusObj.status)"
                    break
                }
            } catch { }
        }

        Write-Host "  Still running... (checking again in ${PollIntervalSeconds}s)"
        Start-Sleep -Seconds $PollIntervalSeconds
    }

    if ((Get-Date) -ge $deadline) {
        Write-Warning "Timed out waiting for task $TaskId. Retrieving partial results."
    }
}

# ── Retrieve artifacts via SCP ────────────────────────────────────────────────
Write-Host "Retrieving results for task $TaskId..."
Write-Host "  From: ${sshTarget}:$RemoteOutputDir"
Write-Host "  To:   $LocalOutputDir"

$artifacts = @("status.json", "summary.md", "worker.log", "result.patch")
$retrieved = @()
$missing = @()

foreach ($artifact in $artifacts) {
    $remotePath = "$RemoteOutputDir/$artifact"
    $localPath  = Join-Path $LocalOutputDir $artifact

    scp -q "${sshTarget}:$remotePath" $localPath 2>$null
    if ($LASTEXITCODE -eq 0) {
        $retrieved += $artifact
    }
    else {
        $missing += $artifact
    }
}

Write-Host ""
Write-Host "Retrieved: $($retrieved -join ', ')"
if ($missing) {
    Write-Warning "Not found on remote: $($missing -join ', ')"
}

# ── Print status and summary ───────────────────────────────────────────────────
$localStatusPath  = Join-Path $LocalOutputDir "status.json"
$localSummaryPath = Join-Path $LocalOutputDir "summary.md"

if (Test-Path $localStatusPath) {
    Write-Host ""
    Write-Host "─── Task Status ───────────────────────────────────────────"
    $statusObj = Get-Content $localStatusPath | ConvertFrom-Json
    Write-Host "  Task ID:    $($statusObj.taskId)"
    Write-Host "  Status:     $($statusObj.status)"
    Write-Host "  Exit Code:  $($statusObj.exitCode)"
    Write-Host "  Started:    $($statusObj.startedAt)"
    Write-Host "  Finished:   $($statusObj.finishedAt)"
    if ($statusObj.errorMessage) {
        Write-Host "  Error:      $($statusObj.errorMessage)"
    }
    Write-Host "───────────────────────────────────────────────────────────"
}

if (Test-Path $localSummaryPath) {
    Write-Host ""
    Write-Host "─── Summary ───────────────────────────────────────────────"
    Get-Content $localSummaryPath | Write-Host
    Write-Host "───────────────────────────────────────────────────────────"
}

$localPatchPath = Join-Path $LocalOutputDir "result.patch"
if (Test-Path $localPatchPath) {
    $patchSize = (Get-Item $localPatchPath).Length
    if ($patchSize -gt 0) {
        Write-Host ""
        Write-Host "result.patch: $patchSize bytes — review and apply with: git apply '$localPatchPath'"
    }
    else {
        Write-Host ""
        Write-Host "result.patch: empty (read-only task, no file changes)"
    }
}

# ── Cleanup remote worktree and output dir ─────────────────────────────────────
if (-not $KeepRemote) {
    Write-Host ""
    Write-Host "Cleaning up remote..."

    # Infer worktree path from standard naming convention
    $remoteWorktreePath = $RemoteOutputDir -replace "\\gal-worker\\$TaskId$", ""
    # The worktree is adjacent to the repo: {repoPath}-worker-{taskId}
    # We need the repo path, which was recorded in status.json worktree field.
    # Since we don't have it directly, look for the worktree via git-worktree list.
    # Simpler: derive from convention — the worktree name is embedded in the task ID.
    # The control plane knows the repo path — derive worktree path from the output dir parent.
    $worktreeInfoCmd = "git worktree list --porcelain | Select-String -Pattern 'worktree.*$TaskId' | Select-Object -First 1"

    # Unlock and remove worktree
    $cleanupCmd = @"
`$wt = (git worktree list --porcelain | Select-String 'worktree.*$TaskId' | Select-Object -First 1)?.Line?.Split(' ')[1]
if (`$wt) {
    git worktree unlock `"`$wt`" 2>`$null
    git worktree remove --force `"`$wt`" 2>&1
    Write-Host "  Worktree removed: `$wt"
} else {
    Write-Host "  Worktree not found (may already be removed)"
}
Remove-Item -Recurse -Force '$RemoteOutputDir' -ErrorAction SilentlyContinue
Write-Host '  Output dir removed.'
"@

    ssh $sshTarget "pwsh -NoProfile -Command `"$cleanupCmd`""

    if ($LASTEXITCODE -ne 0) {
        Write-Warning "Remote cleanup encountered errors. Manual cleanup may be needed:"
        Write-Warning "  ssh $sshTarget 'git worktree prune' (in repo directory)"
        Write-Warning "  Remove-Item '$RemoteOutputDir' on remote"
    }
}

Write-Host ""
Write-Host "Results saved to: $LocalOutputDir"
