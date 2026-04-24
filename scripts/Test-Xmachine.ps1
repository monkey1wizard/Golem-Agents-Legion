<#!
.SYNOPSIS
    Run or stage an xmachine smoke test from a Windows controller.

.DESCRIPTION
        Provides a faster entrypoint for the next user by choosing a committed smoke
        template, staging a task spec when appropriate, and delegating to the
        existing dispatch and retrieval scripts.

    - `remote-windows` performs a full dispatch + optional wait/retrieval cycle.
        - `local-async` is owned by `scripts/Test-Xmachine.sh` on a macOS/Linux
            controller or directly on the controlled machine after connecting by SSH.

.PARAMETER Mode
    `remote-windows` or `local-async`.
#>

param(
    [Parameter(Mandatory)]
    [ValidateSet("local-async", "remote-windows")]
    [string]$Mode,

    [int]$TimeoutMinutes = 30,

    [switch]$Wait,

    [string]$RemoteHost,

    [string]$RemoteUser,

    [string]$RemoteRepoPath,

    [string]$LocalOutputDir,

    [string]$MacRepoPath = "/Users/yourname/Code/Golem-Agents-Legion"
)

$ErrorActionPreference = "Stop"

function Get-RepoRoot {
    $scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
    return (Split-Path -Parent $scriptDir)
}

function New-StagedTaskSpec {
    param(
        [Parameter(Mandatory)]
        [string]$SourceTemplate,

        [Parameter(Mandatory)]
        [string]$ModeName
    )

    $tempRoot = Join-Path $env:TEMP "gal-xmachine-tests"
    $taskDir = Join-Path $tempRoot ([guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Force -Path $taskDir | Out-Null
    $stagedPath = Join-Path $taskDir "task-$ModeName-smoke.md"
    Copy-Item $SourceTemplate $stagedPath -Force
    return $stagedPath
}

$repoRoot = Get-RepoRoot
$remoteTemplate = Join-Path $repoRoot "templates\task-xmachine-remote-smoke.md"
$remoteDispatch = Join-Path $repoRoot "scripts\Invoke-XmachineRemoteTask.ps1"
$remoteRetrieve = Join-Path $repoRoot "scripts\Get-XmachineRemoteResult.ps1"

if ($Mode -eq "local-async") {
    Write-Host "Local async smoke tests are driven by scripts/Test-Xmachine.sh on the target macOS/Linux machine."
    Write-Host ""
    Write-Host "Use SSH to connect to the controlled machine, change into the repo root, then run:"
    Write-Host ""
    Write-Host "  bash scripts/Test-Xmachine.sh --repo-path $MacRepoPath --wait --timeout-minutes $TimeoutMinutes"
    Write-Host ""
    Write-Host "If you want to drive dispatch and retrieval manually instead, use Invoke-XmachineLocalTask.sh and Get-XmachineLocalResult.sh from that same machine."
    exit 0
}

if (-not $RemoteHost -or -not $RemoteUser -or -not $RemoteRepoPath) {
    throw "remote-windows mode requires -RemoteHost, -RemoteUser, and -RemoteRepoPath"
}

if (-not (Test-Path $remoteTemplate)) {
    throw "Missing remote smoke template: $remoteTemplate"
}

$stagedTaskSpec = New-StagedTaskSpec -SourceTemplate $remoteTemplate -ModeName $Mode
Write-Host "Staged smoke task: $stagedTaskSpec"
Write-Host ""

$dispatchOutput = & $remoteDispatch `
    -RemoteHost $RemoteHost `
    -RemoteUser $RemoteUser `
    -RemoteRepoPath $RemoteRepoPath `
    -TaskSpec $stagedTaskSpec `
    -TimeoutMinutes $TimeoutMinutes 2>&1

$dispatchOutput | ForEach-Object { Write-Host $_ }

$taskId = ($dispatchOutput | Select-String 'Task ID:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()
$remoteOutputDir = ($dispatchOutput | Select-String 'Remote output:\s+(.+)$' | Select-Object -First 1).Matches.Groups[1].Value.Trim()

if (-not $taskId -or -not $remoteOutputDir) {
    throw "Could not parse task metadata from Invoke-XmachineRemoteTask output."
}

if (-not $Wait) {
    Write-Host ""
    Write-Host "Smoke task dispatched. Re-run with -Wait or retrieve manually with:"
    Write-Host "  .\scripts\Get-XmachineRemoteResult.ps1 -RemoteHost $RemoteHost -RemoteUser $RemoteUser -TaskId $taskId -RemoteOutputDir '$remoteOutputDir' -RemoteRepoPath '$RemoteRepoPath' -Wait"
    exit 0
}

$retrieveArgs = @{
    RemoteHost = $RemoteHost
    RemoteUser = $RemoteUser
    TaskId = $taskId
    RemoteOutputDir = $remoteOutputDir
    RemoteRepoPath = $RemoteRepoPath
    Wait = $true
    TimeoutMinutes = [Math]::Max($TimeoutMinutes, 60)
}

if ($LocalOutputDir) {
    $retrieveArgs.LocalOutputDir = $LocalOutputDir
}

Write-Host ""
Write-Host "Waiting for smoke task completion..."
& $remoteRetrieve @retrieveArgs