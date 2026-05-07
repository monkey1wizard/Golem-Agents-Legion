param(
    [Parameter(Mandatory)]
    [string]$TaskSpec,

    [Parameter(Mandatory)]
    [string]$RepoPath,

    [int]$TimeoutMinutes = 30,

    [string]$TaskId = ""
)

$ErrorActionPreference = "Stop"

if (-not (Test-Path $TaskSpec)) {
    throw "Task spec not found: $TaskSpec"
}

if (-not (Test-Path (Join-Path $RepoPath ".git"))) {
    throw "Not a git repo: $RepoPath"
}

$runtimeScript = Join-Path $PSScriptRoot "Start-xMachine.ps1"
if (-not (Test-Path $runtimeScript)) {
    throw "Runtime script missing: $runtimeScript"
}

if ([string]::IsNullOrWhiteSpace($TaskId)) {
    $datePart = Get-Date -Format "yyyyMMdd"
    $randPart = -join ((65..90) + (97..122) + (48..57) | Get-Random -Count 6 | ForEach-Object { [char]$_ })
    $TaskId = "$datePart-$randPart"
}

$outputDir = Join-Path $env:TEMP "gal-xmachine\task-$TaskId"
$worktree = "$($RepoPath.TrimEnd('\'))-xmachine-$TaskId"
$taskSpecLocal = Join-Path $outputDir "task.md"

New-Item -ItemType Directory -Force -Path $outputDir | Out-Null
Copy-Item -Path $TaskSpec -Destination $taskSpecLocal -Force

Write-Host "Creating disposable worktree: $worktree"
& git -C $RepoPath -c filter.gal-config.smudge=cat -c filter.gal-config.clean=cat worktree add --detach $worktree HEAD 2>&1 | Write-Host
if ($LASTEXITCODE -ne 0) {
    throw "Failed to create worktree at $worktree"
}

try {
    Start-Process pwsh -ArgumentList '-NoProfile','-File',$runtimeScript,'-TaskId',$TaskId,'-WorktreePath',$worktree,'-TaskSpec',$taskSpecLocal,'-OutputDir',$outputDir,'-TimeoutMinutes',$TimeoutMinutes -WindowStyle Hidden | Out-Null
}
catch {
    & git -C $RepoPath worktree remove --force $worktree 2>$null | Out-Null
    throw "Failed to start the local async task process. $($_.Exception.Message)"
}

Write-Host ""
Write-Host "Dispatched local async task."
Write-Host "  TaskId:    $TaskId"
Write-Host "  Worktree:  $worktree"
Write-Host "  Output:    $outputDir"
Write-Host ""
Write-Host "Inspect output files when done:"
Write-Host "  $outputDir\status.json"
Write-Host "  $outputDir\summary.md"
Write-Host "  $outputDir\runtime.log"
Write-Host "  $outputDir\result.patch"