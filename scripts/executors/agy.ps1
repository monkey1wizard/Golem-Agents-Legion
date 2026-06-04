#Requires -Version 7
<#
.SYNOPSIS
  Thin adapter: forwards a task spec to Antigravity CLI (agy) in headless mode.

.DESCRIPTION
  Called by Invoke-Executor.ps1. Reads the spec from stdin, then pipes it
  to `agy -p --dangerously-skip-permissions`.

  Headless syntax confirmed in T-002 spike:
    echo "spec" | agy -p --dangerously-skip-permissions
  agy reads stdin in print mode (confirmed via log: promptLength matches input).

  Stdout note (T-002): agy uses PTY-style output and does NOT write to stdout
  in a way that can be captured via standard pipe redirect. stdout will be empty
  regardless of success. The durable log (T-013) will record empty stdout for
  agy; this is expected and does NOT indicate failure.

  Authoritative result = in-place file write-back by agy (F-07).
  stdout is not a usable signal for agy — use exit code + file content only.

  Exit codes (same scheme as Invoke-Executor.ps1):
    0  agy exited 0 (task ran; orchestrator must verify file write-back)
    1  agy exited non-zero (task failed)
    2  agy not found on PATH

  Security note: --dangerously-skip-permissions grants the secondary CLI full
  filesystem and terminal access. Enable only in a trusted local environment.
  See docs/manual.md for the bypass-permission warning.
#>
param(
    [string]$WorkDir = $PWD.Path,
    [string]$Model   = ''
)

$ErrorActionPreference = 'Stop'

# Availability check — exit 2 immediately so Invoke-Executor records 'unavailable'
if (-not (Get-Command agy -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('agy.ps1: agy not found on PATH — executor unavailable')
    exit 2
}

# Read the spec from stdin (Invoke-Executor pipes it via StandardInput.Write)
$spec = [Console]::In.ReadToEnd()

# Change to the working directory the orchestrator requested
Set-Location -LiteralPath $WorkDir

# Pipe spec to agy headless
# Note: agy stdout will be empty (PTY output, not captured by redirect) — expected.
# The durable log records this; verification is by file write-back content only.
# -Model is accepted but not forwarded: agy has no --model flag (T-004 spike: env-unverifiable).
if (-not [string]::IsNullOrWhiteSpace($Model)) {
    [Console]::Error.WriteLine("agy.ps1: -Model '$Model' requested but agy has no --model flag — model injection skipped (env-unverifiable)")
}
$spec | agy -p --dangerously-skip-permissions

exit $LASTEXITCODE
