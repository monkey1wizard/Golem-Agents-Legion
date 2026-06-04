#Requires -Version 7
<#
.SYNOPSIS
  Thin adapter: forwards a task spec to GitHub Copilot CLI in headless mode.

.DESCRIPTION
  Called by Invoke-Executor.ps1. Reads the spec from stdin, then passes it
  to `copilot -p <spec> --allow-all` (non-interactive mode).

  Headless syntax confirmed in this session (TC-04R):
    copilot -C <workdir> --model <model> --allow-all -p "<spec>"
  --allow-all is required for non-interactive runs (grants tool/path/url perms).
  The prompt is passed via -p as a single PowerShell-native argument, which
  avoids shell quoting/length issues for multi-line specs.

  Authoritative result = in-place file write-back by Copilot CLI.
  stdout/stderr are signal only — never the result source.

  Exit codes (same scheme as Invoke-Executor.ps1):
    0  copilot exited 0 (task ran; orchestrator must verify file write-back)
    1  copilot exited non-zero (task failed)
    2  copilot not found on PATH

  Security note: --allow-all grants the secondary CLI full filesystem, tool,
  and URL access. Enable only in a trusted local environment.
  See docs/manual.md for the bypass-permission warning.
#>
param(
    [string]$WorkDir = $PWD.Path,
    [string]$Model   = ''
)

$ErrorActionPreference = 'Stop'

# Availability check — exit 2 immediately so Invoke-Executor records 'unavailable'
if (-not (Get-Command copilot -ErrorAction SilentlyContinue)) {
    [Console]::Error.WriteLine('copilot.ps1: copilot not found on PATH — executor unavailable')
    exit 2
}

# Read the spec from stdin (Invoke-Executor pipes it via StandardInput.Write)
$spec = [Console]::In.ReadToEnd()

# Change to the working directory the orchestrator requested
Set-Location -LiteralPath $WorkDir

# Run copilot headless. -p passes the spec as a single argument; --allow-all is
# required for non-interactive mode. stdout/stderr flow through to Invoke-Executor.
$copilotArgs = @('-C', $WorkDir, '--allow-all', '-p', $spec)
if (-not [string]::IsNullOrWhiteSpace($Model)) {
    $copilotArgs += @('--model', $Model)
}
copilot @copilotArgs

exit $LASTEXITCODE
