#Requires -Version 7
<#
.SYNOPSIS
  Thin adapter: forwards a task spec to OpenCode in headless mode.

.DESCRIPTION
  Called by Invoke-Executor.ps1. Reads the spec from stdin, then pipes it
  to `opencode run --dangerously-skip-permissions`.

  Headless syntax confirmed in T-002 spike:
    echo "spec" | opencode run --dangerously-skip-permissions
  When no positional message argument is given, opencode run reads from stdin.

  Startup note (T-002): opencode starts an internal server per invocation;
  expect ~50s startup time. Set -TimeoutMinutes >= 10 in Invoke-Executor.

  Authoritative result = in-place file write-back by OpenCode.
  stdout/stderr are signal only — never the result source.

  Exit codes (same scheme as Invoke-Executor.ps1):
    0  opencode exited 0 (task ran; orchestrator must verify file write-back)
    1  opencode exited non-zero (task failed)
    2  opencode not found on PATH

  Security note: --dangerously-skip-permissions grants the secondary CLI full
  filesystem and terminal access. Enable only in a trusted local environment.
  See docs/manual.md for the bypass-permission warning.
#>
param(
    [string]$WorkDir = $PWD.Path
)

$ErrorActionPreference = 'Stop'

# Availability check — exit 2 immediately so Invoke-Executor records 'unavailable'
if (-not (Get-Command opencode -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('opencode.ps1: opencode not found on PATH — executor unavailable')
    exit 2
}

# Read the spec from stdin (Invoke-Executor pipes it via StandardInput.Write)
$spec = [Console]::In.ReadToEnd()

# Change to the working directory the orchestrator requested
Set-Location -LiteralPath $WorkDir

# Pipe spec to opencode headless (no positional message arg → reads from stdin)
# stdout/stderr flow through to Invoke-Executor for capture (durable log, T-013)
$spec | opencode run --dangerously-skip-permissions

exit $LASTEXITCODE
