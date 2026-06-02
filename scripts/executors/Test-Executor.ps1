#Requires -Version 7
<#
.SYNOPSIS
  Smoke test — validate an executor CLI's availability, headless capability, and exit code.

.DESCRIPTION
  Runs a minimal echo task via Invoke-Executor.ps1 to confirm that the named executor
  can be invoked and returns a predictable exit code. Does NOT verify spec receipt
  (use Test-ExecutorReceipt.ps1 for that). This is a quick sanity check for executor
  infrastructure health.

  Exit codes:
    0  Executor available and responds correctly.
    1  Executor ran but returned unexpected exit code.
    2  Executor unavailable, not installed, or CLI check failed.

.PARAMETER Executor
  Executor name: echo | claude | opencode | agy.

.PARAMETER TimeoutMinutes
  Timeout in minutes passed to Invoke-Executor.ps1. Default: 5.
#>
param(
    [Parameter(Mandatory)]
    [ValidateSet('echo', 'claude', 'opencode', 'agy')]
    [string]$Executor,
    [int]$TimeoutMinutes = 5
)

$ErrorActionPreference = 'Stop'

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot   = Split-Path -Parent (Split-Path -Parent $scriptRoot)
$invoker    = Join-Path $scriptRoot 'Invoke-Executor.ps1'

if (-not (Test-Path $invoker)) {
    Write-Error "Invoke-Executor.ps1 not found at: $invoker"
    exit 1
}

# Minimal self-contained spec — tells the executor to do nothing harmful
$specContent = @"
# Executor Smoke Test

## Task Goal

This is a smoke test. Write the text 'smoke-ok' to standard output and exit successfully.
Do NOT run git commit or git push.

## IMPORTANT: Commit Boundary

**DO NOT run `git commit` or `git push`.**
"@

$tmpDir  = [System.IO.Path]::GetTempPath()
$specPath = Join-Path $tmpDir "gal-smoke-$Executor-$([guid]::NewGuid().ToString('N')).md"
[System.IO.File]::WriteAllText($specPath, $specContent, [System.Text.UTF8Encoding]::new($false))

try {
    Write-Host "[smoke] Testing executor: $Executor"
    & $invoker -Executor $Executor -TaskSpecPath $specPath -WorkDir $repoRoot `
        -TimeoutMinutes $TimeoutMinutes
    $exitCode = $LASTEXITCODE

    switch ($exitCode) {
        0 { Write-Host "[smoke] $Executor — AVAILABLE (exit 0)" }
        2 { Write-Host "[smoke] $Executor — UNAVAILABLE or TIMEOUT (exit 2)" }
        default { Write-Host "[smoke] $Executor — UNEXPECTED exit $exitCode" }
    }
    exit $exitCode
}
finally {
    Remove-Item -LiteralPath $specPath -Force -ErrorAction SilentlyContinue
}
