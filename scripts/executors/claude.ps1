#Requires -Version 7
<#
.SYNOPSIS
  Thin adapter: forwards a task spec to Claude Code in headless mode.

.DESCRIPTION
  Called by Invoke-Executor.ps1. Reads the spec from stdin, then pipes it
  to `claude -p --dangerously-skip-permissions`.

  Headless syntax confirmed in T-002 spike:
    echo "spec" | claude -p --dangerously-skip-permissions

  Authoritative result = in-place file write-back by Claude Code.
  stdout/stderr are signal only — never the result source.

  Exit codes (same scheme as Invoke-Executor.ps1):
    0  claude exited 0 (task ran; orchestrator must verify file write-back)
    1  claude exited non-zero (task failed)
    2  claude not found on PATH

  Security note: --dangerously-skip-permissions grants the secondary CLI full
  filesystem and terminal access. Enable only in a trusted local environment.
  See docs/manual.md for the bypass-permission warning.
#>
param(
    [string]$WorkDir = $PWD.Path
)

$ErrorActionPreference = 'Stop'

# Availability check — exit 2 immediately so Invoke-Executor records 'unavailable'
if (-not (Get-Command claude -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('claude.ps1: claude not found on PATH — executor unavailable')
    exit 2
}

# Read the spec from stdin (Invoke-Executor pipes it via StandardInput.Write)
$spec = [Console]::In.ReadToEnd()

# Change to the working directory the orchestrator requested
Set-Location -LiteralPath $WorkDir

# Pipe spec to claude headless
# stdout/stderr flow through to Invoke-Executor for capture (durable log, T-013)
$spec | claude -p --dangerously-skip-permissions

exit $LASTEXITCODE
